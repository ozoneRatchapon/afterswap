//! Quote-snapshot schema: unit convention, freshness classification, and the
//! logged payload shape.
//!
//! These pin the two things a CUPED control variate cannot be wrong about — its
//! units and its lag — at the point of capture rather than at analysis time. A
//! unit error found after a month of recording is unrecoverable; a lag error is
//! worse, because it looks like a result.

use afterswap_dflow::snapshot::impact_pct_to_bps;
use afterswap_dflow::{Freshness, QuoteSnapshot, QuoteResponse};

/// Live dev-API response, 2026-08-24. Impact is "0" here, so the impact-bearing
/// tests use a modified copy rather than pretending this one carries depth.
const QUOTE_JSON: &str = r#"{"inputMint":"So11111111111111111111111111111111111111112","inAmount":"100000000","outputMint":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","outAmount":"9669517","otherAmountThreshold":"9621170","minOutAmount":"9621170","slippageBps":50,"platformFee":null,"priceImpactPct":"0","routePlan":[{"venue":"Tessera V","marketKey":"FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n","inputMint":"So11111111111111111111111111111111111111112","outputMint":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","inAmount":"100000000","outAmount":"9669517","inputMintDecimals":9,"outputMintDecimals":6}],"contextSlot":441429266,"requestId":"x"}"#;

