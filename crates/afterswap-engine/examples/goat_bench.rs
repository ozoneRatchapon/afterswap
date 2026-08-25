//! GOAT release bench: runs every gate's measurement on all corpora and
//! writes an auto-numbered report under `benches/`.
//!
//! Run: `cargo run -p afterswap-engine --example goat_bench --release`

use std::fmt::Write as _;
use std::time::Instant;

use afterswap_engine::sim::{Regime, load_corpus, simulate, synthetic_corpus, twap_value_norm};
use afterswap_engine::{EngineConfig, ExitEngine};

const OPEN_AT: usize = 30;
const RANDOM_SEEDS: [u64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

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

fn diff_bps(a: f64, b: f64) -> f64 {
    (a - b) / b * 10_000.0
}

fn main() {
    let mut corpora: Vec<(String, Vec<f64>)> = Regime::ALL
        .iter()
        .map(|&r| (r.name().to_string(), synthetic_corpus(r, 300, 42)))
        .collect();
    if let Ok(real) = load_corpus("data/recorded.jsonl") {
        if real.len() >= 100 {
            corpora.push(("recorded_dflow".to_string(), real));
        }
    }

    let mut md = String::from("# GOAT report — AfterSwap exit engine\n\n");
    let _ = writeln!(
        md,
        "Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@{OPEN_AT}.\n"
    );

    // G1
    let mut g1_ok = true;
    for (name, prices) in &corpora {
        let a = simulate(goat_cfg(), prices, OPEN_AT, 1.0);
        let b = simulate(goat_cfg(), prices, OPEN_AT, 1.0);
        g1_ok &= a.events_json == b.events_json
            && a.final_value_norm.to_bits() == b.final_value_norm.to_bits();
        let _ = name;
    }
    let _ = writeln!(
        md,
        "## G1 determinism — {}\n\nBit-identical event stream + final value on every corpus, two runs.\n",
        if g1_ok { "PASS" } else { "FAIL" }
    );

    // G2 table
    let _ = writeln!(
        md,
        "## G2 floors\n\n| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |\n|---|---|---|---|---|---|---|---|"
    );
    let (mut sum_twap, mut sum_rand) = (0.0, 0.0);
    for (name, prices) in &corpora {
        let e = simulate(goat_cfg(), prices, OPEN_AT, 1.0);
        let twap = twap_value_norm(prices, OPEN_AT, 10, 6);
        let rand_mean = RANDOM_SEEDS
            .iter()
            .map(|&seed| {
                simulate(
                    EngineConfig {
                        random_arm_seed: Some(seed),
                        ..goat_cfg()
                    },
                    prices,
                    OPEN_AT,
                    1.0,
                )
                .final_value_norm
            })
            .sum::<f64>()
            / RANDOM_SEEDS.len() as f64;
        let (dt, dr, dh) = (
            diff_bps(e.final_value_norm, twap),
            diff_bps(e.final_value_norm, rand_mean),
            e.edge_vs_hold_bps,
        );
        sum_twap += dt;
        sum_rand += dr;
        let _ = writeln!(
            md,
            "| {name} | {:.5} | {:.5} | {twap:.5} | {rand_mean:.5} | {dt:+.1} bps | {dr:+.1} bps | {dh:+.1} bps |",
            e.final_value_norm, e.hold_value_norm
        );
    }
    let n = corpora.len() as f64;
    let (mean_twap, mean_rand) = (sum_twap / n, sum_rand / n);
    let _ = writeln!(
        md,
        "\n**G2a vs TWAP: {mean_twap:+.2} bps mean — {}** · **G2b vs random-arm: {mean_rand:+.2} bps mean — {}** · vs hold is report-only (regime-dependent opportunity cost).\n",
        if mean_twap > 0.0 { "PASS" } else { "FAIL" },
        if mean_rand >= 0.0 { "PASS" } else { "FAIL" },
    );

    // G3
    let _ = writeln!(md, "## G3 arm-cap ablation (24 vs uncapped)\n");
    let mut g3_worst: f64 = 0.0;
    for (name, prices) in &corpora {
        let capped = simulate(goat_cfg(), prices, OPEN_AT, 1.0);
        let full = simulate(
            EngineConfig {
                max_arms: 4096,
                ..goat_cfg()
            },
            prices,
            OPEN_AT,
            1.0,
        );
        let d = diff_bps(capped.final_value_norm, full.final_value_norm);
        g3_worst = g3_worst.min(d);
        let _ = writeln!(md, "- {name}: {d:+.1} bps");
    }
    let _ = writeln!(
        md,
        "\n**Worst cap cost {g3_worst:+.1} bps — {}** (budget −10 bps).\n",
        if g3_worst > -10.0 { "PASS" } else { "FAIL" }
    );

    // G4 (release timings)
    let prices = synthetic_corpus(Regime::Chop, 300, 7);
    let mut engine = ExitEngine::new(goat_cfg());
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
    let _ = writeln!(
        md,
        "## G4 latency (release)\n\nMean on_tick **{mean:?}**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **{worst:?}**. Budgets: 1 ms / 1 s — {}.\n",
        if mean.as_millis() < 1 && worst.as_secs() < 1 { "PASS" } else { "FAIL" }
    );

    // auto-numbered results dir
    let next = std::fs::read_dir("benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .split('_')
                        .next()?
                        .parse::<u32>()
                        .ok()
                })
                .max()
                .map_or(1, |m| m + 1)
        })
        .unwrap_or(1);
    let dir = format!("benches/{next:03}_goat");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = format!("{dir}/report.md");
    std::fs::write(&path, &md).expect("write report");
    println!("{md}");
    println!("written: {path}");
}
