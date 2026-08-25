//! Plackett–Luce strategy ratings from tournament results (roadmap #3).
//!
//! Raw mean payoff ranks poorly with few, noisy windows: one lucky window
//! dominates the average. PL treats every window as a *race* — each
//! strategy's strength reflects how consistently it outranks the others —
//! which is far more sample-efficient at small window counts (the same
//! reason chess uses ratings, not mean score differential).
//!
//! Fitted with Hunter's (2004) MM algorithm. Fixed iteration count and
//! deterministic tie-breaks keep GOAT G1/G6 bit-reproducibility.

/// MM iterations — converges to plenty of precision for ranking use.
const MM_ITERS: usize = 30;

/// Fit PL strengths from `payoffs[item][race]`. Higher = better. Returns
/// uniform strengths when there are no races or a single item.
pub fn plackett_luce(payoffs: &[Vec<f64>]) -> Vec<f64> {
    let n = payoffs.len();
    if n == 0 {
        return Vec::new();
    }
    let races = payoffs[0].len();
    if n == 1 || races == 0 {
        return vec![1.0; n];
    }

    // Per race: items ordered best-first (deterministic tie-break by index).
    let rankings: Vec<Vec<usize>> = (0..races)
        .map(|r| {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| {
                payoffs[b][r]
                    .total_cmp(&payoffs[a][r])
                    .then(a.cmp(&b))
            });
            order
        })
        .collect();

    // Every item is "chosen" once per race except when it finishes last.
    let mut wins = vec![0.0f64; n];
    for ranking in &rankings {
        for &item in &ranking[..n - 1] {
            wins[item] += 1.0;
        }
    }

    let mut gamma = vec![1.0f64; n];
    for _ in 0..MM_ITERS {
        let mut acc = vec![0.0f64; n];
        for ranking in &rankings {
            // Suffix sums of gamma over the still-unplaced set, walked
            // backwards so each stage's denominator is O(1).
            let mut suffix = 0.0f64;
            let mut inv_terms = vec![0.0f64; n];
            for t in (0..n).rev() {
                suffix += gamma[ranking[t]];
                inv_terms[t] = 1.0 / suffix;
            }
            // Stage t's denominator applies to every item still unplaced
            // at t; accumulate via a running prefix of inverse terms.
            let mut prefix_inv = 0.0f64;
            for t in 0..n - 1 {
                prefix_inv += inv_terms[t];
                acc[ranking[t]] += prefix_inv;
            }
            // Last-placed item participated in all n-1 stages too.
            acc[ranking[n - 1]] += prefix_inv;
        }
        let mut norm = 0.0f64;
        for i in 0..n {
            gamma[i] = match acc[i] > 0.0 {
                true => (wins[i].max(1e-9)) / acc[i],
                false => gamma[i],
            };
            norm += gamma[i];
        }
        for g in &mut gamma {
            *g *= n as f64 / norm;
        }
    }
    gamma
}
