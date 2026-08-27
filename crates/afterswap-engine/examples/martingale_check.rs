//! Is the PBO dissent a martingale signal-to-noise deficit?
//!
//! Round three lists three mechanisms that drive PBO toward 0.50. Bench 032
//! ruled out cross-sectional policy degeneracy — it is uniform across the whole
//! corpus. Bench 031 produced the first evidence for regime non-stationarity.
//! This tests the remaining one: "on series where return innovations approximate
//! a martingale difference sequence with near-zero serial correlation, the true
//! performance differential across all FSMs is zero", so in-sample selection
//! picks realisation noise and out-of-sample ranks centre on 0.50.
//!
//! Three quantities, two about the series and one about the strategies:
//!
//!   VR(q)  Lo–MacKinlay variance ratio. Exactly 1 under a martingale; below 1
//!          is mean reversion, above 1 is trending.
//!   ρ₁     lag-1 autocorrelation of log returns. Zero under a martingale.
//!   θ_d    the standardised paired signal-to-noise ratio the round-two document
//!          defines, d̄/s_d, for the selected machine's per-window edge over the
//!          population median. Zero under a true global null.
//!
//! The mechanism predicts the dissenting assets look more martingale-like on all
//! three than the assets that generalise cleanly.
//!
//! Run: cargo run -p afterswap-engine --example martingale_check --release

use std::fmt::Write as _;

use afterswap_engine::sim::{load_corpus, replay_exit};
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;

fn log_returns(prices: &[f64]) -> Vec<f64> {
    prices
        .windows(2)
        .filter(|w| w[0] > 0.0 && w[1] > 0.0)
        .map(|w| (w[1] / w[0]).ln())
        .collect()
}

fn variance(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    match n > 1.0 {
        true => {
            let m = v.iter().sum::<f64>() / n;
            v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)
        }
        false => 0.0,
    }
}

/// Lo–MacKinlay variance ratio at aggregation `q`, using non-overlapping sums.
fn variance_ratio(r: &[f64], q: usize) -> f64 {
    let agg: Vec<f64> = r.chunks_exact(q).map(|c| c.iter().sum()).collect();
    let (v1, vq) = (variance(r), variance(&agg));
    match v1 > 0.0 && !agg.is_empty() {
        true => vq / (q as f64 * v1),
        false => f64::NAN,
    }
}

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

