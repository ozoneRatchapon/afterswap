//! Power-analysis checks against the externally computed reference table.

use afterswap_engine::power::{
    Z_POWER_80, Z_POWER_90, Z_POWER_95, mde_from_se, power_at_n, power_at_n_unpaired,
    required_n_paired,
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

/// The full 5x6 reference table, recovered from the round-two document's
/// embedded images after the plain-text export dropped every numeric cell.
///
/// Columns: delta, required N at 80/90/95% power, achieved power at N=534.
/// Paired uses sigma_d = 2.6; unpaired uses sigma_u = 6.6, required-N as a
/// total across both arms and achieved power at 534 *per group* (N = 1,068) —
/// the convention the table states in a cell annotation that the text export
/// lost. Reproducing both columns from one implementation is the cross-check
/// a single surviving copy of the table could not provide.
#[test]
fn reproduces_the_full_reference_table() {
    const PAIRED: [(f64, f64, f64, f64, f64); 5] = [
        (0.10, 5306.0, 7104.0, 8785.0, 0.1420),
        (0.25, 849.0, 1137.0, 1406.0, 0.6033),
        (0.50, 213.0, 285.0, 352.0, 0.9935),
        (1.00, 54.0, 72.0, 88.0, 1.0000),
        (2.00, 14.0, 18.0, 22.0, 1.0000),
    ];
    const UNPAIRED: [(f64, f64, f64, f64, f64); 5] = [
        (0.10, 136_760.0, 183_082.0, 226_420.0, 0.0434),
        (0.25, 21882.0, 29294.0, 36228.0, 0.0900),
        (0.50, 5472.0, 7324.0, 9058.0, 0.2351),
        (1.00, 1368.0, 1832.0, 2266.0, 0.6970),
        (2.00, 342.0, 458.0, 568.0, 0.9986),
    ];

    for (delta, n80, n90, n95, power) in PAIRED {
        for (z, want) in [(Z_POWER_80, n80), (Z_POWER_90, n90), (Z_POWER_95, n95)] {
            let got = required_n_paired(delta, SD_PAIRED, z).ceil();
            let tol = (want * 0.001).max(1.0);
            assert!((got - want).abs() <= tol, "paired n at {delta} bps: {got} vs {want}");
        }
        let got = power_at_n(delta, SD_PAIRED, 534);
        assert!((got - power).abs() < 0.001, "paired power at {delta} bps: {got} vs {power}");
    }

    for (delta, n80, n90, n95, power) in UNPAIRED {
        for (z, want) in [(Z_POWER_80, n80), (Z_POWER_90, n90), (Z_POWER_95, n95)] {
            let got = required_n_unpaired(delta, SD_UNPAIRED, z).ceil();
            let tol = (want * 0.001).max(1.0);
            assert!((got - want).abs() <= tol, "unpaired n at {delta} bps: {got} vs {want}");
        }
        // 534 per group, both arms.
        let got = power_at_n_unpaired(delta, SD_UNPAIRED, 1068);
        assert!((got - power).abs() < 0.001, "unpaired power at {delta} bps: {got} vs {power}");
    }
}

/// The worked example the round-two document spells out under the table.
#[test]
fn worked_example_from_the_reference() {
    // N_paired(0.25 bps, 80%) = 7.84886 * 2.6^2 / 0.25^2 = 849 cycles.
    let paired = required_n_paired(0.25, SD_PAIRED, Z_POWER_80).ceil();
    assert!((paired - 849.0).abs() <= 1.0, "paired = {paired}");
    // N_unpaired,total(0.25 bps, 80%) = 4 * 7.84886 * 6.6^2 / 0.25^2 = 21,882.
    let unpaired = required_n_unpaired(0.25, SD_UNPAIRED, Z_POWER_80).ceil();
    assert!((unpaired - 21882.0).abs() <= 22.0, "unpaired = {unpaired}");
    // "An unpaired soak test of 534 cycles achieves only 9.00% power at
    // delta = 0.25 bps, yielding a 91.0% probability of failing to detect."
    let power = power_at_n_unpaired(0.25, SD_UNPAIRED, 1068);
    assert!((power - 0.0900).abs() < 0.001, "power = {power}");
    assert!((1.0 - power - 0.910).abs() < 0.001, "type II = {}", 1.0 - power);
}

/// The soak report is a shell script, so it cannot call `mde_from_se` — it
/// re-declares the two z constants inline. That duplication is how the script
/// drifted once already: it shipped `1.96 * se * 2` (3.92·SE) as the MDE while
/// this crate defined 2.8016·SE, a 40% overstatement that went unnoticed
/// because nothing compared them. This test is that comparison.
#[test]
fn soak_report_script_agrees_with_power_module() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/soak_report.sh");
    let src = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));

    // The script must carry both constants verbatim, at full precision.
    for (name, value) in [("Z_ALPHA", 1.959_963_985_f64), ("Z_POWER80", Z_POWER_80)] {
        let literal = format!("{value}");
        assert!(
            src.contains(&literal),
            "scripts/soak_report.sh no longer contains {name} = {literal}; \
             it must match crates/afterswap-engine/src/power.rs"
        );
    }

    // And it must apply them as (z_alpha + z_power) * se, not any other shape.
    assert!(
        src.contains("(Z_ALPHA + Z_POWER80) * se"),
        "scripts/soak_report.sh MDE is no longer (Z_ALPHA + Z_POWER80) * se; \
         mde_from_se(se, Z_POWER_80) is the single source of truth"
    );

    // Guard the specific regression: the pre-amendment formula must not return.
    // Comment lines are stripped first — the amendment header quotes the old
    // formula on purpose to explain what was wrong, and that prose is not a
    // reintroduction of it.
    let code_only: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code_only.contains("1.96 * se * 2"),
        "scripts/soak_report.sh reintroduced the 3.92*SE MDE bug"
    );

    // Sanity: the shape the script computes equals what this crate computes.
    let se = 1.137_3;
    let script_mde = (1.959_963_985_f64 + Z_POWER_80) * se;
    assert!((script_mde - mde_from_se(se, Z_POWER_80)).abs() < 1e-12);
}
