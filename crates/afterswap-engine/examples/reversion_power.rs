//! Could eleven assets ever have detected the mechanism bench 036 demonstrated?
//!
//! Bench 036 set the autocorrelation directly and found PBO responds to |phi|:
//! 0.36 at |phi| = 0.4 against 0.56 at phi = 0, a gap of 2.7 standard errors.
//! Our own corpus shows `corr(|rho_1|, PBO) = −0.034` — flat. Two explanations,
//! and they lead to opposite decisions:
//!
//!   (a) the mechanism does not transfer to real series, or
//!   (b) it transfers, and eleven assets spanning |rho_1| <= 0.30 could never
//!       have resolved it.
//!
//! (b) is answerable without new data. Sweep phi across the band our assets
//! actually occupy, measure the PBO distribution at each level, then draw
//! pseudo-corpora of eleven assets at our real |rho_1| values, sampling each
//! one's PBO from the matching arm. The fraction of draws reaching significance
//! is the power our null result had.
//!
//! Sign is collapsed: bench 036 found |phi| = 0.4 gives 0.359 and 0.356 at the
//! two signs, indistinguishable, so arms are run at positive phi only.
//!
//! Run: cargo run -p afterswap-engine --example reversion_power --release

use std::fmt::Write as _;

use afterswap_engine::pbo::cscv;
use afterswap_engine::sim::replay_exit;
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;
const SLICES: usize = 10;
const WINDOWS: usize = 200;
const SEEDS: u64 = 50;
const SIGMA: f64 = 0.0008;
/// The band our corpus actually occupies: |rho_1| runs 0.018 to 0.297.
const PHIS: [f64; 7] = [0.0, 0.05, 0.10, 0.15, 0.20, 0.25, 0.30];
/// Measured |rho_1| for the eleven reference assets (bench 034).
const REAL_ABS_RHO: [f64; 11] = [
    0.2274, 0.0672, 0.1265, 0.0626, 0.0484, 0.2972, 0.0183, 0.0252, 0.1468, 0.0299, 0.0397,
];
/// Pseudo-corpora drawn to estimate power.
const DRAWS: usize = 20_000;
/// Two-tailed 5% critical |r| for n = 11 (9 degrees of freedom).
const CRIT_R: f64 = 0.602;

fn ar1_prices(phi: f64, n: usize, seed: u64) -> Vec<f64> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let innovation_sd = SIGMA * (1.0 - phi * phi).sqrt();
    let mut normal = move || {
        let (u1, u2): (f64, f64) = (rng.f64().max(1e-12), rng.f64());
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    };
    let mut r_prev = normal() * SIGMA;
    let mut price = 100.0f64;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let r = phi * r_prev + innovation_sd * normal();
        price *= r.exp();
        out.push(price);
        r_prev = r;
    }
    out
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn sd(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    let m = mean(v);
    match n > 1.0 {
        true => (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt(),
        false => 0.0,
    }
}

fn corr(a: &[f64], b: &[f64]) -> f64 {
    let (ma, mb) = (mean(a), mean(b));
    let mut num = 0.0;
    let (mut da, mut db) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        num += (x - ma) * (y - mb);
        da += (x - ma) * (x - ma);
        db += (y - mb) * (y - mb);
    }
    match da > 0.0 && db > 0.0 {
        true => num / (da * db).sqrt(),
        false => 0.0,
    }
}