fn quote_with(impact: &str, out_amount: &str, slot: u64) -> QuoteResponse {
    let json = QUOTE_JSON
        .replace(r#""priceImpactPct":"0""#, &format!(r#""priceImpactPct":"{impact}""#))
        .replace(r#""outAmount":"9669517""#, &format!(r#""outAmount":"{out_amount}""#))
        .replace(r#""contextSlot":441429266"#, &format!(r#""contextSlot":{slot}"#));
    serde_json::from_str(&json).expect("quote parses")
}

/// The convention `impact_bps` assumes: `priceImpactPct` is a fraction, so
/// 0.0012 is 12 bps. If DFlow ever ships percent-valued impact this fails, and
/// `impact_raw` on every recorded row is what makes recovery possible.
/// Binary floating point cannot hold 0.0012 exactly, so these compare within a
/// tolerance far tighter than any depth reading's precision. The conversion is
/// deliberately left as the naive multiply — rounding here would be a second
/// convention to get wrong.
fn assert_bps(got: Option<f64>, want: f64) {
    let got = got.expect("converted");
    assert!((got - want).abs() < 1e-9, "got {got}, want {want}");
}

#[test]
fn impact_conversion_is_a_fraction_not_a_percent() {
    assert_bps(impact_pct_to_bps("0.0012"), 12.0);
    assert_bps(impact_pct_to_bps("0"), 0.0);
    assert_bps(impact_pct_to_bps("0.01"), 100.0);
    assert_bps(impact_pct_to_bps(" 0.005 "), 50.0);
    assert_eq!(impact_pct_to_bps("not a number"), None);
}

#[test]
fn same_response_impact_is_lag_zero() {
    let q = quote_with("0.0012", "9669517", 441429266);
    let snap = QuoteSnapshot::from_quote(7, 1_700_000_000_000, 4_200, &q).expect("snapshot");
    assert_eq!(snap.seq, 7);
    assert_eq!(snap.context_slot, Some(441429266));
    assert_bps(snap.impact_bps, 12.0);
    assert_eq!(snap.impact_raw.as_deref(), Some("0.0012"));
    assert_eq!(snap.venue.as_deref(), Some("Tessera V"));
    assert_eq!(snap.hops, 1);
    assert_eq!(snap.latency_us, 4_200);
    // The whole point: no second request, so no slot gap can exist.
    assert_eq!(snap.freshness(), Freshness::SameQuote);
    assert!(snap.freshness().is_usable());
}

#[test]
fn raw_impact_is_kept_even_when_unparseable() {
    let q = quote_with("weird", "9669517", 441429266);
    let snap = QuoteSnapshot::from_quote(0, 0, 0, &q).expect("snapshot");
    assert_eq!(snap.impact_bps, None);
    assert_eq!(snap.impact_raw.as_deref(), Some("weird"));
}

#[test]
fn probe_on_the_same_slot_is_usable() {
    let primary = quote_with("0.0012", "9669517", 441429266);
    // Larger clip returns fewer output units per input unit.
    let probe = quote_with("0.004", "9650000", 441429266);
    let snap = QuoteSnapshot::from_quote(1, 0, 0, &primary)
        .expect("snapshot")
        .with_probe(1_000_000_000, &probe);
    let d = snap.probe.as_ref().expect("probe attached");
    assert_eq!(d.probe_amount, 1_000_000_000);
    assert!(d.depth_bps > 0.0, "depth = {}", d.depth_bps);
    assert_eq!(snap.freshness(), Freshness::SameSlot);
    assert!(snap.freshness().is_usable());
}

#[test]
fn probe_from_a_later_slot_reports_its_gap() {
    let primary = quote_with("0.0012", "9669517", 441429266);
    let probe = quote_with("0.004", "9650000", 441429271);
    let snap = QuoteSnapshot::from_quote(1, 0, 0, &primary)
        .expect("snapshot")
        .with_probe(1_000_000_000, &probe);
    assert_eq!(snap.freshness(), Freshness::Stale { gap: 5 });
    // Bench 038: by lag 5 the reduction has fallen from 34.6% to 19.3%.
    assert!(!snap.freshness().is_usable());
}

#[test]
fn control_variate_prefers_the_lag_zero_reading() {
    let primary = quote_with("0.0012", "9669517", 441429266);
    let probe = quote_with("0.004", "9650000", 441429266);
    let snap = QuoteSnapshot::from_quote(1, 0, 0, &primary)
        .expect("snapshot")
        .with_probe(1_000_000_000, &probe);
    assert_bps(snap.control_variate(), 12.0);
}

#[test]
fn filtered_price_is_recorded_beside_the_quoted_one() {
    let q = quote_with("0.0012", "9669517", 441429266);
    let snap = QuoteSnapshot::from_quote(1, 0, 0, &q)
        .expect("snapshot")
        .with_price_used(96.0);
    // The median-of-3 filter can hand the engine a different price than the
    // one this slot quoted. Both must survive to the row.
    assert!((snap.price - 96.69517).abs() < 1e-9);
    assert_eq!(snap.price_used, Some(96.0));
}

#[test]
fn logged_payload_round_trips_and_omits_absent_fields() {
    let q = quote_with("0.0012", "9669517", 441429266);
    let snap = QuoteSnapshot::from_quote(3, 1_700_000_000_000, 900, &q)
        .expect("snapshot")
        .with_price_used(96.69);
    let line = serde_json::to_string(&snap).expect("serialises");
    // Absent optional fields must not appear, so a row's size tracks what was
    // actually captured.
    assert!(!line.contains("probe"), "{line}");
    let back: QuoteSnapshot = serde_json::from_str(&line).expect("round trips");
    assert_eq!(back.seq, 3);
    assert_eq!(back.context_slot, Some(441429266));
    assert_bps(back.impact_bps, 12.0);
    assert_eq!(back.price_used, Some(96.69));
    assert_eq!(back.freshness(), Freshness::SameQuote);
}

/// A row with no slot and no impact cannot support a lag claim, and must say so
/// rather than defaulting to something usable.
#[test]
fn missing_provenance_is_unknown_not_fresh() {
    let json = QUOTE_JSON
        .replace(r#","contextSlot":441429266"#, "")
        .replace(r#""priceImpactPct":"0","#, "");
    let q: QuoteResponse = serde_json::from_str(&json).expect("parses without optionals");
    let snap = QuoteSnapshot::from_quote(0, 0, 0, &q).expect("snapshot");
    assert_eq!(snap.context_slot, None);
    assert_eq!(snap.impact_bps, None);
    assert_eq!(snap.freshness(), Freshness::Unknown);
    assert!(!snap.freshness().is_usable());
    assert_eq!(snap.control_variate(), None);
}
