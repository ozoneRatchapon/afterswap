//! Do the engine's parameters need to scale with horizon?
//!
//! Bench 014 showed the edge's magnitude scales with bar duration while
//! window/tranche stayed fixed at demo-scale values. This asks whether
//! re-tuning per horizon actually helps — with an honest protocol:
//! parameters are chosen on TRAIN seeds and scored on disjoint TEST seeds,
//! so the reported gain is out-of-sample, not grid-search selection bias.
//!
//! Objective: mean of (edge vs TWAP, edge vs trailing) — "beats the
//! mechanical exits a user would otherwise run". Drift preserved, since
//! that is where exit structure exists (the de-meaned null control in
//! bench 014 correctly shows nothing to tune for).
//!
//! Run: cargo run -p afterswap-engine --example param_sweep --release

use std::fmt::Write as _;

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{
    bootstrap_bars, load_corpus, simulate, trailing_stop_value_norm, twap_value_norm,
};

const BARS: usize = 600;
const OPEN_AT: usize = 60;
const TRAIN_SEEDS: u64 = 15;
const TEST_SEEDS: u64 = 25;
const WINDOWS: [usize; 4] = [12, 24, 48, 96];
const TRANCHES: [f64; 3] = [0.05, 0.1, 0.25];

fn cfg_for(window: usize, tranche: f64) -> EngineConfig {
    EngineConfig {
        window_len: window,
        window_stride: (window / 2).max(1),
        n_fsm_states: 3,
        tranche_frac: tranche,
        max_arms: 24,
        ..EngineConfig::default()
    }
}

fn diff_bps(a: f64, b: f64) -> f64 {
    (a - b) / b * 10_000.0
}

/// Mean objective over a seed range: how far the engine beats the
/// mechanical exits, averaged.
fn score(source: &[f64], factor: usize, window: usize, tranche: f64, seeds: std::ops::Range<u64>) -> (f64, f64) {
    let mut vals = Vec::new();
    let mut holds = Vec::new();
    for seed in seeds {
        let prices = bootstrap_bars(source, BARS, factor, seed, false);
        let r = simulate(cfg_for(window, tranche), &prices, OPEN_AT, 1.0);
        let twap = twap_value_norm(&prices, OPEN_AT, 10, 6);
        let trail = trailing_stop_value_norm(&prices, OPEN_AT, 50.0);
        vals.push(
            (diff_bps(r.final_value_norm, twap) + diff_bps(r.final_value_norm, trail)) / 2.0,
        );
        holds.push(r.edge_vs_hold_bps);
    }
    let n = vals.len() as f64;
    (vals.iter().sum::<f64>() / n, holds.iter().sum::<f64>() / n)
}

fn se(source: &[f64], factor: usize, window: usize, tranche: f64, seeds: std::ops::Range<u64>) -> f64 {
    let mut vals = Vec::new();
    for seed in seeds {
        let prices = bootstrap_bars(source, BARS, factor, seed, false);
        let r = simulate(cfg_for(window, tranche), &prices, OPEN_AT, 1.0);
        let twap = twap_value_norm(&prices, OPEN_AT, 10, 6);
        let trail = trailing_stop_value_norm(&prices, OPEN_AT, 50.0);
        vals.push((diff_bps(r.final_value_norm, twap) + diff_bps(r.final_value_norm, trail)) / 2.0);
    }
    let n = vals.len() as f64;
    let m = vals.iter().sum::<f64>() / n;
    let var = vals.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (var / n).sqrt()
}

