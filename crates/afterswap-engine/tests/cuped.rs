//! CUPED estimator and the execution-cycle decomposition.

use afterswap_engine::cuped::{cuped, cycles_needed};
use afterswap_engine::execution::{ExecutionCycle, cuped_inputs};

fn cycle(n: u64, arrival: f64, submit: f64, fill: f64, impact: Option<f64>) -> ExecutionCycle {
    ExecutionCycle {
        cycle: n,
        t_ms: n,
        arrival_slot: Some(1_000 + n),
        arrival_price: arrival,
        arrival_impact_bps: impact,
        fill_slot: Some(1_000 + n),
        submit_price: submit,
        fill_price: fill,
        // A $1,000 clip paying a 0.001 SOL tip at SOL = $100 is $0.10, or 1 bps.
        // Tip cost in bps scales inversely with clip size, which is why the
        // research cost table's 10-15 bps tip figure is a statement about
        // retail-sized clips rather than about the tip itself.
        notional: 1_000.0,
        tip_lamports: 1_000_000,
        l1_fee_lamports: 500_000,
        sol_price: 100.0,
        arrival_route: "Whirlpool|1".into(),
        submit_route: "Whirlpool|1".into(),
        filled: true,
        simulated: false,
        signature: None,
        revert: None,
    }
}

#[test]
fn reduction_equals_one_minus_rho_squared() {
    // Y is X plus independent noise, so rho is known by construction.
    let x: Vec<f64> = (0..200).map(|i| (i as f64 * 0.37).sin() * 10.0).collect();
    let y: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, v)| 0.8 * v + (i as f64 * 1.13).cos() * 5.0)
        .collect();
    let r = cuped(&y, &x).expect("cuped");
    assert!(
        (r.reduction - r.rho * r.rho).abs() < 1e-9,
        "reduction {} vs rho^2 {}",
        r.reduction,
        r.rho * r.rho
    );
    assert!(r.sd_adj_bps < r.sd_raw_bps);
}

#[test]
fn adjustment_does_not_move_the_estimate() {
    let x: Vec<f64> = (0..120).map(|i| (i as f64 * 0.21).sin()).collect();
    let y: Vec<f64> = x.iter().map(|v| 0.42 + 3.0 * v).collect();
    let r = cuped(&y, &x).expect("cuped");
    // CUPED subtracts a mean-zero term: the point estimate is untouched.
    let raw_mean = y.iter().sum::<f64>() / y.len() as f64;
    assert!((r.mean_bps - raw_mean).abs() < 1e-12);
    let adj_mean = r.adjusted.iter().sum::<f64>() / r.adjusted.len() as f64;
    assert!((adj_mean - raw_mean).abs() < 1e-9, "adjusted mean drifted");
}

#[test]
fn a_perfect_covariate_removes_nearly_all_variance() {
    let x: Vec<f64> = (0..80).map(|i| i as f64).collect();
    let y: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
    let r = cuped(&y, &x).expect("cuped");
    assert!(r.reduction > 0.999, "reduction = {}", r.reduction);
    assert!(r.mde_adj_bps < r.mde_raw_bps * 0.05);
}

#[test]
fn an_uninformative_covariate_is_refused_or_useless() {
    // Constant covariate: theta undefined, not merely unhelpful.
    let y: Vec<f64> = (0..50).map(|i| (i as f64 * 0.7).sin()).collect();
    let x = vec![3.0; 50];
    assert!(cuped(&y, &x).is_none());
    // Too few observations to estimate theta at all.
    assert!(cuped(&[1.0, 2.0], &[1.0, 2.0]).is_none());
}

/// Bench 038's sample-size table. If the CUPED path or the power formulas
/// change, the cycle counts the experiment was scoped against move with them.
#[test]
fn cycles_needed_matches_bench_038() {
    const SD: f64 = 2.6;
    const REDUCTION: f64 = 0.346;
    let at = |d: f64| cycles_needed(d, SD, REDUCTION).ceil();
    assert!((at(0.35) - 284.0).abs() <= 1.0, "0.35 bps -> {}", at(0.35));
    assert!((at(0.25) - 556.0).abs() <= 1.0, "0.25 bps -> {}", at(0.25));
    assert!((at(0.10) - 3470.0).abs() <= 2.0, "0.10 bps -> {}", at(0.10));
    // Without the control variate, 0.25 bps costs 849 cycles.
    assert!((cycles_needed(0.25, SD, 0.0).ceil() - 849.0).abs() <= 1.0);
}

#[test]
fn breakdown_splits_drift_from_impact() {
    // Arrival 100, price drifts to 100.05 before submit, fills at 100.02.
    let c = cycle(1, 100.0, 100.05, 100.02, Some(3.0));
    let b = c.breakdown().expect("breakdown");
    assert!((b.drift_bps - 5.0).abs() < 1e-6, "drift = {}", b.drift_bps);
    assert!((b.impact_bps + 2.9985).abs() < 1e-3, "impact = {}", b.impact_bps);
    assert!((b.realised_bps - 2.0).abs() < 1e-6, "realised = {}", b.realised_bps);
    // 10,000 lamports at SOL=100 on 10,000 notional = 0.001 SOL = $0.10 = 1 bps.
    assert!((b.tip_bps - 1.0).abs() < 1e-9, "tip = {}", b.tip_bps);
    assert!((b.l1_bps - 0.5).abs() < 1e-9, "l1 = {}", b.l1_bps);
    assert!((b.net_margin_bps - 0.5).abs() < 1e-6, "net = {}", b.net_margin_bps);
}

#[test]
fn unfilled_cycles_have_no_margin_and_are_not_zeros() {
    let mut c = cycle(1, 100.0, 100.0, 0.0, Some(3.0));
    c.filled = false;
    assert!(c.breakdown().is_none());
    assert!(!c.admissible(1));
}

#[test]
fn route_change_makes_a_cycle_inadmissible() {
    let mut c = cycle(1, 100.0, 100.0, 100.0, Some(3.0));
    c.submit_route = "Raydium CLMM|2".into();
    assert!(!c.route_stable());
    assert!(!c.admissible(1));
}

#[test]
fn stale_control_variate_is_excluded_at_the_configured_lag() {
    let mut c = cycle(1, 100.0, 100.0, 100.0, Some(3.0));
    c.fill_slot = Some(c.arrival_slot.unwrap() + 5);
    assert_eq!(c.control_lag_slots(), Some(5));
    assert!(c.admissible(5));
    // Bench 038: by lag 5 the reduction has fallen from 34.6% to 19.3%.
    assert!(!c.admissible(1));
}

#[test]
fn cuped_inputs_filters_and_keeps_components_aligned() {
    let mut cycles = vec![
        cycle(1, 100.0, 100.02, 100.01, Some(2.0)),
        cycle(2, 100.0, 100.01, 100.03, Some(3.0)),
        cycle(3, 100.0, 99.99, 100.00, Some(1.0)),
        cycle(4, 100.0, 100.00, 100.02, Some(4.0)),
    ];
    // One with no control variate, one with a changed route: both dropped.
    cycles.push(cycle(5, 100.0, 100.0, 100.0, None));
    let mut bad = cycle(6, 100.0, 100.0, 100.0, Some(9.0));
    bad.submit_route = "other|3".into();
    cycles.push(bad);

    let (net, impact, control) = cuped_inputs(&cycles, 1);
    assert_eq!(net.len(), 4);
    assert_eq!(impact.len(), 4);
    assert_eq!(control, vec![2.0, 3.0, 1.0, 4.0]);
    assert!(cuped(&net, &control).is_some());
}