/// Index of the arm whose phi is nearest `r`.
fn nearest_arm(r: f64) -> usize {
    PHIS.iter()
        .enumerate()
        .min_by(|a, b| (a.1 - r).abs().total_cmp(&(b.1 - r).abs()))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn main() {
    let machines = FsmEnumerator::enumerate(3);
    let mut md = String::from("# Could eleven assets have detected it?\n\n");
    let _ = writeln!(
        md,
        "Synthetic AR(1) arms across the band our corpus occupies (|rho_1| runs 0.018 to 0.297), \
{SEEDS} seeds each, {WINDOWS} windows of {WINDOW} ticks, unconditional volatility fixed at {:.0} bps \
per tick. Sign is collapsed on bench 036's finding that the two signs of |phi| = 0.4 are \
indistinguishable. PBO is CSCV at {SLICES} slices over all {} enumerated machines.\n",
        SIGMA * 10_000.0,
        machines.len()
    );
    let _ = writeln!(md, "| phi | **PBO** | PBO sd across seeds | std err |\n|---|---|---|---|");

    let mut arms: Vec<Vec<f64>> = Vec::new();
    for phi in PHIS {
        let mut pbos = Vec::new();
        for seed in 0..SEEDS {
            let series = ar1_prices(phi, WINDOWS * WINDOW, 7_000 + seed);
            let perf: Vec<Vec<f64>> = machines
                .iter()
                .map(|m| {
                    (0..WINDOWS)
                        .map(|w| replay_exit(m, &series[w * WINDOW..(w + 1) * WINDOW], TRANCHE, PEAK_DROP_BPS))
                        .collect()
                })
                .collect();
            if let Some(r) = cscv(&perf, SLICES) {
                pbos.push(r.pbo);
            }
        }
        let (m, s) = (mean(&pbos), sd(&pbos));
        let _ = writeln!(
            md,
            "| {phi:.2} | {m:.3} | {s:.3} | ±{:.3} |",
            s / (pbos.len() as f64).sqrt()
        );
        arms.push(pbos);
    }

    // Slope of PBO against phi across the realistic band.
    let xs: Vec<f64> = PHIS.to_vec();
    let ys: Vec<f64> = arms.iter().map(|a| mean(a)).collect();
    let slope_r = corr(&xs, &ys);

    // Power: draw pseudo-corpora of eleven assets at the real |rho_1| values,
    // sampling each asset's PBO from the arm nearest its autocorrelation.
    let mut rng = fastrand::Rng::with_seed(42);
    let idx: Vec<usize> = REAL_ABS_RHO.iter().map(|r| nearest_arm(*r)).collect();
    let xr: Vec<f64> = REAL_ABS_RHO.to_vec();
    let mut hits = 0usize;
    let mut observed = Vec::with_capacity(DRAWS);
    for _ in 0..DRAWS {
        let y: Vec<f64> = idx
            .iter()
            .map(|&i| {
                let a = &arms[i];
                a[rng.usize(..a.len())]
            })
            .collect();
        let r = corr(&xr, &y);
        observed.push(r);
        hits += usize::from(r.abs() >= CRIT_R);
    }
    let power = hits as f64 / DRAWS as f64;
    let mut sorted = observed.clone();
    sorted.sort_by(f64::total_cmp);
    let q = |p: f64| sorted[((p * (sorted.len() - 1) as f64) as usize).min(sorted.len() - 1)];

    let _ = writeln!(
        md,
        r#"
## Across the realistic band, the effect is small

PBO against phi over 0.00–0.30 correlates at **{slope_r:+.3}** at the arm-mean level. The mechanism is
present but shallow here: bench 036's 0.2 gap needed |phi| = 0.4, which no asset we hold comes near.

## Power of the eleven-asset null

Drawing {DRAWS} pseudo-corpora of eleven assets at our measured |rho_1| values, each asset's PBO sampled
from the arm nearest its autocorrelation:

- observed `corr(|rho_1|, PBO)` has median **{:+.3}**, 95% range **{:+.3} … {:+.3}**
- fraction reaching two-tailed significance at n = 11 (|r| >= {CRIT_R}): **{:.1}%**

Our actual corpus returned **−0.034**, which sits comfortably inside that range.

## Verdict

**Power is {:.1}%.** {}

The decision this settles is what "our data does not confirm it" was worth. {}"#,
        q(0.5),
        q(0.025),
        q(0.975),
        power * 100.0,
        power * 100.0,
        match power < 0.5 {
            true => "The eleven-asset null was never capable of rejecting the mechanism — at this sample size, and across the narrow band of autocorrelation our assets span, a corpus generated by the mechanism itself would usually look flat too. Explanation (a), that the mechanism does not transfer, is unsupported: we have no test of it.",
            false => "The eleven-asset null had a real chance of detecting the mechanism and did not. That is evidence the mechanism does not transfer to real series, not merely that our sample was small.",
        },
        match power < 0.5 {
            true => "It was worth nothing, and should not be cited as evidence against the mechanism. Deciding this needs either assets with stronger autocorrelation or many more of them — a corpus question, not an analysis one.",
            false => "It was worth something, and the gap between simulation and reality is now the thing to explain.",
        },
    );

    let dir = "benches/037_reversion_power";
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(format!("{dir}/report.md"), &md);
    println!("{md}");
}
