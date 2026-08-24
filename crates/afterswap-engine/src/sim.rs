//! Exit-strategy replay: run one FSM over a price window, score vs hold.
//!
//! Encoding: FSM input at tick t is 1 when price ticked up (p[t] > p[t-1]),
//! else 0. FSM output 1 = sell one tranche of the remaining position at p[t];
//! output 0 = hold. Payoff is the exit value edge vs pure hold, in bps.

use katgpt_ruliology::{FsmStrategy, SimpleProgram, WinMatrix};

/// Replay `fsm` over `window` (raw prices), selling `tranche_frac` of the
/// original position on each SELL signal. Returns edge vs hold in bps.
///
/// Prices are normalized to `window[0]`, so the result is entry-invariant.
pub fn replay_exit(fsm: &FsmStrategy, window: &[f64], tranche_frac: f64) -> f64 {
    if window.len() < 2 {
        return 0.0;
    }
    let entry = window[0];
    let mut m = fsm.clone();
    m.reset();

    let mut remaining = 1.0f64;
    let mut cash = 0.0f64;

    for t in 1..window.len() {
        let input: u8 = match window[t] > window[t - 1] {
            true => 1,
            false => 0,
        };
        let action = m.next_action(&[input]);
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
) -> (WinMatrix, Vec<f32>) {
    let payoffs: Vec<Vec<f64>> = strategies
        .iter()
        .map(|s| {
            windows
                .iter()
                .map(|w| replay_exit(s, w, tranche_frac))
                .collect()
        })
        .collect();
    let ids: Vec<u64> = strategies.iter().map(|s| s.id()).collect();
    let complexities: Vec<f32> = strategies.iter().map(|s| s.complexity()).collect();
    (WinMatrix::new(payoffs, ids), complexities)
}
