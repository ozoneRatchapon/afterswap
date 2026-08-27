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
    replay_exit_cost(fsm, window, tranche_frac, peak_drop_bps, 0.0)
}

/// `replay_exit` with an execution cost charged on every fill (bps).
pub fn replay_exit_cost(
    fsm: &FsmStrategy,
    window: &[f64],
    tranche_frac: f64,
    peak_drop_bps: f64,
    cost_bps: f64,
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
            cash += frac * (window[t] / entry) * (1.0 - cost_bps * 1e-4);
        }
    }

    let last = window[window.len() - 1] / entry;
    let strategy_value = cash + remaining * last;
    let hold_value = last;
    (strategy_value - hold_value) / hold_value * 10_000.0
}

/// Replay with an arbitrary precomputed third input bit.
///
/// Bits per tick: direction, off-peak, then `bits[t]`. Any candidate signal
/// (executable depth, route churn, hop count, …) can be tested by supplying
/// it here rather than by writing another replay function — which keeps every
/// candidate on exactly the same code path as the shipping protocol.
pub fn replay_exit_with_bit(
    fsm: &FsmStrategy,
    window: &[f64],
    bits: &[u8],
    tranche_frac: f64,
    peak_drop_bps: f64,
) -> f64 {
    if window.len() < 2 || bits.len() != window.len() {
        return 0.0;
    }
    let entry = window[0];
    let mut m = fsm.clone();
    m.reset();
    let mut remaining = 1.0f64;
    let mut cash = 0.0f64;
    let mut peak = entry;

    for t in 1..window.len() {
        let dir: u8 = u8::from(window[t] > window[t - 1]);
        peak = peak.max(window[t]);
        let off_peak: u8 = u8::from((peak - window[t]) / peak * 10_000.0 >= peak_drop_bps);
        m.next_action(&[dir]);
        m.next_action(&[off_peak]);
        let action = m.next_action(&[bits[t]]);
        if action == 1 && remaining > 0.0 {
            let frac = tranche_frac.min(remaining);
            remaining -= frac;
            cash += frac * (window[t] / entry);
        }
    }
    let last = window[window.len() - 1] / entry;
    (cash + remaining * last - last) / last * 10_000.0
}

/// Replay with a third input bit derived from DFlow's executable depth.
///
/// Bits per tick: direction, off-peak, then **good depth** — 1 when the
/// small-vs-large clip spread is at or below the median spread seen so far in
/// this window (expanding median, so no lookahead). Depth is information only
/// an aggregator's quotes carry; CEX candles cannot reconstruct it.
///
/// `depths[i]` is the spread in bps aligned with `window[i]`.
pub fn replay_exit_depth(
    fsm: &FsmStrategy,
    window: &[f64],
    depths: &[f64],
    tranche_frac: f64,
    peak_drop_bps: f64,
) -> f64 {
    if window.len() < 2 || depths.len() != window.len() {
        return 0.0;
    }
    let entry = window[0];
    let mut m = fsm.clone();
    m.reset();
    let mut remaining = 1.0f64;
    let mut cash = 0.0f64;
    let mut peak = entry;
    let mut seen: Vec<f64> = Vec::with_capacity(window.len());

    for t in 1..window.len() {
        let dir: u8 = u8::from(window[t] > window[t - 1]);
        peak = peak.max(window[t]);
        let off_peak: u8 = u8::from((peak - window[t]) / peak * 10_000.0 >= peak_drop_bps);

        seen.push(depths[t]);
        let mut sorted = seen.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        let good_depth: u8 = u8::from(depths[t] <= median);

        m.next_action(&[dir]);
        m.next_action(&[off_peak]);
        let action = m.next_action(&[good_depth]);
        if action == 1 && remaining > 0.0 {
            let frac = tranche_frac.min(remaining);
            remaining -= frac;
            cash += frac * (window[t] / entry);
        }
    }
    let last = window[window.len() - 1] / entry;
    (cash + remaining * last - last) / last * 10_000.0
}

/// A DFlow quote recording: price plus the venue-level metadata that only an
/// aggregator's quotes carry.
pub struct QuoteCorpus {
    pub prices: Vec<f64>,
    /// Spread in bps between a small and a large clip (executable depth).
    pub depths: Vec<f64>,
    /// First venue in the route plan.
    pub venues: Vec<String>,
    /// Number of hops in the route plan.
    pub hops: Vec<u8>,
    /// Price impact reported by the same quote as the price, in bps.
    ///
    /// This is the lag-0 CUPED control variate: it shares `context_slot` with
    /// its price by construction, which the two-quote `depths` probe cannot.
    /// `None` on rows recorded before snapshot capture existed.
    pub impact_bps: Vec<Option<f64>>,
    /// Slot the quote was computed against — the freshness key. Wall clock is
    /// not a substitute; poll interval and slot time drift apart.
    pub slots: Vec<Option<u64>>,
}

