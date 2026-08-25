//! Learning-state persistence roundtrip.

use afterswap_engine::sim::{Regime, synthetic_corpus};
use afterswap_engine::{EngineConfig, ExitEngine};

#[test]
fn learning_roundtrip_preserves_artifacts() {
    let cfg = EngineConfig {
        window_len: 12,
        window_stride: 6,
        n_fsm_states: 3,
        tranche_frac: 0.1,
        max_arms: 24,
        ..EngineConfig::default()
    };
    let prices = synthetic_corpus(Regime::TrendDown, 200, 9);
    let mut a = ExitEngine::new(cfg.clone());
    for (i, &p) in prices.iter().enumerate() {
        a.on_tick(p);
        if i == 30 {
            a.open_position(1.0);
        }
    }
    let exported = a.export_learning();
    assert!(
        !exported.realized.is_empty(),
        "run should produce realized rewards"
    );

    let mut b = ExitEngine::new(cfg);
    b.import_learning(&exported);
    let re = b.export_learning();
    assert_eq!(re.realized.len(), exported.realized.len());
    assert_eq!(re.evolved.len(), exported.evolved.len());
    assert_eq!(re.generations.len(), exported.generations.len());
}
