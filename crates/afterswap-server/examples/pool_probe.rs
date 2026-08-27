//! Step 0 of the execution experiment: pick the pool, and verify the control
//! variate before spending anything.
//!
//! Bench 038 measured `rho(depth_t, depth_{t+1}) = 0.588` — a 34.6% CUPED
//! reduction — **on BONK**, a reserve-driven CPMM. The 300-cycle experiment is
//! scoped against that figure, but the target is a liquid CLMM major, whose
//! depth is tick-concentrated rather than reserve-driven and may not persist
//! the same way. Running the experiment first and discovering the covariate is
//! weak there would spend real capital establishing that the sample was
//! underpowered.
//!
//! So this measures the same quantity on candidate pairs using quotes only. No
//! keypair, no fills, no capital.
//!
//! It uses the **two-quote depth spread**, not `impact_bps`. A dry run found
//! DFlow's dev endpoint returns `priceImpactPct: "0"` on every pair tested, so
//! the free lag-0 reading is present and identically zero — no variance, and
//! nothing for CUPED to regress on. The probe costs a second request per
//! observation and can straddle slots, which is why the slot-gap column is
//! reported beside the correlation rather than assumed away.
//!
//! **Probe one pair at a time.** The dev endpoint returns HTTP 429 well before
//! six pairs at two requests each will fit in a 6 s tick, and a rate-limited
//! run does not degrade gracefully — it silently returns a series with holes in
//! it, which is worse than no series. `--only` selects a single pair.
//!
//! ```sh
//! cargo run -p afterswap-server --example pool_probe --release -- \
//!     --only SOL/USDC --minutes 60 --interval-ms 4000
//! ```

use std::collections::BTreeMap;
use std::time::Duration;

use afterswap_dflow::{DflowClient, PricePoller, QuoteRequest, mints};
use afterswap_engine::cuped::cycles_needed;

/// Candidate pairs, USDC-quoted so notionals are comparable.
///
/// BONK is included deliberately as a control: bench 038's 34.6% was measured
/// on it, so if this harness does not roughly reproduce that figure the harness
/// is wrong, not the pool.
fn candidates() -> Vec<(&'static str, QuoteRequest)> {
    vec![
        (
            "SOL/USDC",
            QuoteRequest {
                input_mint: mints::SOL.to_string(),
                output_mint: mints::USDC.to_string(),
                amount: 1_000_000_000,
                slippage_bps: 50,
            },
        ),
        (
            "BONK/USDC (bench 038 control)",
            QuoteRequest {
                input_mint: mints::BONK.to_string(),
                output_mint: mints::USDC.to_string(),
                amount: 1_000_000_000,
                slippage_bps: 100,
            },
        ),
        (
            "WIF/USDC",
            QuoteRequest {
                input_mint: mints::WIF.to_string(),
                output_mint: mints::USDC.to_string(),
                amount: 1_000_000_000,
                slippage_bps: 100,
            },
        ),
        // Candidates chosen on one principle: route churn tracks how many
        // venues quote within a basis point of each other. SOL/USDC churned
        // 5 of 6 cycles in the dry run because dozens do. These should have
        // fewer — but that is a structural argument, and this probe exists
        // because structural arguments are not measurements.
        (
            "USDC/USDT",
            QuoteRequest {
                input_mint: mints::USDC.to_string(),
                output_mint: mints::USDT.to_string(),
                amount: 1_000_000_000, // 1,000 USDC
                slippage_bps: 20,
            },
        ),
        (
            "cbBTC/USDC",
            QuoteRequest {
                input_mint: mints::CBBTC.to_string(),
                output_mint: mints::USDC.to_string(),
                amount: 1_000_000, // 0.01 cbBTC
                slippage_bps: 50,
            },
        ),
        (
            "JitoSOL/SOL",
            QuoteRequest {
                input_mint: mints::JITOSOL.to_string(),
                output_mint: mints::SOL.to_string(),
                amount: 1_000_000_000, // 1 JitoSOL
                slippage_bps: 30,
            },
        ),
    ]
}

fn arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1)?.parse().ok()
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

struct Series {
    impact: Vec<f64>,
    /// Slot deltas between consecutive observations. Lag-1 in observations is
    /// only lag-1 in slots if polling keeps up; recording this stops the two
    /// from being conflated.
    slot_gaps: Vec<u64>,
    routes: BTreeMap<String, usize>,
    missing_impact: usize,
    errors: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let minutes: u64 = arg(&args, "--minutes").unwrap_or(60);
    let interval_ms: u64 = arg(&args, "--interval-ms").unwrap_or(2_000);
    let ticks = (minutes * 60_000) / interval_ms;

