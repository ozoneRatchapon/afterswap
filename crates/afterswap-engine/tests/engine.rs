//! Integration tests for the AfterSwap exit engine.

use afterswap_engine::engine::EngineEvent;
use afterswap_engine::sim::{evaluate_matrix, replay_exit};
use afterswap_engine::{EngineConfig, ExitEngine, WindowStore};
use katgpt_ruliology::{FsmEnumerator, FsmStrategy};

/// Always-sell machine: single state, output 1.
fn always_sell() -> FsmStrategy {
    FsmStrategy::new([[0, 0]; 4], [1, 0, 0, 0], 1, 0)
}

/// Never-sell machine: single state, output 0.
fn never_sell() -> FsmStrategy {
    FsmStrategy::new([[0, 0]; 4], [0, 0, 0, 0], 1, 0)
}

#[test]
fn replay_exit_hand_math() {
    // Window 100 → 110 → 90 → 80, tranche 50%.
    // Always-sell: sells 0.5 @ 1.1 (cash 0.55) and 0.5 @ 0.9 (cash 1.0).
    // Hold ends at 0.8 → edge = (1.0 - 0.8) / 0.8 * 1e4 = 2500 bps.
    let window = [100.0, 110.0, 90.0, 80.0];
    let edge = replay_exit(&always_sell(), &window, 0.5);
    assert!((edge - 2500.0).abs() < 1e-9, "edge = {edge}");
}

#[test]
fn hold_machine_is_always_zero_edge() {
    let windows = [
        vec![100.0, 105.0, 111.0, 103.0, 98.0],
        vec![50.0, 49.0, 48.0, 47.5, 51.0],
    ];
    for w in &windows {
        let edge = replay_exit(&never_sell(), w, 0.25);
        assert!(edge.abs() < 1e-12, "hold edge must be 0, got {edge}");
    }
}

#[test]
fn nobody_beats_hold_on_monotone_rise() {
    // On a strictly rising window every sell locks in a lower price,
    // so the best possible edge is exactly 0 (the hold machines).
    let rising: Vec<f64> = (0..32).map(|i| 100.0 * 1.01f64.powi(i)).collect();
    let strategies = FsmEnumerator::enumerate(2);
    let (matrix, _cx) = evaluate_matrix(&strategies, &[rising], 0.25);
    let best = (0..strategies.len())
        .map(|i| matrix.avg_payoff(i))
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(best.abs() < 1e-9, "best edge on rise must be 0, got {best}");
}

#[test]
fn sellers_win_on_monotone_crash() {
    let falling: Vec<f64> = (0..32).map(|i| 100.0 * 0.99f64.powi(i)).collect();
    let strategies = FsmEnumerator::enumerate(2);
    let (matrix, _cx) = evaluate_matrix(&strategies, &[falling], 0.25);
    let best = (0..strategies.len())
        .map(|i| matrix.avg_payoff(i))
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(best > 100.0, "expected a seller to beat hold, best = {best}");
}

#[test]
fn window_store_strided_extraction() {
    let mut store = WindowStore::new(4, 2, 3);
    for i in 1..=10 {
        store.push(f64::from(i));
    }
    let windows = store.windows();
    assert_eq!(windows.len(), 3);
    assert_eq!(windows[0], vec![3.0, 4.0, 5.0, 6.0]);
    assert_eq!(windows[2], vec![7.0, 8.0, 9.0, 10.0]);
}

#[test]
fn engine_end_to_end_crash_path() {
    let config = EngineConfig {
        n_fsm_states: 2,
        window_len: 8,
        window_stride: 4,
        max_windows: 4,
        tranche_frac: 0.25,
        refresh_every_windows: 2,
        ..EngineConfig::default()
    };
    let mut engine = ExitEngine::new(config);

    // Deterministic path: 40 zigzag ticks, then a strict 1%/tick crash.
    let mut prices: Vec<f64> = Vec::new();
    let mut p = 100.0;
    for i in 0..40 {
        p *= match i % 2 {
            0 => 1.004,
            _ => 0.997,
        };
        prices.push(p);
    }
    for _ in 0..80 {
        p *= 0.99;
        prices.push(p);
    }

    let mut tournaments = 0usize;
    let mut selections = 0usize;
    let mut fills = 0usize;
    let mut windows_closed = 0usize;
    let mut final_value: Option<f64> = None;

    for (i, price) in prices.iter().enumerate() {
        let events = engine.on_tick(*price);
        for ev in &events {
            match ev {
                EngineEvent::Tournament { .. } => tournaments += 1,
                EngineEvent::ArmSelected { .. } => selections += 1,
                EngineEvent::TrancheFilled { .. } => fills += 1,
                EngineEvent::WindowClosed { .. } => windows_closed += 1,
                EngineEvent::PositionClosed {
                    final_value_norm, ..
                } => final_value = Some(*final_value_norm),
            }
        }
        // Open the position right before the crash begins.
        if i == 39 {
            assert!(engine.open_position(0.05).is_some(), "position must open");
        }
    }

    assert!(tournaments >= 1, "no tournament ran");
    assert!(selections >= 1, "no arm ever selected");
    assert!(fills >= 1, "no tranche was ever sold on a crash path");

    let snap = engine.snapshot(64);
    assert!(snap.strategies_enumerated >= 20);

    // The crash grinds price to ~0.45× entry. Either the engine fully
    // exited (early sells → final value well above the hold floor), or it
    // is still managing the position and must be beating hold.
    match final_value {
        Some(v) => {
            assert!(v > 0.6, "full exit on a crash should lock value > 0.6, got {v}");
        }
        None => {
            assert!(windows_closed >= 2, "windows closed = {windows_closed}");
            match (snap.position_value_norm, snap.hold_value_norm) {
                (Some(v), Some(h)) => assert!(v >= h, "exit value {v} < hold {h} on crash"),
                _ => panic!("position vanished without PositionClosed"),
            }
        }
    }
}
