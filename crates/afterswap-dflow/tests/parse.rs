//! Wire-type parsing against captured live dev-API responses (2026-08-24).

use afterswap_dflow::{OrderResponse, QuoteResponse};

const QUOTE_JSON: &str = r#"{"inputMint":"So11111111111111111111111111111111111111112","inAmount":"100000000","outputMint":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","outAmount":"9669517","otherAmountThreshold":"9621170","minOutAmount":"9621170","slippageBps":50,"platformFee":null,"outTransferFee":null,"priceImpactPct":"0","routePlan":[{"venue":"Tessera V","marketKey":"FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n","inputMint":"So11111111111111111111111111111111111111112","outputMint":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","inAmount":"100000000","outAmount":"9669517","inputMintDecimals":9,"outputMintDecimals":6}],"contextSlot":441429266,"requestId":"5717c400-dbf2-41b0-800c-27879d3a8e13","forJitoBundle":false}"#;

const ORDER_JSON: &str = r#"{"inputMint":"So11111111111111111111111111111111111111112","inAmount":"100000000","outputMint":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","outAmount":"9670755","otherAmountThreshold":"9622402","minOutAmount":"9622402","slippageBps":50,"platformFee":null,"priceImpactPct":"0","routePlan":[{"venue":"Tessera V","marketKey":"FLckHLGMJy5gEoXWwcE68Nprde1D4araK4TGLw4pQq2n","inputMint":"So11111111111111111111111111111111111111112","outputMint":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","inAmount":"100000000","outAmount":"9670755","inputMintDecimals":9,"outputMintDecimals":6}],"contextSlot":441429269,"executionMode":"sync","transaction":"AQAAAAAA","lastValidBlockHeight":419478367,"prioritizationFeeLamports":10000,"computeUnitLimit":200000,"prioritizationType":{"computeBudget":{"microLamports":50000,"estimatedMicroLamports":50000}}}"#;

#[test]
fn parses_live_quote_and_implies_price() {
    let q: QuoteResponse = serde_json::from_str(QUOTE_JSON).expect("quote parses");
    assert_eq!(q.slippage_bps, 50);
    assert_eq!(q.route_plan.len(), 1);
    assert_eq!(q.route_plan[0].venue, "Tessera V");
    let price = q.price().expect("price");
    // 9.669517 USDC / 0.1 SOL = 96.69517 USDC per SOL.
    assert!((price - 96.69517).abs() < 1e-9, "price = {price}");
}

#[test]
fn parses_live_order_with_transaction() {
    let o: OrderResponse = serde_json::from_str(ORDER_JSON).expect("order parses");
    assert_eq!(o.execution_mode.as_deref(), Some("sync"));
    assert!(!o.transaction.is_empty());
    assert_eq!(o.last_valid_block_height, Some(419478367));
    let price = o.price().expect("price");
    assert!((price - 96.70755).abs() < 1e-9, "price = {price}");
}
