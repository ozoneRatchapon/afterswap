//! Which DFlow-only signal, if any, belongs in the machines' alphabet?
//!
//! Every candidate third bit is tested on the identical protocol and code
//! path as the shipping two-bit machines: pick the best machine on TRAIN
//! windows, score it on disjoint TEST windows. A candidate that carries no
//! information cannot win, and the baseline row is the shipping protocol.
//!
//! Candidates, all derived from data only an aggregator's quotes contain:
//!   - **depth**: small-vs-large clip spread at or below its expanding median
//!   - **route change**: the filling venue differs from the previous tick
//!   - **single hop**: the route plan is direct rather than multi-hop
//!
//! Run: cargo run -p afterswap-engine --example signal_bits --release [file] [window]

use std::fmt::Write as _;

use afterswap_engine::sim::{load_quote_corpus, replay_exit, replay_exit_with_bit};
use katgpt_ruliology::FsmEnumerator;

const DEFAULT_WINDOW: usize = 60;
const PEAK_DROP_BPS: f64 = 30.0;
const TRANCHE: f64 = 0.1;

fn stat(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (m, f64::NAN);
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

/// Expanding-median comparison: no lookahead beyond the current tick.
fn below_expanding_median(series: &[f64]) -> Vec<u8> {
    let mut seen: Vec<f64> = Vec::with_capacity(series.len());
    series
        .iter()
        .map(|&x| {
            seen.push(x);
            let mut sorted = seen.clone();
            sorted.sort_by(f64::total_cmp);
            u8::from(x <= sorted[sorted.len() / 2])
        })
        .collect()
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/incoming/bonk_depth.jsonl".to_string());
    let window: usize = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WINDOW);
    let c = load_quote_corpus(&path).expect("quote corpus");
    let n_windows = c.prices.len() / window;
    if n_windows < 4 {
        println!("only {n_windows} windows in {path} — let the recorder run longer");
        return;
    }
    let split = (n_windows * 6 / 10).max(1);
    let machines = FsmEnumerator::enumerate(3);

    // Candidate bits, aligned tick-for-tick with prices.
    let depth_bits = below_expanding_median(&c.depths);
    let route_bits: Vec<u8> = c
        .venues
        .iter()
        .enumerate()
        .map(|(i, v)| u8::from(i > 0 && *v != c.venues[i - 1]))
        .collect();
    let hop_bits: Vec<u8> = c.hops.iter().map(|&h| u8::from(h <= 1)).collect();

    let candidates: Vec<(&str, Option<&Vec<u8>>)> = vec![
        ("2-bit (shipping today)", None),
        ("+ depth below median", Some(&depth_bits)),
        ("+ route changed venue", Some(&route_bits)),
        ("+ single-hop route", Some(&hop_bits)),
    ];

    let mut md = String::from("# Which DFlow-only signal belongs in the alphabet?\n\n");
    let _ = writeln!(
        md,
        "{} ticks from `{path}`, {window}-tick windows: {split} train / {} test. Best machine picked on train, scored on test, identical protocol for every candidate. Edge is vs holding, in bps.\n",
        c.prices.len(),
        n_windows - split
    );
    let _ = writeln!(md, "| third bit | selection | train | **test (±SE)** |\n|---|---|---|---|");

    for (name, bits) in candidates {
        let score = |w: usize, m: &katgpt_ruliology::FsmStrategy| {
            let lo = w * window;
            let hi = lo + window;
            match bits {
                Some(b) => replay_exit_with_bit(m, &c.prices[lo..hi], &b[lo..hi], TRANCHE, PEAK_DROP_BPS),
                None => replay_exit(m, &c.prices[lo..hi], TRANCHE, PEAK_DROP_BPS),
            }
        };
        // Single-argmax selection breaks ties by index, and with few train
        // windows the ties are enormous — the first run of this bench picked
        // the same machine for three different signals because that machine
        // ignores the third bit entirely, which reads as "no signal carries
        // information" when it actually means "the selector never looked".
        // Averaging the top-K removes the tie artefact and the selection noise.
        const TOP_K: usize = 5;
        let mut scored: Vec<(f64, usize)> = machines
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let mean: f64 = (0..split).map(|w| score(w, m)).sum::<f64>() / split as f64;
                (mean, i)
            })
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let train_best = scored[0].0;
        let ties = scored.iter().filter(|(v, _)| (*v - train_best).abs() < 1e-9).count();
        let per_window: Vec<f64> = (split..n_windows)
            .map(|w| {
                scored[..TOP_K]
                    .iter()
                    .map(|&(_, i)| score(w, &machines[i]))
                    .sum::<f64>()
                    / TOP_K as f64
            })
            .collect();
        let (tm, tse) = stat(&per_window);
        let _ = writeln!(
            md,
            "| {name} | top-{TOP_K} of {ties} tied | {train_best:+.1} | **{tm:+.1} ± {tse:.1}** |"
        );
    }

    if n_windows - split < 12 {
        let _ = writeln!(
            md,
            "\n> ⚠️ **Preliminary — {} test windows is too few to separate these.** Treat as a pipeline check; re-run as the recorder fills.\n",
            n_windows - split
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
    let dir = format!("benches/{next:03}_signal_bits");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
