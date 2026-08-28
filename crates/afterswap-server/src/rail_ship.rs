//! Ships attested rail records from the executor to the production Sequencer
//! (`POST /rail/ingest`) — RAIL.md §7.5.
//!
//! Three properties the trading loop cannot give up, and how each is bought:
//!
//! * **The loop never blocks on the network.** Records go over a channel to a
//!   dedicated shipper task. A slow edge delays the audit trail becoming
//!   public; it must never delay or reorder an execution.
//! * **Order is preserved.** The Sequencer *enforces* the executor-signed
//!   chain (`prev_hash == tip`), so a record posted out of order is rejected
//!   409 and the chain stalls. One task, one in-flight request, FIFO channel —
//!   concurrency here would be a correctness bug, not a speed-up.
//! * **The local file stays the source of truth.** `--rail-out` is written and
//!   flushed *before* a record is enqueued, so an edge outage costs visibility,
//!   never the record itself: the file replays into ingest afterwards.
//!
//! Failure classification matters as much as retrying. A 400 means the record
//! did not verify — retrying a bad signature is a busy-loop, not resilience —
//! whereas a 5xx or a timeout is the edge, not the record. The one subtle case
//! is 409 `seq not monotonic`: that is exactly what a *successful* POST whose
//! response was lost looks like on retry, so it is read against the returned
//! `tip_seq` and treated as already-ingested when the tip has passed us.

use std::time::Duration;

use afterswap_rail::AuditRecord;
use log::{error, info, warn};
use tokio::sync::mpsc;

/// Per-request ceiling. Generous next to the ~230 ms production median, tight
/// enough that a black-holed connection cannot pin the queue for minutes.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Attempts per record, including the first. Backoff doubles from
/// `BACKOFF_BASE`, so 5 attempts span ~6 s of edge unavailability.
const MAX_ATTEMPTS: u32 = 5;
const BACKOFF_BASE: Duration = Duration::from_millis(200);
/// Bound on records queued but not yet acknowledged. Reached only if the edge
/// is down for ~an hour at the experiment's cycle rate; the bound exists so a
/// wedged shipper shows up as a logged drop rather than unbounded memory.
const QUEUE_DEPTH: usize = 1024;

/// Handle held by the execution loop.
pub struct RailShipper {
    tx: mpsc::Sender<AuditRecord>,
    task: tokio::task::JoinHandle<ShipStats>,
    /// Shared with the shipper task: one connection pool, one TLS handshake.
    http: reqwest::Client,
    base: String,
}

#[derive(Default, Debug)]
pub struct ShipStats {
    pub accepted: u64,
    pub already_present: u64,
    pub rejected: u64,
    pub failed: u64,
}

/// Reject a plaintext destination up front. Rail records carry the executor's
/// signature over the venue evidence; shipping them over `http://` to a remote
/// host offers a network position the chance to drop or delay records, and the
/// operator no way to tell that from an outage. Loopback is exempt because the
/// falsifier's `wrangler dev` has no TLS.
pub fn check_scheme(base: &str) -> anyhow::Result<()> {
    match base.split_once("://") {
        Some(("https", _)) => Ok(()),
        Some(("http", rest)) => {
            let host = rest.split(['/', ':']).next().unwrap_or("");
            match host {
                "localhost" | "127.0.0.1" | "[::1]" => Ok(()),
                _ => anyhow::bail!(
                    "--rail-ingest must be https for a remote host (got {base}); \
plaintext would let the path drop records indistinguishably from an outage"
                ),
            }
        }
        _ => anyhow::bail!("--rail-ingest must be an absolute URL (got {base})"),
    }
}

/// Outcome of one POST, after classification.
enum Outcome {
    Accepted,
    /// The Sequencer already holds this seq — a lost-response retry, or a
    /// replay of a file the rail has seen. Idempotent, not an error.
    AlreadyPresent,
    /// The record will never be accepted: bad attestation, fork, oversize.
    Rejected(String),
    /// The edge, not the record. Worth retrying.
    Transient(String),
}

async fn post_once(http: &reqwest::Client, url: &str, record: &AuditRecord) -> Outcome {
    let resp = match http.post(url).json(record).send().await {
        Ok(r) => r,
        Err(e) => return Outcome::Transient(format!("send: {e}")),
    };
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    match status.as_u16() {
        200..=299 => Outcome::Accepted,
        409 => {
            // Distinguish "we already landed" from a genuine fork by the tip
            // the Sequencer reports back, never by the message text.
            let tip = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("tip_seq").and_then(serde_json::Value::as_u64));
            match tip {
                Some(t) if t >= record.seq => Outcome::AlreadyPresent,
                _ => Outcome::Rejected(format!("409 {body}")),
            }
        }
        400 | 413 => Outcome::Rejected(format!("{status} {body}")),
        // 503 is the Sequencer with no registered pubkey — an operator error,
        // but one that a redeploy fixes while the run continues, so retry.
        _ => Outcome::Transient(format!("{status} {body}")),
    }
}

