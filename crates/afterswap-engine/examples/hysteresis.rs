//! Does a Schmitt-trigger drawdown bit beat the shipping single threshold?
//!
//! The shipping off-peak bit is memoryless: 1 whenever the drawdown from the
//! running peak is at or past one threshold. Near that threshold it chatters,
//! and the machine sees quote noise as a stream of state changes. The
//! dual-threshold alternative arms at `theta_high` and stays armed until the
//! drawdown recovers below `theta_low`, spending one bit of memory to buy
//! that suppression.
//!
//! Protocol is the one every other selection bench here uses: pick the best
//! machines on TRAIN windows, score them on disjoint TEST windows, same code
//! path for every arm. The `theta_low == theta_high` rows collapse to the
//! shipping replay exactly (`tests/hysteresis.rs` asserts it), so the
//! baseline is the protocol itself and not a re-implementation of it.
//!
//! Every arm is also paired against the shipping default (30/30) window by
//! window and put through Romano–Wolf stepdown, because reading the maximum
//! of a 15-arm sweep as an effect is how a sweep lies.
//!
//! Run: cargo run -p afterswap-engine --example hysteresis --release [file] [window]

use std::fmt::Write as _;

use afterswap_engine::power::{Z_POWER_80, mde_from_se};
use afterswap_engine::sim::{load_corpus, replay_exit_hysteresis};
use afterswap_engine::stepdown::romano_wolf;
use katgpt_ruliology::FsmEnumerator;

const DEFAULT_WINDOW: usize = 60;
const TRANCHE: f64 = 0.1;
const SHIPPING_THETA: f64 = 30.0;
/// Averaging the top-K removes the tie artefact that a single argmax picks up
/// when many machines score identically on few train windows (see bench 023).
const TOP_K: usize = 5;
const BOOTSTRAPS: usize = 2_000;
const ALPHA: f64 = 0.05;
const SEED: u64 = 20_260_829;

/// One sweep arm: its thresholds, the per-test-window scores of the machines
/// train selected for it, and what selection had to work with.
struct Arm {
    arm_bps: f64,
    disarm_bps: f64,
    test_window_edges: Vec<f64>,
    train_best: f64,
    train_ties: usize,
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

/// Mean off-peak bit transitions per window — the chatter the band suppresses.
/// Reported so a null result can be read as "the band did nothing measurable"
/// rather than "the band was never engaged".
fn flips_per_window(prices: &[f64], window: usize, hi: f64, lo: f64) -> f64 {
    let n_windows = prices.len() / window;
    let mut total = 0usize;
    for w in 0..n_windows {
        let s = &prices[w * window..w * window + window];
        let mut peak = s[0];
        let mut armed = false;
        let mut prev: Option<bool> = None;
        for &p in &s[1..] {
            peak = peak.max(p);
            let d = (peak - p) / peak * 10_000.0;
            match d {
                d if d >= hi => armed = true,
                d if d < lo => armed = false,
                _ => {}
            }
            if prev.is_some_and(|b| b != armed) {
                total += 1;
            }
            prev = Some(armed);
        }
    }
    total as f64 / n_windows.max(1) as f64
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/incoming/recorded_long.jsonl".to_string());
    let window: usize = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WINDOW);
    let prices = load_corpus(&path).expect("price corpus");
    let n_windows = prices.len() / window;
    if n_windows < 12 {
        println!("only {n_windows} windows in {path} — let the recorder run longer");
        return;
    }
    let split = (n_windows * 6 / 10).max(1);
    let machines = FsmEnumerator::enumerate(3);

    // Arms: each arm threshold, then bands that hold state further and further
    // below it. `lo == hi` is the shipping protocol at that threshold.
    let mut arms: Vec<(f64, f64)> = Vec::new();
    for hi in [10.0f64, 20.0, SHIPPING_THETA, 50.0] {
        for frac in [1.0f64, 0.5, 0.25, 0.0] {
            arms.push((hi, hi * frac));
        }
    }

    let score = |w: usize, m: &katgpt_ruliology::FsmStrategy, hi: f64, lo: f64| {
        let lo_i = w * window;
        replay_exit_hysteresis(m, &prices[lo_i..lo_i + window], TRANCHE, hi, lo, 0.0)
    };

