//! Pre-registration: the hash must bind, and the power gate must refuse.

use afterswap_engine::prereg::PreRegistration;

fn manifest() -> PreRegistration {
    PreRegistration {
        hypothesis: "The enumerated exit machines beat a same-cadence TWAP by at least 1 bps".into(),
        benchmark: "TWAP, 10 slices, stride 6".into(),
        target_effect_bps: 1.0,
        target_power: 0.8,
        alpha: 0.05,
        train_windows: 150,
        test_windows: 100,
        null_control: "de-meaned block bootstrap (random walk)".into(),
        corpora: vec!["data/reference/sol_usdc_1m.jsonl".into()],
    }
}

#[test]
fn moving_the_goalposts_changes_the_hash() {
    let original = manifest();
    let before = original.hash();
    let mut widened = original.clone();
    // The classic post-hoc move: quietly accept a smaller effect.
    widened.target_effect_bps = 0.25;
    assert_ne!(before, widened.hash(), "manifest hash must bind the effect size");
    assert!(!widened.verify_report(&before), "a drifted manifest must not verify");
    assert!(original.verify_report(&before));
}

#[test]
fn power_gate_refuses_an_experiment_that_cannot_answer() {
    // Our real paired spread is ~2.6 bps; 100 windows can see 1 bps.
    assert!(manifest().power_check(2.6).is_ok());

    // The same design against the unpaired spread cannot, and the error must
    // say what it would take instead of failing silently.
    let err = manifest().power_check(6.6).unwrap_err();
    assert!(err.contains("underpowered"), "{err}");
    assert!(err.contains("minimum detectable effect"), "{err}");
}

#[test]
fn hash_is_stable_across_formatting() {
    let a = manifest();
    let b: PreRegistration = serde_json::from_str(&a.canonical()).expect("round-trips");
    assert_eq!(a.hash(), b.hash());
}
