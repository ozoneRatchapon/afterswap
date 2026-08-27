//! Fill confirmation from post-trade balance deltas.
//!
//! **These fixtures are hand-built to the documented `getTransaction`
//! `jsonParsed` shape. They have not been validated against a captured mainnet
//! response.** That distinction matters: the parser's job is to stop a quote
//! being recorded as a fill, and a parser verified only against its author's
//! idea of the format could fail silently in exactly the direction that
//! reintroduces the problem. Capture one real confirmation and pin it here
//! before any run is treated as evidence.



use afterswap_dflow::parse_confirmed;

const OWNER: &str = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
const SOL: &str = "So11111111111111111111111111111111111111112";
const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

/// Balance entry with `amount` and `uiAmount` consistent, as mainnet emits
/// them. An earlier version of this fixture carried the value only in
/// `uiAmount` and left `amount` at "0" — a shape the chain never produces, and
/// one that hid which field the parser was actually reading.
fn tx(pre_sol: f64, post_sol: f64, pre_usdc: f64, post_usdc: f64, fee: u64, err: &str) -> serde_json::Value {
    let bal = |mint: &str, amt: f64, idx: u64| {
        let decimals = match mint == SOL {
            true => 9u32,
            false => 6,
        };
        let raw = (amt * 10f64.powi(decimals as i32)).round() as u64;
        serde_json::json!({
            "accountIndex": idx, "mint": mint, "owner": OWNER,
            "uiTokenAmount": {
                "amount": raw.to_string(),
                "decimals": decimals,
                "uiAmount": amt,
            }
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

// ---------------------------------------------------------------------------
// Real mainnet transaction. Everything above this line is hand-built; this is
// not.
// ---------------------------------------------------------------------------

/// Orca Whirlpool swap, signature
/// `3zjjH2zPyhxGq5Fp1HYxsntKnpmrZfdwaRZoiKk5zHbFSbKgz8nu2nVDWyYsU8dTN5iuE3RW4BDEHtgto6CZesh3`,
/// slot 442142115. Trimmed to the fields the parser reads, with their shapes
/// exactly as mainnet emitted them.
///
/// Fetched with `cargo run -p afterswap-dflow --example fetch_tx --release --
/// <SIG> --save <PATH>`. Two things this caught that the fixtures above did
/// not: `uiTokenAmount.amount` is the authoritative field and `uiAmount` can be
/// `null`, and a swap's SOL leg may arrive as a wrapped-SOL token account or as
/// a native lamport delta.
const REAL_SWAP: &str = include_str!("fixtures/orca_whirlpool_sol_cbbtc.json");

const CBBTC: &str = "cbbtcf3aa214zXHbiAZQwf4122FBYbraNdFqgw4iMij";
const REAL_OWNER: &str = "MfDuWeqSHEqTFVYZ7LoexgAK9dxk7cy4DFJWjWMGVWa";

#[test]
fn parses_a_real_orca_whirlpool_swap() {
    let v: serde_json::Value = serde_json::from_str(REAL_SWAP).expect("fixture parses");
    let f = parse_confirmed(&v, REAL_OWNER).expect("parses");

    assert_eq!(f.slot, 442_142_115);
    assert_eq!(f.fee_lamports, 5_000);
    assert!(f.err.is_none(), "transaction succeeded on chain");

    // Sold 16.924376031 wSOL, received 0.022724880 cbBTC.
    let sol = f.delta_for(SOL);
    let btc = f.delta_for(CBBTC);
    assert!((sol + 16.924_376_031).abs() < 1e-9, "SOL delta = {sol}");
    assert!((btc - 0.022_724_880).abs() < 1e-9, "cbBTC delta = {btc}");

    let price = f.effective_price(SOL, CBBTC).expect("two-sided swap");
    assert!((price - 0.001_342_731).abs() < 1e-9, "price = {price}");

    // The check that matters is economic, not arithmetic: this price implies
    // ~745 SOL per BTC. A parser reading the wrong accounts would still produce
    // *a* number, and only the magnitude reveals it.
    let sol_per_btc = 1.0 / price;
    assert!(
        (600.0..900.0).contains(&sol_per_btc),
        "implied {sol_per_btc:.1} SOL per BTC is not a market rate — the parse found the wrong legs"
    );
}

/// The fee payer is not always a party to the swap. Arbitrage transactions
/// start and end in one mint, so there is no two-sided leg to find, and a live
/// run must record that as unfilled rather than invent a price.
#[test]
fn a_round_trip_has_no_two_sided_price() {
    let v: serde_json::Value = serde_json::from_str(REAL_SWAP).expect("fixture parses");
    let f = parse_confirmed(&v, "SomeoneWhoIsNotInThisTransaction11111111111").expect("parses");
    assert!(f.deltas.is_empty(), "no balances belong to a stranger");
    assert_eq!(f.effective_price(SOL, CBBTC), None);
}
