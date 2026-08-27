//! Observational execution study: realised fills and pre-trade pool state, from
//! transactions that already happened.
//!
//! The live path is blocked on two structural facts. Lag-1 is unreachable —
//! arrival quote, decision delay and network round trip always span two slots.
//! And the quote spread on deep pools has no persistence: the clip probe found
//! rho = +0.050 at 1,000 SOL, a 0.2% CUPED reduction, against 34.6% on BONK.
//!
//! Reading history fixes the first and lets us test whether a different
//! covariate fixes the second. **The pre-trade pool state is inside the
//! transaction.** `meta.preTokenBalances` carries the pool's own vault balances,
//! not just the trader's — reserves at exactly slot-1, verifiable from the same
//! record, at no extra RPC call. That is lag-0 by construction, which live
//! measurement structurally cannot offer.
//!
//! What this does *not* recover: active tick liquidity. That lives in the
//! Whirlpool account's own data (`liquidity`, `sqrtPrice`, `tickCurrentIndex`),
//! and reading it at a past slot needs archival account state, which public RPC
//! does not serve. Reserves are the covariate this script tests; tick liquidity
//! is a separate, and more expensive, question.
//!
//! This is observational, not interventional. Every swap here is someone else's
//! decision about size and timing, so it carries selection bias no amount of n
//! removes. It answers "does pool state predict realised slippage" — not "would
//! our strategy have profited".
//!
//! ```sh
//! # 1. find the busiest SOL/USDC Whirlpool from recent program activity
//! cargo run -p afterswap-dflow --example historical_ingest --release -- --discover
//!
//! # 2. ingest that pool
//! cargo run -p afterswap-dflow --example historical_ingest --release -- \
//!     --pool <POOL_ACCOUNT> --limit 2000 --out data/historical/whirlpool_sol_usdc.jsonl
//! ```
//!
//! Re-running with the same `--out` resumes: signatures already present are
//! skipped, so a run interrupted by rate limiting picks up where it stopped.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const WHIRLPOOL_PROGRAM: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
const SOL: &str = "So11111111111111111111111111111111111111112";
const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

/// One observed swap: what the trader got, and what the pool looked like the
/// instant before.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Observation {
    signature: String,
    slot: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_time: Option<i64>,
    /// Vault authority — the pool's identity in balance terms.
    pool: String,
    in_mint: String,
    out_mint: String,
    /// Trader legs, from realised balance deltas.
    in_amount: f64,
    out_amount: f64,
    realized_price: f64,
    /// Pool reserves **before** this swap, from `preTokenBalances`. The lag-0
    /// control variate.
    reserve_in_pre: f64,
    reserve_out_pre: f64,
    /// Trade size as a fraction of the input-side reserve. The natural
    /// scale-free measure of how hard this order pushed the pool.
    size_frac: f64,
    /// Reserve of the *base* mint before the swap, held to one side regardless
    /// of trade direction.
    ///
    /// `reserve_in_pre` alternates between the two legs as traders buy and
    /// sell, so a series built from it jumps between ~67,000 SOL and
    /// ~18,000,000 USDC and every statistic computed on it is an artefact of
    /// that alternation rather than a property of the pool.
    reserve_base_pre: f64,
    /// Pool's implied mid before the swap, quote per base.
    mid_pre: f64,
    /// True when the base mint was sold into the pool.
    base_is_input: bool,
}

fn arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1)?.parse().ok()
}

/// One RPC call, retrying through rate limits with linear backoff.
///
/// Distinguishes "the endpoint refused" from "there was nothing there". An
/// earlier version folded both into a skip, which made a rate-limited run look
/// like a pool with no swaps in it — the sample would have been silently
/// truncated to whatever the limiter let through, and nothing in the output
/// would have said so.
async fn rpc(
    client: &reqwest::Client,
    url: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    for attempt in 0..6u32 {
        let resp = match client.post(url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => return Err(format!("http: {e}")),
        };
        if resp.status().as_u16() == 429 {
            tokio::time::sleep(Duration::from_millis(500 * u64::from(attempt + 1))).await;
            continue;
        }
        let v: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => return Err(format!("decode: {e}")),
        };
        if let Some(e) = v.get("error") {
            let msg = e.to_string();
            if msg.contains("429") || msg.to_lowercase().contains("rate") {
                tokio::time::sleep(Duration::from_millis(500 * u64::from(attempt + 1))).await;
                continue;
            }
            return Err(msg);
        }
        return v
            .get("result")
            .filter(|r| !r.is_null())
            .cloned()
            .ok_or_else(|| "null result".to_string());
    }
    Err("rate limited after 6 attempts".to_string())
}

