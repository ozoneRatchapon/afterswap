//! Horizon sweep on REAL market structure (not bootstrapped).
//!
//! Bench 015 ended with a blocking dependency: block-bootstrapped paths have
//! no structure above the block scale, so they cannot answer how the engine
//! behaves at hour-scale horizons. This runs the same question on 31 days of
//! genuine 1-minute SOL/USDC bars, aggregated to progressively longer bars,
//! over many non-overlapping windows so the means carry standard errors.
//!
//! Data note: these are CEX reference prices (public Binance klines), not
//! DFlow quotes. They are used only to study market *structure* across
//! timescales — every product claim elsewhere is measured on DFlow data.
//!
//! Run: cargo run -p afterswap-engine --example real_horizon --release

use std::fmt::Write as _;

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{
    bracket_value_norm, load_corpus, simulate, tp_ladder_value_norm, trailing_stop_value_norm,
    twap_value_norm,
};

const WINDOW_BARS: usize = 200;
const OPEN_AT: usize = 30;

fn cfg() -> EngineConfig {
    EngineConfig {
        window_len: 24,
        window_stride: 12,
        n_fsm_states: 3,
        tranche_frac: 0.1,
        max_arms: 24,
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
    let bars = load_corpus("data/reference/sol_usdc_1m.jsonl").expect("reference bars");
    assert!(bars.len() > 10_000, "need the 1-minute reference series");

    let mut md = String::from("# Horizon sweep on real market structure\n\n");
    let _ = writeln!(
        md,
        "{} genuine 1-minute SOL/USDC bars (~{:.0} days), aggregated to longer bars and split into non-overlapping {WINDOW_BARS}-bar windows. Engine window 24 bars, 10% tranches. Means ±SE across windows.\n",
        bars.len(),
        bars.len() as f64 / 1440.0
    );
    let _ = writeln!(
        md,
        "Data: public CEX reference prices, **not** DFlow quotes — used only to study structure across timescales.\n"
    );
    let _ = writeln!(
        md,
        "| bar | position spans | windows | vs hold | vs TWAP | vs trailing | vs ladder | vs bracket |\n|---|---|---|---|---|---|---|---|"
    );

    for step in [1usize, 5, 15, 30, 60] {
        let series: Vec<f64> = bars.iter().step_by(step).copied().collect();
        let n_windows = series.len() / WINDOW_BARS;
        if n_windows < 2 {
            continue;
        }
        let (mut h, mut t, mut tr, mut la, mut br) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for w in 0..n_windows {
            let slice = &series[w * WINDOW_BARS..(w + 1) * WINDOW_BARS];
            let r = simulate(cfg(), slice, OPEN_AT, 1.0);
            h.push(r.edge_vs_hold_bps);
            t.push(diff_bps(r.final_value_norm, twap_value_norm(slice, OPEN_AT, 10, 6)));
            tr.push(diff_bps(
                r.final_value_norm,
                trailing_stop_value_norm(slice, OPEN_AT, 50.0),
            ));
            la.push(diff_bps(
                r.final_value_norm,
                tp_ladder_value_norm(slice, OPEN_AT, 10, 10.0),
            ));
            br.push(diff_bps(
                r.final_value_norm,
                bracket_value_norm(slice, OPEN_AT, 50.0, 50.0),
            ));
        }
        let cell = |v: &[f64]| {
            let (m, se) = stat(v);
            format!("{m:+.0} ± {se:.0}")
        };
        let span_min = step * (WINDOW_BARS - OPEN_AT);
        let _ = writeln!(
            md,
            "| {step} min | ~{:.1} h | {n_windows} | {} | {} | {} | {} | {} |",
            span_min as f64 / 60.0,
            cell(&h),
            cell(&t),
            cell(&tr),
            cell(&la),
            cell(&br)
        );
    }

    let next = std::fs::read_dir("benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().to_string_lossy().split('_').next()?.parse::<u32>().ok())
                .max()
                .map_or(1, |m| m + 1)
        })
        .unwrap_or(1);
    let dir = format!("benches/{next:03}_real_horizon");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
