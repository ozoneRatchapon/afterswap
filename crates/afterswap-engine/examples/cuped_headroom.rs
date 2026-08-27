//! Would CUPED actually reduce our variance, or is 30–50% someone else's number?
//!
//! Round three answers L1 by prescribing CUPED: use a pre-experiment control
//! variate `X` to compress outcome variance by `1 − ρ²(Y, X)`, "typically 30–50%
//! in high-frequency crypto", cutting the sample needed for a +0.25 bps effect
//! from 849 paired cycles to roughly 420–590. That would bring the +0.10 to
//! +0.35 bps CLMM margin inside reach.
//!
//! The reduction is entirely a function of ρ², so the claim is checkable before
//! building anything. Two caveats bound what this bench can say:
//!
//! 1. The control variates round three names — pre-trade pool volatility and
//!    order arrival imbalance — are not in our corpus, which is `{t, price}`.
//!    We test price-derived proxies instead, which is a lower bound on what a
//!    depth-aware feed could achieve.
//! 2. The outcome it would ultimately be applied to is a paired *execution*
//!    A/B, and that recorder was stopped when Plan 001 closed. Here `Y` is the
//!    per-window paired edge differential we do have — an objective the same
//!    document calls ill-conditioned. Read this as headroom for the machinery,
//!    not as a result about execution.
//!
//! Run: cargo run -p afterswap-engine --example cuped_headroom --release

use std::fmt::Write as _;

use afterswap_engine::sim::{load_corpus, replay_exit};
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;
/// Ticks before each window used to build the control variate. Must not overlap
/// the window itself or the variate stops being pre-experiment and CUPED
/// becomes a way of regressing the outcome on itself.
const LOOKBACK: usize = 120;

fn corr(y: &[f64], x: &[f64]) -> f64 {
    let n = y.len() as f64;
    let (my, mx) = (y.iter().sum::<f64>() / n, x.iter().sum::<f64>() / n);
    let mut num = 0.0;
    let (mut dy, mut dx) = (0.0, 0.0);
    for (a, b) in y.iter().zip(x) {
        num += (a - my) * (b - mx);
        dy += (a - my) * (a - my);
        dx += (b - mx) * (b - mx);
    }
    match dy > 0.0 && dx > 0.0 {
        true => num / (dy * dx).sqrt(),
        false => 0.0,
    }
}

/// Realised volatility of log returns over a slice.
fn realised_vol(slice: &[f64]) -> f64 {
    let r: Vec<f64> = slice.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
    let n = r.len() as f64;
    match n > 1.0 {
        true => {
            let m = r.iter().sum::<f64>() / n;
            (r.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / n).sqrt()
        }
        false => 0.0,
    }
}

/// Signed drift over a slice, in bps.
fn drift_bps(slice: &[f64]) -> f64 {
    match slice.first().zip(slice.last()) {
        Some((a, b)) if *a > 0.0 => (b / a - 1.0) * 10_000.0,
        _ => 0.0,
    }
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
    let mut md = String::from("# Does CUPED have anything to work with here?\n\n");
    md.push_str(
        "> **Superseded by [bench 038](../038_depth_control/report.md).** This bench concluded that \
1.9% was CUPED's ceiling for us because the corpus is `{t, price}`. That is true of `data/reference/` \
but not of the repository — `data/incoming/bonk_depth.jsonl` holds 1,207 paired price/depth rows kept \
when the Plan 001 recorder stopped. On real depth, a one-tick-old reading delivers **34.6%**, inside \
the prescribed band. The measurements below stand as a bound on price-derived proxies; the conclusion \
drawn about CUPED does not.\n\n",
    );
    let _ = writeln!(
        md,
        "Correlation between the per-window paired edge differential and two pre-window control variates \
built from the {LOOKBACK} ticks preceding each window. CUPED compresses variance by `1 − ρ²`, so the \
reduction column is that identity, not a measurement of CUPED itself. Outcome is the best-in-sample \
machine's per-window edge against the population median, over {} enumerated machines.\n",
        machines.len()
    );
    let _ = writeln!(
        md,
        "| asset | windows | ρ(Y, prior vol) | reduction | ρ(Y, prior drift) | reduction | best |\n|---|---|---|---|---|---|---|"
    );

    let mut best_overall: Vec<f64> = Vec::new();
    for path in &assets {
        let Ok(series) = load_corpus(path) else { continue };
        let n = series.len() / WINDOW;
        // Windows that have a full lookback available in front of them.
        let start = LOOKBACK.div_ceil(WINDOW);
        if n <= start + 2 {
            continue;
        }
        let perf: Vec<Vec<f64>> = machines
            .iter()
            .map(|m| {
                (start..n)
                    .map(|w| replay_exit(m, &series[w * WINDOW..(w + 1) * WINDOW], TRANCHE, PEAK_DROP_BPS))
                    .collect()
            })
            .collect();
        let cols = n - start;
        // Y: the selected machine's per-window edge over the population median,
        // which is the quantity a paired experiment would be estimating.
        let totals: Vec<f64> = perf.iter().map(|r| r.iter().sum::<f64>()).collect();
        let pick = totals
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let y: Vec<f64> = (0..cols)
            .map(|w| {
                let mut col: Vec<f64> = perf.iter().map(|r| r[w]).collect();
                col.sort_by(f64::total_cmp);
                perf[pick][w] - col[col.len() / 2]
            })
            .collect();
        let vol: Vec<f64> = (start..n)
            .map(|w| realised_vol(&series[w * WINDOW - LOOKBACK..w * WINDOW]))
            .collect();
        let dft: Vec<f64> = (start..n)
            .map(|w| drift_bps(&series[w * WINDOW - LOOKBACK..w * WINDOW]))
            .collect();

        let (rv, rd) = (corr(&y, &vol), corr(&y, &dft));
        let best = rv.abs().max(rd.abs());
        best_overall.push(best);
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or("?")
            .trim_end_matches("_1m.jsonl")
            .to_uppercase();
        let _ = writeln!(
            md,
            "| {name} | {cols} | {rv:+.3} | {:.1}% | {rd:+.3} | {:.1}% | {:.1}% |",
            rv * rv * 100.0,
            rd * rd * 100.0,
            best * best * 100.0
        );
    }

    let mean_best = best_overall.iter().map(|r| r * r).sum::<f64>() / best_overall.len() as f64;
    let _ = writeln!(
        md,
        r#"
## Verdict: {:.1}% mean reduction, against a prescribed 30–50%

Price-derived control variates carry almost nothing about the paired edge differential. The best of the
two, per asset, averages a **{:.1}% variance reduction** — against the 30–50% round three cites and the
drop from 849 to 420–590 cycles that figure implies. On these variates the required sample barely moves.

That is a bound on the proxies, not a refutation of the method. The variates round three names —
pre-trade pool volatility and order arrival imbalance — are depth-book quantities, and our corpus is
`{{t, price}}`. A price series cannot express order arrival imbalance at all. What this bench rules out
is the cheap version: CUPED on data we already hold does not bring the +0.10 to +0.35 bps CLMM margin
inside reach.

Making L1 answerable needs the depth-aware feed back, which is the recorder Plan 001 closed. That is a
real decision with a cost attached, not an implementation detail — and it is the same feed twice over,
since the outcome CUPED would be applied to is a paired execution A/B rather than the edge-vs-hold
objective used here as a stand-in."#,
        mean_best * 100.0,
        mean_best * 100.0
    );

    let dir = "benches/033_cuped_headroom";
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(format!("{dir}/report.md"), &md);
    println!("{md}");
}
