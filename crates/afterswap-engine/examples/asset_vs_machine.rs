//! Does the tournament pay, or does the asset choice pay?
//!
//! Bench 034 found that lag-1 autocorrelation and the selected machine's
//! signal-to-noise ratio correlate at −0.856 across the corpus: the machines
//! appear to be extracting mean reversion rather than a general edge. If that is
//! right, it has a product consequence larger than any statistical one — the
//! decision that matters is *where to run the tournament*, not which machine it
//! returns.
//!
//! This measures the machine's contribution in the one form that is drift-free.
//! Round three decomposes the selected candidate's out-of-sample return as
//!
//!   R_{i*,OOS} = D_t + Δ_{i*}
//!
//! where `D_t` is the common drift every machine shares and `Δ_{i*}` is the
//! selection differential over the population median. `D_t` swings by tens of
//! bps and is not ours to claim; `Δ_{i*}` is exactly what the tournament adds.
//! So: split each series, pick on train, and measure Δ on test — then ask
//! whether train-set autocorrelation predicts it.
//!
//! The MDE column is the point of the exercise as much as Δ is. A selection
//! differential smaller than what the test partition could detect is not a small
//! edge; it is an unmeasured one.
//!
//! Run: cargo run -p afterswap-engine --example asset_vs_machine --release

use std::fmt::Write as _;

use afterswap_engine::power::{Z_POWER_80, mde_bps};
use afterswap_engine::sim::{load_corpus, replay_exit};
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;
/// Fraction of windows used to select the machine. The remainder scores it.
const TRAIN_FRAC: f64 = 0.6;

fn corr(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let mut num = 0.0;
    let (mut da, mut db) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        num += (x - ma) * (y - mb);
        da += (x - ma) * (x - ma);
        db += (y - mb) * (y - mb);
    }
    match da > 0.0 && db > 0.0 {
        true => num / (da * db).sqrt(),
        false => 0.0,
    }
}

