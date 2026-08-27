//! The Sequencer DO, in Rust.
//!
//! Same contract as the TypeScript original, restated here because this file
//! is now the authority: the DO **enforces** the executor-signed chain, it
//! never authors it. The executor's attestation covers `seq` and
//! `prev_hash`, so acceptance is: full record verification (the same
//! `verify_record` an auditor compiles), then monotonic `seq`, then
//! `prev_hash == tip`. Forks come back 409 with the expected tip; gaps are
//! counted, never hidden; this object holds no keys.

use worker::*;

use afterswap_rail::{AuditRecord, merkle_proof, merkle_root, record_hash, verify_record};

/// Records per closed segment: proofs stay 6 hops, objects a few hundred KB.
const SEGMENT_SIZE: usize = 64;
/// Hot rows kept after archival to R2. Only trimmed when the segment body
/// is safely in R2 — with no bucket bound, everything is retained (free-tier
/// SQLite holds >1M records).
const RING_KEEP: usize = 512;
/// A record is a few KB; a megabyte is not a record.
const MAX_BODY: usize = 1 << 20;
const ZERO64: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}



fn json_response(body: &serde_json::Value, status: u16) -> Result<Response> {
    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    headers.set("access-control-allow-origin", "*")?;
    Ok(Response::ok(body.to_string())?
        .with_status(status)
        .with_headers(headers))
}

#[derive(serde::Deserialize)]
struct SeqHashRow {
    seq: u64,
    hash: String,
}

#[derive(serde::Deserialize)]
struct BodyRow {
    body: String,
}

#[derive(serde::Deserialize)]
struct CountRow {
    n: u64,
}

#[durable_object]
pub struct RailSequencer {
    state: State,
    env: Env,
}

impl DurableObject for RailSequencer {
    fn new(state: State, env: Env) -> Self {
        let sql = state.storage().sql();
        for ddl in [
            "CREATE TABLE IF NOT EXISTS records (seq INTEGER PRIMARY KEY, hash TEXT NOT NULL, body TEXT NOT NULL, archived INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS segments (root TEXT PRIMARY KEY, from_seq INTEGER NOT NULL, to_seq INTEGER NOT NULL, count INTEGER NOT NULL, r2_key TEXT NOT NULL, closed_ms INTEGER NOT NULL, anchor_sig TEXT)",
            "CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v TEXT NOT NULL)",
        ] {
            if let Err(e) = sql.exec(ddl, None) {
                // A failed DDL leaves every later query throwing through the
                // JS boundary as an opaque "Critical error" — say it here.
                console_error!("sequencer DDL failed: {e} — {ddl}");
            }
        }
        Self { state, env }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let path = req.path();
        match (req.method(), path.as_str()) {
            (Method::Post, "/rail/ingest") => self.ingest(&mut req).await,
            (Method::Get, "/rail/records") => self.records(&req),
            (Method::Get, p) if p.starts_with("/rail/proof/") => {
                let seq: Option<u64> = p.rsplit('/').next().and_then(|s| s.parse().ok());
                match seq {
                    Some(seq) => self.proof(seq).await,
                    None => json_response(&serde_json::json!({"error": "bad seq"}), 400),
                }
            }
            (Method::Get, "/rail/segments") => self.segments(),
            (Method::Post, "/rail/anchored") => self.anchored(&mut req).await,
            (Method::Get, "/rail/stats") => self.stats(),
            _ => json_response(&serde_json::json!({"error": "unknown rail route"}), 404),
        }
    }
}

impl RailSequencer {
    fn sql(&self) -> SqlStorage {
        self.state.storage().sql()
    }

    fn meta(&self, k: &str) -> Option<String> {
        #[derive(serde::Deserialize)]
        struct V {
            v: String,
        }
        // `.one()` maps to the JS cursor's one(), which *throws* on zero
        // rows — and that surfaces as an uncatchable critical error, not an
        // Err. An empty meta table is the normal genesis state, so: to_array
        // and take the first.
        let rows: Vec<V> = self
            .sql()
            .exec("SELECT v FROM meta WHERE k = ?", vec![k.into()])
            .ok()?
            .to_array()
            .ok()?;
        rows.into_iter().next().map(|r| r.v)
    }

