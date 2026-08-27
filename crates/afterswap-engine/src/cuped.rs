//! CUPED — variance reduction by a pre-experiment control variate.
//!
//! For an outcome `Y` and a covariate `X` measured *before* the outcome and
//! unaffected by the treatment:
//!
//! ```text
//! Y_adj = Y - theta * (X - mean(X)),   theta = Cov(Y, X) / Var(X)
//! Var(Y_adj) = Var(Y) * (1 - rho^2)
//! ```
//!
//! The adjustment is unbiased for the mean of `Y` — subtracting a mean-zero
//! term cannot move it — while shrinking its variance by `rho^2`. That is the
//! whole trick, and it is why the reduction is bounded entirely by `rho`: no
//! amount of modelling makes a weak covariate strong.
//!
//! Bench 038 measured `rho(depth_t, depth_{t+1}) = 0.588` on BONK, a 34.6%
//! reduction, against 1.6% for anything derived from price. That is the
//! difference between needing 849 paired cycles to detect 0.25 bps and needing
//! 555.
//!
//! **Two conditions are not checkable by this module and must hold by design.**
//! `X` has to be measured before `Y` — a covariate contaminated by the outcome
//! silently absorbs the effect being measured — and it must not respond to the
//! treatment. In this project both are enforced at capture: the control variate
//! is `impact_bps` from the arrival quote, which exists before any order is
//! sent.

use crate::power::{Z_POWER_80, mde_bps, power_at_n};

/// Result of a CUPED adjustment over one paired sample.
#[derive(Debug, Clone)]
pub struct CupedResult {
    pub n: usize,
    /// Mean outcome. Identical before and after adjustment up to float error —
    /// CUPED moves variance, not the estimate.
    pub mean_bps: f64,
    /// Standard deviation of the raw outcome.
    pub sd_raw_bps: f64,
    /// Standard deviation after adjustment.
    pub sd_adj_bps: f64,
    /// Correlation between outcome and control variate.
    pub rho: f64,
    /// Regression coefficient applied.
    pub theta: f64,
    /// Achieved variance reduction, `1 - sd_adj^2 / sd_raw^2`.
    pub reduction: f64,
    /// Smallest effect detectable at 80% power, before adjustment.
    pub mde_raw_bps: f64,
    /// Smallest effect detectable at 80% power, after adjustment.
    pub mde_adj_bps: f64,
    /// Adjusted outcomes, in input order.
    pub adjusted: Vec<f64>,
}

impl CupedResult {
    /// Power to detect `delta` at the adjusted dispersion.
    pub fn power_at(&self, delta_bps: f64) -> f64 {
        power_at_n(delta_bps, self.sd_adj_bps, self.n)
    }

    /// Whether the sample resolves an effect of `delta` — the mean must both
    /// exceed the detection floor and reach the power target.
    pub fn resolves(&self, delta_bps: f64, min_power: f64) -> bool {
        self.mean_bps.abs() >= self.mde_adj_bps && self.power_at(delta_bps) >= min_power
    }
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn sd(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    match n > 1.0 {
        true => {
            let m = mean(v);
            (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt()
        }
        false => 0.0,
    }
}

/// Apply CUPED to paired `(outcome, covariate)` observations.
///
/// Returns `None` below four observations, or when the covariate has no
/// variance — a constant control variate carries no information and `theta`
/// would be undefined rather than merely useless.
pub fn cuped(y: &[f64], x: &[f64]) -> Option<CupedResult> {
    if y.len() != x.len() || y.len() < 4 {
        return None;
    }
    let n = y.len();
    let (my, mx) = (mean(y), mean(x));
    let mut cov = 0.0;
    let mut var_x = 0.0;
    for (a, b) in y.iter().zip(x) {
        cov += (a - my) * (b - mx);
        var_x += (b - mx) * (b - mx);
    }
    if var_x <= 0.0 {
        return None;
    }
    let theta = cov / var_x;
    let adjusted: Vec<f64> = y
        .iter()
        .zip(x)
        .map(|(a, b)| a - theta * (b - mx))
        .collect();
    let (sd_raw, sd_adj) = (sd(y), sd(&adjusted));
    let var_y: f64 = y.iter().map(|a| (a - my) * (a - my)).sum();
    let rho = match var_y > 0.0 {
        true => cov / (var_y * var_x).sqrt(),
        false => 0.0,
    };
    let reduction = match sd_raw > 0.0 {
        true => 1.0 - (sd_adj * sd_adj) / (sd_raw * sd_raw),
        false => 0.0,
    };
    Some(CupedResult {
        n,
        mean_bps: my,
        sd_raw_bps: sd_raw,
        sd_adj_bps: sd_adj,
        rho,
        theta,
        reduction,
        mde_raw_bps: mde_bps(n, sd_raw, Z_POWER_80),
        mde_adj_bps: mde_bps(n, sd_adj, Z_POWER_80),
        adjusted,
    })
}

/// Paired cycles needed to detect `delta` at 80% power, given a dispersion and
/// an assumed CUPED reduction. Inverts `N ∝ sigma^2`.
pub fn cycles_needed(delta_bps: f64, sd_bps: f64, reduction: f64) -> f64 {
    let eff = sd_bps * (1.0 - reduction).max(0.0).sqrt();
    crate::power::required_n_paired(delta_bps, eff, Z_POWER_80)
}
