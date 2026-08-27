//! Does DFlow's executable depth carry exit signal?
//!
//! Cheap test before any engine refactor: at the strategy-replay level, pick
//! the best machine on TRAIN windows under each protocol and score it on
//! disjoint TEST windows.
//!   2-bit: direction, off-peak (what ships today)
//!   3-bit: direction, off-peak, good-depth (small/large clip spread at or
//!          below its expanding median)
//! If the third bit carries nothing, the 3-bit protocol will not beat the
//! 2-bit one out-of-sample and the refactor is not worth its risk.
//!
//! Run: cargo run -p afterswap-engine --example depth_bit --release

use std::fmt::Write as _;

use afterswap_engine::sim::{load_depth_corpus, replay_exit, replay_exit_depth};
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
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

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/incoming/bonk_depth.jsonl".to_string());
    let (prices, depths) = load_depth_corpus(&path).expect("depth corpus");
    let n_windows = prices.len() / WINDOW;
    if n_windows < 6 {
        println!("only {n_windows} windows in {path} — let the recorder run longer");
        return;
    }
    let split = n_windows * 6 / 10;
    let machines = FsmEnumerator::enumerate(3);

    let score = |w: usize, m: &katgpt_ruliology::FsmStrategy, depth: bool| {
        let lo = w * WINDOW;
        let hi = lo + WINDOW;
        match depth {
            true => replay_exit_depth(m, &prices[lo..hi], &depths[lo..hi], TRANCHE, PEAK_DROP_BPS),
            false => replay_exit(m, &prices[lo..hi], TRANCHE, PEAK_DROP_BPS),
        }
    };

    let mut md = String::from("# Does DFlow depth carry exit signal?\n\n");
    let _ = writeln!(
        md,
        "{} ticks from `{path}`, {WINDOW}-tick windows: {split} train / {} test. Best machine chosen on train under each protocol, scored on test. Edge is vs holding, in bps.\n",
        prices.len(),
        n_windows - split
    );
    let _ = writeln!(md, "| protocol | best machine (train) | train edge | **test edge (±SE)** |\n|---|---|---|---|");

    for depth in [false, true] {
        let mut best = (f64::NEG_INFINITY, 0usize);
        for (i, m) in machines.iter().enumerate() {
            let mean: f64 = (0..split).map(|w| score(w, m, depth)).sum::<f64>() / split as f64;
            if mean > best.0 {
                best = (mean, i);
            }
        }
        let test: Vec<f64> = (split..n_windows)
            .map(|w| score(w, &machines[best.1], depth))
            .collect();
        let (tm, tse) = stat(&test);
        let _ = writeln!(
            md,
            "| {} | #{} | {:+.1} bps | **{tm:+.1} ± {tse:.1} bps** |",
            match depth {
                true => "3-bit (with depth)",
                false => "2-bit (shipping today)",
            },
            best.1,
            best.0
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
    let dir = format!("benches/{next:03}_depth_bit");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
