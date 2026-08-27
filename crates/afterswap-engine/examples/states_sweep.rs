//! Ruliology frontier: is the complete 3-state space enough?
//!
//! The product's core claim is exhaustive enumeration, currently at
//! 3 states (1,054 behaviorally distinct machines after blake3 dedup).
//! 4-state machines are reachable today only through evolution. This asks
//! the Wolfram question directly: does enumerating the *complete* larger
//! space buy anything, or do simple programs already suffice?
//!
//! Run: cargo run -p afterswap-engine --example states_sweep --release

use std::fmt::Write as _;
use std::time::Instant;

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{
    Regime, load_corpus, simulate, synthetic_corpus, trailing_stop_value_norm, twap_value_norm,
};

fn cfg(states: u8) -> EngineConfig {
    EngineConfig {
        window_len: 12,
        window_stride: 6,
        n_fsm_states: states,
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
    for path in ["data/recorded.jsonl", "data/recorded2.jsonl"] {
        if let Ok(c) = load_corpus(path) {
            if c.len() >= 100 {
                corpora.push((path.rsplit('/').next().unwrap_or("rec").to_string(), c));
            }
        }
    }

    let mut md = String::from("# Ruliology frontier — does a bigger complete space help?\n\n");
    let _ = writeln!(
        md,
        "Objective: mean(edge vs TWAP, edge vs trailing) on real corpora and synthetic regimes, window 12, 10% tranches. Enumeration is exhaustive at each state count (blake3 behavioral dedup).\n"
    );
    let _ = writeln!(md, "| n_states | machines | tournament setup | mean objective | per-corpus |\n|---|---|---|---|---|");

    for states in [2u8, 3, 4] {
        let t0 = Instant::now();
        let probe = simulate(cfg(states), &corpora[0].1, 30, 1.0);
        let setup = t0.elapsed();
        let _ = probe;
        let mut objs = Vec::new();
        for (_, prices) in &corpora {
            let open_at = 30.min(prices.len() / 4);
            let r = simulate(cfg(states), prices, open_at, 1.0);
            let twap = twap_value_norm(prices, open_at, 10, 6);
            let trail = trailing_stop_value_norm(prices, open_at, 50.0);
            objs.push(
                (diff_bps(r.final_value_norm, twap) + diff_bps(r.final_value_norm, trail)) / 2.0,
            );
        }
        let mean = objs.iter().sum::<f64>() / objs.len() as f64;
        let count = afterswap_engine::sim::enumerate_count(states);
        let per: Vec<String> = objs.iter().map(|o| format!("{o:+.0}")).collect();
        let _ = writeln!(
            md,
            "| {states} | {count} | {setup:?} | **{mean:+.1} bps** | {} |",
            per.join(", ")
        );
    }

    let next = std::fs::read_dir("benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().to_string_lossy().split('_').next()?.parse::<u32>().ok())
                .max()
                .map_or(1, |m| m + 1)
        })
        .unwrap_or(1);
    let dir = format!("benches/{next:03}_states");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