    let client = DflowClient::dev();
    let only = arg::<String>(&args, "--only");
    let pairs: Vec<_> = candidates()
        .into_iter()
        .filter(|(n, _)| only.as_ref().is_none_or(|f| n.contains(f.as_str())))
        .collect();
    if pairs.is_empty() {
        eprintln!("no pair matches --only");
        return Ok(());
    }
    let mut series: Vec<Series> = pairs
        .iter()
        .map(|_| Series {
            impact: Vec::new(),
            slot_gaps: Vec::new(),
            routes: BTreeMap::new(),
            missing_impact: 0,
            errors: 0,
        })
        .collect();
    let mut last_slot: Vec<Option<u64>> = vec![None; pairs.len()];

    println!(
        "probing {} pairs, {ticks} observations each at {interval_ms} ms (~{minutes} min)\n",
        pairs.len()
    );
    if pairs.len() > 2 {
        eprintln!(
            "warning: {} pairs at 2 requests each will likely hit the endpoint rate limit. \
Use --only to probe one pair at a time.\n",
            pairs.len()
        );
    }

    let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    for seq in 0..ticks {
        interval.tick().await;
        for (i, (name, req)) in pairs.iter().enumerate() {
            let poller = PricePoller::new(client.clone(), req.clone());
            // Probe at 10x the primary clip: large enough to move the pool
            // measurably, small enough that the aggregator still routes it the
            // way it would route a real order.
            let probe_amount = req.amount.saturating_mul(10);
            match poller.poll_snapshot_probed(seq, probe_amount).await {
                Ok(snap) => {
                    match snap.probe.as_ref().map(|p| p.depth_bps) {
                        Some(v) => series[i].impact.push(v),
                        None => {
                            // Pushing a zero would invent a covariate value.
                            series[i].missing_impact += 1;
                            continue;
                        }
                    }
                    if let (Some(prev), Some(now)) = (last_slot[i], snap.context_slot) {
                        series[i].slot_gaps.push(now.saturating_sub(prev));
                    }
                    last_slot[i] = snap.context_slot;
                    let route = format!("{}|{}", snap.venue.as_deref().unwrap_or("?"), snap.hops);
                    *series[i].routes.entry(route).or_insert(0) += 1;
                }
                Err(e) => {
                    series[i].errors += 1;
                    if series[i].errors <= 3 {
                        eprintln!("{name}: quote failed: {e}");
                    }
                }
            }
            // Space requests across pairs; the endpoint 429s otherwise.
            if pairs.len() > 1 {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
        if seq % 60 == 0 && seq > 0 {
            println!("  {seq}/{ticks} observations");
        }
    }

    println!("\n| pair | n | mean depth bps | rho lag-1 | **reduction** | median slot gap | routes | no depth |");
    println!("|---|---|---|---|---|---|---|---|");
    for (i, (name, _)) in pairs.iter().enumerate() {
        let s = &series[i];
        if s.impact.len() < 30 {
            println!(
                "| {name} | {} | — | — | — | — | — | {} |",
                s.impact.len(),
                s.missing_impact
            );
            continue;
        }
        let r = corr(&s.impact[..s.impact.len() - 1], &s.impact[1..]);
        let mut gaps = s.slot_gaps.clone();
        gaps.sort_unstable();
        let med = gaps.get(gaps.len() / 2).copied().unwrap_or(0);
        let mean_depth = s.impact.iter().sum::<f64>() / s.impact.len() as f64;
        println!(
            "| {name} | {} | {mean_depth:.2} | {r:+.3} | **{:.1}%** | {med} | {} | {} |",
            s.impact.len(),
            r * r * 100.0,
            s.routes.len(),
            s.missing_impact
        );
    }

    println!("\n## Cycles the experiment needs, at each pair's measured reduction\n");
    println!("| pair | reduction | +0.35 bps | +0.25 bps | +0.10 bps |");
    println!("|---|---|---|---|---|");
    for (i, (name, _)) in pairs.iter().enumerate() {
        let s = &series[i];
        if s.impact.len() < 30 {
            continue;
        }
        let r = corr(&s.impact[..s.impact.len() - 1], &s.impact[1..]);
        let red = r * r;
        let n = |d: f64| cycles_needed(d, 2.6, red).ceil() as u64;
        println!(
            "| {name} | {:.1}% | {} | {} | {} |",
            red * 100.0,
            n(0.35),
            n(0.25),
            n(0.10)
        );
    }

    println!(
        "\nRead the route column first. A pair that changed route more than once during the probe \
will change it during the experiment too, and each change moves venue, fee tier and depth profile \
together — those cycles are a separate stratum, not noise. A median slot gap far above 1 means the \
poll interval is coarser than the chain, so lag-1 in observations is not lag-1 in slots."
    );
    Ok(())
}
