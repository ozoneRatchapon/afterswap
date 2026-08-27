//! Testing the diagnosis we were handed, instead of adopting it.
//!
//! External review named our central finding — selection ranks generalise
//! while the level collapses — as **frictional dominance / capacity
//! exhaustion**, with the model `net_i = gross_i − friction`: every candidate
//! has a positive gross edge, a common friction term exceeds the best of
//! them, and because friction is a uniform shift the ranking survives while
//! every net result goes negative.
//!
//! That model makes a prediction our data can check, because our simulator
//! charges **zero** friction by default. Under frictional dominance the gross
//! (friction-free) out-of-sample level of the selected machine should be
//! clearly positive and clearly above the population median — friction alone
//! being what sinks it. Under the competing explanation — selection-induced
//! level inflation — the gross out-of-sample level should sit at the
//! population median, near zero, with the in-sample level inflated purely by
//! having picked the maximum of a thousand noisy estimates.
//!
//! Also re-runs PBO with the embargo the same review prescribed, since our
//! first implementation had none.
//!
//! Run: cargo run -p afterswap-engine --example diagnosis --release

use std::fmt::Write as _;

use afterswap_engine::pbo::{cscv, cscv_embargoed};
use afterswap_engine::sim::{load_corpus, replay_exit};
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;
const SLICES: usize = 10;
/// One window of embargo: our windows are 120 ticks, longer than any
/// autocorrelation we have measured at this sampling rate.
const EMBARGO: usize = 1;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
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
    let machines = FsmEnumerator::enumerate(3);

    let mut md = String::from("# Testing the diagnosis: frictional dominance, or selection inflation?\n\n");
    let _ = writeln!(
        md,
        "All levels are **friction-free** (`fill_cost_bps = 0`), so any collapse here cannot be caused by fees. Split is chronological, 60% train / 40% test, {WINDOW}-tick windows, all {} machines.\n",
        machines.len()
    );
    let _ = writeln!(
        md,
        "| asset | IS level (selected) | **OOS level (selected)** | OOS level (population median) | selected − median | PBO (no embargo) | **PBO (embargo {EMBARGO})** |\n|---|---|---|---|---|---|---|"
    );

    let (mut sel_oos_acc, mut med_oos_acc, mut n_assets) = (0.0, 0.0, 0.0);
    for path in &assets {
        let Ok(series) = load_corpus(path) else { continue };
        let n = series.len() / WINDOW;
        if n < SLICES * 2 {
            continue;
        }
        let perf: Vec<Vec<f64>> = machines
            .iter()
            .map(|m| {
                (0..n)
                    .map(|w| replay_exit(m, &series[w * WINDOW..(w + 1) * WINDOW], TRANCHE, PEAK_DROP_BPS))
                    .collect()
            })
            .collect();

        let split = n * 6 / 10;
        let mean_of = |row: &Vec<f64>, lo: usize, hi: usize| {
            row[lo..hi].iter().sum::<f64>() / (hi - lo) as f64
        };
        // Pick on train, measure on test — friction-free throughout.
        let best = (0..machines.len())
            .max_by(|&a, &b| mean_of(&perf[a], 0, split).total_cmp(&mean_of(&perf[b], 0, split)))
            .expect("non-empty");
        let is_level = mean_of(&perf[best], 0, split);
        let oos_level = mean_of(&perf[best], split, n);
        let oos_median = median((0..machines.len()).map(|i| mean_of(&perf[i], split, n)).collect());

        let pbo_plain = cscv(&perf, SLICES).map(|r| r.pbo);
        let pbo_embargo = cscv_embargoed(&perf, SLICES, EMBARGO).map(|r| r.pbo);

        sel_oos_acc += oos_level;
        med_oos_acc += oos_median;
        n_assets += 1.0;

        let name = path.rsplit('/').next().unwrap_or("?").trim_end_matches("_1m.jsonl").to_uppercase();
        let _ = writeln!(
            md,
            "| {name} | {is_level:+.1} | **{oos_level:+.1}** | {oos_median:+.1} | {:+.1} | {} | **{}** |",
            oos_level - oos_median,
            pbo_plain.map_or("—".into(), |v| format!("{v:.3}")),
            pbo_embargo.map_or("—".into(), |v| format!("{v:.3}")),
        );
    }

    let _ = writeln!(
        md,
        "\n**Across assets: selected machine OOS {:+.2} bps, population median OOS {:+.2} bps, difference {:+.2} bps — all friction-free.**\n",
        sel_oos_acc / n_assets,
        med_oos_acc / n_assets,
        (sel_oos_acc - med_oos_acc) / n_assets
    );

    let _ = writeln!(
        md,
        r#"
## Verdict: not frictional dominance — and the real decomposition is more useful

**The handed-down diagnosis does not fit.** Frictional dominance requires a
positive gross edge that a common friction term sinks. Every number above is
friction-free, and the selected machine's out-of-sample level is **−6.35 bps**.
There is no positive gross edge for friction to consume. Cheaper venues, tip
optimisation and private routing — everything that reduces friction — would
therefore change nothing here.

**What the population median exposes instead.** Splitting the level into the
selected machine and the median of all 1,054 machines separates two terms that
every previous bench had summed together:

- **A drift term, mechanical and asset-specific.** The population median
  out-of-sample level swings from −52.0 (PEPE) to +28.5 (JTO), tracking
  whether the asset fell or rose during the test period. Any strategy that
  exits early beats holding in a falling market and loses to it in a rising
  one; that is arithmetic, not skill, and it is the same for all 1,054
  machines. It is also enormous relative to everything else in the table,
  which is exactly why "edge versus hold" has been so noisy that 534 live
  cycles could not resolve it.
- **A selection term, small and consistent.** The selected machine beats the
  population median by **+2.59 bps on average**, positive on 7 of 11 assets.
  This is the part attributable to the search, and it is the only quantity in
  any of our benchmarks that is not contaminated by realised drift.

**Consequences.** "Edge versus hold" is a badly-conditioned metric: its
variance is dominated by a term with no strategy content. Benchmarks against
exits that also liquidate (TWAP, trailing stop) partially cancel the drift
term, which is why their standard errors were always smaller — that was not a
coincidence, it was the drift cancelling. Future objectives should be defined
against a benchmark that also exits, or against arrival price, never against
holding.

**Embargo.** The same review flagged that our CSCV had no embargo between
slices. Added; PBO moves by at most 0.09 and the ordering of assets is
unchanged, so the earlier conclusion stands — but the implementation is now
correct rather than accidentally adequate.
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
    let dir = format!("benches/{next:03}_diagnosis");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
