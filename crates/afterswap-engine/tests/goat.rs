//! GOAT gates (G1–G4) — the discipline ported from katgpt-rs: no claim
//! without a named floor. Debug-lenient latency budgets here; release
//! numbers come from `examples/goat_bench.rs`.

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{Regime, load_corpus, simulate, synthetic_corpus, twap_value_norm};

const OPEN_AT: usize = 30;
const TWAP_SLICES: usize = 10;
const TWAP_STRIDE: usize = 6;
const RANDOM_SEEDS: [u64; 4] = [1, 2, 3, 4];

fn goat_cfg() -> EngineConfig {
    EngineConfig {
        window_len: 12,
        window_stride: 6,
        n_fsm_states: 3,
        tranche_frac: 0.1,
        max_arms: 24,
        ..EngineConfig::default()
    }
}

/// All corpora: 4 synthetic regimes (seeded) + the bundled real recording.
fn corpora() -> Vec<(String, Vec<f64>)> {
    let mut out: Vec<(String, Vec<f64>)> = Regime::ALL
        .iter()
        .map(|&r| (r.name().to_string(), synthetic_corpus(r, 300, 42)))
        .collect();
    if let Ok(real) = load_corpus("../../data/recorded.jsonl") {
        if real.len() >= 100 {
            out.push(("recorded_dflow".to_string(), real));
        }
    }
    out
}

fn diff_bps(a: f64, b: f64) -> f64 {
    (a - b) / b * 10_000.0
}

/// G1 — determinism: identical config + corpus → bit-identical event
/// stream and final value, twice.
#[test]
fn g1_bit_reproducible() {
    for (name, prices) in corpora() {
        let a = simulate(goat_cfg(), &prices, OPEN_AT, 1.0);
        let b = simulate(goat_cfg(), &prices, OPEN_AT, 1.0);
        assert_eq!(a.events_json, b.events_json, "G1 FAIL: event drift on {name}");
        assert_eq!(
            a.final_value_norm.to_bits(),
            b.final_value_norm.to_bits(),
            "G1 FAIL: value drift on {name}"
        );
    }
}

/// G2a — floor: mean edge vs a same-cadence TWAP exit must be positive
/// across regimes. TWAP is the honest like-for-like floor (both fully
/// exit); hold is regime-dependent and reported, not gated.
#[test]
fn g2a_beats_twap_floor() {
    let mut diffs = Vec::new();
    for (name, prices) in corpora() {
        let engine = simulate(goat_cfg(), &prices, OPEN_AT, 1.0);
        let twap = twap_value_norm(&prices, OPEN_AT, TWAP_SLICES, TWAP_STRIDE);
        let d = diff_bps(engine.final_value_norm, twap);
        println!("G2a {name}: engine {:.5} vs twap {twap:.5} → {d:+.1} bps", engine.final_value_norm);
        diffs.push(d);
    }
    let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
    println!("G2a mean vs TWAP: {mean:+.2} bps");
    assert!(mean > 0.0, "G2a FAIL: mean edge vs TWAP {mean:+.2} bps ≤ 0");
}

/// G2b — floor: UCB1 selection must beat seeded random arm selection
/// (same machines, same tranches) on mean across corpora × seeds.
#[test]
fn g2b_beats_random_arm_floor() {
    let mut diffs = Vec::new();
    for (name, prices) in corpora() {
        let ucb = simulate(goat_cfg(), &prices, OPEN_AT, 1.0);
        let rand_mean = RANDOM_SEEDS
            .iter()
            .map(|&seed| {
                let cfg = EngineConfig {
                    random_arm_seed: Some(seed),
                    ..goat_cfg()
                };
                simulate(cfg, &prices, OPEN_AT, 1.0).final_value_norm
            })
            .sum::<f64>()
            / RANDOM_SEEDS.len() as f64;
        let d = diff_bps(ucb.final_value_norm, rand_mean);
        println!("G2b {name}: ucb {:.5} vs random {rand_mean:.5} → {d:+.1} bps", ucb.final_value_norm);
        diffs.push(d);
    }
    let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
    println!("G2b mean vs random-arm: {mean:+.2} bps");
    assert!(mean >= 0.0, "G2b FAIL: mean edge vs random {mean:+.2} bps < 0");
}

