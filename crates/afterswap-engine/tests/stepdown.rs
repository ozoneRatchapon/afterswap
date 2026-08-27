//! Romano–Wolf calibration: false positives near α on noise, power on signal.

use afterswap_engine::stepdown::romano_wolf;

#[test]
fn noise_yields_almost_no_rejections() {
    // 200 machines with no edge: familywise control means we should almost
    // never reject any of them, not 5% of them.
    let mut rejected_runs = 0;
    for seed in 0..10u64 {
        let mut rng = fastrand::Rng::with_seed(seed);
        let diffs: Vec<Vec<f64>> = (0..200)
            .map(|_| (0..120).map(|_| rng.f64() - 0.5).collect())
            .collect();
        let v = romano_wolf(&diffs, 300, 0.05, seed);
        if v.iter().any(|x| x.rejected) {
            rejected_runs += 1;
        }
    }
    assert!(
        rejected_runs <= 2,
        "{rejected_runs}/10 noise runs produced a rejection (familywise α = 0.05)"
    );
}

#[test]
fn a_real_edge_is_found() {
    let mut rng = fastrand::Rng::with_seed(3);
    let diffs: Vec<Vec<f64>> = (0..200)
        .map(|i| {
            (0..120)
                .map(|_| (rng.f64() - 0.5) + if i == 7 { 0.6 } else { 0.0 })
                .collect()
        })
        .collect();
    let v = romano_wolf(&diffs, 300, 0.05, 5);
    assert!(v[7].rejected, "planted edge missed: p = {}", v[7].p_adjusted);
}
