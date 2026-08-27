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
//! keypair, no fills, no capital. `impact_bps` rides free on every `/quote`
//! response, so the only cost is rate limit.
//!
//! Run: cargo run -p afterswap-server --example pool_probe --release -- \
//!          --minutes 60 --interval-ms 2000

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
    let pairs = candidates();
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

    let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    for seq in 0..ticks {
        interval.tick().await;
        for (i, (name, req)) in pairs.iter().enumerate() {
            let poller = PricePoller::new(client.clone(), req.clone());
            match poller.poll_snapshot(seq).await {
                Ok(snap) => {
                    match snap.impact_bps {
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
        }
        if seq % 60 == 0 && seq > 0 {
            println!("  {seq}/{ticks} observations");
        }
    }

    println!("\n| pair | n | rho lag-1 | **reduction** | median slot gap | routes | missing impact |");
    println!("|---|---|---|---|---|---|---|");
    for (i, (name, _)) in pairs.iter().enumerate() {
        let s = &series[i];
        if s.impact.len() < 30 {
            println!(
                "| {name} | {} | — | — | — | — | {} |",
                s.impact.len(),
                s.missing_impact
            );
            continue;
        }
        let r = corr(&s.impact[..s.impact.len() - 1], &s.impact[1..]);
        let mut gaps = s.slot_gaps.clone();
        gaps.sort_unstable();
        let med = gaps.get(gaps.len() / 2).copied().unwrap_or(0);
        println!(
            "| {name} | {} | {r:+.3} | **{:.1}%** | {med} | {} | {} |",
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
