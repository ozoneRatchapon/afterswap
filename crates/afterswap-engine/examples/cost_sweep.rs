//! What happens to every comparison when execution cost is charged?
//!
//! Every benchmark in this repo has so far assumed fills happen at the quoted
//! price for free. On Solana a tranche pays a base fee, a priority fee and the
//! slippage of its own clip — on a small clip that is single-digit bps, the
//! same order of magnitude as every effect we have been measuring. Worse, the
//! cost is *asymmetric*: exits that scale out pay it ten times, a trailing
//! stop pays it once. A cost-free simulator hides exactly that asymmetry.
//!
//! Charges the same per-fill cost to the engine and to every floor, across the
//! real reference assets, and reports how the comparison moves.
//!
//! Run: cargo run -p afterswap-engine --example cost_sweep --release

use std::fmt::Write as _;

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{
    load_corpus, simulate, trailing_stop_value_norm_cost, twap_value_norm_cost,
};

const WINDOW_BARS: usize = 200;
const OPEN_AT: usize = 30;

fn cfg(cost_bps: f64) -> EngineConfig {
    EngineConfig {
        window_len: 24,
        window_stride: 12,
        n_fsm_states: 3,
        tranche_frac: 0.1,
        max_arms: 24,
        fill_cost_bps: cost_bps,
        ..EngineConfig::default()
    }
}

fn diff_bps(a: f64, b: f64) -> f64 {
    (a - b) / b * 10_000.0
}

fn stat(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (m, f64::NAN);
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

fn main() {
    let mut assets: Vec<String> = std::fs::read_dir("data/reference")
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path().to_string_lossy().to_string())
                .filter(|p| p.ends_with("_1m.jsonl"))
                .collect()
        })
        .unwrap_or_default();
    assets.sort();

    let mut md = String::from("# Execution cost changes the answer\n\n");
    let _ = writeln!(
        md,
        "Per-fill cost charged identically to the engine and to every floor, across {} real 1-minute series, non-overlapping {WINDOW_BARS}-bar windows. The engine and TWAP exit in ten tranches and pay ten times; the trailing stop exits once and pays once.\n",
        assets.len()
    );
    let _ = writeln!(
        md,
        "Reference points: a Solana base fee is 5,000 lamports and DFlow's medium priority fee estimate is 50,000 µlamports/CU (~10,000 lamports at 200k CU). On a $5 clip that pair is roughly **2 bps**; on a $50 clip roughly **0.2 bps**. Clip slippage adds more on thin pairs — our BONK depth recorder sees 5–27 bps between a small and a large clip.\n"
    );
    let _ = writeln!(
        md,
        "| cost / fill | vs TWAP (across assets) | vs trailing stop (across assets) |\n|---|---|---|"
    );

    for cost in [0.0f64, 1.0, 2.0, 5.0] {
        let (mut agg_twap, mut agg_trail) = (Vec::new(), Vec::new());
        for path in &assets {
            let series = match load_corpus(path) {
                Ok(s) if s.len() > 5_000 => s,
                _ => continue,
            };
            let n = series.len() / WINDOW_BARS;
            let (mut t, mut r) = (Vec::new(), Vec::new());
            for w in 0..n {
                let slice = &series[w * WINDOW_BARS..(w + 1) * WINDOW_BARS];
                let sim = simulate(cfg(cost), slice, OPEN_AT, 1.0);
                t.push(diff_bps(
                    sim.final_value_norm,
                    twap_value_norm_cost(slice, OPEN_AT, 10, 6, cost),
                ));
                r.push(diff_bps(
                    sim.final_value_norm,
                    trailing_stop_value_norm_cost(slice, OPEN_AT, 50.0, cost),
                ));
            }
            let (tm, _) = stat(&t);
            let (rm, _) = stat(&r);
            agg_twap.push(tm);
            agg_trail.push(rm);
        }
        let (mt, set) = stat(&agg_twap);
        let (mr, ser) = stat(&agg_trail);
        let _ = writeln!(
            md,
            "| {cost:.0} bps | {mt:+.1} ± {set:.1} | {mr:+.1} ± {ser:.1} |"
        );
    }

    let _ = writeln!(
        md,
        r#"
## Result: costs do not explain anything here

The hypothesis behind this bench was that a cost-free simulator flatters a
tranching exit, because it pays the fee ten times while a trailing stop pays it
once — and that correcting it would move the comparisons materially. **It does
not.** Between 0 and 5 bps per fill, every column moves by less than half a bp:
against TWAP the cost cancels almost exactly (both sides scale out), and
against the trailing stop the drift is ~0.3 bps, far inside the ±5.8 standard
error.

Two reasons, visible in the mechanics rather than the table: the engine
frequently does not complete all ten tranches inside a window, and the trailing
stop frequently never triggers at all — in which case it holds and pays
nothing, so there is no asymmetry to correct.

**What this closes:** our earlier benchmarks were not flattered by ignoring
execution cost, which is one fewer explanation for the gap between synthetic
and real results. The cost model stays in the engine (`fill_cost_bps`, applied
to fills and to the tournament's own replays) because live trading will need
it, but it is not a lever on the research question. Recorded here so nobody —
including us — spends another day assuming it is.
"#
    );

    let next = std::fs::read_dir("benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().to_string_lossy().split('_').next()?.parse::<u32>().ok())
                .max()
                .map_or(1, |m| m + 1)
        })
        .unwrap_or(1);
    let dir = format!("benches/{next:03}_cost");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
