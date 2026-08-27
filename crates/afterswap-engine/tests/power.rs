//! Power-analysis checks against the externally computed reference table.

use afterswap_engine::power::{
    Z_POWER_80, mde_from_se, power_at_n, power_at_n_unpaired, required_n_paired,
    required_n_unpaired,
};

/// Observed spreads in this project: 2.6 bps paired, 6.6 unpaired.
const SD_PAIRED: f64 = 2.6;
const SD_UNPAIRED: f64 = 6.6;

#[test]
fn matches_reference_sample_sizes() {
    // Reference: 1.0 bps at 80% power needs 54 paired / 1,368 unpaired cycles.
    let paired = required_n_paired(1.0, SD_PAIRED, Z_POWER_80).ceil();
    let unpaired = required_n_unpaired(1.0, SD_UNPAIRED, Z_POWER_80).ceil();
    assert!((paired - 54.0).abs() <= 1.0, "paired n = {paired}");
    assert!((unpaired - 1368.0).abs() <= 3.0, "unpaired n = {unpaired}");
}

#[test]
fn our_534_cycle_soak_was_underpowered() {
    // The soak that returned t = 0.37: what could it ever have found?
    let quarter = power_at_n_unpaired(0.25, SD_UNPAIRED, 534);
    let one = power_at_n_unpaired(1.0, SD_UNPAIRED, 534);
    assert!(quarter < 0.15, "power at 0.25 bps = {quarter}");
    assert!(one < 0.60, "power at 1 bps = {one}");
}

#[test]
fn paired_power_matches_the_reference_table() {
    // Reference: 534 paired cycles give 60.33% power at 0.25 bps.
    let p = power_at_n(0.25, SD_PAIRED, 534);
    assert!((p - 0.6033).abs() < 0.01, "paired power = {p}");
}

#[test]
fn mde_from_reported_se_is_usable() {
    // A bench reporting ±5.7 bps could not have detected anything under
    // ~16 bps, which is the sentence that belongs next to its null result.
    let mde = mde_from_se(5.7, Z_POWER_80);
    assert!((15.0..17.0).contains(&mde), "mde = {mde}");
}
