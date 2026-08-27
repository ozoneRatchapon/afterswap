//! Analyse a recorded execution run: decompose margins, apply CUPED, and state
//! whether the sample resolves the target band.
//!
//! Run: cargo run -p afterswap-engine --example cuped_analysis --release -- \
//!          data/execution/run_001.jsonl
//!
//! The target is the +0.10 to +0.35 bps net margin the cost research quotes for
//! liquid CLMM majors. The question is not only "what was the mean" but "could
//! this sample have seen it" — bench 037 is the standing reminder that a null
//! without a power figure is not a result.

use std::fmt::Write as _;

use afterswap_engine::cuped::{cuped, cycles_needed};
use afterswap_engine::execution::{ExecutionCycle, cuped_inputs};
use afterswap_engine::power::Z_POWER_80;

/// Slots of staleness tolerated in the control variate, overridable as the
/// second argument.
///
/// Defaults to 2, not 1. Bench 038's 34.6% is a lag-1 figure, but lag-1 is not
/// reachable through a network round trip: Solana slots are ~400 ms, and an
/// arrival quote, a decision delay and a fill span at least two of them. Dry
/// runs measured a median gap of 2. Setting this to 1 does not buy the lag-1
/// reduction — it empties the sample. At lag 2 the expected reduction is 26.1%.
const DEFAULT_MAX_LAG_SLOTS: u64 = 2;
const BAND: [f64; 2] = [0.10, 0.35];

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/execution/run_001.jsonl".to_string());
    let max_lag: u64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(DEFAULT_MAX_LAG_SLOTS);
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("cannot read {path}");
        return;
    };
    let cycles: Vec<ExecutionCycle> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if cycles.is_empty() {
        eprintln!("no cycles parsed from {path}");
        return;
    }

    let mut md = format!("# Execution run — {path}\n\n");
    let simulated = cycles.iter().filter(|c| c.simulated).count();
    if simulated > 0 {
        let _ = writeln!(
            md,
            "> **{simulated} of {} cycles are `simulated`.** Paper fills restate the submission \
quote, so any margin below is an artefact of the harness rather than a measurement of execution. \
This run verifies plumbing; it cannot answer whether the band is cleared.\n",
            cycles.len()
        );
    }

    // Admissibility funnel. Every exclusion is a decision about what the sample
    // represents, so it is reported rather than applied silently.
    let filled = cycles.iter().filter(|c| c.filled).count();
    let route_stable = cycles.iter().filter(|c| c.filled && c.route_stable()).count();
    let has_control = cycles
        .iter()
        .filter(|c| c.filled && c.route_stable() && c.arrival_impact_bps.is_some())
        .count();
    let admissible = cycles.iter().filter(|c| c.admissible(max_lag)).count();

    let _ = writeln!(md, "## Admissibility\n");
    let _ = writeln!(md, "| stage | cycles | dropped |\n|---|---|---|");
    let _ = writeln!(md, "| recorded | {} | — |", cycles.len());
    let _ = writeln!(md, "| filled | {filled} | {} |", cycles.len() - filled);
    let _ = writeln!(md, "| route stable | {route_stable} | {} |", filled - route_stable);
    let _ = writeln!(md, "| control variate present | {has_control} | {} |", route_stable - has_control);
    let _ = writeln!(
        md,
        "| control lag <= {max_lag} slot | **{admissible}** | {} |",
        has_control - admissible
    );
    let fill_rate = filled as f64 / cycles.len() as f64;
    if fill_rate < 0.95 {
        let _ = writeln!(
            md,
            "\n**Fill rate {:.1}%.** Unfilled cycles are not missing at random — an order fails to \
land when the market moves against it, so excluding them biases the surviving sample toward \
favourable conditions. Treat the margin below as conditional on landing.",
            fill_rate * 100.0
        );
    }

    let (net, impact, control) = cuped_inputs(&cycles, max_lag);
    if net.len() < 4 {
        let _ = writeln!(md, "\nToo few admissible cycles ({}) to estimate.", net.len());
        println!("{md}");
        return;
    }

    // Component decomposition, so the aggregate can be read against the cost
    // table's identity rather than as one opaque number.
    let breakdowns: Vec<_> = cycles
        .iter()
        .filter(|c| c.admissible(max_lag))
        .filter_map(|c| c.breakdown())
        .collect();
    let comp = |f: fn(&afterswap_engine::execution::MarginBreakdown) -> f64| {
        mean(&breakdowns.iter().map(f).collect::<Vec<_>>())
    };
    let _ = writeln!(md, "\n## Margin decomposition (mean bps over {} cycles)\n", breakdowns.len());
    let _ = writeln!(md, "| component | bps |\n|---|---|");
    let _ = writeln!(md, "| realised vs arrival | {:+.4} |", comp(|b| b.realised_bps));
    let _ = writeln!(md, "| of which drift (arrival→submit) | {:+.4} |", comp(|b| b.drift_bps));
    let _ = writeln!(md, "| of which impact (submit→fill) | {:+.4} |", comp(|b| b.impact_bps));
    let _ = writeln!(md, "| priority tip | {:+.4} |", -comp(|b| b.tip_bps));
    let _ = writeln!(md, "| L1 base fee | {:+.4} |", -comp(|b| b.l1_bps));
    let _ = writeln!(md, "| **net margin** | **{:+.4}** |", comp(|b| b.net_margin_bps));

    // CUPED on both targets. The control variate predicts impact; whether that
    // survives into the aggregate is exactly what is in question.
    let _ = writeln!(md, "\n## CUPED\n");
    let _ = writeln!(
        md,
        "| outcome | n | mean | sd raw | sd adj | rho | **reduction** | MDE raw | MDE adj |\n\
|---|---|---|---|---|---|---|---|---|"
    );
    let mut net_result = None;
    for (label, y) in [("net margin", &net), ("impact component", &impact)] {
        let Some(r) = cuped(y, &control) else { continue };
        let _ = writeln!(
            md,
            "| {label} | {} | {:+.4} | {:.4} | {:.4} | {:+.3} | **{:.1}%** | {:.4} | {:.4} |",
            r.n, r.mean_bps, r.sd_raw_bps, r.sd_adj_bps, r.rho, r.reduction * 100.0,
            r.mde_raw_bps, r.mde_adj_bps
        );
        if label == "net margin" {
            net_result = Some(r);
        }
    }

    let Some(r) = net_result else {
        println!("{md}");
        return;
    };

    // Did the control variate behave as bench 038 predicted?
    let _ = writeln!(md, "\n## Verdict\n");
    let predicted = 0.346;
    let _ = writeln!(
        md,
        "Bench 038 predicted a {:.1}% reduction from a lag-1 depth reading, measured on BONK. \
Achieved here: **{:.1}%**. {}",
        predicted * 100.0,
        r.reduction * 100.0,
        match r.reduction >= predicted * 0.75 {
            true => "The control variate transfers.",
            false =>
                "**The control variate does not transfer at the predicted strength.** CLMM depth is \
tick-concentrated rather than reserve-driven, and the BONK figure should not be carried forward to \
this pool. Re-scope the cycle count from the reduction measured here, not from bench 038.",
        }
    );

    let _ = writeln!(md, "\n| target | power at n={} | resolved? |\n|---|---|---|", r.n);
    for delta in [BAND[1], 0.25, BAND[0]] {
        let p = r.power_at(delta);
        let _ = writeln!(
            md,
            "| {delta:+.2} bps | {:.1}% | {} |",
            p * 100.0,
            match r.resolves(delta, 0.80) {
                true => "**yes**",
                false => "no",
            }
        );
    }
    let _ = writeln!(
        md,
        "\nAt the achieved reduction, cycles required: **{}** for +0.35 bps, **{}** for +0.25, \
**{}** for +0.10.",
        cycles_needed(BAND[1], r.sd_raw_bps, r.reduction).ceil(),
        cycles_needed(0.25, r.sd_raw_bps, r.reduction).ceil(),
        cycles_needed(BAND[0], r.sd_raw_bps, r.reduction).ceil()
    );

    let _ = writeln!(
        md,
        "\nRead the MDE column before the mean. A net margin inside the target band but below \
`MDE adj` is not a small positive result — it is an unmeasured one, and the sample cannot \
distinguish it from zero. `Z_POWER_80 = {Z_POWER_80:.5}` throughout."
    );

    println!("{md}");
}