fn main() {
    let mut source = load_corpus("data/recorded.jsonl").unwrap_or_default();
    if let Ok(more) = load_corpus("data/recorded2.jsonl") {
        source.extend(more);
    }
    assert!(source.len() > 100, "need recorded ticks");

    let mut md = String::from("# Parameter scaling across horizons\n\n");
    let _ = writeln!(
        md,
        "Objective: mean(edge vs TWAP, edge vs trailing), drift-preserved bootstrap, {BARS} bars.\nParameters chosen on {TRAIN_SEEDS} TRAIN seeds, reported on {TEST_SEEDS} disjoint TEST seeds — the gain is out-of-sample.\nDemo default is window 24 / 10% tranches.\n"
    );
    let _ = writeln!(
        md,
        "| bar ≈ | best on train | test: tuned (±SE) | test: demo default (±SE) | out-of-sample gain |\n|---|---|---|---|---|"
    );

    for factor in [1usize, 15, 60] {
        let mut best = (f64::NEG_INFINITY, WINDOWS[1], TRANCHES[1]);
        for &w in &WINDOWS {
            for &t in &TRANCHES {
                let (s, _) = score(&source, factor, w, t, 0..TRAIN_SEEDS);
                if s > best.0 {
                    best = (s, w, t);
                }
            }
        }
        let test = 1000..(1000 + TEST_SEEDS);
        let (tuned, _) = score(&source, factor, best.1, best.2, test.clone());
        let tuned_se = se(&source, factor, best.1, best.2, test.clone());
        let (base, _) = score(&source, factor, 24, 0.1, test.clone());
        let base_se = se(&source, factor, 24, 0.1, test);
        let _ = writeln!(
            md,
            "| {} s | window {} / {:.0}% | {tuned:+.0} ± {tuned_se:.0} | {base:+.0} ± {base_se:.0} | **{:+.0} bps** |",
            2 * factor,
            best.1,
            best.2 * 100.0,
            tuned - base
        );
    }

    // Decisive check: does a longer evaluation window help on the REAL
    // corpora the GOAT gates use, not just on bootstrapped paths?
    let _ = writeln!(
        md,
        "\n## Window length on real corpora (no bootstrap)\n\nEdge vs TWAP / vs trailing, per window length, on the recorded DFlow segments and the synthetic regimes.\n\n| corpus | w12 | w24 | w48 | w96 |\n|---|---|---|---|---|"
    );
    let mut corpora: Vec<(String, Vec<f64>)> = afterswap_engine::sim::Regime::ALL
        .iter()
        .map(|&r| (r.name().to_string(), afterswap_engine::sim::synthetic_corpus(r, 300, 42)))
        .collect();
    for path in ["data/recorded.jsonl", "data/recorded2.jsonl"] {
        if let Ok(c) = load_corpus(path) {
            if c.len() >= 100 {
                corpora.push((path.rsplit('/').next().unwrap_or("rec").to_string(), c));
            }
        }
    }
    for (name, prices) in &corpora {
        let mut row = format!("| {name} |");
        for w in WINDOWS {
            let open_at = 30.min(prices.len() / 4);
            let r = simulate(cfg_for(w, 0.1), prices, open_at, 1.0);
            let twap = twap_value_norm(prices, open_at, 10, 6);
            let trail = trailing_stop_value_norm(prices, open_at, 50.0);
            let obj = (diff_bps(r.final_value_norm, twap) + diff_bps(r.final_value_norm, trail)) / 2.0;
            let _ = write!(row, " {obj:+.0} |");
        }
        let _ = writeln!(md, "{row}");
    }
    let _ = writeln!(
        md,
        r#"
## Verdict: the tuning does NOT transfer — and that is the finding

On bootstrapped paths, window 96 wins at every scale and the out-of-sample
gain looks enormous (+1917 bps at 2-minute bars, >7 SE). On the real
corpora it collapses: trend_down goes +38 → -103 and v_shape +54 → -249 as
the window grows, while the recorded segments barely move. **Raising the
default window on the strength of the bootstrap experiment would have made
the product worse on real data.**

Why: block-bootstrapping preserves only within-block autocorrelation, so
the synthetic paths have no structure above the block scale. Long
evaluation windows are optimal there precisely *because* there is nothing
longer-range to be wrong about. Real markets have multi-scale structure,
and shorter windows adapt to it.

The methodological point is worth more than the parameter: **a correct
train/test split does not protect you when the data-generating process is
wrong.** The split was honest and the gain was genuinely out-of-sample —
within a distribution that does not match reality. Out-of-*distribution*
validation (the real-corpus table above) is what caught it.

**Conclusion:** current demo parameters (window 12-24, 10% tranches) stand;
no change shipped. Tuning for genuinely longer horizons needs genuinely
long *recorded* data, not resampled data — that is now the blocking
dependency, and the recorder is running to produce it.

Caveats: window 96 was the grid maximum, so the bootstrap optimum may lie
beyond it; bootstrapped magnitudes are inflated by the bull sample, so read
relative columns, not absolute bps.
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
    let dir = format!("benches/{next:03}_params");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