/// Exact amount from a balance entry: raw integer plus decimals, never the
/// pre-divided `uiAmount` float.
fn amount_of(entry: &serde_json::Value) -> Option<f64> {
    let raw: f64 = match entry.pointer("/uiTokenAmount/amount")? {
        serde_json::Value::String(s) => s.parse().ok()?,
        v => v.as_f64()?,
    };
    let d = entry
        .pointer("/uiTokenAmount/decimals")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    Some(raw / 10f64.powi(d as i32))
}

/// Balances grouped by `(owner, mint)`.
fn balances(meta: &serde_json::Value, key: &str) -> HashMap<(String, String), f64> {
    let mut out = HashMap::new();
    if let Some(arr) = meta.get(key).and_then(|v| v.as_array()) {
        for b in arr {
            let (Some(owner), Some(mint), Some(amt)) = (
                b.get("owner").and_then(|v| v.as_str()),
                b.get("mint").and_then(|v| v.as_str()),
                amount_of(b),
            ) else {
                continue;
            };
            *out.entry((owner.to_string(), mint.to_string())).or_insert(0.0) += amt;
        }
    }
    out
}

/// Extract one swap **from the pool's side**.
///
/// The obvious approach is to read the trader's legs, and it fails on most real
/// volume: when a swap arrives through an aggregator the fee payer is a relayer
/// whose own balances never move, and on a multi-hop route the payer's net
/// position spans mints this pool never touched. Measuring the trader recovered
/// 23 of 69 fetched swaps here; the other 46 were real swaps read as noise.
///
/// The pool has no such ambiguity. Whoever initiated it and however it routed,
/// the pool gained exactly one mint and lost exactly one, and the realised
/// price is that ratio. Reserves come from the same `preTokenBalances` entry,
/// so covariate and outcome share a slot by construction.
///
/// `pool_filter` restricts to a known vault authority. Without it the busiest
/// two-sided counterparty in the transaction is taken, which is right for a
/// single-pool query and wrong for a program-wide one.
fn extract(result: &serde_json::Value, pool_filter: Option<&str>) -> Option<Observation> {
    let meta = result.get("meta")?;
    if meta.get("err").is_some_and(|e| !e.is_null()) {
        return None;
    }
    let slot = result.get("slot")?.as_u64()?;
    let (pre, post) = (balances(meta, "preTokenBalances"), balances(meta, "postTokenBalances"));

    let mut deltas: BTreeMap<(String, String), f64> = BTreeMap::new();
    for (k, v) in &post {
        *deltas.entry(k.clone()).or_insert(0.0) += v;
    }
    for (k, v) in &pre {
        *deltas.entry(k.clone()).or_insert(0.0) -= v;
    }

    // Group each owner's non-trivial legs.
    let mut by_owner: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
    for ((owner, mint), d) in &deltas {
        if d.abs() > 1e-12 {
            by_owner
                .entry(owner.clone())
                .or_default()
                .push((mint.clone(), *d));
        }
    }

    // A pool is an owner with exactly one leg in and one leg out.
    let mut best: Option<(String, (String, f64), (String, f64))> = None;
    for (owner, legs) in &by_owner {
        if let Some(f) = pool_filter
            && owner != f
        {
            continue;
        }
        let gained: Vec<_> = legs.iter().filter(|(_, d)| *d > 0.0).collect();
        let lost: Vec<_> = legs.iter().filter(|(_, d)| *d < 0.0).collect();
        if gained.len() != 1 || lost.len() != 1 {
            continue;
        }
        let (in_mint, in_amt) = (gained[0].0.clone(), gained[0].1);
        let (out_mint, out_amt) = (lost[0].0.clone(), -lost[0].1);
        // Largest by input reserve share: the pool that actually did the work.
        let size = pre.get(&(owner.clone(), in_mint.clone())).copied().unwrap_or(0.0);
        if best.as_ref().is_none_or(|(o, _, _)| {
            pre.get(&((*o).clone(), in_mint.clone())).copied().unwrap_or(0.0) < size
        }) {
            best = Some((owner.clone(), (in_mint, in_amt), (out_mint, out_amt)));
        }
    }
    let (pool, (in_mint, in_amount), (out_mint, out_amount)) = best?;
    if in_amount <= 0.0 || out_amount <= 0.0 {
        return None;
    }

    let reserve_in_pre = pre.get(&(pool.clone(), in_mint.clone())).copied().unwrap_or(0.0);
    let reserve_out_pre = pre.get(&(pool.clone(), out_mint.clone())).copied().unwrap_or(0.0);
    // Orient the record: the base mint is SOL when present, else the input leg.
    let base_is_input = in_mint == SOL || (out_mint != SOL && in_mint < out_mint);
    Some(Observation {
        signature: String::new(),
        slot,
        block_time: result.get("blockTime").and_then(serde_json::Value::as_i64),
        pool,
        in_mint,
        out_mint,
        in_amount,
        out_amount,
        // Pool gained `in_amount`, gave up `out_amount`: the trader's price.
        realized_price: out_amount / in_amount,
        reserve_in_pre,
        reserve_out_pre,
        size_frac: match reserve_in_pre > 0.0 {
            true => in_amount / reserve_in_pre,
            false => 0.0,
        },
        reserve_base_pre: match base_is_input {
            true => reserve_in_pre,
            false => reserve_out_pre,
        },
        mid_pre: {
            let (base_r, quote_r) = match base_is_input {
                true => (reserve_in_pre, reserve_out_pre),
                false => (reserve_out_pre, reserve_in_pre),
            };
            match base_r > 0.0 {
                true => quote_r / base_r,
                false => 0.0,
            }
        },
        base_is_input,
    })
}

