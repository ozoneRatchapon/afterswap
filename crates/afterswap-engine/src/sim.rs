//! Exit-strategy replay: run one FSM over a price window, score vs hold.
//!
//! Encoding: FSM input at tick t is 1 when price ticked up (p[t] > p[t-1]),
//! else 0. FSM output 1 = sell one tranche of the remaining position at p[t];
//! output 0 = hold. Payoff is the exit value edge vs pure hold, in bps.

use katgpt_ruliology::{FsmStrategy, SimpleProgram, WinMatrix};

/// Replay `fsm` over `window` (raw prices), selling `tranche_frac` of the
/// original position on each SELL signal. Returns edge vs hold in bps.
///
/// Two machine steps per tick: first the direction bit (price up-tick),
/// then the off-peak bit (price ≥ `peak_drop_bps` below its running peak).
/// The sell decision is the output after both bits — so the enumerated
/// binary machines gain trailing-stop expressiveness with zero new types.
///
/// Prices are normalized to `window[0]`, so the result is entry-invariant.
pub fn replay_exit(
    fsm: &FsmStrategy,
    window: &[f64],
    tranche_frac: f64,
    peak_drop_bps: f64,
) -> f64 {
    if window.len() < 2 {
        return 0.0;
    }
    let entry = window[0];
    let mut m = fsm.clone();
    m.reset();

    let mut remaining = 1.0f64;
    let mut cash = 0.0f64;
    let mut peak = entry;

    for t in 1..window.len() {
        let dir: u8 = match window[t] > window[t - 1] {
            true => 1,
            false => 0,
        };
        peak = peak.max(window[t]);
        let off_peak: u8 = match (peak - window[t]) / peak * 10_000.0 >= peak_drop_bps {
            true => 1,
            false => 0,
        };
        m.next_action(&[dir]);
        let action = m.next_action(&[off_peak]);
        if action == 1 && remaining > 0.0 {
            let frac = tranche_frac.min(remaining);
            remaining -= frac;
            cash += frac * (window[t] / entry);
        }
    }

    let last = window[window.len() - 1] / entry;
    let strategy_value = cash + remaining * last;
    let hold_value = last;
    (strategy_value - hold_value) / hold_value * 10_000.0
}

/// Evaluate every strategy on every window.
///
/// Returns a `WinMatrix` whose `payoffs[i][j]` is strategy `i`'s edge (bps)
/// on window `j`, plus per-strategy complexities for Pareto pruning.
///
/// NOTE: `WinMatrix::new` computes `rankings` averages with a `/(n-1)`
/// divisor meant for square round-robin matrices. With strategy×window rows
/// that scales every average by the same constant `W/(n-1)`, which preserves
/// ordering and Pareto dominance; use `WinMatrix::avg_payoff` when the
/// absolute bps number matters.
pub fn evaluate_matrix(
    strategies: &[FsmStrategy],
    windows: &[Vec<f64>],
    tranche_frac: f64,
    peak_drop_bps: f64,
) -> (WinMatrix, Vec<f32>) {
    let payoffs: Vec<Vec<f64>> = strategies
        .iter()
        .map(|s| {
            windows
                .iter()
                .map(|w| replay_exit(s, w, tranche_frac, peak_drop_bps))
                .collect()
        })
        .collect();
    let ids: Vec<u64> = strategies.iter().map(|s| s.id()).collect();
    let complexities: Vec<f32> = strategies.iter().map(|s| s.complexity()).collect();
    (WinMatrix::new(payoffs, ids), complexities)
}

// ---------------------------------------------------------------------------
// GOAT harness: pure paper simulation over a full corpus, floor strategies,
// deterministic synthetic corpora. No I/O except the corpus loader.
// ---------------------------------------------------------------------------

use crate::engine::{EngineEvent, ExitEngine};
use crate::types::EngineConfig;

