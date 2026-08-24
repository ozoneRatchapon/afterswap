//! Core data types for the exit engine.

use serde::Serialize;

/// Engine tuning knobs.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// FSM state count fed to `FsmEnumerator::enumerate` (2 → ~22, 3 → ~956).
    pub n_fsm_states: u8,
    /// Ticks per evaluation window.
    pub window_len: usize,
    /// Stride between rolling windows (ticks).
    pub window_stride: usize,
    /// Max historical windows kept for tournaments.
    pub max_windows: usize,
    /// Fraction of the original position sold per SELL signal.
    pub tranche_frac: f64,
    /// Re-run the window tournament every N completed windows.
    pub refresh_every_windows: usize,
    /// Pareto pruner payoff floor (bps edge vs hold; keep permissive).
    pub payoff_threshold_bps: f64,
    /// Pareto pruner complexity ceiling (normalized 0..=1).
    pub complexity_threshold: f32,
    /// Cap on bandit arms after Pareto pruning (top by sim edge, then
    /// simplicity). Keeps UCB1 exploration meaningful — a flat market
    /// degenerates the Pareto front to the whole enumeration.
    pub max_arms: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            n_fsm_states: 3,
            window_len: 64,
            window_stride: 32,
            max_windows: 16,
            tranche_frac: 0.25,
            refresh_every_windows: 2,
            payoff_threshold_bps: -1e9,
            complexity_threshold: 1.1,
            max_arms: 24,
        }
    }
}

/// One executed tranche sell.
#[derive(Debug, Clone, Serialize)]
pub struct TrancheFill {
    /// Global tick index at fill time.
    pub tick: u64,
    /// Price at fill (output units per input unit, e.g. USDC per SOL).
    pub price: f64,
    /// Fraction of the ORIGINAL position sold.
    pub frac: f64,
}

/// An open post-swap position being managed by the engine.
#[derive(Debug, Clone, Serialize)]
pub struct Position {
    /// Entry price (from the swap fill).
    pub entry_price: f64,
    /// Original size in input units (e.g. SOL).
    pub size: f64,
    /// Global tick index when opened.
    pub opened_at_tick: u64,
    /// Fraction of the original size still held (1.0 → 0.0).
    pub remaining_frac: f64,
    /// Cash accumulated from tranche sells, in output units per 1.0 size,
    /// normalized by entry price (i.e. sum of frac * price/entry).
    pub cash_norm: f64,
    /// Executed tranche fills.
    pub fills: Vec<TrancheFill>,
}

impl Position {
    /// Open a fresh position at `entry_price`.
    pub fn open(entry_price: f64, size: f64, tick: u64) -> Self {
        Self {
            entry_price,
            size,
            opened_at_tick: tick,
            remaining_frac: 1.0,
            cash_norm: 0.0,
            fills: Vec::new(),
        }
    }

    /// Normalized value of the position at price `p` (1.0 == held at entry).
    pub fn value_norm(&self, p: f64) -> f64 {
        self.cash_norm + self.remaining_frac * (p / self.entry_price)
    }

    /// Apply a tranche sell of `frac` (of original) at `price`.
    pub fn apply_fill(&mut self, tick: u64, price: f64, frac: f64) {
        let frac = frac.min(self.remaining_frac);
        self.remaining_frac -= frac;
        self.cash_norm += frac * (price / self.entry_price);
        self.fills.push(TrancheFill { tick, price, frac });
    }

    /// Whether the position is fully exited.
    pub fn is_closed(&self) -> bool {
        self.remaining_frac <= f64::EPSILON
    }
}