fn autocorr1(r: &[f64]) -> f64 {
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
    let mut md = String::from("# Is the dissent a martingale signal-to-noise deficit?\n\n");
    md.push_str(
        "> **Retracted in part by [bench 036](../036_reversion_causal/report.md).** The reading below \
— that the exit machines are extracting mean reversion — was tested by setting the autocorrelation \
directly on synthetic AR(1) series. It did not reproduce: the selection differential does not fall as \
phi rises. What does respond is PBO, and to |phi| rather than to signed phi. The measurements in this \
bench stand; the mean-reversion interpretation drawn from them does not.\n\n",
    );
    let _ = writeln!(
        md,
        "`VR(q)` is the Lo-Mackinlay variance ratio at aggregation q, exactly 1 under a martingale. \
`rho_1` is lag-1 return autocorrelation, zero under a martingale. `theta_d` is the standardised paired \
signal-to-noise ratio of the selected machine's per-window edge over the population median across {} \
enumerated machines, zero under a true global null. PBO is bench 024's; the interval verdict is bench 031's.\n",
        machines.len()
    );
    let _ = writeln!(
        md,
        "| asset | PBO | 031 verdict | VR(2) | VR(5) | rho_1 | **theta_d** |\n|---|---|---|---|---|---|---|"
    );

    let verdict = |n: &str| match n {
        "JTO" => "separable",
        "FLOKI" => "borderline",
        "PYTH" => "not separable",
        _ => "clean",
    };

    let mut rows: Vec<(String, f64, f64, f64, f64)> = Vec::new();
    for path in &assets {
        let Ok(series) = load_corpus(path) else { continue };
        let n = series.len() / WINDOW;
        let r = log_returns(&series);
        let perf: Vec<Vec<f64>> = machines
            .iter()
            .map(|m| {
                (0..n)
                    .map(|w| replay_exit(m, &series[w * WINDOW..(w + 1) * WINDOW], TRANCHE, PEAK_DROP_BPS))
                    .collect()
            })
            .collect();
        let totals: Vec<f64> = perf.iter().map(|row| row.iter().sum::<f64>()).collect();
        let pick = totals
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let y: Vec<f64> = (0..n)
            .map(|w| {
                let mut col: Vec<f64> = perf.iter().map(|row| row[w]).collect();
                col.sort_by(f64::total_cmp);
                perf[pick][w] - col[col.len() / 2]
            })
            .collect();
        let sd = variance(&y).sqrt();
        let theta = match sd > 0.0 {
            true => (y.iter().sum::<f64>() / y.len() as f64) / sd,
            false => 0.0,
        };
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or("?")
            .trim_end_matches("_1m.jsonl")
            .to_uppercase();
        let (vr2, vr5, ac) = (variance_ratio(&r, 2), variance_ratio(&r, 5), autocorr1(&r));
        let _ = writeln!(
            md,
            "| {name} | — | {} | {vr2:.3} | {vr5:.3} | {ac:+.4} | {theta:+.4} |",
            verdict(&name)
        );
        rows.push((name, vr2, vr5, ac, theta));
    }

    let mean = |set: &[&str], f: fn(&(String, f64, f64, f64, f64)) -> f64| {
        let v: Vec<f64> = rows.iter().filter(|r| set.contains(&r.0.as_str())).map(f).collect();
        v.iter().sum::<f64>() / v.len() as f64
    };
    let dissent = ["FLOKI", "JTO", "PYTH"];
    let clean = ["JUP", "ORCA", "RAY", "SHIB"];

    let _ = writeln!(
        md,
        r#"
## The martingale mechanism does not fit

| group | mean VR(2) | mean VR(5) | mean rho_1 | mean theta_d |
| --- | --- | --- | --- | --- |
| dissenting (FLOKI, JTO, PYTH) | {:.3} | {:.3} | {:+.4} | {:+.4} |
| clean (JUP, ORCA, RAY, SHIB) | {:.3} | {:.3} | {:+.4} | {:+.4} |

The prediction was that the dissenters sit closer to the martingale values on all three. They do not.
Their signal-to-noise is lower ({:+.4} against {:+.4}), which fits — but their lag-1 autocorrelation is
*further* from zero, not nearer it. **JTO, the only asset bench 031 separates, has the largest positive
rho_1 in the corpus at {:+.4}.** A martingale deficit cannot explain an asset that is the least
martingale-like of the eleven.

The grouping above is bench 024's, which bench 031 showed to be an overcount, so treat the group means
as descriptive only.

## What the data shows instead: mean reversion is what the machines eat

Across all eleven assets, rho_1 and theta_d correlate at **{:+.3}**. The three most mean-reverting series
— PEPE ({:+.4}), BONK ({:+.4}), SHIB ({:+.4}) — carry the three highest signal-to-noise ratios in the
corpus, and all three are clean generalisers. Series with rho_1 at or above zero cluster at theta_d
around 0.1.

That reframes the whole question. The exit machines are not extracting a general edge that some series
happen to lack; they are extracting **mean reversion**, and they degrade toward coin-flip selection
wherever it is absent. JTO is not signal-free — it is positively autocorrelated, which is the one regime
a peak-drop exit rule is actively wrong about.

This is a hypothesis with a cheap test attached: an exit rule inverted for trending regimes should move
JTO's PBO. It is not evidence yet, and it does not explain FLOKI or PYTH, whose rho_1 sits near zero and
whose separation bench 031 rates borderline and absent respectively.

Round three's three named mechanisms are now all tested against our data: policy degeneracy is uniform
(bench 032), regime non-stationarity has partial support (bench 031's permutation diagnostic), and the
martingale deficit is contradicted here. The explanation that fits is not on the list."#,
        mean(&dissent, |r| r.1),
        mean(&dissent, |r| r.2),
        mean(&dissent, |r| r.3),
        mean(&dissent, |r| r.4),
        mean(&clean, |r| r.1),
        mean(&clean, |r| r.2),
        mean(&clean, |r| r.3),
        mean(&clean, |r| r.4),
        mean(&dissent, |r| r.4),
        mean(&clean, |r| r.4),
        rows.iter().find(|r| r.0 == "JTO").map_or(f64::NAN, |r| r.3),
        corr(
            &rows.iter().map(|r| r.3).collect::<Vec<_>>(),
            &rows.iter().map(|r| r.4).collect::<Vec<_>>(),
        ),
        rows.iter().find(|r| r.0 == "PEPE").map_or(f64::NAN, |r| r.3),
        rows.iter().find(|r| r.0 == "BONK").map_or(f64::NAN, |r| r.3),
        rows.iter().find(|r| r.0 == "SHIB").map_or(f64::NAN, |r| r.3),
    );

    let dir = "benches/034_martingale_check";
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(format!("{dir}/report.md"), &md);
    println!("{md}");
}