/// G2c — report-only: edge vs hold per regime (an exit product should
/// win in down/chop and pay opportunity cost in up; we report, not gate).
#[test]
fn g2c_report_vs_hold() {
    for (name, prices) in corpora() {
        let r = simulate(goat_cfg(), &prices, OPEN_AT, 1.0);
        println!(
            "G2c {name}: engine {:.5} vs hold {:.5} → {:+.1} bps ({} fills, closed={})",
            r.final_value_norm, r.hold_value_norm, r.edge_vs_hold_bps, r.fills, r.closed
        );
    }
}

/// G3 — ablation: the 24-arm cap must not cost more than 10 bps vs an
/// effectively uncapped front on any corpus.
#[test]
fn g3_arm_cap_ablation() {
    for (name, prices) in corpora() {
        let capped = simulate(goat_cfg(), &prices, OPEN_AT, 1.0);
        let full = simulate(
            EngineConfig {
                max_arms: 4096,
                ..goat_cfg()
            },
            &prices,
            OPEN_AT,
            1.0,
        );
        let d = diff_bps(capped.final_value_norm, full.final_value_norm);
        println!("G3 {name}: cap24 vs uncapped → {d:+.1} bps");
        assert!(d > -10.0, "G3 FAIL on {name}: cap costs {d:+.1} bps");
    }
}

/// G5 — evolution ablation: mutants must not degrade the TWAP edge by
/// more than 5 bps mean across corpora (they should help or be neutral).
#[test]
fn g5_evolution_ablation() {
    let (mut on_sum, mut off_sum, mut n) = (0.0, 0.0, 0.0);
    for (name, prices) in corpora() {
        let on = simulate(goat_cfg(), &prices, OPEN_AT, 1.0).final_value_norm;
        let off = simulate(
            EngineConfig {
                evolve_every_windows: 0,
                ..goat_cfg()
            },
            &prices,
            OPEN_AT,
            1.0,
        )
        .final_value_norm;
        let twap = twap_value_norm(&prices, OPEN_AT, TWAP_SLICES, TWAP_STRIDE);
        let (d_on, d_off) = (diff_bps(on, twap), diff_bps(off, twap));
        println!("G5 {name}: evolve-on {d_on:+.1} bps vs TWAP, evolve-off {d_off:+.1} bps");
        on_sum += d_on;
        off_sum += d_off;
        n += 1.0;
    }
    let (mean_on, mean_off) = (on_sum / n, off_sum / n);
    println!("G5 mean: on {mean_on:+.2} / off {mean_off:+.2} bps");
    assert!(
        mean_on >= mean_off - 5.0,
        "G5 FAIL: evolution costs {:.2} bps",
        mean_off - mean_on
    );
}

/// G4 — latency (debug-lenient): mean on_tick under 5 ms, worst tick
/// (bootstrap tournament) under 5 s. Real numbers in the release bench.
#[test]
fn g4_latency_budget() {
    use std::time::Instant;
    let prices = synthetic_corpus(Regime::Chop, 300, 7);
    let mut engine = afterswap_engine::ExitEngine::new(goat_cfg());
    let mut worst = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    for (i, &p) in prices.iter().enumerate() {
        let t0 = Instant::now();
        engine.on_tick(p);
        let dt = t0.elapsed();
        total += dt;
        worst = worst.max(dt);
        if i == OPEN_AT {
            engine.open_position(1.0);
        }
    }
    let mean = total / prices.len() as u32;
    println!("G4: mean on_tick {mean:?}, worst (tournament) {worst:?}");
    assert!(mean.as_millis() < 5, "G4 FAIL: mean {mean:?}");
    assert!(worst.as_secs() < 5, "G4 FAIL: worst {worst:?}");
}
