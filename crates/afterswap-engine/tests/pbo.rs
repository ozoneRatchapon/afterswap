//! CSCV sanity: a rigged winner must score low PBO, pure noise ~0.5.

use afterswap_engine::pbo::cscv;

#[test]
fn genuine_edge_scores_low_pbo() {
    // Strategy 0 is genuinely better everywhere; the rest are noise.
    let mut rng = fastrand::Rng::with_seed(7);
    let perf: Vec<Vec<f64>> = (0..40)
        .map(|i| {
            (0..80)
                .map(|_| (rng.f64() - 0.5) + if i == 0 { 1.0 } else { 0.0 })
                .collect()
        })
        .collect();
    let r = cscv(&perf, 8).expect("cscv runs");
    assert!(r.pbo < 0.2, "pbo = {} (expected low)", r.pbo);
}

#[test]
fn pure_noise_scores_near_coin_flip() {
    // PBO is itself a random variable: a single seed at this size ranges
    // 0.19–0.86, so the calibration claim is about the mean across seeds.
    let mut vals = Vec::new();
    for seed in 0..8u64 {
        let mut rng = fastrand::Rng::with_seed(seed);
        let perf: Vec<Vec<f64>> = (0..200)
            .map(|_| (0..400).map(|_| rng.f64() - 0.5).collect())
            .collect();
        vals.push(cscv(&perf, 10).expect("cscv runs").pbo);
    }
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    assert!(
        (0.4..=0.6).contains(&mean),
        "mean pbo on noise = {mean} (expected ≈ 0.5)"
    );
}
