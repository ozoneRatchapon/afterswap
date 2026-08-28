//! The Schmitt-trigger replay must reduce to the shipping replay.
//!
//! A sweep is only readable if its baseline row *is* the shipping protocol,
//! not a re-implementation that happens to agree. These tests pin that:
//! at a collapsed band the two functions agree bit-for-bit on every
//! enumerated machine, and a widened band actually changes behaviour
//! (otherwise the first test would pass vacuously).

use afterswap_engine::sim::{load_corpus, replay_exit_cost, replay_exit_hysteresis};
use katgpt_ruliology::FsmEnumerator;

const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;

fn window() -> Vec<f64> {
    match load_corpus("../../data/incoming/recorded_long.jsonl") {
        Ok(v) if v.len() >= 600 => v[..600].to_vec(),
        // Synthetic fallback keeps the invariant testable without the corpus:
        // a saw-tooth crosses the threshold repeatedly, which is the only
        // regime where the two functions could ever disagree.
        _ => (0..600)
            .map(|t| 100.0 + (t as f64 * 0.7).sin() * 0.5 - t as f64 * 0.002)
            .collect(),
    }
}

#[test]
fn collapsed_band_is_the_shipping_replay() {
    let w = window();
    for m in FsmEnumerator::enumerate(3) {
        let shipping = replay_exit_cost(&m, &w, TRANCHE, PEAK_DROP_BPS, 0.0);
        let collapsed =
            replay_exit_hysteresis(&m, &w, TRANCHE, PEAK_DROP_BPS, PEAK_DROP_BPS, 0.0);
        assert_eq!(
            shipping.to_bits(),
            collapsed.to_bits(),
            "collapsed band diverged from the shipping replay"
        );
    }
}

#[test]
fn a_widened_band_changes_some_machine() {
    let w = window();
    // Deliberately not at PEAK_DROP_BPS: at 30 bps the drawdown bit crosses
    // roughly once per window on this corpus and hysteresis has nothing to
    // suppress. The band only bites where the bit actually chatters, which
    // is the low-threshold regime — so that is where vacuity is checked.
    const ARM: f64 = 10.0;
    const DISARM: f64 = 3.0;
    let changed = FsmEnumerator::enumerate(3).iter().any(|m| {
        let shipping = replay_exit_cost(m, &w, TRANCHE, ARM, 0.0);
        let widened = replay_exit_hysteresis(m, &w, TRANCHE, ARM, DISARM, 0.0);
        (shipping - widened).abs() > 1e-9
    });
    assert!(
        changed,
        "no machine reacted to the memory band — the equivalence test is vacuous"
    );
}
