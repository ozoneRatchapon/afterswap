//! DFlow Trading API wire types (shapes verified against live dev API
//! responses, 2026-08-24).

use serde::Deserialize;

/// Parameters shared by `GET /quote` and `GET /order`.
#[derive(Debug, Clone)]
pub struct QuoteRequest {
    pub input_mint: String,
    pub output_mint: String,
    /// Amount in the input mint's smallest units.
    pub amount: u64,
    pub slippage_bps: u16,
}

/// One hop of the route plan.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlanStep {
    pub venue: String,
    pub market_key: String,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub input_mint_decimals: u8,
    pub output_mint_decimals: u8,
}

/// `GET /quote` response (imperative quote, no transaction).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    pub input_mint: String,
    pub in_amount: String,
    pub output_mint: String,
    pub out_amount: String,
    pub min_out_amount: String,
    pub slippage_bps: u32,
    #[serde(default)]
    pub price_impact_pct: Option<String>,
    #[serde(default)]
    pub route_plan: Vec<RoutePlanStep>,
    #[serde(default)]
    pub context_slot: Option<u64>,
    #[serde(default)]
    pub request_id: Option<String>,
}

/// `GET /order` response — quote plus a ready-to-sign base64 transaction.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub input_mint: String,
    pub in_amount: String,
    pub output_mint: String,
    pub out_amount: String,
    pub min_out_amount: String,
    pub slippage_bps: u32,
    #[serde(default)]
    pub route_plan: Vec<RoutePlanStep>,
    #[serde(default)]
    pub context_slot: Option<u64>,
    #[serde(default)]
    pub execution_mode: Option<String>,
    /// Base64-encoded unsigned `VersionedTransaction`.
    pub transaction: String,
    #[serde(default)]
    pub last_valid_block_height: Option<u64>,
    #[serde(default)]
    pub prioritization_fee_lamports: Option<u64>,
    #[serde(default)]
    pub compute_unit_limit: Option<u64>,
}

/// Price implied by an in/out amount pair, in output units per input unit
/// (e.g. USDC per SOL). Decimals come from the route plan.
fn implied_price(
    in_amount: &str,
    out_amount: &str,
    route_plan: &[RoutePlanStep],
) -> Option<f64> {
    let in_raw: f64 = in_amount.parse().ok()?;
    let out_raw: f64 = out_amount.parse().ok()?;
    let first = route_plan.first()?;
    let last = route_plan.last()?;
    let in_scale = 10f64.powi(i32::from(first.input_mint_decimals));
    let out_scale = 10f64.powi(i32::from(last.output_mint_decimals));
    match in_raw > 0.0 {
        true => Some((out_raw / out_scale) / (in_raw / in_scale)),
        false => None,
    }
}

impl QuoteResponse {
    /// USDC-per-SOL-style price implied by this quote.
    pub fn price(&self) -> Option<f64> {
        implied_price(&self.in_amount, &self.out_amount, &self.route_plan)
    }
}

impl OrderResponse {
    /// Price implied by this order's quote leg.
    pub fn price(&self) -> Option<f64> {
        implied_price(&self.in_amount, &self.out_amount, &self.route_plan)
    }
}