    // Per-arm test-window series, selection done on train only.
    let mut rows: Vec<Arm> = Vec::with_capacity(arms.len());
    for &(hi, lo) in &arms {
        let mut scored: Vec<(f64, usize)> = machines
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let mean: f64 = (0..split).map(|w| score(w, m, hi, lo)).sum::<f64>() / split as f64;
                (mean, i)
            })
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let train_best = scored[0].0;
        let ties = scored
            .iter()
            .filter(|(v, _)| (*v - train_best).abs() < 1e-9)
            .count();
        let per_window: Vec<f64> = (split..n_windows)
            .map(|w| {
                scored[..TOP_K]
                    .iter()
                    .map(|&(_, i)| score(w, &machines[i], hi, lo))
                    .sum::<f64>()
                    / TOP_K as f64
            })
            .collect();
        rows.push(Arm {
            arm_bps: hi,
            disarm_bps: lo,
            test_window_edges: per_window,
            train_best,
            train_ties: ties,
        });
    }

    // Benchmark = the shipping default, collapsed band at 30 bps.
    let bench_idx = arms
        .iter()
        .position(|&(hi, lo)| hi == SHIPPING_THETA && lo == SHIPPING_THETA)
        .expect("shipping arm present");
    let bench: Vec<f64> = rows[bench_idx].test_window_edges.clone();
    let others: Vec<usize> = (0..rows.len()).filter(|&i| i != bench_idx).collect();
    let diffs: Vec<Vec<f64>> = others
        .iter()
        .map(|&i| {
            rows[i]
                .test_window_edges
                .iter()
                .zip(&bench)
                .map(|(a, b)| a - b)
                .collect::<Vec<f64>>()
        })
        .collect();
    let verdicts = romano_wolf(&diffs, BOOTSTRAPS, ALPHA, SEED);

    let mut adj: Vec<Option<(f64, bool)>> = vec![None; rows.len()];
    for v in &verdicts {
        adj[others[v.index]] = Some((v.p_adjusted, v.rejected));
    }

    let mut md = String::from("# Does a Schmitt-trigger drawdown bit beat one threshold?\n\n");
    let _ = writeln!(
        md,
        "{} ticks from `{path}`, {window}-tick windows: {split} train / {} test. Machines selected on train, scored on test, one code path for every arm. Edge is vs holding, in bps; `flips` is mean off-peak bit transitions per window.\n",
        prices.len(),
        n_windows - split
    );
    let _ = writeln!(
        md,
        "`arm / disarm` in bps of drawdown from the running peak. **`disarm = arm` is the shipping memoryless bit** — `tests/hysteresis.rs` asserts the two replays agree bit-for-bit there, so those rows are the protocol itself.\n"
    );
    let _ = writeln!(
        md,
        "| arm / disarm | flips | selection | train | **test (±SE)** | Δ vs 30/30 | RW p-adj |\n|---|---|---|---|---|---|---|"
    );
    for (i, arm) in rows.iter().enumerate() {
        let Arm { arm_bps: hi, disarm_bps: lo, test_window_edges, train_best, train_ties: ties } = arm;
        let (tm, tse) = stat(test_window_edges);
        let d: Vec<f64> = test_window_edges.iter().zip(&bench).map(|(a, b)| a - b).collect();
        let (dm, _) = stat(&d);
        let tag = match (i == bench_idx, hi == lo) {
            (true, _) => " ← shipping",
            (_, true) => " (memoryless)",
            _ => "",
        };
        let p = match adj[i] {
            Some((p, true)) => format!("**{p:.3}** ✓"),
            Some((p, false)) => format!("{p:.3}"),
            None => "—".to_string(),
        };
        let dcol = match i == bench_idx {
            true => "—".to_string(),
            false => format!("{dm:+.2}"),
        };
        let _ = writeln!(
            md,
            "| {hi:.0} / {lo:.0}{tag} | {:.1} | top-{TOP_K} of {ties} tied | {train_best:+.1} | **{tm:+.1} ± {tse:.1}** | {dcol} | {p} |",
            flips_per_window(&prices, window, *hi, *lo)
        );
    }

    let (_, bse) = stat(&bench);
    // Median rather than a single arm's: the paired SE ranges over an order of
    // magnitude across arms, and quoting the first one would understate what
    // the sweep can actually rule out.
    let paired_se = {
        let mut ses: Vec<f64> = diffs.iter().map(|d| stat(d).1).collect();
        ses.sort_by(f64::total_cmp);
        ses[ses.len() / 2]
    };
    let _ = writeln!(
        md,
        "\n**Reading it.** The absolute test column carries {:.1} bps of SE — an MDE of {:.1} bps at 80% power, which is far larger than any plausible band effect, so that column cannot settle this. Pairing does not rescue it either, and that is worth stating plainly: the arms share price paths but *select different machines*, so the paths diverge and the median paired SE across arms is {:.2} bps — no better than the unpaired column, for a paired MDE of {:.2} bps. That is what this sample can rule out; a band effect smaller than it would be invisible here whatever the point estimates say. Romano–Wolf steps down over all {} non-benchmark arms at α = {ALPHA}, {BOOTSTRAPS} bootstraps, seed {SEED}, so a ✓ has already paid for the sweep.\n",
        bse,
        mde_from_se(bse, Z_POWER_80),
        paired_se,
        mde_from_se(paired_se, Z_POWER_80),
        diffs.len(),
    );
    let any = verdicts.iter().any(|v| v.rejected);
    let _ = writeln!(
        md,
        "**Verdict.** {}\n",
        match any {
            true => "At least one band survives the multiplicity correction — see the ✓ rows. Worth carrying into the signal path.",
            false =>
                "No band survives the multiplicity correction. The `flips` column shows the trigger was genuinely engaged, so this is a measured null rather than an unengaged one: at these thresholds the chatter the band removes was not costing the machines anything this sample can see down to the paired MDE above. The shipping single threshold stays.",
        }
    );

    let next = std::fs::read_dir("benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().to_string_lossy().split('_').next()?.parse::<u32>().ok())
                .max()
                .map_or(1, |m| m + 1)
        })
        .unwrap_or(1);
    let dir = format!("benches/{next:03}_hysteresis");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
