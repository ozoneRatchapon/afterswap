//! Fill confirmation from post-trade balance deltas.
//!
//! **These fixtures are hand-built to the documented `getTransaction`
//! `jsonParsed` shape. They have not been validated against a captured mainnet
//! response.** That distinction matters: the parser's job is to stop a quote
//! being recorded as a fill, and a parser verified only against its author's
//! idea of the format could fail silently in exactly the direction that
//! reintroduces the problem. Capture one real confirmation and pin it here
//! before any run is treated as evidence.

#![cfg(feature = "live")]

use afterswap_dflow::parse_confirmed;

const OWNER: &str = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
const SOL: &str = "So11111111111111111111111111111111111111112";
const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn tx(pre_sol: f64, post_sol: f64, pre_usdc: f64, post_usdc: f64, fee: u64, err: &str) -> serde_json::Value {
    let bal = |mint: &str, amt: f64, idx: u64| {
        serde_json::json!({
            "accountIndex": idx, "mint": mint, "owner": OWNER,
            "uiTokenAmount": {"amount": "0", "decimals": 6, "uiAmount": amt}
        })
    };
    serde_json::json!({
        "slot": 441_429_300u64,
        "transaction": {"message": {"accountKeys": [{"pubkey": OWNER}, {"pubkey": "other"}]}},
        "meta": {
            "fee": fee,
            "err": serde_json::from_str::<serde_json::Value>(err).unwrap(),
            "preBalances": [2_000_000_000u64, 0u64],
            "postBalances": [1_999_990_000u64, 0u64],
            "preTokenBalances": [bal(SOL, pre_sol, 1), bal(USDC, pre_usdc, 2)],
            "postTokenBalances": [bal(SOL, post_sol, 1), bal(USDC, post_usdc, 2)],
        }
    })
}

#[test]
fn effective_price_comes_from_realised_deltas_not_the_quote() {
    // Sold 1.0 SOL, received 96.42 USDC.
    let f = parse_confirmed(&tx(1.0, 0.0, 0.0, 96.42, 10_000, "null"), OWNER).expect("parses");
    assert_eq!(f.slot, 441_429_300);
    assert_eq!(f.fee_lamports, 10_000);
    assert!(f.err.is_none());
    let price = f.effective_price(SOL, USDC).expect("price");
    assert!((price - 96.42).abs() < 1e-9, "price = {price}");
}

#[test]
fn a_reverted_transaction_is_a_recorded_failure_not_a_gap() {
    let f = parse_confirmed(
        &tx(1.0, 1.0, 0.0, 0.0, 10_000, r#"{"InstructionError": [3, "Custom"]}"#),
        OWNER,
    )
    .expect("parses");
    assert!(f.err.is_some(), "revert must survive to the record");
    // Nothing moved, so there is no price to report — and none is invented.
    assert_eq!(f.effective_price(SOL, USDC), None);
}

#[test]
fn wrong_signed_or_zero_legs_yield_no_price() {
    // Both legs received: the parse found the wrong accounts.
    let f = parse_confirmed(&tx(0.0, 1.0, 0.0, 96.0, 10_000, "null"), OWNER).expect("parses");
    assert_eq!(f.effective_price(SOL, USDC), None);
}

#[test]
fn balances_owned_by_others_are_ignored() {
    let mut v = tx(1.0, 0.0, 0.0, 96.42, 10_000, "null");
    v["meta"]["postTokenBalances"][1]["owner"] = serde_json::json!("someone-else");
    let f = parse_confirmed(&v, OWNER).expect("parses");
    // The USDC leg belonged to another owner, so it is not our fill.
    assert_eq!(f.effective_price(SOL, USDC), None);
}

#[test]
fn native_lamport_delta_adds_the_fee_back() {
    let f = parse_confirmed(&tx(1.0, 0.0, 0.0, 96.42, 10_000, "null"), OWNER).expect("parses");
    // post - pre = -10_000, plus fee 10_000 = 0 trade flow in native SOL.
    assert_eq!(f.lamport_delta, 0);
}