/// Load a quote recording.
///
/// Reads both shapes: the flat `{"price", "depth_bps", "venue"?, "hops"?}` rows
/// the Plan 001 recorder wrote, and the `QuoteSnapshot` rows the live path
/// writes now, where the depth spread sits under `probe` and the lag-0 impact
/// figure sits at the top level. A row needs a price and at least one depth
/// reading of either kind to be kept — a price-only row carries no control
/// variate and would otherwise pad the series with silent gaps.
pub fn load_quote_corpus(path: &str) -> std::io::Result<QuoteCorpus> {
    let text = std::fs::read_to_string(path)?;
    let mut c = QuoteCorpus {
        prices: Vec::new(),
        depths: Vec::new(),
        venues: Vec::new(),
        hops: Vec::new(),
        impact_bps: Vec::new(),
        slots: Vec::new(),
    };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(p) = v.get("price").and_then(|x| x.as_f64()) else {
            continue;
        };
        let impact = v.get("impact_bps").and_then(|x| x.as_f64());
        let depth = v
            .get("depth_bps")
            .and_then(|x| x.as_f64())
            .or_else(|| v.pointer("/probe/depth_bps").and_then(|x| x.as_f64()));
        // Fall back to the same-quote impact when no spread probe was taken:
        // it measures the same thing more cheaply and with a better lag.
        let Some(d) = depth.or(impact) else {
            continue;
        };
        if p > 0.0 && d.is_finite() {
            c.prices.push(p);
            c.depths.push(d);
            c.venues.push(
                v.get("venue")
                    .and_then(|x| x.as_str())
                    .unwrap_or("?")
                    .to_string(),
            );
            c.hops.push(v.get("hops").and_then(|x| x.as_u64()).unwrap_or(0) as u8);
            c.impact_bps.push(impact);
            c.slots.push(v.get("context_slot").and_then(|x| x.as_u64()));
        }
    }
    Ok(c)
}

/// Load a `{"price": f, "depth_bps": f}` recording as parallel series.
pub fn load_depth_corpus(path: &str) -> std::io::Result<(Vec<f64>, Vec<f64>)> {
    let c = load_quote_corpus(path)?;
    Ok((c.prices, c.depths))
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
    twap_value_norm_cost(prices, open_at, n_slices, stride, 0.0)
}

/// TWAP with a per-fill execution cost (bps) — it pays `n_slices` times.
pub fn twap_value_norm_cost(
    prices: &[f64],
    open_at: usize,
    n_slices: usize,
    stride: usize,
    cost_bps: f64,
) -> f64 {
    let entry = prices[open_at];
    let mut cash = 0.0f64;
    let mut remaining = 1.0f64;
    let frac = 1.0 / n_slices as f64;
    let mut k = 0usize;
    for (i, &p) in prices.iter().enumerate().skip(open_at + 1) {
        if (i - open_at).is_multiple_of(stride) && remaining > 1e-12 {
            let f = frac.min(remaining);
            cash += f * (p / entry) * (1.0 - cost_bps * 1e-4);
            remaining -= f;
            k += 1;
            if k >= n_slices {
                break;
            }
        }
    }
    cash + remaining * (prices[prices.len() - 1] / entry)
}

/// Temporary price impact of one clip, in bps, Almgren–Chriss form.
///
/// Impact depends on the *rate* of liquidation, not just clip size: selling
/// 10% of a position in one tick removes far more liquidity per unit time
/// than the same 10% spread over six. Without this term a simulator rewards
/// dumping — which is exactly the artifact that invalidated our first
/// shortfall result, where the variance-minimising machine simply liquidated
/// four times faster than TWAP and paid nothing for it.
///
/// `eta` is calibrated so a 10% clip at TWAP's cadence (one per 6 ticks) pays
/// ~2 bps, matching the fixed per-fill cost we already charge; the same clip
/// at one per tick pays six times that. Real CPMM impact is convex in size
/// too, but rate is the term that distinguishes these schedules.
pub fn temporary_impact_bps(frac: f64, ticks_since_last: usize, eta: f64) -> f64 {
    let dt = ticks_since_last.max(1) as f64;
    eta * frac / dt
}

/// Default `eta`: 10% clip every 6 ticks → 2 bps.
pub const DEFAULT_ETA: f64 = 120.0;

