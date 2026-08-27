//! Venue capture → rail boundary, against the pinned live Jupiter response.

use afterswap_dflow::{QuoteRequest, parse_jupiter};
use afterswap_rail::QuoteEvidence;

/// Live Jupiter quote, captured 2026-08-28 (SOL→USDC, 1 SOL). The shape the
/// production parser must keep reading — and the venue that signs nothing.
const JUPITER_BODY: &[u8] = include_bytes!("fixtures/jupiter_sol_usdc_quote.json");

fn req() -> QuoteRequest {
    QuoteRequest {
        input_mint: "So11111111111111111111111111111111111111112".into(),
        output_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
        amount: 1_000_000_000,
        slippage_bps: 50,
    }
}

#[test]
fn jupiter_fixture_parses_with_raw_amounts_and_slot() {
    let cap = parse_jupiter(JUPITER_BODY.to_vec(), 6_100).expect("parses");
    assert_eq!(cap.venue, "jupiter");
    assert_eq!(cap.in_amount, "1000000000");
    assert_eq!(cap.out_amount, "108036854");
    assert_eq!(cap.context_slot, Some(442_154_067));
    assert_eq!(cap.route, "Quantum|1");
    assert!(cap.sig_headers.is_none(), "Jupiter signs nothing");
}

#[test]
fn conversion_produces_observed_evidence_with_a_true_digest() {
    use sha2::Digest as _;
    let cap = parse_jupiter(JUPITER_BODY.to_vec(), 6_100).expect("parses");
    let vq = cap.into_venue_quote(&req());
    assert_eq!(vq.venue, "jupiter");
    assert_eq!(vq.in_amount, "1000000000");
    match &vq.evidence {
        QuoteEvidence::Observed { body_sha256, .. } => {
            let expect: [u8; 32] = sha2::Sha256::digest(JUPITER_BODY).into();
            assert_eq!(*body_sha256, expect, "digest covers the exact body");
        }
        other => panic!("expected Observed, got {other:?}"),
    }
}

#[test]
fn a_body_without_amounts_is_refused_not_defaulted() {
    assert!(parse_jupiter(br#"{"contextSlot": 1}"#.to_vec(), 0).is_err());
}
