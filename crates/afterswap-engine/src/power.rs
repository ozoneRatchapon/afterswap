//! Pre-run power analysis — refuse experiments that cannot answer.
//!
//! Adopted after external review pointed out the obvious: this project ran a
//! 534-cycle live soak looking for a sub-1 bps effect against a 6.6 bps
//! per-cycle standard deviation, and only discovered afterwards that the
//! experiment had ~9% power to detect 0.25 bps. Twice. A power calculation
//! costs microseconds and would have refused both runs.
//!
//! Formulas are the standard normal-approximation ones:
//!   paired:   n = (z_{1-α/2} + z_{power})² · σ² / δ²
//!   unpaired: N = 2 · (z_{1-α/2} + z_{power})² · σ² / δ²   (total, both arms)
//! and the inverse, the minimum detectable effect at a given sample size.

/// z for the two-sided 5% significance level.
const Z_ALPHA: f64 = 1.959_963_985;
/// z for 80% power.
pub const Z_POWER_80: f64 = 0.841_621_234;
/// z for 90% power.
pub const Z_POWER_90: f64 = 1.281_551_566;
/// z for 95% power.
pub const Z_POWER_95: f64 = 1.644_853_627;

/// Cycles needed to detect `delta` bps at 5% significance and the given power,
/// with paired measurement (the reference exits run on the same price path).
pub fn required_n_paired(delta_bps: f64, sd_bps: f64, z_power: f64) -> f64 {
    let z = Z_ALPHA + z_power;
    z * z * sd_bps * sd_bps / (delta_bps * delta_bps)
}

/// Same, unpaired — **total** across both arms (each arm gets half), which is
/// why unpaired measurement costs ~25× more data at our variances.
///
/// Convention note (resolved): the reference table's unpaired *required-N*
/// column is a total across both arms, while its unpaired *power* column is
/// quoted at 534 **per group** — i.e. N = 1,068 total. The round-one document
/// leaves that implicit and the two columns read as contradictory; the
/// round-two document annotates the cell "(534/group)", but the 534 was an
/// embedded image and did not survive the text export, so the annotation
/// reached us as a bare "( /group)". With the label recovered, both columns
/// agree with this implementation exactly — see `tests/power.rs`.
pub fn required_n_unpaired(delta_bps: f64, sd_bps: f64, z_power: f64) -> f64 {
    4.0 * required_n_paired(delta_bps, sd_bps, z_power)
}

/// The smallest effect an experiment of size `n` could have detected, given
/// the observed spread. Report this beside every null result: "we found
/// nothing" is only informative next to "we could only have found ≥ X".
pub fn mde_bps(n: usize, sd_bps: f64, z_power: f64) -> f64 {
    (Z_ALPHA + z_power) * sd_bps / (n as f64).sqrt()
}

/// MDE straight from a reported standard error (SE = σ/√n), which is what
/// benches already print.
pub fn mde_from_se(se_bps: f64, z_power: f64) -> f64 {
    (Z_ALPHA + z_power) * se_bps
}

/// Achieved power to detect `delta` with `n` **paired** observations.
pub fn power_at_n(delta_bps: f64, sd_bps: f64, n: usize) -> f64 {
    let z = delta_bps * (n as f64).sqrt() / sd_bps - Z_ALPHA;
    normal_cdf(z)
}

/// Achieved power with `n_total` observations split across two independent
/// arms (standard error of the difference is 2σ/√N).
pub fn power_at_n_unpaired(delta_bps: f64, sd_bps: f64, n_total: usize) -> f64 {
    let z = delta_bps * (n_total as f64).sqrt() / (2.0 * sd_bps) - Z_ALPHA;
    normal_cdf(z)
}

/// Verdict for a planned run: run it, or say what it would take.
pub enum PowerVerdict {
    Adequate { power: f64 },
    Underpowered { power: f64, need_n: usize, mde_bps: f64 },
}

/// Gate an experiment before spending the data on it.
pub fn gate(delta_bps: f64, sd_bps: f64, n: usize, min_power: f64) -> PowerVerdict {
    let power = power_at_n(delta_bps, sd_bps, n);
    match power >= min_power {
        true => PowerVerdict::Adequate { power },
        false => PowerVerdict::Underpowered {
            power,
            need_n: required_n_paired(delta_bps, sd_bps, Z_POWER_80).ceil() as usize,
            mde_bps: mde_bps(n, sd_bps, Z_POWER_80),
        },
    }
}

/// Abramowitz-Stegun 7.1.26 error function approximation; plenty for power.
fn normal_cdf(z: f64) -> f64 {
    let sign = if z < 0.0 { -1.0 } else { 1.0 };
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    0.5 * (1.0 + sign * y)
}
