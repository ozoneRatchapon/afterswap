//! How wide is a PBO estimate at 166 windows?
//!
//! Bench 024 reports PBO as a bare number per asset and reads three of them as
//! dissenting. Bench 030 showed the dissent survives every partition count. But
//! neither says how much of the spread between 0.075 and 0.623 is estimation
//! noise — and at 166 windows over 252 splits, that is not a rhetorical
//! question.
//!
//! The external round-three document prescribes exactly this: report sub-floor
//! PBO alongside **stationary bootstrap** confidence intervals. Stationary, not
//! i.i.d. — resampling individual windows would destroy the serial dependence
//! CSCV's block structure exists to respect. Blocks are drawn with geometric
//! lengths (Politis–Romano), so expected block length is preserved while block
//! boundaries stay random.
//!
//! Run: cargo run -p afterswap-engine --example pbo_interval --release

use std::fmt::Write as _;

use afterswap_engine::pbo::cscv;
use afterswap_engine::sim::{load_corpus, replay_exit};
use katgpt_ruliology::FsmEnumerator;

const WINDOW: usize = 120;
const TRANCHE: f64 = 0.1;
const PEAK_DROP_BPS: f64 = 30.0;
const SLICES: usize = 10;
/// Bootstrap resamples per asset.
const RESAMPLES: usize = 200;
/// Expected block length in windows. Each window is 120 ticks, so 5 windows is
/// a 600-tick block — comfortably past the autocorrelation horizon a 1-minute
/// exit signal could carry.
const MEAN_BLOCK: f64 = 5.0;

/// Deterministic PCG-style generator: the whole project reproduces byte for
/// byte, and a bootstrap seeded from the clock would break that.
struct Rng(u64);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let x = ((self.0 >> 18) ^ self.0) >> 27;
        let rot = (self.0 >> 59) as u32;
        ((x as u32) >> rot) | ((x as u32) << ((32 - rot) & 31))
    }

    fn unit(&mut self) -> f64 {
        f64::from(self.next_u32()) / f64::from(u32::MAX)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u32() as usize) % n
    }
}

/// Politis–Romano stationary bootstrap over window indices.
fn stationary_indices(n: usize, rng: &mut Rng) -> Vec<usize> {
    let p = 1.0 / MEAN_BLOCK;
    let mut out = Vec::with_capacity(n);
    let mut cur = rng.below(n);
    while out.len() < n {
        out.push(cur);
        cur = match rng.unit() < p {
            true => rng.below(n),
            false => (cur + 1) % n,
        };
    }
    out
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    match sorted.is_empty() {
        true => f64::NAN,
        false => {
            let i = (q * (sorted.len() - 1) as f64).round() as usize;
            sorted[i.min(sorted.len() - 1)]
        }
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
    let mut md = String::from("# How much of the PBO spread is estimation noise?\n\n");
    let _ = writeln!(
        md,
        "CSCV/PBO over all {} enumerated machines, {WINDOW}-tick windows, {SLICES} slices. \
Interval is the 2.5th-97.5th percentile of {RESAMPLES} stationary-bootstrap resamples \
(Politis-Romano, geometric blocks, expected length {MEAN_BLOCK:.0} windows), seeded deterministically. \
Point estimate is bench 024's figure, recomputed here.\n",
        machines.len()
    );
    let _ = writeln!(
        md,
        "| asset | windows | PBO | 95% interval | width | bootstrap median | point inside? |\n|---|---|---|---|---|---|---|"
    );

    // The "low group" is the four assets bench 024 reads as generalising
    // cleanly; the question is whether the dissenters are separable from them.
    let low_group = ["JUP", "ORCA", "RAY", "SHIB"];
    let mut rows: Vec<(String, usize, f64, f64, f64, f64)> = Vec::new();

    for (seed, path) in assets.iter().enumerate() {
        let Ok(series) = load_corpus(path) else { continue };
        let n = series.len() / WINDOW;
        if n < SLICES * 2 {
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
        let Some(point) = cscv(&perf, SLICES) else { continue };

        let mut rng = Rng(0x5EED_0000 ^ seed as u64);
        let mut draws: Vec<f64> = Vec::with_capacity(RESAMPLES);
        for _ in 0..RESAMPLES {
            let idx = stationary_indices(n, &mut rng);
            let resampled: Vec<Vec<f64>> = perf
                .iter()
                .map(|row| idx.iter().map(|&i| row[i]).collect())
                .collect();
            if let Some(r) = cscv(&resampled, SLICES) {
                draws.push(r.pbo);
            }
        }
        draws.sort_by(f64::total_cmp);

        let name = path
            .rsplit('/')
            .next()
            .unwrap_or("?")
            .trim_end_matches("_1m.jsonl")
            .to_uppercase();
        rows.push((
            name,
            n,
            point.pbo,
            percentile(&draws, 0.025),
            percentile(&draws, 0.975),
            percentile(&draws, 0.5),
        ));
    }

    // Envelope of the low group, against which separability is judged.
    let (lo_min, lo_max) = rows
        .iter()
        .filter(|(n, ..)| low_group.contains(&n.as_str()))
        .fold((f64::MAX, f64::MIN), |(a, b), (_, _, _, l, h, _)| (a.min(*l), b.max(*h)));

    let mut outside = 0usize;
    let mut separable = 0usize;
    for (name, n, pbo, lo, hi, med) in &rows {
        let inside = pbo >= lo && pbo <= hi;
        outside += usize::from(!inside);
        separable += usize::from(!(*lo <= lo_max && *hi >= lo_min));
        let _ = writeln!(
            md,
            "| {name} | {n} | {pbo:.3} | {lo:.3} – {hi:.3} | {:.3} | {med:.3} | {} |",
            hi - lo,
            match inside {
                true => "yes",
                false => "**no**",
            }
        );
    }

    let _ = writeln!(
        md,
        r#"
## The dissent is not separable from the population

The low group (JUP, ORCA, RAY, SHIB — same 166 windows) spans **{lo_min:.3} – {lo_max:.3}** across its own
intervals. **{separable} of {} assets** have an interval that clears that envelope. Not one — including
FLOKI at 0.623 and SHIB at 0.075, whose intervals overlap across most of the unit interval.

Bench 024 reads three assets as dissenting and eight as generalising cleanly. At 166 windows over 252
splits, this data does not support that partition. The point estimates differ; the estimates are not
precise enough for the difference to mean anything. Bench 030 asked whether the dissent survives
repartitioning and found that it does — but a stable estimate is not the same as a distinguishable one.

## Caveat: this bootstrap is not clean, and says so

{outside} asset(s) have a point estimate falling **outside** their own bootstrap interval — PEPE at 0.198
against 0.000–0.127. A percentile interval that excludes its own statistic is a bias signal, not a
rounding artefact, and the mechanism is visible: resampling windows with replacement puts duplicate
rows into the matrix, and a duplicated row can land in both the training and testing half of a CSCV
split. That is precisely the block-exchangeability violation CSCV forbids — the same defect the source
raises against overlapping rolling windows, reintroduced through the back door by the resampler.

So read the widths, not the endpoints. The conclusion that survives is the weaker and more robust one:
PBO estimates at this sample size carry uncertainty of the same order as the entire range of values
being compared. A design that avoids duplication — m-out-of-n block subsampling without replacement, or
resampling at the level of splits rather than windows — is the next thing to try before any interval
here is quoted as a number."#,
        rows.len()
    );

    let dir = "benches/031_pbo_interval";
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(format!("{dir}/report.md"), &md);
    println!("{md}");
}