    fn set_meta(&self, k: &str, v: &str) -> Result<()> {
        self.sql().exec(
            "INSERT INTO meta (k, v) VALUES (?, ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            vec![k.into(), v.into()],
        )?;
        Ok(())
    }

    async fn ingest(&self, req: &mut Request) -> Result<Response> {
        let Ok(pubkey_hex) = self.env.var("RAIL_PUBKEY") else {
            return json_response(&serde_json::json!({"error": "no attestation pubkey registered"}), 503);
        };
        let Some(pubkey) = hex32(&pubkey_hex.to_string()) else {
            return json_response(&serde_json::json!({"error": "malformed RAIL_PUBKEY"}), 503);
        };
        let body = req.text().await?;
        if body.len() > MAX_BODY {
            return json_response(&serde_json::json!({"error": "record too large"}), 413);
        }
        let record: AuditRecord = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => return json_response(&serde_json::json!({"error": format!("parse: {e}")}), 400),
        };

        // Step 1: the exact verification an auditor's build of the crate
        // performs — typed call, no string boundary.
        if let Err(e) = verify_record(&record, &pubkey) {
            return json_response(&serde_json::json!({"error": format!("verify: {e}")}), 400);
        }

        // Step 2: chain enforcement against the stored tip.
        let prev_hex = to_hex(&record.prev_hash);
        match (self.meta("tip_seq"), self.meta("tip_hash")) {
            (None, _) => {
                if prev_hex != ZERO64 {
                    return json_response(
                        &serde_json::json!({"error": "genesis must cite the zero hash", "expected_prev": ZERO64}),
                        409,
                    );
                }
            }
            (Some(tip_seq), tip_hash) => {
                let tip_seq: u64 = tip_seq.parse().unwrap_or(0);
                if record.seq <= tip_seq {
                    return json_response(
                        &serde_json::json!({"error": "seq not monotonic", "tip_seq": tip_seq}),
                        409,
                    );
                }
                if Some(prev_hex.clone()) != tip_hash {
                    return json_response(
                        &serde_json::json!({"error": "prev_hash does not extend the tip", "expected_prev": tip_hash, "tip_seq": tip_seq}),
                        409,
                    );
                }
                if record.seq > tip_seq + 1 {
                    let gaps: u64 = self.meta("gaps").and_then(|g| g.parse().ok()).unwrap_or(0);
                    self.set_meta("gaps", &(gaps + 1).to_string())?;
                }
            }
        }

        let hash = to_hex(&record_hash(&record));
        self.sql().exec(
            "INSERT INTO records (seq, hash, body) VALUES (?, ?, ?)",
            vec![(record.seq as i64).into(), hash.as_str().into(), body.as_str().into()],
        )?;
        self.set_meta("tip_seq", &record.seq.to_string())?;
        self.set_meta("tip_hash", &hash)?;
        let total: u64 = self.meta("total").and_then(|t| t.parse().ok()).unwrap_or(0);
        self.set_meta("total", &(total + 1).to_string())?;

