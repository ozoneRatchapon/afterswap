//! Does institutional size break the surface tension of a deep AMM?
//!
//! The pool probe found a paradox. CUPED needs depth *variation* to regress on,
//! and bench 038 got 34.6% from BONK because BONK is shallow — mean spread
//! 15.23 bps. On the pairs whose net margin is actually positive, a retail clip
//! registers nothing: SOL/USDC returned a mean spread of −0.04 bps at 10x, with
//! rho = +0.006. No signal, no control variate, no reduction.
//!
//! So the control variate and the target margin looked mutually exclusive. But
//! that conclusion was drawn at one clip size. A deep pool is deep *relative to
//! the clip*, and impact on a concentrated-liquidity pool grows sharply once an
//! order walks past the active tick range. This sweeps clip size to find where
//! that happens — if it happens inside a notional anyone would trade.
//!
//! Quotes only. No keypair, no fills, no capital. Each observation prices the
//! same pair at every size against the same chain state, so the spreads are
//! directly comparable rather than sampled at different moments.
//!
//! ```sh
//! cargo run -p afterswap-server --example clip_probe --release -- \
//!     --minutes 20 --interval-ms 4000
//! ```

use std::collections::BTreeMap;
use std::time::Duration;

use afterswap_dflow::{DflowClient, PricePoller, QuoteRequest, mints};
use afterswap_engine::cuped::cycles_needed;

/// Multiples of the base clip. 1 SOL is retail; 1,000 SOL is roughly $100k at
/// current prices, which is where an execution desk lives.
const MULTIPLES: [u64; 4] = [1, 10, 100, 1_000];
const BASE_AMOUNT: u64 = 1_000_000_000; // 1 SOL

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

fn sd(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    match n > 1.0 {
        true => (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt(),
        false => 0.0,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let minutes: u64 = arg(&args, "--minutes").unwrap_or(20);
    let interval_ms: u64 = arg(&args, "--interval-ms").unwrap_or(4_000);
    let ticks = (minutes * 60_000) / interval_ms;

    let client = DflowClient::dev();
    let base = QuoteRequest {
        input_mint: mints::SOL.to_string(),
        output_mint: mints::USDC.to_string(),
        amount: BASE_AMOUNT,
        slippage_bps: 50,
    };

    // Spread of each clip against the 1x reference, per observation.
    let mut spreads: Vec<Vec<f64>> = vec![Vec::new(); MULTIPLES.len()];
    let mut routes: Vec<BTreeMap<String, usize>> = vec![BTreeMap::new(); MULTIPLES.len()];
    let mut errors = 0usize;

    println!("SOL/USDC, clips {MULTIPLES:?} x 1 SOL, {ticks} observations at {interval_ms} ms\n");

    let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    for seq in 0..ticks {
        interval.tick().await;
        let mut prices = Vec::with_capacity(MULTIPLES.len());
        let mut ok = true;
        for (i, m) in MULTIPLES.iter().enumerate() {
            let req = QuoteRequest {
                amount: BASE_AMOUNT.saturating_mul(*m),
                ..base.clone()
            };
            let poller = PricePoller::new(client.clone(), req);
            match poller.poll_snapshot(seq).await {
                Ok(s) => {
                    let route = format!("{}|{}", s.venue.as_deref().unwrap_or("?"), s.hops);
                    *routes[i].entry(route).or_insert(0) += 1;
                    prices.push(s.price);
                }
                Err(e) => {
                    errors += 1;
                    if errors <= 3 {
                        eprintln!("quote failed at {}x: {e}", MULTIPLES[i]);
                    }
                    ok = false;
                    break;
                }
            }
            // The endpoint 429s under bursts; four sizes per tick needs spacing.
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        // A partial observation cannot be compared against its own reference.
        if !ok || prices.len() != MULTIPLES.len() {
            continue;
        }
        let reference = prices[0];
        for (i, p) in prices.iter().enumerate() {
            spreads[i].push((reference - p) / reference * 10_000.0);
        }
        if seq % 30 == 0 && seq > 0 {
            println!("  {seq}/{ticks}");
        }
    }

    let n = spreads[0].len();
    println!("\n{n} complete observations, {errors} failed quotes\n");
    println!("| clip | notional (SOL) | mean spread bps | sd | rho lag-1 | **CUPED reduction** | routes |");
    println!("|---|---|---|---|---|---|---|");
    let mut best = (0u64, 0.0f64);
    for (i, m) in MULTIPLES.iter().enumerate() {
        let s = &spreads[i];
        if s.len() < 10 {
            println!("| {m}x | {m} | — | — | — | — | — |");
            continue;
        }
        let r = corr(&s[..s.len() - 1], &s[1..]);
        let red = r * r;
        if red > best.1 {
            best = (*m, red);
        }
        println!(
            "| {m}x | {m} | {:+.3} | {:.3} | {r:+.3} | **{:.1}%** | {} |",
            s.iter().sum::<f64>() / s.len() as f64,
            sd(s),
            red * 100.0,
            routes[i].len()
        );
    }

    println!("\n## What each clip would cost the experiment\n");
    println!("| clip | reduction | cycles for +0.35 bps | +0.25 bps | +0.10 bps |");
    println!("|---|---|---|---|---|");
    for (i, m) in MULTIPLES.iter().enumerate() {
        if spreads[i].len() < 10 {
            continue;
        }
        let s = &spreads[i];
        let red = corr(&s[..s.len() - 1], &s[1..]).powi(2);
        let c = |d: f64| cycles_needed(d, 2.6, red).ceil() as u64;
        println!(
            "| {m}x | {:.1}% | {} | {} | {} |",
            red * 100.0,
            c(0.35),
            c(0.25),
            c(0.10)
        );
    }

    println!(
        "\nThe 1x row is the reference and is zero by construction — read it as a check that the \
harness is differencing correctly, not as a result.\n\nWhat decides this: a clip whose spread has \
both a non-trivial mean **and** a lag-1 correlation. Mean without persistence is a constant cost, \
not a covariate — CUPED regresses on variation, so a spread that is large but unpredictable \
reduces nothing. Best reduction found: {:.1}% at {}x.",
        best.1 * 100.0,
        best.0
    );
    Ok(())
}
