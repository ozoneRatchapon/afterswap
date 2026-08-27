//! Romano–Wolf stepdown multiple testing (Romano & Wolf, 2005).
//!
//! CSCV told us the *selection* generalises; this asks the individual
//! question: after correcting for the fact that we tested 1,054 machines, does
//! **any single one** have a genuine edge over the benchmark?
//!
//! Method: studentise each machine's mean paired difference, bootstrap the
//! recentred null by resampling windows, and step down through the ordered
//! statistics comparing each against the bootstrap distribution of the maximum
//! over the still-unrejected set. This controls the familywise error rate
//! without the conservatism of a Bonferroni correction, and it names the
//! survivors rather than only answering "any?" the way an omnibus test does.
//!
//! Deterministic for a fixed seed, so it stays inside the G1 reproducibility
//! gate.

/// One machine's verdict.
pub struct Verdict {
    pub index: usize,
    pub mean_diff: f64,
    pub t_stat: f64,
    /// Familywise-error-adjusted p-value.
    pub p_adjusted: f64,
    pub rejected: bool,
}

/// `diffs[strategy][window]` = paired difference versus the benchmark on that
/// window (positive = strategy ahead). `alpha` is the familywise level.
pub fn romano_wolf(diffs: &[Vec<f64>], bootstraps: usize, alpha: f64, seed: u64) -> Vec<Verdict> {
    let m = diffs.len();
    if m == 0 || diffs[0].len() < 8 {
        return Vec::new();
    }
    let n = diffs[0].len();

    let stats: Vec<(f64, f64)> = diffs
        .iter()
        .map(|row| {
            let mean = row.iter().sum::<f64>() / n as f64;
            let var = row.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
            let se = (var / n as f64).sqrt().max(1e-12);
            (mean, mean / se)
        })
        .collect();

    // Bootstrap the recentred null: resample windows, subtract the observed
    // mean so the resampled series has no edge by construction.
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut boot_t: Vec<Vec<f64>> = Vec::with_capacity(bootstraps);
    for _ in 0..bootstraps {
        let idx: Vec<usize> = (0..n).map(|_| rng.usize(..n)).collect();
        let row_t: Vec<f64> = diffs
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mean = idx.iter().map(|&j| row[j]).sum::<f64>() / n as f64 - stats[i].0;
                let var = idx
                    .iter()
                    .map(|&j| (row[j] - stats[i].0 - mean).powi(2))
                    .sum::<f64>()
                    / (n - 1) as f64;
                let se = (var / n as f64).sqrt().max(1e-12);
                mean / se
            })
            .collect();
        boot_t.push(row_t);
    }

    // Order by observed statistic, then step down.
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&a, &b| stats[b].1.total_cmp(&stats[a].1));

    let mut verdicts: Vec<Verdict> = (0..m)
        .map(|i| Verdict {
            index: i,
            mean_diff: stats[i].0,
            t_stat: stats[i].1,
            p_adjusted: 1.0,
            rejected: false,
        })
        .collect();

    let mut remaining: Vec<usize> = order.clone();
    let mut step = 0usize;
    while step < order.len() {
        let candidate = order[step];
        // Bootstrap distribution of the maximum statistic over the machines
        // not yet rejected — the stepdown that buys back power.
        let mut maxima: Vec<f64> = boot_t
            .iter()
            .map(|row| {
                remaining
                    .iter()
                    .map(|&i| row[i])
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .collect();
        maxima.sort_by(f64::total_cmp);
        let exceed = maxima
            .iter()
            .filter(|&&v| v >= stats[candidate].1)
            .count();
        let p_adj = (exceed as f64 + 1.0) / (bootstraps as f64 + 1.0);
        // p-values are monotone down the stepdown path.
        let p_adj = match step {
            0 => p_adj,
            _ => p_adj.max(verdicts[order[step - 1]].p_adjusted),
        };
        verdicts[candidate].p_adjusted = p_adj;
        if p_adj > alpha {
            // Once one fails, everything below it fails too.
            for &i in &order[step..] {
                verdicts[i].p_adjusted = verdicts[i].p_adjusted.max(p_adj);
            }
            break;
        }
        verdicts[candidate].rejected = true;
        remaining.retain(|&i| i != candidate);
        step += 1;
    }
    verdicts
}