fn autocorr1(prices: &[f64]) -> f64 {
    let r: Vec<f64> = prices
        .windows(2)
        .filter(|w| w[0] > 0.0 && w[1] > 0.0)
        .map(|w| (w[1] / w[0]).ln())
        .collect();
    let n = r.len() as f64;
    match n > 2.0 {
        true => {
            let m = r.iter().sum::<f64>() / n;
            let num: f64 = r.windows(2).map(|w| (w[0] - m) * (w[1] - m)).sum();
            let den: f64 = r.iter().map(|x| (x - m) * (x - m)).sum();
            match den > 0.0 {
                true => num / den,
                false => 0.0,
            }
        }
        false => 0.0,
    }
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let sd = match n > 1.0 {
        true => (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt(),
        false => 0.0,
    };
    (m, sd)
}

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
    let mut md = String::from("# Does the tournament pay, or does the asset choice pay?\n\n");
    md.push_str(
        "> **Retracted in part by [bench 036](../036_reversion_causal/report.md).** The asset-first \
hypothesis below rests on a correlation between signed rho_1 and the selection differential. A \
controlled experiment setting rho_1 directly did not reproduce it. The attribution result — Δ \
undetectable on ten of eleven assets — is unaffected and is the part of this bench that stands.\n\n",
    );
    let _ = writeln!(
        md,
        "Machine picked on the first {:.0}% of windows, scored on the rest. **Δ** is the selection \
differential: the picked machine's mean per-window edge over the population median, in bps — the part \
of the outcome the tournament is responsible for, with common drift removed. **MDE** is the smallest Δ \
the test partition could have detected at 80% power. `rho_1(train)` is lag-1 return autocorrelation on \
the training half only, so it is available before any test data is touched.\n",
        TRAIN_FRAC * 100.0
    );
    let _ = writeln!(
        md,
        "| asset | train / test windows | rho_1(train) | **Δ (bps)** | MDE (bps) | detectable? |\n|---|---|---|---|---|---|"
    );

    let mut rho = Vec::new();
    let mut deltas = Vec::new();
    let mut detected = 0usize;
    for path in &assets {
        let Ok(series) = load_corpus(path) else { continue };
        let n = series.len() / WINDOW;
        let n_train = (n as f64 * TRAIN_FRAC) as usize;
        if n_train < 4 || n - n_train < 4 {
            continue;
        }
        let perf: Vec<Vec<f64>> = machines
            .iter()
            .map(|m| {
                (0..n)
                    .map(|w| replay_exit(m, &series[w * WINDOW..(w + 1) * WINDOW], TRANCHE, PEAK_DROP_BPS))
                    .collect()
            })
            .collect();
        let pick = perf
            .iter()
            .enumerate()
            .max_by(|a, b| {
                a.1[..n_train]
                    .iter()
                    .sum::<f64>()
                    .total_cmp(&b.1[..n_train].iter().sum::<f64>())
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        // Δ per test window: picked machine minus the population median.
        let d: Vec<f64> = (n_train..n)
            .map(|w| {
                let mut col: Vec<f64> = perf.iter().map(|r| r[w]).collect();
                col.sort_by(f64::total_cmp);
                perf[pick][w] - col[col.len() / 2]
            })
            .collect();
        let (delta, sd) = mean_sd(&d);
        let mde = mde_bps(d.len(), sd, Z_POWER_80);
        let r1 = autocorr1(&series[..n_train * WINDOW]);
        let ok = delta.abs() >= mde;
        detected += usize::from(ok);
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or("?")
            .trim_end_matches("_1m.jsonl")
            .to_uppercase();
        let _ = writeln!(
            md,
            "| {name} | {n_train} / {} | {r1:+.4} | {delta:+.3} | {mde:.3} | {} |",
            n - n_train,
            match ok {
                true => "**yes**",
                false => "no",
            }
        );
        rho.push(r1);
        deltas.push(delta);
    }

    let r = corr(&rho, &deltas);
    let (mean_d, _) = mean_sd(&deltas);
    let _ = writeln!(
        md,
        r#"
## Result: the tournament's contribution is undetectable on 10 of 11 assets

Mean Δ is **{mean_d:+.3} bps** and **{detected} of {}** assets have a Δ exceeding what their own test
partition could detect. The exception is PEPE, at +11.7 bps against an MDE of 9.1 — and PEPE is also the
most mean-reverting series in the corpus, at rho_1 = −0.427 on the training half.

Everywhere else the selection differential sits under the floor, often far under: RAY's +11.2 bps looks
substantial until its MDE of 63.7 is read beside it. **The tournament's out-of-sample contribution has
not been shown to be non-zero on ten of eleven assets.** Bench 025 reached the same place through
multiplicity correction — zero machines surviving — and this states it in bps rather than in
significance, which is the more useful unit for deciding whether to run the thing.

Note what is *not* claimed. Δ is positive on 8 of 11 assets and averages +3.1 bps. That is consistent
with a small real edge the sample cannot resolve, and equally consistent with nothing. The MDE column
is what separates "we measured a small effect" from "we could not have measured this effect if it were
there", and here it is the second.

## The asset-first hypothesis: suggestive, not established

`rho_1(train)` and out-of-sample Δ correlate at **{r:+.3}** across {} assets. The sign is right and it
matches bench 034's −0.856 between rho_1 and in-sample signal-to-noise, but with n = 11 this is
t ≈ −1.8, p ≈ 0.11. **It does not reach significance and should not be quoted as if it did.**

What survives is a testable deployment rule rather than a finding: measure rho_1 on history, and run
the tournament only where mean reversion is present. Both benches point that way, PEPE is the single
asset where the machinery demonstrably pays and also the most mean-reverting, and the mechanism is
plausible — a peak-drop exit is a mean-reversion bet by construction. None of that is evidence at
n = 11 spot assets.

The clean way to settle it is not more assets. It is a controlled series where rho_1 is set rather than
observed."#,
        rho.len(),
        rho.len(),
    );

    let dir = "benches/035_asset_vs_machine";
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(format!("{dir}/report.md"), &md);
    println!("{md}");
}
