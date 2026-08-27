//! After correcting for 1,054 simultaneous tests, does any machine survive?
//!
//! CSCV (bench 024) showed the selection generalises while the profit does
//! not. This asks the individual-level question the omnibus tests cannot:
//! is there a single machine whose edge over holding survives familywise
//! error control across the whole enumerated population?
//!
//! Romano–Wolf stepdown, bootstrap-recentred, deterministic per seed;
//! calibrated in `tests/stepdown.rs` (almost no rejections on noise, planted
//! edges recovered).
//!
//! Run: cargo run -p afterswap-engine --example multiplicity --release

use std::fmt::Write as _;

use afterswap_engine::power::{Z_POWER_80, mde_from_se};
use afterswap_engine::sim::{load_corpus, replay_exit};
use afterswap_engine::stepdown::romano_wolf;
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;
const BOOTSTRAPS: usize = 400;
const ALPHA: f64 = 0.05;

fn main() {
    let mut assets: Vec<String> = std::fs::read_dir("data/reference")
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path().to_string_lossy().to_string())
                .filter(|p| p.ends_with("_1m.jsonl"))
                .collect()
        })
        .unwrap_or_default();
    assets.sort();

    let machines = FsmEnumerator::enumerate(3);
    let mut md = String::from("# Does any single machine survive multiplicity correction?\n\n");
    let _ = writeln!(
        md,
        "Romano–Wolf stepdown over all {} enumerated machines against holding, {BOOTSTRAPS} bootstrap resamples, familywise α = {ALPHA}, {WINDOW}-tick windows. `MDE` is the smallest effect the sample could have detected at 80% power — a null result is only meaningful beside it.\n",
        machines.len()
    );
    let _ = writeln!(
        md,
        "| asset | windows | best machine mean | best t | **adjusted p** | survivors | MDE |\n|---|---|---|---|---|---|---|"
    );

    let mut total_survivors = 0usize;
    for path in &assets {
        let Ok(series) = load_corpus(path) else { continue };
        let n = series.len() / WINDOW;
        if n < 16 {
            continue;
        }
        let diffs: Vec<Vec<f64>> = machines
            .iter()
            .map(|m| {
                (0..n)
                    .map(|w| replay_exit(m, &series[w * WINDOW..(w + 1) * WINDOW], TRANCHE, PEAK_DROP_BPS))
                    .collect()
            })
            .collect();
        let verdicts = romano_wolf(&diffs, BOOTSTRAPS, ALPHA, 42);
        let best = verdicts
            .iter()
            .max_by(|a, b| a.t_stat.total_cmp(&b.t_stat))
            .expect("non-empty");
        let survivors = verdicts.iter().filter(|v| v.rejected).count();
        total_survivors += survivors;

        // Spread of the best machine, for the minimum detectable effect.
        let row = &diffs[best.index];
        let mean = row.iter().sum::<f64>() / n as f64;
        let var = row.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        let se = (var / n as f64).sqrt();

        let name = path
            .rsplit('/')
            .next()
            .unwrap_or("?")
            .trim_end_matches("_1m.jsonl")
            .to_uppercase();
        let _ = writeln!(
            md,
            "| {name} | {n} | {:+.1} bps | {:.2} | **{:.3}** | {survivors} | {:.1} bps |",
            best.mean_diff,
            best.t_stat,
            best.p_adjusted,
            mde_from_se(se, Z_POWER_80)
        );
    }

    let _ = writeln!(
        md,
        "\n**Total machines surviving familywise correction across every asset: {total_survivors}.**\n"
    );
    let _ = writeln!(
        md,
        "The enumerate-and-select pipeline is now tested end to end: the search is\nreproducible (G1), the browser and native engines agree byte for byte (G6), the\nselection generalises rather than mining noise (bench 024, PBO 0.05–0.20), and\nthis test asks whether any individual member of the space has an edge that\nsurvives having looked at a thousand candidates. Read the MDE column beside any\nzero — it states what the data could have found had it been there.\n"
    );

    let next = std::fs::read_dir("benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().to_string_lossy().split('_').next()?.parse::<u32>().ok())
                .max()
                .map_or(1, |m| m + 1)
        })
        .unwrap_or(1);
    let dir = format!("benches/{next:03}_multiplicity");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(format!("{dir}/report.md"), &md).expect("write");
    println!("{md}\nwritten: {dir}/report.md");
}
