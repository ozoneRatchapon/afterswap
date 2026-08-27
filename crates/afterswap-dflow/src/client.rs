//! Thin async REST client for the DFlow Trading API.

use serde::de::DeserializeOwned;

use crate::types::{OrderResponse, QuoteRequest, QuoteResponse};

/// Developer endpoint — works without an API key (lower rate limits).
pub const DEV_BASE: &str = "https://dev-quote-api.dflow.net";

/// Client errors.
#[derive(Debug, thiserror::Error)]
pub enum DflowError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api status {status}: {body}")]
    Api { status: u16, body: String },
    #[error("malformed response: {0}")]
    Malformed(String),
}

/// DFlow Trading API client.
#[derive(Clone)]
pub struct DflowClient {
    http: reqwest::Client,
    base: String,
    api_key: Option<String>,
}

impl DflowClient {
    /// Client against the keyless developer endpoint.
    pub fn dev() -> Self {
        Self::new(DEV_BASE, None)
    }

    /// Client against `base` with an optional `x-api-key`.
    pub fn new(base: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into(),
            api_key,
        }
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, DflowError> {
        let url = format!("{}{path}", self.base);
        let mut req = self.http.get(url).query(query);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        match status.is_success() {
            true => Ok(resp.json::<T>().await?),
            false => Err(DflowError::Api {
                status: status.as_u16(),
                body: resp.text().await.unwrap_or_default(),
            }),
        }
    }

    /// `GET /quote` with the raw body and response headers retained —
    /// the capture path for audit evidence. Sends `x-sign-request: true`;
    /// whether signature headers come back is the venue's choice and the
    /// caller's evidence classification, not ours to assume.
    pub async fn quote_raw(
        &self,
        req: &QuoteRequest,
    ) -> Result<(Vec<(String, String)>, Vec<u8>), DflowError> {
        let url = format!("{}/quote", self.base);
        let mut r = self
            .http
            .get(url)
            .header("x-sign-request", "true")
            .query(&[
                ("inputMint", req.input_mint.clone()),
                ("outputMint", req.output_mint.clone()),
                ("amount", req.amount.to_string()),
                ("slippageBps", req.slippage_bps.to_string()),
            ]);
        if let Some(key) = &self.api_key {
            r = r.header("x-api-key", key);
        }
        let resp = r.send().await?;
        let status = resp.status();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(n, v)| (n.as_str().to_ascii_lowercase(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
            .collect();
        let body = resp.bytes().await?.to_vec();
        match status.is_success() {
            true => Ok((headers, body)),
            false => Err(DflowError::Api {
                status: status.as_u16(),
                body: String::from_utf8_lossy(&body).into_owned(),
            }),
        }
    }

    /// `GET /quote` — imperative quote (no transaction).
    pub async fn quote(&self, req: &QuoteRequest) -> Result<QuoteResponse, DflowError> {
        self.get_json(
            "/quote",
            &[
                ("inputMint", req.input_mint.clone()),
                ("outputMint", req.output_mint.clone()),
                ("amount", req.amount.to_string()),
                ("slippageBps", req.slippage_bps.to_string()),
            ],
        )
        .await
    }

    /// `GET /order` — quote plus ready-to-sign base64 transaction.
    pub async fn order(
        &self,
        req: &QuoteRequest,
        user_public_key: &str,
    ) -> Result<OrderResponse, DflowError> {
        self.get_json(
            "/order",
            &[
                ("inputMint", req.input_mint.clone()),
                ("outputMint", req.output_mint.clone()),
                ("amount", req.amount.to_string()),
                ("slippageBps", req.slippage_bps.to_string()),
                ("userPublicKey", user_public_key.to_string()),
            ],
        )
        .await
    }
}
