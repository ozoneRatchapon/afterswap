//! Pre-registration — commit to the hypothesis before seeing the data.
//!
//! This project shipped a bench whose conclusion paragraph was written before
//! its table existed, and the table then contradicted it. Open-science reform
//! has a standard fix, and external review prescribed the machine form of it:
//! serialise the hypothesis, the effect size sought, the power target, the
//! split boundaries and the null control; hash it; record the hash *before*
//! the data is touched; and refuse to publish a report whose parameters do not
//! match the hash.
//!
//! The point is not ceremony. A manifest makes two specific frauds impossible
//! without detection: moving the goalposts after seeing the result, and
//! quietly widening the analysis until something passes.

use serde::{Deserialize, Serialize};

/// What the experiment claims it will do, fixed before it runs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreRegistration {
    /// One sentence, in the falsifiable form "X beats Y by at least Z".
    pub hypothesis: String,
    /// The benchmark the effect is measured against — never "the market".
    pub benchmark: String,
    /// Effect size sought, in bps. Drives the power calculation.
    pub target_effect_bps: f64,
    /// Power the run must have to be worth executing.
    pub target_power: f64,
    /// Significance level.
    pub alpha: f64,
    /// Exact data partitions, so they cannot be re-cut after the fact.
    pub train_windows: usize,
    pub test_windows: usize,
    /// The control that should return nothing if the pipeline is honest.
    pub null_control: String,
    /// Corpus files, frozen for the duration of the run.
    pub corpora: Vec<String>,
}

impl PreRegistration {
    /// Canonical JSON, so the hash depends on content rather than formatting.
    pub fn canonical(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Content hash, recorded before the data is read.
    pub fn hash(&self) -> String {
        blake3::hash(self.canonical().as_bytes()).to_hex().to_string()
    }

    /// Refuse experiments that cannot answer their own question. Returns the
    /// minimum detectable effect when the sample is too small.
    pub fn power_check(&self, sd_bps: f64) -> Result<f64, String> {
        let n = self.test_windows;
        let z_power = match self.target_power {
            p if p >= 0.9 => crate::power::Z_POWER_90,
            _ => crate::power::Z_POWER_80,
        };
        let achieved = crate::power::power_at_n(self.target_effect_bps, sd_bps, n);
        match achieved >= self.target_power {
            true => Ok(achieved),
            false => Err(format!(
                "underpowered: {n} test windows give {:.1}% power for a {:.2} bps effect at σ = {sd_bps:.2}; \
                 need {:.0} windows, or accept a minimum detectable effect of {:.2} bps",
                achieved * 100.0,
                self.target_effect_bps,
                crate::power::required_n_paired(self.target_effect_bps, sd_bps, z_power).ceil(),
                crate::power::mde_bps(n, sd_bps, z_power),
            )),
        }
    }

    /// A report may only be published if it names the manifest that produced
    /// it. Any drift in parameters changes the hash and fails this check.
    pub fn verify_report(&self, claimed_hash: &str) -> bool {
        self.hash() == claimed_hash
    }
}