/// Result of one simulated position lifecycle.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SimResult {
    /// Exit value in units of entry (cash + residual at last price).
    pub final_value_norm: f64,
    /// Hold-to-end value in units of entry.
    pub hold_value_norm: f64,
    /// Edge vs holding, in bps.
    pub edge_vs_hold_bps: f64,
    /// Fully serialized event stream (G1 bit-identity check).
    pub events_json: String,
    pub fills: usize,
    pub closed: bool,
}

/// Feed `prices` through a fresh engine, opening one position at
/// `open_at`. Fully deterministic for a fixed `(cfg, prices, open_at)`.
pub fn simulate(cfg: EngineConfig, prices: &[f64], open_at: usize, size: f64) -> SimResult {
    let mut engine = ExitEngine::new(cfg);
    let mut events_json = String::new();
    let mut fills = 0usize;
    for (i, &p) in prices.iter().enumerate() {
        for ev in engine.on_tick(p) {
            if matches!(ev, EngineEvent::TrancheFilled { .. }) {
                fills += 1;
            }
            events_json.push_str(&serde_json::to_string(&ev).expect("event serializes"));
            events_json.push('\n');
        }
        if i == open_at {
            engine.open_position(size);
        }
    }
    let last = *prices.last().expect("non-empty corpus");
    let snap = engine.snapshot(1);
    let entry = prices[open_at];
    let hold_value_norm = last / entry;
    // Open → engine live values; fully exited → locked summary re-based to
    // end-of-corpus (cash is final; hold counterfactual runs to the end).
    let (final_value_norm, closed) = match (snap.position_value_norm, &snap.last_closed) {
        (Some(v), _) => (v, false),
        (None, Some(c)) => (c.final_value_norm * entry / c.position.entry_price, true),
        (None, None) => (1.0, false),
    };
    let edge_vs_hold_bps = (final_value_norm - hold_value_norm) / hold_value_norm * 10_000.0;
    SimResult {
        final_value_norm,
        hold_value_norm,
        edge_vs_hold_bps,
        events_json,
        fills,
        closed,
    }
}

/// TWAP floor: sell `1/n_slices` every `stride` ticks from `open_at`,
/// unconditionally. Returns final value in units of entry.
pub fn twap_value_norm(prices: &[f64], open_at: usize, n_slices: usize, stride: usize) -> f64 {
    let entry = prices[open_at];
    let mut cash = 0.0f64;
    let mut remaining = 1.0f64;
    let frac = 1.0 / n_slices as f64;
    let mut k = 0usize;
    for (i, &p) in prices.iter().enumerate().skip(open_at + 1) {
        if (i - open_at).is_multiple_of(stride) && remaining > 1e-12 {
            let f = frac.min(remaining);
            cash += f * (p / entry);
            remaining -= f;
            k += 1;
            if k >= n_slices {
                break;
            }
        }
    }
    cash + remaining * (prices[prices.len() - 1] / entry)
}

/// Trailing stop (Jupiter's July-2026 flagship exit): sell everything the
/// first time price drops `drop_bps` below its running peak since entry.
pub fn trailing_stop_value_norm(prices: &[f64], open_at: usize, drop_bps: f64) -> f64 {
    let entry = prices[open_at];
    let mut peak = entry;
    for &p in &prices[open_at + 1..] {
        peak = peak.max(p);
        if (peak - p) / peak * 10_000.0 >= drop_bps {
            return p / entry;
        }
    }
    prices[prices.len() - 1] / entry
}

/// Take-profit ladder: sell `1/n_rungs` each time price first reaches
/// entry × (1 + k·step_bps), k = 1..=n_rungs. Residual rides to the end.
pub fn tp_ladder_value_norm(
    prices: &[f64],
    open_at: usize,
    n_rungs: usize,
    step_bps: f64,
) -> f64 {
    let entry = prices[open_at];
    let (mut cash, mut remaining, mut next) = (0.0f64, 1.0f64, 1usize);
    let frac = 1.0 / n_rungs as f64;
    for &p in &prices[open_at + 1..] {
        while next <= n_rungs
            && remaining > 1e-12
            && p >= entry * (1.0 + next as f64 * step_bps * 1e-4)
        {
            let f = frac.min(remaining);
            cash += f * (p / entry);
            remaining -= f;
            next += 1;
        }
    }
    cash + remaining * (prices[prices.len() - 1] / entry)
}

