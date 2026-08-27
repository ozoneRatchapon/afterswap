//! Probability of Backtest Overfitting via Combinatorially Symmetric
//! Cross-Validation (Bailey, Borwein, López de Prado, Zhu).
//!
//! The question this answers is the one that matters for an enumerate-and-
//! select system: *when we pick the best of 1,054 machines on some windows,
//! how often does that winner land below median on the windows we did not
//! look at?* If the answer is "about half the time", the selection procedure
//! is extracting noise, no matter how good the winner's in-sample number is.
//!
//! CSCV splits the observation windows into `s` contiguous slices, forms every
//! way of choosing `s/2` of them as in-sample, ranks strategies there, then
//! measures the chosen strategy's out-of-sample rank on the complement. PBO is
//! the fraction of splits where the in-sample winner is below the out-of-sample
//! median. Non-parametric, no Sharpe ratio, no distributional assumption.

/// Result of a CSCV run.
pub struct PboResult {
    /// Probability of backtest overfitting: 0 is ideal, 0.5 is coin-flip
    /// selection, above 0.5 means the procedure is actively anti-predictive.
    pub pbo: f64,
    /// Splits evaluated.
    pub splits: usize,
    /// Mean out-of-sample relative rank of the in-sample winner (0.5 = median).
    pub mean_oos_rank: f64,
    /// Mean in-sample and out-of-sample performance of the selected strategy.
    pub mean_is_perf: f64,
    pub mean_oos_perf: f64,
}

/// `perf[strategy][window]`; `s` must be even and ≤ the window count.
pub fn cscv(perf: &[Vec<f64>], s: usize) -> Option<PboResult> {
    let m = perf.len();
    if m < 2 || s < 4 || !s.is_multiple_of(2) {
        return None;
    }
    let t = perf[0].len();
    if t < s * 2 {
        return None; // each slice needs enough windows to mean anything
    }

    // Per-slice mean performance for every strategy: every split's score is a
    // sum over slice means, so the combinatorics stay cheap.
    let slice_len = t / s;
    let slice_means: Vec<Vec<f64>> = perf
        .iter()
        .map(|row| {
            (0..s)
                .map(|k| {
                    let lo = k * slice_len;
                    let hi = match k == s - 1 {
                        true => t,
                        false => lo + slice_len,
                    };
                    row[lo..hi].iter().sum::<f64>() / (hi - lo) as f64
                })
                .collect()
        })
        .collect();

    let half = s / 2;
    let mut splits = 0usize;
    let mut overfit = 0usize;
    let mut rank_acc = 0.0;
    let mut is_acc = 0.0;
    let mut oos_acc = 0.0;

    // Every combination of `half` slices out of `s`, by bitmask.
    for mask in 0u32..(1u32 << s) {
        if mask.count_ones() as usize != half {
            continue;
        }
        let mean_over = |row: &Vec<f64>, in_sample: bool| -> f64 {
            let mut acc = 0.0;
            let mut n = 0.0;
            for k in 0..s {
                let selected = mask & (1 << k) != 0;
                if selected == in_sample {
                    acc += row[k];
                    n += 1.0;
                }
            }
            acc / n
        };

        // In-sample winner.
        let mut best = (f64::NEG_INFINITY, 0usize);
        for (i, row) in slice_means.iter().enumerate() {
            let v = mean_over(row, true);
            if v > best.0 {
                best = (v, i);
            }
        }
        // Its out-of-sample rank among all strategies.
        let winner_oos = mean_over(&slice_means[best.1], false);
        let worse = slice_means
            .iter()
            .filter(|row| mean_over(row, false) < winner_oos)
            .count();
        let rel_rank = worse as f64 / m as f64;

        splits += 1;
        rank_acc += rel_rank;
        is_acc += best.0;
        oos_acc += winner_oos;
        if rel_rank < 0.5 {
            overfit += 1;
        }
    }

    match splits {
        0 => None,
        _ => Some(PboResult {
            pbo: overfit as f64 / splits as f64,
            splits,
            mean_oos_rank: rank_acc / splits as f64,
            mean_is_perf: is_acc / splits as f64,
            mean_oos_perf: oos_acc / splits as f64,
        }),
    }
}
