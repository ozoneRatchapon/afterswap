//! Anchor poller — R3 of `docs/RAIL.md`.
//!
//! Polls the Sequencer DO for closed segments whose roots have no on-chain
//! anchor yet, posts one memo transaction per root, and reports the
//! signature back. Runs in the native executor because that is where the
//! keypair lives; the Worker's only involvement is answering `GET
//! /rail/segments` and storing the reported signature as a *claim* — an
//! auditor verifies the anchor against Solana directly, never against the
//! Worker's word.
//!
//! `--dry-run` prints the exact memo bodies without touching a keypair or
//! the chain — the mode this ships verified under, since posting real
//! anchors spends real fees from a real key, which is an owner action.

use log::{info, warn};

/// The memo format. Convention matches the shipped quote binding
/// (`afterswap:quote sha-256=<digest>`): algorithm=value, space-separated.
pub fn memo_for(root: &str, from_seq: u64, to_seq: u64) -> String {
    format!("afterswap:rail blake3={root} seq={from_seq}..{to_seq}")
}

#[derive(serde::Deserialize)]
struct Segment {
    root: String,
    from_seq: u64,
    to_seq: u64,
    anchor_sig: Option<String>,
}

/// One polling pass: list segments, anchor the unanchored, report back.
pub async fn run_once(
    rail_base: &str,
    #[cfg(feature = "live")] executor: Option<&afterswap_dflow::LiveExecutor>,
    dry_run: bool,
) -> anyhow::Result<usize> {
    let http = reqwest::Client::new();
    let segments: Vec<Segment> = http
        .get(format!("{rail_base}/rail/segments"))
        .send()
        .await?
        .json()
        .await?;
    let pending: Vec<&Segment> = segments.iter().filter(|s| s.anchor_sig.is_none()).collect();
    info!("{} segment(s), {} unanchored", segments.len(), pending.len());

    // Only the live arm increments; without that feature the binding is
    // read-only and clippy rightly notices.
    #[cfg_attr(not(feature = "live"), allow(unused_mut))]
    let mut anchored = 0usize;
    for seg in pending {
        let memo = memo_for(&seg.root, seg.from_seq, seg.to_seq);
        if dry_run {
            info!("DRY RUN — would anchor: {memo}");
            continue;
        }
        #[cfg(feature = "live")]
        if let Some(exec) = executor {
            match exec.anchor_memo(&memo).await {
                Ok(sig) => {
                    info!("anchored {} -> {sig}", &seg.root[..16]);
                    let resp = http
                        .post(format!("{rail_base}/rail/anchored"))
                        .json(&serde_json::json!({"root": seg.root, "signature": sig}))
                        .send()
                        .await?;
                    match resp.status().is_success() {
                        true => anchored += 1,
                        false => warn!("anchor report rejected: {}", resp.status()),
                    }
                }
                Err(e) => warn!("anchor failed for {}: {e}", &seg.root[..16]),
            }
        }
        #[cfg(not(feature = "live"))]
        {
            warn!("not built with --features live; use --dry-run or rebuild");
        }
    }
    Ok(anchored)
}

/// CLI entry for `--anchor`.
///
/// ```sh
/// # inspect what would be anchored (no keys, no chain):
/// cargo run -p afterswap-server --release -- --anchor \
///     --rail-base http://localhost:8791 --dry-run
/// # real anchoring (owner action — spends fees from the keypair):
/// cargo run -p afterswap-server --features live --release -- --anchor \
///     --rail-base https://<worker-host> --keypair ~/.config/solana/id.json \
///     --rpc <rpc-url> [--interval-secs 60]
/// ```
pub async fn run_cli(args: &[String]) -> anyhow::Result<()> {
    fn arg<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
        let idx = args.iter().position(|a| a == name)?;
        args.get(idx + 1)?.parse().ok()
    }
    let rail_base: String =
        arg(args, "--rail-base").unwrap_or_else(|| "http://localhost:8791".to_string());
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let interval: Option<u64> = arg(args, "--interval-secs");

    #[cfg(feature = "live")]
    let executor = match arg::<String>(args, "--keypair") {
        Some(path) if !dry_run => {
            let rpc = arg::<String>(args, "--rpc")
                .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
            Some(afterswap_dflow::LiveExecutor::from_keypair_file(
                afterswap_dflow::DflowClient::dev(),
                &path,
                rpc,
            )?)
        }
        _ => None,
    };

    loop {
        let n = run_once(
            &rail_base,
            #[cfg(feature = "live")]
            executor.as_ref(),
            dry_run,
        )
        .await?;
        if n > 0 {
            info!("anchored {n} segment(s) this pass");
        }
        match interval {
            Some(secs) => tokio::time::sleep(std::time::Duration::from_secs(secs)).await,
            None => break,
        }
    }
    Ok(())
}