async fn ship_one(http: &reqwest::Client, url: &str, record: &AuditRecord, stats: &mut ShipStats) {
    let mut delay = BACKOFF_BASE;
    for attempt in 1..=MAX_ATTEMPTS {
        match post_once(http, url, record).await {
            Outcome::Accepted => {
                stats.accepted += 1;
                return;
            }
            Outcome::AlreadyPresent => {
                stats.already_present += 1;
                return;
            }
            Outcome::Rejected(why) => {
                stats.rejected += 1;
                error!("rail ingest rejected seq {} (permanent): {why}", record.seq);
                return;
            }
            Outcome::Transient(why) => match attempt == MAX_ATTEMPTS {
                true => {
                    stats.failed += 1;
                    error!(
                        "rail ingest seq {} failed after {MAX_ATTEMPTS} attempts: {why} \
— the record is in --rail-out and can be replayed",
                        record.seq
                    );
                }
                false => {
                    warn!("rail ingest seq {} attempt {attempt}: {why}; retrying", record.seq);
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            },
        }
    }
}

/// The Sequencer's tip, or `None` when the rail is empty (genesis expected).
async fn fetch_tip_seq(http: &reqwest::Client, base: &str) -> anyhow::Result<Option<u64>> {
    let stats: serde_json::Value = http.get(format!("{base}/rail/stats")).send().await?
        .error_for_status()?.json().await?;
    Ok(stats.get("tip_seq").and_then(serde_json::Value::as_u64))
}

/// Pull one published record back. The whole record is needed, not its hash:
/// `link` hashes the predecessor exactly as published, attestation included.
async fn fetch_record(http: &reqwest::Client, base: &str, seq: u64) -> anyhow::Result<AuditRecord> {
    // `since` is exclusive and typed i64 on the Worker, so seq 0 asks for -1.
    let url = format!("{base}/rail/records?since={}&limit=1", seq as i64 - 1);
    let mut records: Vec<AuditRecord> =
        http.get(url).send().await?.error_for_status()?.json().await?;
    match records.pop() {
        Some(r) if r.seq == seq => Ok(r),
        other => anyhow::bail!("rail returned seq {:?} asking for {seq}", other.map(|r| r.seq)),
    }
}

impl RailShipper {
    /// Spawn the shipper. `base` is the rail origin, e.g.
    /// `https://afterswap-rail.solana-thailand.workers.dev`.
    pub fn spawn(base: &str) -> anyhow::Result<Self> {
        check_scheme(base)?;
        let url = format!("{}/rail/ingest", base.trim_end_matches('/'));
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            // One destination, one in-flight request: keep the connection warm
            // so TLS is negotiated once for the whole run, not per record.
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent("afterswap-executor/1")
            .build()?;
        let (tx, mut rx) = mpsc::channel::<AuditRecord>(QUEUE_DEPTH);
        info!("rail: shipping records to {url}");
        let worker_http = http.clone();
        let task = tokio::spawn(async move {
            let mut stats = ShipStats::default();
            while let Some(record) = rx.recv().await {
                ship_one(&worker_http, &url, &record, &mut stats).await;
            }
            stats
        });
        Ok(Self { tx, task, http, base: base.trim_end_matches('/').to_string() })
    }

    /// Hand a record to the shipper. Never awaits the network; when the queue
    /// is full the record is dropped from *shipping* only — it is already
    /// durable in `--rail-out`, and the loss is logged rather than hidden.
    pub fn enqueue(&self, record: AuditRecord) {
        let seq = record.seq;
        if self.tx.try_send(record).is_err() {
            error!("rail ingest queue full, seq {seq} not shipped (still in --rail-out)");
        }
    }

    /// Reconcile the local chain file against the live rail before the run
    /// starts, and return the tip to continue from.
    ///
    /// Two states are possible after an interrupted run, and they need
    /// opposite responses:
    ///
    /// * **File ahead of rail** — records were written but never landed (an
    ///   ingest outage, or retries exhausted). They are replayed now. Without
    ///   this the gap is permanent, since the chain only ever moves forward.
    /// * **Rail ahead of file** — the local file was lost or truncated. The
    ///   rail's tip is adopted, because it is the tip the Sequencer will
    ///   enforce `prev_hash` against; continuing from a stale local tip would
    ///   fork and be rejected for the rest of the run.
    ///
    /// A failure to reach the rail here is fatal rather than a warning: it
    /// would silently degrade into starting a fork.
    pub async fn reconcile(
        &self,
        local_tip: Option<AuditRecord>,
        rail_out: &std::path::Path,
    ) -> anyhow::Result<Option<AuditRecord>> {
        let remote_tip_seq = fetch_tip_seq(&self.http, &self.base).await?;
        match (local_tip, remote_tip_seq) {
            (local, None) => {
                // Empty rail: everything local is backlog.
                if let Some(t) = &local {
                    info!("rail: empty rail, replaying local records 0..={}", t.seq);
                    self.replay(rail_out, 0)?;
                }
                Ok(local)
            }
            (Some(local), Some(remote)) if local.seq > remote => {
                warn!(
                    "rail: local chain is ahead (local {} > rail {remote}); replaying the backlog",
                    local.seq
                );
                self.replay(rail_out, remote + 1)?;
                Ok(Some(local))
            }
            (local, Some(remote)) => {
                let stale = local.as_ref().is_some_and(|t| t.seq < remote);
                if stale || local.is_none() {
                    warn!("rail: adopting the live tip at seq {remote} — the local file is behind it");
                    return Ok(Some(fetch_record(&self.http, &self.base, remote).await?));
                }
                Ok(local)
            }
        }
    }

    /// Enqueue every local record at or after `from_seq`.
    fn replay(&self, rail_out: &std::path::Path, from_seq: u64) -> anyhow::Result<()> {
        let text = std::fs::read_to_string(rail_out)?;
        let mut n = 0;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let record: AuditRecord = serde_json::from_str(line)?;
            if record.seq >= from_seq {
                self.enqueue(record);
                n += 1;
            }
        }
        info!("rail: queued {n} record(s) for replay from seq {from_seq}");
        Ok(())
    }

    /// Drain the queue and report. Called at the end of a run so the operator
    /// learns the audit trail is complete before the process exits.
    pub async fn finish(self) -> ShipStats {
        drop(self.tx);
        self.task.await.unwrap_or_default()
    }
}
