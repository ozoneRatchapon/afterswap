//! Out-of-distribution validation on real data, split across time.
//!
//! Two questions at once:
//!   1. Does tuning on real history transfer forward in time? (The bootstrap
//!      experiment in bench 015 showed tuning that did *not* transfer; this
//!      is the same protocol on genuine bars, split chronologically.)
//!   2. Does the engine's value depend on the asset? Exit discipline should
//!      matter most where drawdowns are deep and fat-tailed, and least on a
//!      liquid, efficient pair.
//!
//! Parameters are chosen on the first 60% of each series and scored on the
//! last 40% — no window is ever in both halves. Data is public CEX reference
//! history, not DFlow quotes.
//!
//! Run: cargo run -p afterswap-engine --example train_test --release

use std::fmt::Write as _;

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{load_corpus, simulate, trailing_stop_value_norm, twap_value_norm};

const WINDOW_BARS: usize = 200;
const OPEN_AT: usize = 30;
const WINDOWS: [usize; 3] = [12, 24, 48];
const TRANCHES: [f64; 3] = [0.05, 0.1, 0.25];

fn cfg(window: usize, tranche: f64) -> EngineConfig {
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

fn stat(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (m, f64::NAN);
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

/// Mean edge vs TWAP and vs hold over every non-overlapping window.
fn score(series: &[f64], window: usize, tranche: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = series.len() / WINDOW_BARS;
    let (mut twap_d, mut hold_d, mut trail_d) = (Vec::new(), Vec::new(), Vec::new());
    for w in 0..n {
        let slice = &series[w * WINDOW_BARS..(w + 1) * WINDOW_BARS];
        let r = simulate(cfg(window, tranche), slice, OPEN_AT, 1.0);
        twap_d.push(diff_bps(r.final_value_norm, twap_value_norm(slice, OPEN_AT, 10, 6)));
        hold_d.push(r.edge_vs_hold_bps);
        trail_d.push(diff_bps(
            r.final_value_norm,
            trailing_stop_value_norm(slice, OPEN_AT, 50.0),
        ));
    }
    (twap_d, hold_d, trail_d)
}

fn main() {
    // Every reference series present, so adding an asset means dropping a
    // file in — no cherry-picking a favourable subset.
    let mut assets: Vec<(String, String)> = std::fs::read_dir("data/reference")
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path().to_string_lossy().to_string())
                .filter(|p| p.ends_with("_1m.jsonl"))
                .map(|p| {
                    let name = p
                        .rsplit('/')
                        .next()
                        .unwrap_or("?")
                        .trim_end_matches("_1m.jsonl")
                        .to_uppercase();
                    (name, p)
                })
                .collect()
        })
        .unwrap_or_default();
    assets.sort();

    let mut md = String::from("# Train/test across time, on real bars\n\n");
    let _ = writeln!(
        md,
        "Parameters chosen on the first 60% of each 1-minute series, scored on the last 40%. Non-overlapping {WINDOW_BARS}-bar windows; means ±SE. Public CEX reference history, not DFlow quotes.\n"
    );
    let _ = writeln!(
        md,
        "| asset | test windows | best on train | test vs TWAP | test vs trailing | test vs hold | baseline (24/10%) vs TWAP |\n|---|---|---|---|---|---|---|"
    );

    let mut agg_twap: Vec<f64> = Vec::new();
    let mut agg_trail: Vec<f64> = Vec::new();
    for (name, path) in &assets {
        let series = match load_corpus(path) {
            Ok(s) if s.len() > 5_000 => s,
            _ => continue,
        };
        let split = series.len() * 6 / 10;
        let (train, test) = (&series[..split], &series[split..]);

        let mut best = (f64::NEG_INFINITY, WINDOWS[1], TRANCHES[1]);
        for &w in &WINDOWS {
            for &t in &TRANCHES {
                let (twap_d, _, _) = score(train, w, t);
                let (m, _) = stat(&twap_d);
                if m > best.0 {
                    best = (m, w, t);
                }
            }
        }
        let (twap_d, hold_d, trail_d) = score(test, best.1, best.2);
        let (base_twap, _, _) = score(test, 24, 0.1);
        let (tm, tse) = stat(&twap_d);
        let (hm, hse) = stat(&hold_d);
        let (rm, rse) = stat(&trail_d);
        let (bm, bse) = stat(&base_twap);
        agg_twap.push(tm);
        agg_trail.push(rm);
        let _ = writeln!(
            md,
            "| {name} | {} | w{} / {:.0}% | {tm:+.0} ± {tse:.0} | {rm:+.0} ± {rse:.0} | {hm:+.0} ± {hse:.0} | {bm:+.0} ± {bse:.0} |",
            twap_d.len(),
            best.1,
            best.2 * 100.0
        );
    }

    // Across-asset aggregate: the per-asset SEs describe window noise, this
    // one asks whether the effect survives asset selection at all.
    if agg_twap.len() >= 3 {
        let (mt, set) = stat(&agg_twap);
        let (mr, ser) = stat(&agg_trail);
        let _ = writeln!(
            md,
            "\n## Across {} assets (SE over assets, not windows)\n\n- vs TWAP: **{mt:+.1} ± {set:.1} bps**\n- vs trailing stop: **{mr:+.1} ± {ser:.1} bps**\n\nEach asset contributes one number, so this asks whether the result survives\nasset selection rather than whether one asset's windows were lucky.\n",
            agg_twap.len()
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
    let dir = format!("benches/{next:03}_train_test");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
