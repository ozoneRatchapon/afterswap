//! Multi-venue quote capture and the boundary into `afterswap-rail`.
//!
//! The rail's schema rule is raw integer amounts, and this module is where
//! that rule meets the network: a [`CapturedQuote`] keeps the venue's raw
//! response body and its *string* amounts, so no `f64` exists anywhere on the
//! path from HTTP response to audit record. (`QuoteSnapshot`'s implied price
//! is for the engine and the CUPED work; the rail never sees it.)
//!
//! Evidence strength is decided by what actually arrived, not by which venue
//! we asked: a DFlow response *with* RFC 9421 headers becomes
//! `ProviderSigned`; without them it degrades to `Observed` and the record
//! says so. Fabricating provider evidence from a venue that did not sign
//! would be the one lie this whole rail exists to make impossible.

use std::time::Instant;

use afterswap_rail::{QuoteEvidence, VenueQuote};

use crate::client::{DflowClient, DflowError};
use crate::types::QuoteRequest;

/// The RFC 9421 headers that constitute provider evidence when present.
const SIG_HEADERS: [&str; 3] = ["signature", "signature-input", "content-digest"];

/// One venue's quote with everything the rail needs, amounts as raw strings.
#[derive(Debug, Clone)]
pub struct CapturedQuote {
    pub venue: &'static str,
    pub context_slot: Option<u64>,
    pub latency_us: u64,
    pub in_amount: String,
    pub out_amount: String,
    pub route: String,
    /// Raw response body, exactly as received.
    pub body: Vec<u8>,
    /// RFC 9421 headers when the venue sent them, joined `name: value\n`.
    pub sig_headers: Option<String>,
}

impl CapturedQuote {
    /// Cross the boundary into the rail's schema.
    pub fn into_venue_quote(self, req: &QuoteRequest) -> VenueQuote {
        let evidence = match &self.sig_headers {
            Some(h) => QuoteEvidence::provider_signed(h.clone(), &self.body),
            None => QuoteEvidence::observed(&self.body),
        };
        VenueQuote {
            venue: self.venue.to_string(),
            context_slot: self.context_slot,
            latency_us: self.latency_us,
            in_mint: req.input_mint.clone(),
            out_mint: req.output_mint.clone(),
            in_amount: self.in_amount,
            out_amount: self.out_amount,
            route: self.route,
            evidence,
        }
    }
}

fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(str::to_string)
}

/// Capture a DFlow quote with its raw body and any signature headers.
///
/// Sends `x-sign-request: true`; whether the response is `ProviderSigned`
/// depends entirely on whether the signature headers come back.
pub async fn capture_dflow(
    client: &DflowClient,
    req: &QuoteRequest,
) -> Result<CapturedQuote, DflowError> {
    let started = Instant::now();
    let (headers, body) = client.quote_raw(req).await?;
    let latency_us = started.elapsed().as_micros() as u64;

    let v: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| DflowError::Malformed(format!("dflow body: {e}")))?;
    let (Some(in_amount), Some(out_amount)) = (str_field(&v, "inAmount"), str_field(&v, "outAmount"))
    else {
        return Err(DflowError::Malformed("dflow quote missing amounts".into()));
    };
    let hops = v.get("routePlan").and_then(|r| r.as_array()).map_or(0, Vec::len);
    let venue0 = v
        .pointer("/routePlan/0/venue")
        .and_then(|x| x.as_str())
        .unwrap_or("?");

    let mut sig = String::new();
    for name in SIG_HEADERS {
        if let Some(val) = headers.iter().find(|(n, _)| n == name) {
            sig.push_str(&format!("{name}: {}\n", val.1));
        }
    }
    Ok(CapturedQuote {
        venue: "dflow",
        context_slot: v.get("contextSlot").and_then(|x| x.as_u64()),
        latency_us,
        in_amount,
        out_amount,
        route: format!("{venue0}|{hops}"),
        body,
        // Partial header sets do not count: evidence is all-or-nothing, and
        // a signature without its input spec is unverifiable decoration.
        sig_headers: (sig.matches('\n').count() >= 2).then_some(sig),
    })
}

/// Parse a Jupiter quote body into a capture. Split from the fetch so the
/// pinned fixture in `tests/` exercises the exact production path.
pub fn parse_jupiter(body: Vec<u8>, latency_us: u64) -> Result<CapturedQuote, DflowError> {
    let v: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| DflowError::Malformed(format!("jupiter body: {e}")))?;
    let (Some(in_amount), Some(out_amount)) = (str_field(&v, "inAmount"), str_field(&v, "outAmount"))
    else {
        return Err(DflowError::Malformed("jupiter quote missing amounts".into()));
    };
    let plan = v.get("routePlan").and_then(|r| r.as_array());
    let hops = plan.map_or(0, Vec::len);
    let label = v
        .pointer("/routePlan/0/swapInfo/label")
        .and_then(|x| x.as_str())
        .unwrap_or("?");
    Ok(CapturedQuote {
        venue: "jupiter",
        context_slot: v.get("contextSlot").and_then(|x| x.as_u64()),
        latency_us,
        in_amount,
        out_amount,
        route: format!("{label}|{hops}"),
        body,
        // Verified 2026-08-28: Jupiter's quote API signs nothing.
        sig_headers: None,
    })
}

/// Fetch a Jupiter shadow quote (keyless lite endpoint).
pub async fn capture_jupiter(
    http: &reqwest::Client,
    req: &QuoteRequest,
) -> Result<CapturedQuote, DflowError> {
    let url = format!(
        "https://lite-api.jup.ag/swap/v1/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
        req.input_mint, req.output_mint, req.amount, req.slippage_bps
    );
    let started = Instant::now();
    let resp = http.get(url).send().await?;
    let status = resp.status();
    let body = resp.bytes().await?.to_vec();
    let latency_us = started.elapsed().as_micros() as u64;
    if !status.is_success() {
        return Err(DflowError::Api {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&body).into_owned(),
        });
    }
    parse_jupiter(body, latency_us)
}