fn corr(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let mut num = 0.0;
    let (mut da, mut db) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        num += (x - ma) * (y - mb);
        da += (x - ma) * (x - ma);
        db += (y - mb) * (y - mb);
    }
    match da > 0.0 && db > 0.0 {
        true => num / (da * db).sqrt(),
        false => 0.0,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let url = arg::<String>(&args, "--rpc")
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    let limit: usize = arg(&args, "--limit").unwrap_or(500);
    let spacing = Duration::from_millis(arg(&args, "--spacing-ms").unwrap_or(120));
    let http = reqwest::Client::new();
    let discover = args.iter().any(|a| a == "--discover");
    let target = arg::<String>(&args, "--pool").unwrap_or_else(|| WHIRLPOOL_PROGRAM.to_string());
    let vault_filter = arg::<String>(&args, "--vault");

    let sigs = rpc(
        &http,
        &url,
        "getSignaturesForAddress",
        serde_json::json!([target, {"limit": limit.min(1000)}]),
    )
    .await
    .map_err(|e| format!("signature fetch failed: {e}"))?
    .as_array()
    .cloned()
    .unwrap_or_default();
    eprintln!("{} signatures for {target}", sigs.len());

    // Resume: never re-fetch what the output already holds.
    let out_path = arg::<String>(&args, "--out");
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(p) = &out_path
        && let Ok(text) = std::fs::read_to_string(p)
    {
        for line in text.lines() {
            if let Ok(o) = serde_json::from_str::<Observation>(line) {
                seen.insert(o.signature);
            }
        }
        eprintln!("resuming: {} already ingested", seen.len());
    }
    let mut writer = match &out_path {
        Some(p) => {
            if let Some(d) = std::path::Path::new(p).parent() {
                std::fs::create_dir_all(d)?;
            }
            Some(std::fs::OpenOptions::new().create(true).append(true).open(p)?)
        }
        None => None,
    };

    let mut observations: Vec<Observation> = Vec::new();
    let mut pools: BTreeMap<String, usize> = BTreeMap::new();
    let mut fetched = 0usize;
    // Skip reasons, counted separately. A single "skipped" total cannot
    // distinguish a quiet pool from a throttled connection.
    let (mut reverted, mut rpc_failed, mut not_a_swap) = (0usize, 0usize, 0usize);

    for s in &sigs {
        let (Some(sig), None) = (
            s.get("signature").and_then(|v| v.as_str()),
            s.get("err").filter(|e| !e.is_null()),
        ) else {
            reverted += 1;
            continue;
        };
        if seen.contains(sig) {
            continue;
        }
        tokio::time::sleep(spacing).await;
        let tx = match rpc(
            &http,
            &url,
            "getTransaction",
            serde_json::json!([sig, {
                "encoding": "jsonParsed",
                "maxSupportedTransactionVersion": 0,
                "commitment": "confirmed"
            }]),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                rpc_failed += 1;
                if rpc_failed <= 3 {
                    eprintln!("  rpc: {e}");
                }
                continue;
            }
        };
        fetched += 1;
        // `--pool` is the Whirlpool account, which is what `getSignaturesForAddress`
        // keys on. Balance entries key on the *vault authority*, a different
        // address — so filtering by `--pool` here would match nothing. Pass
        // `--vault` to pin the authority; otherwise take the busiest two-sided
        // counterparty, which is unambiguous for a single-pool query.
        let Some(mut o) = extract(&tx, vault_filter.as_deref()) else {
            not_a_swap += 1;
            continue;
        };
        o.signature = sig.to_string();

        if discover {
            let pair_is_sol_usdc = matches!(
                (o.in_mint.as_str(), o.out_mint.as_str()),
                (SOL, USDC) | (USDC, SOL)
            );
            if pair_is_sol_usdc {
                *pools.entry(o.pool.clone()).or_insert(0) += 1;
            }
        }
        if let Some(w) = writer.as_mut() {
            writeln!(w, "{}", serde_json::to_string(&o)?)?;
        }
        observations.push(o);
        if fetched % 50 == 0 {
            eprintln!("  {fetched} fetched, {} usable", observations.len());
        }
    }

    eprintln!(
        "\n{} signatures: {reverted} reverted on chain, {rpc_failed} rpc failures, \
{fetched} fetched, {not_a_swap} not two-sided swaps, {} usable",
        sigs.len(),
        observations.len()
    );
    if rpc_failed > fetched / 4 {
        eprintln!(
            "warning: {rpc_failed} RPC failures against {fetched} successes — the sample is \
truncated by the endpoint, not by the pool. Raise --spacing-ms or use a paid endpoint before \
reading anything below."
        );
    }

    if discover {
        println!("\n## SOL/USDC pools by swap count in this sample\n");
        println!("| vault authority | swaps |\n|---|---|");
        let mut v: Vec<_> = pools.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (pool, n) in v.iter().take(10) {
            println!("| `{pool}` | {n} |");
        }
        if v.is_empty() {
            println!("| — | none found; widen --limit |");
        }
        println!(
            "\nPass the busiest authority as `--pool` to ingest it. Note this is the vault \
authority, which is what balance entries key on — it identifies the pool for our purposes even \
though it is not the Whirlpool account address."
        );
        return Ok(());
    }

    // ---- Does pre-trade pool state predict realised slippage? ----
    if observations.len() < 30 {
        eprintln!("too few observations ({}) to analyse", observations.len());
        return Ok(());
    }
    observations.sort_by_key(|o| o.slot);

    // Multi-hop routes touch other pools in the same transaction, and the
    // busiest-counterparty heuristic can pick one of those. Pin the modal vault
    // and drop the rest rather than averaging across unrelated liquidity.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for o in &observations {
        *counts.entry(o.pool.as_str()).or_insert(0) += 1;
    }
    let modal = counts
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(p, _)| (*p).to_string())
        .unwrap_or_default();
    let foreign = observations.len() - counts.get(modal.as_str()).copied().unwrap_or(0);
    observations.retain(|o| o.pool == modal);
    eprintln!(
        "pinned vault {modal}: kept {}, dropped {foreign} from other pools in multi-hop routes",
        observations.len()
    );
    if observations.len() < 30 {
        eprintln!("too few after pinning ({})", observations.len());
        return Ok(());
    }

    // Reference price. **Not** the vault ratio.
    //
    // A first version used `quote_reserve / base_reserve` as the pre-trade mid.
    // On this pool that implies 267.8 USDC/SOL against an executed 109.4 — off
    // by 2.4x, because a Whirlpool's marginal price is set by `sqrtPrice` and
    // the active tick, not by the ratio of tokens sitting in its vaults. Most
    // of that inventory is in ranges nowhere near the current price.
    //
    // Worse than being wrong, it was circular: the "mid" was a function of
    // reserves, so regressing reserves against slippage measured from it
    // returned rho = -0.977 and a 95.4% reduction. That number was the
    // covariate predicting itself.
    //
    // Without archival account state the only honest chain-derived reference is
    // the previous trade's realised price in the same pool. It carries genuine
    // mid drift between trades, so it inflates the outcome's variance — which
    // makes any reduction found against it a lower bound, not an overstatement.
    let priced: Vec<(&Observation, f64)> = observations
        .iter()
        .filter(|o| o.realized_price > 0.0)
        // Quote per base, so both trade directions land on one scale.
        .map(|o| {
            let executed = match o.base_is_input {
                true => o.realized_price,
                false => 1.0 / o.realized_price,
            };
            (o, executed)
        })
        .collect();

    // Reject prices far from the median. Leg selection takes the largest
    // movement per side, and a swap that also moves dust in a third mint can
    // have that dust chosen — one such row here priced SOL at 60,445 against a
    // 109.19-109.54 distribution, and alone drove the mean to +48,311 bps.
    // Median-absolute-deviation rather than standard deviation, because a
    // single outlier of that size inflates the very scale used to detect it.
    let mut sorted: Vec<f64> = priced.iter().map(|(_, p)| *p).collect();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    let mut devs: Vec<f64> = sorted.iter().map(|p| (p - median).abs()).collect();
    devs.sort_by(f64::total_cmp);
    let mad = devs[devs.len() / 2].max(1e-12);

    let mut y = Vec::new();
    let mut x_reserve = Vec::new();
    let mut x_size = Vec::new();
    let mut prev: Option<f64> = None;
    let mut outliers = 0usize;
    for (o, executed) in &priced {
        if (executed - median).abs() / mad >= 20.0 {
            outliers += 1;
            continue;
        }
        if let Some(p) = prev {
            y.push((executed - p) / p * 10_000.0);
            x_reserve.push(o.reserve_base_pre);
            x_size.push(o.size_frac);
        }
        prev = Some(*executed);
    }
    eprintln!("median price {median:.4}, dropped {outliers} outliers beyond 20 MAD");
    let mean_y = y.iter().sum::<f64>() / y.len() as f64;
    let sd_y = {
        let n = y.len() as f64;
        (y.iter().map(|v| (v - mean_y).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    };
    println!("\n## Price change against the previous trade in this pool\n");
    println!("n = {}, mean {mean_y:+.2} bps, sd {sd_y:.2} bps", y.len());

    println!("\n## Does slot-1 pool state predict it?\n");
    println!("| covariate (measured at slot-1) | rho with slippage | **CUPED reduction** |");
    println!("|---|---|---|");
    for (label, x) in [("base reserve", &x_reserve), ("size / reserve", &x_size)] {
        let r = corr(x, &y);
        println!("| {label} | {r:+.3} | **{:.1}%** |", r * r * 100.0);
    }

    let persist = corr(&x_reserve[..x_reserve.len() - 1], &x_reserve[1..]);
    println!(
        "\nReserve autocorrelation is {persist:+.3}, which in this design is **not** the binding \
constraint and is reported only as a data-quality check. Persistence matters live, where the \
covariate must be a stale proxy for the state at fill time. Read from history the state *is* the \
state at slot-1, so what decides CUPED is the correlation with the outcome directly — the table \
above, not this number.\n\n\
The outcome is the move against the previous trade in this pool, which contains real mid drift as \
well as this trade's impact. That inflates its variance, so a reduction measured here understates \
what a clean impact measure would give. Getting that clean measure needs `sqrtPrice` and the active \
tick at slot-1 — archival account state, which public RPC does not serve."
    );
    Ok(())
}
