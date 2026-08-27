//! Live price source: poll `GET /quote` with a fixed probe notional.

use crate::client::{DflowClient, DflowError};
use crate::snapshot::QuoteSnapshot;
use crate::types::QuoteRequest;

/// Well-known mint addresses.
pub mod mints {
    pub const SOL: &str = "So11111111111111111111111111111111111111112";
    pub const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    /// Volatile tokens are where the out-of-sample evidence says exit
    /// discipline pays (bench 018: +34 bps vs trailing on BONK).
    pub const BONK: &str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";
    pub const WIF: &str = "EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm";
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

    /// BONK→USDC poller with a probe clip sized for its decimals.
    pub fn bonk_usdc(client: DflowClient) -> Self {
        Self::new(
            client,
            QuoteRequest {
                input_mint: mints::BONK.to_string(),
                output_mint: mints::USDC.to_string(),
                amount: 1_000_000_000, // 10k BONK
                slippage_bps: 100,
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
    ///
    /// Prefer [`poll_snapshot`](Self::poll_snapshot) for anything that will be
    /// recorded: this returns the price and discards the depth reading that
    /// arrived with it, which cannot be recovered later.
    pub async fn poll(&self) -> Result<f64, DflowError> {
        Ok(self.poll_snapshot(0).await?.price)
    }

    /// One poll → price **and** the depth reading from the same response.
    ///
    /// `impact_bps` shares `context_slot` with `price` because both come from
    /// one quote, which is what makes the CUPED pairing lag-0 rather than
    /// merely recent. Latency is measured around the request so a row can be
    /// discarded if the quote was already stale on arrival.
    pub async fn poll_snapshot(&self, seq: u64) -> Result<QuoteSnapshot, DflowError> {
        let started = std::time::Instant::now();
        let quote = self.client.quote(&self.request).await?;
        let latency_us = started.elapsed().as_micros() as u64;
        let t_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        QuoteSnapshot::from_quote(seq, t_ms, latency_us, &quote)
            .ok_or_else(|| DflowError::Malformed("no price in quote".to_string()))
    }

    /// Poll with an extra larger-clip probe, producing an executable-depth
    /// spread alongside the same-response impact figure.
    ///
    /// Costs a second request and can straddle slots — the returned snapshot
    /// records both `context_slot`s so
    /// [`freshness`](QuoteSnapshot::freshness) can report the gap rather than
    /// leaving it to be assumed. Use only where the spread is genuinely needed;
    /// `poll_snapshot` is lag-0 for free.
    pub async fn poll_snapshot_probed(
        &self,
        seq: u64,
        probe_amount: u64,
    ) -> Result<QuoteSnapshot, DflowError> {
        let snap = self.poll_snapshot(seq).await?;
        let probe_req = QuoteRequest {
            amount: probe_amount,
            ..self.request.clone()
        };
        match self.client.quote(&probe_req).await {
            Ok(probe) => Ok(snap.with_probe(probe_amount, &probe)),
            // A failed probe must not cost us the lag-0 row we already have.
            Err(e) => {
                log::warn!("depth probe failed (keeping primary snapshot): {e}");
                Ok(snap)
            }
        }
    }
}
