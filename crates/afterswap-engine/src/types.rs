//! Core data types for the exit engine.

use serde::Serialize;

/// Engine tuning knobs.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// FSM state count fed to `FsmEnumerator::enumerate` after behavioural
    /// dedup: 1 → 2, 2 → 26, 3 → 1,054 (asserted by `tests/fsm_table.rs`).
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
    /// GOAT floor override: pick arms uniformly at random (seeded) instead
    /// of UCB1. Exists so the bandit has an honest baseline to beat (G2b).
    pub random_arm_seed: Option<u64>,
    /// Run an evolution step every N completed live windows (0 = off).
    /// Mutants of current arms (incl. 4-state growth beyond the enumerable
    /// frontier) replace the worst arm when they win on replayed windows.
    pub evolve_every_windows: usize,
    /// Mutant candidates proposed per evolution step.
    pub evolve_candidates: usize,
    /// Second input bit: 1 when price sits at least this many bps below
    /// its running peak. Lets machines express trailing-stop behavior
    /// (Bench 004 showed the binary alphabet loses to trailing stops in
    /// up-trends precisely for lack of this signal).
    pub peak_drop_bps: f64,
    /// Temporal-derivative surprise trigger (roadmap #2): force a full
    /// re-tournament when |fast-EMA − slow-EMA| of signed returns exceeds
    /// this multiple of volatility — "the market changed its mind, re-audition
    /// everyone now" instead of waiting out the refresh cadence. 0 = off.
    pub surprise_ratio: f64,
    /// Off-policy credit assignment (roadmap: sample efficiency). At each
    /// window boundary the seated arm gets its realized reward AND every
    /// other arm is replayed on the same realized window and credited with
    /// its counterfactual edge. ~24x more learning signal per unit time,
    /// using replay machinery the tournament already pays for.
    pub off_policy_credit: bool,
    /// Keep realized statistics per market regime (chop / trend-up /
    /// trend-down) instead of pooling them. A machine that excels in
    /// downtrends should not have its record diluted by rallies — the
    /// population specializes by niche instead of averaging to the middle.
    pub per_regime_stats: bool,
    /// Execution cost charged on every fill, in bps of the filled notional:
    /// priority fee + base fee + the slippage a clip pays versus the top of
    /// book. Zero by default so historical benchmarks stay comparable; the
    /// cost-aware benches set it explicitly. Strategies that exit in many
    /// tranches pay it many times — which is exactly the asymmetry a
    /// cost-free simulator hides.
    pub fill_cost_bps: f64,
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
            random_arm_seed: None,
            evolve_every_windows: 1,
            evolve_candidates: 12,
            peak_drop_bps: 30.0,
            // Off by default: a proper sweep (bench 021_sensitivity) puts the
            // spread across 0.0–4.0 at **0 bps**, and a direct A/B moves the
            // floors by under 1.5 bps in both directions — noise. The original
            // claim that it "improved every floor" (bench 012) came from a
            // single measurement with no control. Flag kept; the concept may
            // still matter at horizons these benches do not cover.
            surprise_ratio: 0.0,
            off_policy_credit: true,
            // Off by default: the bench cannot resolve this feature.
            // Bench-length runs (~300 ticks) rebuild the arm set only a few
            // times, so regime-keyed *seeding* barely moves the result —
            // ON and OFF score bit-identically (bench 019). It only bites
            // across long sessions with persistence, which needs a long
            // soak A/B to settle. Shipping the simpler pooled default until
            // there is a measurement that can tell the difference.
            per_regime_stats: false,
            fill_cost_bps: 0.0,
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
    /// Running peak price since entry (drives the off-peak input bit).
    pub peak_price: f64,
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
            peak_price: entry_price,
            fills: Vec::new(),
        }
    }

    /// Normalized value of the position at price `p` (1.0 == held at entry).
    pub fn value_norm(&self, p: f64) -> f64 {
        self.cash_norm + self.remaining_frac * (p / self.entry_price)
    }

    /// Apply a tranche sell of `frac` (of original) at `price`, net of
    /// `cost_bps` execution cost on the filled notional.
    pub fn apply_fill_with_cost(&mut self, tick: u64, price: f64, frac: f64, cost_bps: f64) {
        let frac = frac.min(self.remaining_frac);
        self.remaining_frac -= frac;
        self.cash_norm += frac * (price / self.entry_price) * (1.0 - cost_bps * 1e-4);
        self.fills.push(TrancheFill { tick, price, frac });
    }

    /// Apply a tranche sell with no execution cost.
    pub fn apply_fill(&mut self, tick: u64, price: f64, frac: f64) {
        self.apply_fill_with_cost(tick, price, frac, 0.0);
    }

    /// Whether the position is fully exited.
    pub fn is_closed(&self) -> bool {
        self.remaining_frac <= f64::EPSILON
    }
}