        let closed = self.maybe_close_segment().await?;
        json_response(&serde_json::json!({"accepted": record.seq, "hash": hash, "segment_closed": closed}), 200)
    }

    /// Close a segment once enough unarchived records accumulate. The root is
    /// computed by the same `merkle_root` the native verifier uses. R2 write
    /// first, index after — a crash between them re-closes idempotently
    /// (same content, same root, same key). **With no bucket bound, rows are
    /// retained in SQLite instead of trimmed**: the free-tier mode keeps the
    /// full archive queryable at zero cost until R2 is enabled.
    async fn maybe_close_segment(&self) -> Result<Option<String>> {
        #[derive(serde::Deserialize)]
        struct Row {
            seq: u64,
            hash: String,
            body: String,
        }
        let pending: Vec<Row> = self
            .sql()
            .exec("SELECT seq, hash, body FROM records WHERE archived = 0 ORDER BY seq ASC", None)?
            .to_array()?;
        if pending.len() < SEGMENT_SIZE {
            return Ok(None);
        }
        let batch = &pending[..SEGMENT_SIZE];
        let mut leaves = Vec::with_capacity(batch.len());
        for r in batch {
            match hex32(&r.hash) {
                Some(h) => leaves.push(h),
                None => return Err(Error::RustError(format!("bad stored hash at seq {}", r.seq))),
            }
        }
        let root = merkle_root(&leaves).map_err(|e| Error::RustError(e.to_string()))?;
        let root_hex = to_hex(&root);
        let key = format!("segments/{root_hex}.jsonl");

        let archived_to_r2 = match self.env.bucket("RAIL_ARCHIVE") {
            Ok(bucket) => {
                let joined: Vec<&str> = batch.iter().map(|r| r.body.as_str()).collect();
                bucket.put(&key, joined.join("\n")).execute().await?;
                true
            }
            // No bucket bound: free-tier retention mode.
            Err(_) => false,
        };

        let (from, to) = (batch[0].seq, batch[batch.len() - 1].seq);
        self.sql().exec(
            "INSERT OR REPLACE INTO segments (root, from_seq, to_seq, count, r2_key, closed_ms, anchor_sig) VALUES (?, ?, ?, ?, ?, ?, NULL)",
            vec![
                root_hex.as_str().into(),
                (from as i64).into(),
                (to as i64).into(),
                (batch.len() as i64).into(),
                key.as_str().into(),
                (Date::now().as_millis() as i64).into(),
            ],
        )?;
        self.sql().exec(
            "UPDATE records SET archived = 1 WHERE seq >= ? AND seq <= ?",
            vec![(from as i64).into(), (to as i64).into()],
        )?;
        if archived_to_r2 {
            self.sql().exec(
                "DELETE FROM records WHERE archived = 1 AND seq NOT IN (SELECT seq FROM records ORDER BY seq DESC LIMIT ?)",
                vec![(RING_KEEP as i64).into()],
            )?;
        }
        Ok(Some(root_hex))
    }

    fn records(&self, req: &Request) -> Result<Response> {
        let url = req.url()?;
        let get = |k: &str| url.query_pairs().find(|(q, _)| q == k).map(|(_, v)| v.to_string());
        let since: i64 = get("since").and_then(|v| v.parse().ok()).unwrap_or(-1);
        let limit: i64 = get("limit").and_then(|v| v.parse().ok()).unwrap_or(50).min(200);
        let rows: Vec<BodyRow> = self
            .sql()
            .exec(
                "SELECT body FROM records WHERE seq > ? ORDER BY seq ASC LIMIT ?",
                vec![since.into(), limit.into()],
            )?
            .to_array()?;
        let bodies: Vec<&str> = rows.iter().map(|r| r.body.as_str()).collect();
        let headers = Headers::new();
        headers.set("content-type", "application/json")?;
        headers.set("access-control-allow-origin", "*")?;
        Ok(Response::ok(format!("[{}]", bodies.join(",")))?.with_headers(headers))
    }

    /// Proofs exist only against closed segments — a proof that anchors to
    /// nothing is theater, so open-segment records get told to wait.
    async fn proof(&self, seq: u64) -> Result<Response> {
        #[derive(serde::Deserialize)]
        struct Seg {
            root: String,
            from_seq: u64,
            to_seq: u64,
            r2_key: String,
        }
        let seg: Vec<Seg> = self
            .sql()
            .exec(
                "SELECT root, from_seq, to_seq, r2_key FROM segments WHERE from_seq <= ? AND to_seq >= ?",
                vec![(seq as i64).into(), (seq as i64).into()],
            )?
            .to_array()?;
        let Some(seg) = seg.first() else {
            return json_response(
                &serde_json::json!({"error": "record not in a closed segment yet; proofs exist once the segment closes"}),
                409,
            );
        };

        let mut lines: Vec<SeqHashRow> = self
            .sql()
            .exec(
                "SELECT seq, hash FROM records WHERE seq >= ? AND seq <= ? ORDER BY seq ASC",
                vec![(seg.from_seq as i64).into(), (seg.to_seq as i64).into()],
            )?
            .to_array()?;
        if lines.is_empty() {
            // Trimmed from SQLite — the archived path via R2.
            let bucket = self.env.bucket("RAIL_ARCHIVE")?;
            let Some(obj) = bucket.get(&seg.r2_key).execute().await? else {
                return json_response(&serde_json::json!({"error": "segment object missing from archive"}), 500);
            };
            let text = obj.body().ok_or_else(|| Error::RustError("empty object".into()))?.text().await?;
            for body in text.split('\n') {
                let record: AuditRecord = serde_json::from_str(body)
                    .map_err(|e| Error::RustError(format!("archived record parse: {e}")))?;
                lines.push(SeqHashRow { seq: record.seq, hash: to_hex(&record_hash(&record)) });
            }
        }

        let Some(index) = lines.iter().position(|l| l.seq == seq) else {
            return json_response(&serde_json::json!({"error": "seq absent from its segment"}), 500);
        };
        let mut leaves = Vec::with_capacity(lines.len());
        for l in &lines {
            match hex32(&l.hash) {
                Some(h) => leaves.push(h),
                None => return Err(Error::RustError("bad stored hash".into())),
            }
        }
        let proof = merkle_proof(&leaves, index).map_err(|e| Error::RustError(e.to_string()))?;
        let steps: Vec<(String, bool)> = proof.iter().map(|(s, l)| (to_hex(s), *l)).collect();
        json_response(
            &serde_json::json!({
                "seq": seq,
                "record_hash": lines[index].hash,
                "segment_root": seg.root,
                "proof": steps,
                "note": "verify with afterswap_rail::merkle_verify; anchor via /rail/segments",
            }),
            200,
        )
    }

    fn segments(&self) -> Result<Response> {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct Seg {
            root: String,
            from_seq: u64,
            to_seq: u64,
            count: u64,
            anchor_sig: Option<String>,
            closed_ms: i64,
        }
        let rows: Vec<Seg> = self
            .sql()
            .exec(
                "SELECT root, from_seq, to_seq, count, anchor_sig, closed_ms FROM segments ORDER BY from_seq ASC",
                None,
            )?
            .to_array()?;
        json_response(&serde_json::to_value(rows).map_err(|e| Error::RustError(e.to_string()))?, 200)
    }

    /// Store the executor's anchor signature as a *claim*, verbatim. Auditors
    /// verify anchors against Solana directly; a Worker vouching for one adds
    /// nothing. Only roots this object closed can be marked.
    async fn anchored(&self, req: &mut Request) -> Result<Response> {
        #[derive(serde::Deserialize)]
        struct Claim {
            root: String,
            signature: String,
        }
        let claim: Claim = match req.json().await {
            Ok(c) => c,
            Err(_) => return json_response(&serde_json::json!({"error": "root and signature required"}), 400),
        };
        let hit: Vec<CountRow> = self
            .sql()
            .exec("SELECT COUNT(*) AS n FROM segments WHERE root = ?", vec![claim.root.as_str().into()])?
            .to_array()?;
        if hit.first().map_or(0, |r| r.n) == 0 {
            return json_response(&serde_json::json!({"error": "unknown segment root"}), 404);
        }
        self.sql().exec(
            "UPDATE segments SET anchor_sig = ? WHERE root = ?",
            vec![claim.signature.as_str().into(), claim.root.as_str().into()],
        )?;
        json_response(&serde_json::json!({"anchored": claim.root, "signature": claim.signature}), 200)
    }

    fn stats(&self) -> Result<Response> {
        let nseg: Vec<CountRow> = self
            .sql()
            .exec("SELECT COUNT(*) AS n FROM segments", None)?
            .to_array()?;
        let nanchored: Vec<CountRow> = self
            .sql()
            .exec("SELECT COUNT(*) AS n FROM segments WHERE anchor_sig IS NOT NULL", None)?
            .to_array()?;
        #[derive(serde::Deserialize)]
        struct Latest {
            root: String,
            to_seq: u64,
        }
        let latest: Vec<Latest> = self
            .sql()
            .exec("SELECT root, to_seq FROM segments ORDER BY to_seq DESC LIMIT 1", None)?
            .to_array()?;
        json_response(
            &serde_json::json!({
                "attest_pubkey": self.env.var("RAIL_PUBKEY").map(|v| v.to_string()).ok(),
                "tip_seq": self.meta("tip_seq").and_then(|v| v.parse::<u64>().ok()),
                "tip_hash": self.meta("tip_hash"),
                "total_accepted": self.meta("total").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
                "seq_gaps": self.meta("gaps").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
                "segments_closed": nseg.first().map_or(0, |r| r.n),
                "segments_anchored": nanchored.first().map_or(0, |r| r.n),
                "latest_root": latest.first().map(|r| r.root.clone()),
                "latest_root_to_seq": latest.first().map(|r| r.to_seq),
            }),
            200,
        )
    }
}
