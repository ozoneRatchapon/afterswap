//! Which of our magic numbers actually matter?
//!
//! Written after a reviewer asked the obvious question: *what else did you
//! assert without measuring?* An audit found several load-bearing constants
//! chosen by intuition and shipped as defaults — `peak_drop_bps = 30`,
//! `surprise_ratio = 1.2`, `tranche_frac = 0.1`, `window_len`,
//! `refresh_every_windows`. One of them (a sampling rate for signature
//! verification) had already been caught the same way, and it was wrong.
//!
//! This sweeps each constant one at a time across the real corpora and
//! reports the spread. A constant whose sweep is flat is not a tuning knob
//! and should stop being presented as one; a constant with a steep sweep is
//! a claim that needs its own evidence.
//!
//! Run: cargo run -p afterswap-engine --example sensitivity --release

use std::fmt::Write as _;

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{
    Regime, load_corpus, simulate, synthetic_corpus, trailing_stop_value_norm, twap_value_norm,
};

const OPEN_AT: usize = 30;

fn base() -> EngineConfig {
    EngineConfig {
        window_len: 12,
        window_stride: 6,
        n_fsm_states: 3,
        tranche_frac: 0.1,
        max_arms: 24,
        ..EngineConfig::default()
    }
}

fn corpora() -> Vec<(String, Vec<f64>)> {
    let mut out: Vec<(String, Vec<f64>)> = Regime::ALL
        .iter()
        .map(|&r| (r.name().to_string(), synthetic_corpus(r, 300, 42)))
        .collect();
    for p in ["data/recorded.jsonl", "data/recorded2.jsonl"] {
        if let Ok(c) = load_corpus(p)
            && c.len() >= 100 {
                out.push((p.to_string(), c));
            }
    }
    out
}

/// Mean edge vs TWAP and vs trailing over all corpora, for one config.
fn objective(cfg: EngineConfig) -> f64 {
    let corpora = corpora();
    let mut acc = 0.0;
    for (_, prices) in &corpora {
        let open_at = OPEN_AT.min(prices.len() / 4);
        let r = simulate(cfg.clone(), prices, open_at, 1.0);
        let twap = twap_value_norm(prices, open_at, 10, 6);
        let trail = trailing_stop_value_norm(prices, open_at, 50.0);
        let d = |a: f64, b: f64| (a - b) / b * 10_000.0;
        acc += (d(r.final_value_norm, twap) + d(r.final_value_norm, trail)) / 2.0;
    }
    acc / corpora.len() as f64
}

fn main() {
    let mut md = String::from("# Sensitivity of the constants we chose by intuition\n\n");
    let _ = writeln!(
        md,
        "Objective: mean(edge vs TWAP, edge vs trailing) over 6 corpora. One constant varied at a time from the shipped default (**bold**). A flat sweep means the number was never a knob; a steep one means it needs its own evidence.\n"
    );

    let sweeps: Vec<(&str, Vec<(String, EngineConfig)>)> = vec![
        (
            "peak_drop_bps (off-peak input bit)",
            [10.0, 20.0, 30.0, 50.0, 100.0]
                .iter()
                .map(|&v| {
                    (
                        format!("{v:.0}"),
                        EngineConfig { peak_drop_bps: v, ..base() },
                    )
                })
                .collect(),
        ),
        (
            "surprise_ratio (forced re-tournament)",
            [0.0, 0.8, 1.2, 2.0, 4.0]
                .iter()
                .map(|&v| {
                    (
                        format!("{v:.1}"),
                        EngineConfig { surprise_ratio: v, ..base() },
                    )
                })
                .collect(),
        ),
        (
            "tranche_frac (clip size)",
            [0.05, 0.1, 0.2, 0.34, 0.5]
                .iter()
                .map(|&v| {
                    (
                        format!("{:.0}%", v * 100.0),
                        EngineConfig { tranche_frac: v, ..base() },
                    )
                })
                .collect(),
        ),
        (
            "window_len (evaluation window)",
            [6usize, 12, 24, 48]
                .iter()
                .map(|&v| {
                    (
                        format!("{v}"),
                        EngineConfig {
                            window_len: v,
                            window_stride: (v / 2).max(1),
                            ..base()
                        },
                    )
                })
                .collect(),
        ),
        (
            "refresh_every_windows (re-tournament cadence)",
            [1usize, 2, 4, 8]
                .iter()
                .map(|&v| {
                    (
                        format!("{v}"),
                        EngineConfig { refresh_every_windows: v, ..base() },
                    )
                })
                .collect(),
        ),
        (
            "max_arms (population cap)",
            [8usize, 16, 24, 48]
                .iter()
                .map(|&v| (format!("{v}"), EngineConfig { max_arms: v, ..base() }))
                .collect(),
        ),
    ];

    for (name, variants) in sweeps {
        let scored: Vec<(String, f64)> = variants
            .into_iter()
            .map(|(label, cfg)| (label, objective(cfg)))
            .collect();
        let lo = scored.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
        let hi = scored.iter().map(|(_, v)| *v).fold(f64::NEG_INFINITY, f64::max);
        let cells: Vec<String> = scored
            .iter()
            .map(|(l, v)| format!("{l}: {v:+.0}"))
            .collect();
        let _ = writeln!(
            md,
            "- **{name}** — spread **{:.0} bps**\n  - {}",
            hi - lo,
            cells.join(" · ")
        );
    }

    let _ = writeln!(
        md,
        "\nRead the spreads, not the individual numbers: a constant with a large spread is a\nresult that depends on a choice nobody validated, and every such choice in this table\nwas made by intuition before this bench existed.\n"
    );

    let next = std::fs::read_dir("benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().to_string_lossy().split('_').next()?.parse::<u32>().ok())
                .max()
                .map_or(1, |m| m + 1)
        })
        .unwrap_or(1);
    let dir = format!("benches/{next:03}_sensitivity");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
