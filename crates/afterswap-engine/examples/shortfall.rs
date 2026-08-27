//! The redirect: can the machines beat TWAP on execution *risk*, if not on
//! execution *return*?
//!
//! Every earlier benchmark asked whether the enumerated machines beat a
//! benchmark on mean edge, and the honest answer across eleven assets, a
//! multiplicity correction and two live soaks is no. External review
//! prescribed a different objective — Almgren–Chriss arrival-price
//! implementation shortfall — and noted that risk-centric variants (shortfall
//! variance, CVaR tail containment) are validated the same paired way against
//! a TWAP baseline.
//!
//! That distinction matters statistically, not just commercially: a variance
//! ratio is estimable from far fewer observations than a sub-bps mean, and a
//! promise of *predictable* execution is one this data can actually confirm
//! or refute. Selection is on train, measurement on disjoint test, and each
//! selection objective is reported against the same TWAP baseline.
//!
//! Run: cargo run -p afterswap-engine --example shortfall --release

use std::fmt::Write as _;

use afterswap_engine::sim::{
    DEFAULT_ETA, load_corpus, shortfall_bps_impact, twap_shortfall_bps_impact,
};
use katgpt_ruliology::{FsmEnumerator, SimpleProgram};

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;
const COST_BPS: f64 = 2.0;
const TWAP_SLICES: usize = 10;
const TWAP_STRIDE: usize = 6;
const BOOTSTRAPS: usize = 500;

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn sd(v: &[f64]) -> f64 {
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// Mean of the worst decile — the tail an execution product is judged on.
fn cvar90(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp); // shortfall: larger is worse
    let k = (s.len() / 10).max(1);
    mean(&s[s.len() - k..])
}

/// Paired bootstrap CI for the SD ratio (engine / TWAP); below 1 is better.
fn sd_ratio_ci(a: &[f64], b: &[f64], seed: u64) -> (f64, f64, f64) {
    let mut rng = fastrand::Rng::with_seed(seed);
    let n = a.len();
    let mut ratios: Vec<f64> = (0..BOOTSTRAPS)
        .map(|_| {
            let idx: Vec<usize> = (0..n).map(|_| rng.usize(..n)).collect();
            let ra: Vec<f64> = idx.iter().map(|&i| a[i]).collect();
            let rb: Vec<f64> = idx.iter().map(|&i| b[i]).collect();
            sd(&ra) / sd(&rb).max(1e-12)
        })
        .collect();
    ratios.sort_by(f64::total_cmp);
    (
        sd(a) / sd(b).max(1e-12),
        ratios[(BOOTSTRAPS as f64 * 0.025) as usize],
        ratios[(BOOTSTRAPS as f64 * 0.975) as usize],
    )
}

#[derive(Clone, Copy)]
enum Objective {
    MeanShortfall,
    ShortfallSd,
    Cvar,
}