/// TP+SL bracket (OCO, the third-party-bot default): all-out at
/// entry+tp_bps or entry−sl_bps, whichever first.
pub fn bracket_value_norm(prices: &[f64], open_at: usize, tp_bps: f64, sl_bps: f64) -> f64 {
    let entry = prices[open_at];
    for &p in &prices[open_at + 1..] {
        let d = (p - entry) / entry * 10_000.0;
        if d >= tp_bps || d <= -sl_bps {
            return p / entry;
        }
    }
    prices[prices.len() - 1] / entry
}

/// Synthetic market regimes (seeded, deterministic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    TrendUp,
    TrendDown,
    Chop,
    VShape,
}

impl Regime {
    pub const ALL: [Regime; 4] = [
        Regime::TrendUp,
        Regime::TrendDown,
        Regime::Chop,
        Regime::VShape,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Regime::TrendUp => "trend_up",
            Regime::TrendDown => "trend_down",
            Regime::Chop => "chop",
            Regime::VShape => "v_shape",
        }
    }
}

/// Generate a deterministic synthetic corpus: geometric walk with
/// per-regime drift (in bps/tick) plus seeded noise.
pub fn synthetic_corpus(regime: Regime, len: usize, seed: u64) -> Vec<f64> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut p = 100.0f64;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let drift_bps = match regime {
            Regime::TrendUp => 1.5,
            Regime::TrendDown => -1.5,
            Regime::Chop => 0.0,
            Regime::VShape => match i < len / 2 {
                true => -3.0,
                false => 3.0,
            },
        };
        let noise_bps = (rng.f64() - 0.5) * 8.0;
        let revert_bps = match regime {
            Regime::Chop => (100.0 - p) / 100.0 * 40.0 * 100.0 / 100.0,
            _ => 0.0,
        };
        p *= 1.0 + (drift_bps + noise_bps + revert_bps) * 1e-4;
        out.push(p);
    }
    out
}

/// Block-bootstrap longer-horizon bars from a recorded tick series.
///
/// Each output bar aggregates `factor` consecutive real returns sampled
/// as a block, so the return distribution and short-range autocorrelation
/// of the source are preserved while the effective bar duration (and
/// per-bar volatility) scales up. This is how we test whether the engine's
/// edge depends on horizon without waiting hours per experiment. Labeled
/// bootstrapped, never presented as raw recorded data.
pub fn bootstrap_bars(
    source: &[f64],
    n_bars: usize,
    factor: usize,
    seed: u64,
    demean: bool,
) -> Vec<f64> {
    if source.len() < factor + 2 || factor == 0 {
        return source.to_vec();
    }
    let returns: Vec<f64> = source
        .windows(2)
        .map(|w| (w[1] / w[0]).ln())
        .filter(|r| r.is_finite())
        .collect();
    // Drift confound: the recorded window was strongly bullish, and
    // bootstrapping compounds that drift over long horizons until holding
    // wins by construction. `demean` removes the mean return so the
    // experiment isolates exit timing from market direction.
    let drift = match demean {
        true => returns.iter().sum::<f64>() / returns.len() as f64,
        false => 0.0,
    };
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut price = source[0];
    let mut out = Vec::with_capacity(n_bars + 1);
    out.push(price);
    for _ in 0..n_bars {
        let start = rng.usize(..returns.len().saturating_sub(factor).max(1));
        let bar: f64 = returns[start..(start + factor).min(returns.len())]
            .iter()
            .map(|r| r - drift)
            .sum();
        price *= bar.exp();
        out.push(price);
    }
    out
}

/// Number of behaviorally distinct machines at a given state count.
pub fn enumerate_count(n_states: u8) -> usize {
    katgpt_ruliology::FsmEnumerator::enumerate(n_states).len()
}

/// Load a `{"price": f}` jsonl recording.
pub fn load_corpus(path: &str) -> std::io::Result<Vec<f64>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("price").and_then(|p| p.as_f64()))
        .collect())
}
