//! Live price source: poll `GET /quote` with a fixed probe notional.

use crate::client::{DflowClient, DflowError};
use crate::types::QuoteRequest;

/// Well-known mint addresses.
pub mod mints {
    pub const SOL: &str = "So11111111111111111111111111111111111111112";
    pub const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
}

/// Polls the quote endpoint for an implied pair price.
pub struct PricePoller {
    client: DflowClient,
    request: QuoteRequest,
}

impl PricePoller {
    /// SOL→USDC poller with a 0.1 SOL probe notional.
    pub fn sol_usdc(client: DflowClient) -> Self {
        Self::new(
            client,
            QuoteRequest {
                input_mint: mints::SOL.to_string(),
                output_mint: mints::USDC.to_string(),
                amount: 100_000_000, // 0.1 SOL
                slippage_bps: 50,
            },
        )
    }

    /// Poller for an arbitrary pair/notional.
    pub fn new(client: DflowClient, request: QuoteRequest) -> Self {
        Self { client, request }
    }

    /// The probe request (mints, notional).
    pub fn request(&self) -> &QuoteRequest {
        &self.request
    }

    /// One poll → implied price (output units per input unit).
    pub async fn poll(&self) -> Result<f64, DflowError> {
        let quote = self.client.quote(&self.request).await?;
        quote
            .price()
            .ok_or_else(|| DflowError::Malformed("no price in quote".to_string()))
    }
}