/// Arrival-price implementation shortfall, in bps of the position.
///
/// The objective external review prescribed in place of "edge versus hold":
/// measure the gap between liquidating everything at the arrival price and
/// what the trajectory actually realised, residual marked at the terminal
/// price, net of per-fill cost. **Positive is worse.** Unlike edge-versus-hold
/// this is defined for any schedule without reference to a counterfactual
/// holder, which is what makes it comparable across strategies that liquidate
/// on different cadences.
pub fn shortfall_bps(
    fsm: &FsmStrategy,
    window: &[f64],
    tranche_frac: f64,
    peak_drop_bps: f64,
    cost_bps: f64,
) -> f64 {
    shortfall_bps_impact(fsm, window, tranche_frac, peak_drop_bps, cost_bps, 0.0)
}

/// `shortfall_bps` with rate-dependent temporary impact (`eta`, see
/// [`temporary_impact_bps`]). Pass `DEFAULT_ETA` for the calibrated model,
/// or 0.0 for the impact-free simulator that flatters fast liquidation.
pub fn shortfall_bps_impact(
    fsm: &FsmStrategy,
    window: &[f64],
    tranche_frac: f64,
    peak_drop_bps: f64,
    cost_bps: f64,
    eta: f64,
) -> f64 {
    if window.len() < 2 {
        return 0.0;
    }
    let arrival = window[0];
    let mut m = fsm.clone();
    m.reset();
    let (mut remaining, mut cash, mut peak) = (1.0f64, 0.0f64, arrival);
    let mut last_fill_tick = 0usize;

    for t in 1..window.len() {
        let dir: u8 = u8::from(window[t] > window[t - 1]);
        peak = peak.max(window[t]);
        let off_peak: u8 = u8::from((peak - window[t]) / peak * 10_000.0 >= peak_drop_bps);
        m.next_action(&[dir]);
        let action = m.next_action(&[off_peak]);
        if action == 1 && remaining > 0.0 {
            let frac = tranche_frac.min(remaining);
            remaining -= frac;
            let impact = temporary_impact_bps(frac, t - last_fill_tick, eta);
            last_fill_tick = t;
            cash += frac * (window[t] / arrival) * (1.0 - (cost_bps + impact) * 1e-4);
        }
    }
    let realised = cash + remaining * (window[window.len() - 1] / arrival);
    (1.0 - realised) * 10_000.0
}

/// TWAP's implementation shortfall on the same window, same convention.
pub fn twap_shortfall_bps(
    window: &[f64],
    n_slices: usize,
    stride: usize,
    cost_bps: f64,
) -> f64 {
    twap_shortfall_bps_impact(window, n_slices, stride, cost_bps, 0.0)
}

/// TWAP shortfall charged the same impact model as the machines.
pub fn twap_shortfall_bps_impact(
    window: &[f64],
    n_slices: usize,
    stride: usize,
    cost_bps: f64,
    eta: f64,
) -> f64 {
    if window.len() < 2 {
        return 0.0;
    }
    let arrival = window[0];
    let (mut remaining, mut cash, mut done) = (1.0f64, 0.0f64, 0usize);
    let frac = 1.0 / n_slices as f64;
    for (i, &p) in window.iter().enumerate().skip(1) {
        if i.is_multiple_of(stride) && done < n_slices && remaining > 1e-12 {
            let f = frac.min(remaining);
            let impact = temporary_impact_bps(f, stride, eta);
            cash += f * (p / arrival) * (1.0 - (cost_bps + impact) * 1e-4);
            remaining -= f;
            done += 1;
        }
    }
    let realised = cash + remaining * (window[window.len() - 1] / arrival);
    (1.0 - realised) * 10_000.0
}

/// Trailing stop (Jupiter's July-2026 flagship exit): sell everything the
/// first time price drops `drop_bps` below its running peak since entry.
pub fn trailing_stop_value_norm(prices: &[f64], open_at: usize, drop_bps: f64) -> f64 {
    trailing_stop_value_norm_cost(prices, open_at, drop_bps, 0.0)
}

/// Trailing stop with execution cost — it exits all at once, so it pays once.
pub fn trailing_stop_value_norm_cost(
    prices: &[f64],
    open_at: usize,
    drop_bps: f64,
    cost_bps: f64,
) -> f64 {
    let entry = prices[open_at];
    let mut peak = entry;
    for &p in &prices[open_at + 1..] {
        peak = peak.max(p);
        if (peak - p) / peak * 10_000.0 >= drop_bps {
            return p / entry * (1.0 - cost_bps * 1e-4);
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
