//! Paired online evaluation.
//!
//! The soak's absolute "edge vs hold" is dominated by price-path noise
//! (per-cycle SD ≈ 6.6 bps vs a sub-1 bps effect). Reference strategies
//! driven by the *same* ticks from the *same* entry make the comparison
//! paired: the path cancels, and a real difference becomes detectable in
//! hours instead of weeks. Same strategies and parameters as the GOAT
//! ecosystem floors, so live numbers and bench numbers are comparable.

use serde::Serialize;

/// TWAP: sell 1/slices every `stride` ticks.
const TWAP_SLICES: usize = 10;
const TWAP_STRIDE: usize = 6;
/// Trailing stop: all-out this far below the running peak.
const TRAIL_DROP_BPS: f64 = 50.0;
/// Take-profit ladder: `rungs` steps of `step` bps above entry.
const LADDER_RUNGS: usize = 10;
const LADDER_STEP_BPS: f64 = 10.0;
/// Bracket (OCO): all-out at ±these bps.
const BRACKET_TP_BPS: f64 = 50.0;
const BRACKET_SL_BPS: f64 = 50.0;

/// Reference exits tracked alongside one live position.
pub struct Shadow {
    entry: f64,
    peak: f64,
    ticks: usize,
    twap_cash: f64,
    twap_remaining: f64,
    twap_slices_done: usize,
    ladder_cash: f64,
    ladder_remaining: f64,
    ladder_next_rung: usize,
    /// Locked value (× entry) once an all-out strategy has fired.
    trailing_locked: Option<f64>,
    bracket_locked: Option<f64>,
}

/// One cycle's paired comparison, in bps (positive = engine ahead).
#[derive(Debug, Clone, Serialize)]
pub struct PairedResult {
    pub engine_value_norm: f64,
    pub hold_value_norm: f64,
    pub twap_value_norm: f64,
    pub trailing_value_norm: f64,
    pub ladder_value_norm: f64,
    pub bracket_value_norm: f64,
    pub vs_hold_bps: f64,
    pub vs_twap_bps: f64,
    pub vs_trailing_bps: f64,
    pub vs_ladder_bps: f64,
    pub vs_bracket_bps: f64,
    pub ticks: usize,
}

fn diff_bps(a: f64, b: f64) -> f64 {
    match b.abs() > f64::EPSILON {
        true => (a - b) / b * 10_000.0,
        false => 0.0,
    }
}

impl Shadow {
    /// Start tracking from the live position's entry price.
    pub fn new(entry: f64) -> Self {
        Self {
            entry,
            peak: entry,
            ticks: 0,
            twap_cash: 0.0,
            twap_remaining: 1.0,
            twap_slices_done: 0,
            ladder_cash: 0.0,
            ladder_remaining: 1.0,
            ladder_next_rung: 1,
            trailing_locked: None,
            bracket_locked: None,
        }
    }

    /// Feed the same tick the engine sees.
    pub fn on_tick(&mut self, price: f64) {
        self.ticks += 1;
        self.peak = self.peak.max(price);
        let rel = price / self.entry;

        if self.ticks.is_multiple_of(TWAP_STRIDE)
            && self.twap_slices_done < TWAP_SLICES
            && self.twap_remaining > 1e-12
        {
            let frac = (1.0 / TWAP_SLICES as f64).min(self.twap_remaining);
            self.twap_cash += frac * rel;
            self.twap_remaining -= frac;
            self.twap_slices_done += 1;
        }

        while self.ladder_next_rung <= LADDER_RUNGS
            && self.ladder_remaining > 1e-12
            && price >= self.entry * (1.0 + self.ladder_next_rung as f64 * LADDER_STEP_BPS * 1e-4)
        {
            let frac = (1.0 / LADDER_RUNGS as f64).min(self.ladder_remaining);
            self.ladder_cash += frac * rel;
            self.ladder_remaining -= frac;
            self.ladder_next_rung += 1;
        }

        if self.trailing_locked.is_none()
            && (self.peak - price) / self.peak * 10_000.0 >= TRAIL_DROP_BPS
        {
            self.trailing_locked = Some(rel);
        }

        if self.bracket_locked.is_none() {
            let move_bps = (price - self.entry) / self.entry * 10_000.0;
            if move_bps >= BRACKET_TP_BPS || move_bps <= -BRACKET_SL_BPS {
                self.bracket_locked = Some(rel);
            }
        }
    }

    /// Compare the engine's realized value against every reference at the
    /// same closing price. All values are × entry, so paired-comparable.
    pub fn compare(&self, engine_value_norm: f64, close_price: f64) -> PairedResult {
        let rel = close_price / self.entry;
        let hold = rel;
        let twap = self.twap_cash + self.twap_remaining * rel;
        let ladder = self.ladder_cash + self.ladder_remaining * rel;
        let trailing = self.trailing_locked.unwrap_or(rel);
        let bracket = self.bracket_locked.unwrap_or(rel);
        PairedResult {
            engine_value_norm,
            hold_value_norm: hold,
            twap_value_norm: twap,
            trailing_value_norm: trailing,
            ladder_value_norm: ladder,
            bracket_value_norm: bracket,
            vs_hold_bps: diff_bps(engine_value_norm, hold),
            vs_twap_bps: diff_bps(engine_value_norm, twap),
            vs_trailing_bps: diff_bps(engine_value_norm, trailing),
            vs_ladder_bps: diff_bps(engine_value_norm, ladder),
            vs_bracket_bps: diff_bps(engine_value_norm, bracket),
            ticks: self.ticks,
        }
    }
}