impl Objective {
    fn name(self) -> &'static str {
        match self {
            Objective::MeanShortfall => "min mean shortfall",
            Objective::ShortfallSd => "min shortfall SD",
            Objective::Cvar => "min CVaR(90)",
        }
    }
    fn score(self, v: &[f64]) -> f64 {
        match self {
            Objective::MeanShortfall => mean(v),
            Objective::ShortfallSd => sd(v),
            Objective::Cvar => cvar90(v),
        }
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

    let mut md = String::from("# New objective: implementation shortfall, and its risk variants\n\n");
    let _ = writeln!(
        md,
        "Arrival-price shortfall in bps (**positive is worse**), {COST_BPS:.0} bps charged per fill to every strategy including the baseline. {WINDOW}-tick windows, chronological 60/40 split, machine selected on train under each objective and measured on test, TWAP({TWAP_SLICES}×{TWAP_STRIDE}) as the baseline on the same windows.\n"
    );

    for (eta, regime) in [
        (0.0, "no price impact (the flawed simulator)"),
        (DEFAULT_ETA, "**with rate-dependent temporary impact**"),
    ] {
    let _ = writeln!(md, "\n# Regime: {regime}\n");
    for obj in [Objective::MeanShortfall, Objective::ShortfallSd, Objective::Cvar] {
        let _ = writeln!(md, "\n## Selection objective: {}\n", obj.name());
        let _ = writeln!(
            md,
            "| asset | test windows | engine mean | TWAP mean | paired Δmean (±SE) | engine SD | TWAP SD | **SD ratio vs TWAP** | t̄ eng/matched | **SD ratio vs speed-matched TWAP** | engine CVaR90 | TWAP CVaR90 |\n|---|---|---|---|---|---|---|---|---|---|---|---|"
        );
        let (mut ratio_acc, mut n_assets, mut better) = (0.0, 0.0, 0usize);
        let (mut mratio_acc, mut mbetter) = (0.0, 0usize);
        for path in &assets {
            let Ok(series) = load_corpus(path) else { continue };
            let n = series.len() / WINDOW;
            if n < 20 {
                continue;
            }
            let split = n * 6 / 10;
            let win = |w: usize| &series[w * WINDOW..(w + 1) * WINDOW];

            let best = (0..machines.len())
                .min_by(|&a, &b| {
                    let sa: Vec<f64> = (0..split)
                        .map(|w| shortfall_bps_impact(&machines[a], win(w), TRANCHE, PEAK_DROP_BPS, COST_BPS, eta))
                        .collect();
                    let sb: Vec<f64> = (0..split)
                        .map(|w| shortfall_bps_impact(&machines[b], win(w), TRANCHE, PEAK_DROP_BPS, COST_BPS, eta))
                        .collect();
                    obj.score(&sa).total_cmp(&obj.score(&sb))
                })
                .expect("non-empty");

            // Speed-matched control. If the machines' variance advantage is
            // just "liquidate sooner", a plain TWAP compressed to the same
            // mean liquidation time captures it with no strategy at all —
            // moving along the Almgren–Chriss frontier rather than beating it.
            let mean_time = {
                let (mut acc, mut wins) = (0.0, 0.0);
                for w in split..n {
                    let win = win(w);
                    let mut fsm = machines[best].clone();
                    fsm.reset();
                    let (mut rem, mut peak, mut wsum, mut fsum) = (1.0f64, win[0], 0.0f64, 0.0f64);
                    for t in 1..win.len() {
                        let dir = u8::from(win[t] > win[t - 1]);
                        peak = peak.max(win[t]);
                        let off = u8::from((peak - win[t]) / peak * 1e4 >= PEAK_DROP_BPS);
                        fsm.next_action(&[dir]);
                        if fsm.next_action(&[off]) == 1 && rem > 0.0 {
                            let f = TRANCHE.min(rem);
                            rem -= f;
                            wsum += f * t as f64;
                            fsum += f;
                        }
                    }
                    wsum += rem * (win.len() - 1) as f64;
                    fsum += rem;
                    acc += wsum / fsum.max(1e-12);
                    wins += 1.0;
                }
                acc / wins
            };
            let matched_stride =
                ((2.0 * mean_time / (TWAP_SLICES as f64 + 1.0)).round() as usize).max(1);
            let matched: Vec<f64> = (split..n)
                .map(|w| twap_shortfall_bps_impact(win(w), TWAP_SLICES, matched_stride, COST_BPS, eta))
                .collect();

            let eng: Vec<f64> = (split..n)
                .map(|w| shortfall_bps_impact(&machines[best], win(w), TRANCHE, PEAK_DROP_BPS, COST_BPS, eta))
                .collect();
            let twap: Vec<f64> = (split..n)
                .map(|w| twap_shortfall_bps_impact(win(w), TWAP_SLICES, TWAP_STRIDE, COST_BPS, eta))
                .collect();
            let diff: Vec<f64> = eng.iter().zip(&twap).map(|(a, b)| a - b).collect();
            let (ratio, lo, hi) = sd_ratio_ci(&eng, &twap, 99);
            let (mratio, mlo, mhi) = sd_ratio_ci(&eng, &matched, 99);
            ratio_acc += ratio;
            mratio_acc += mratio;
            n_assets += 1.0;
            if hi < 1.0 {
                better += 1;
            }
            if mhi < 1.0 {
                mbetter += 1;
            }

            let name = path.rsplit('/').next().unwrap_or("?").trim_end_matches("_1m.jsonl").to_uppercase();
            let _ = writeln!(
                md,
                "| {name} | {} | {:+.1} | {:+.1} | {:+.1} ± {:.1} | {:.1} | {:.1} | **{ratio:.2} [{lo:.2}, {hi:.2}]** | {mean_time:.0}/{:.0} | **{mratio:.2} [{mlo:.2}, {mhi:.2}]** | {:+.1} | {:+.1} |",
                eng.len(),
                mean(&eng),
                mean(&twap),
                mean(&diff),
                sd(&diff) / (diff.len() as f64).sqrt(),
                sd(&eng),
                sd(&twap),
                (matched_stride * (TWAP_SLICES + 1)) as f64 / 2.0,
                cvar90(&eng),
                cvar90(&twap),
            );
        }
        let _ = writeln!(
            md,
            "\n**vs TWAP: mean SD ratio {:.2}, significant on {better}/{:.0}. vs speed-matched TWAP: {:.2}, significant on {mbetter}/{:.0}.**\n",
            ratio_acc / n_assets,
            n_assets,
            mratio_acc / n_assets,
            n_assets
        );
    }
    }

    let _ = writeln!(
        md,
        r#"
## Verdict: the objective changed, the answer did not

Read against TWAP alone, this looked like the project's first durable result:
selecting machines for minimum shortfall variance gives an **SD ratio of 0.70,
significantly below 1 on 8 of 11 assets** — execution ~30% more predictable
than TWAP, out of sample, on real bars, with bootstrap confidence intervals.
It survived adding a rate-dependent impact model (0.72), which was the first
control we ran.

**The second control destroys it.** The selected machines liquidate roughly
four times sooner than TWAP (mean liquidation time ~6–15 ticks against TWAP's
33). Faster liquidation mechanically reduces timing variance — that is the
Almgren–Chriss frontier, not skill. Compressing a plain TWAP to the *same mean
liquidation time* and comparing against that reproduces the entire advantage:
**SD ratio 1.12, significant on 0 of 11 assets.** The machines are marginally
*worse* than the trivial schedule at their own urgency.

So the finding was never "these machines execute better". It was "these
machines execute sooner", restated in units of variance. We moved along the
efficient frontier and mistook it for beating it — until the benchmark was
matched on the dimension that was actually doing the work.

**Method note.** The first principle in our own research method is *name the
incumbent*: not "the market", but the thing a user would otherwise do. This
result is what happens when the named incumbent is right in kind but wrong in
parameter. A benchmark must be matched on every dimension the strategy is free
to vary, or the comparison measures the mismatch instead of the strategy.
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
    let dir = format!("benches/{next:03}_shortfall");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
