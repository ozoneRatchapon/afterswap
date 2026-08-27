//! Live tranche execution (feature = "live"): request a DFlow `/order`,
//! sign the returned transaction with a local keypair, submit via JSON-RPC.
//!
//! Deliberately minimal — throwaway-keypair buildathon proof, not custody
//! software. No retries, no confirmation tracking beyond the signature.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use solana_sdk::signature::{Keypair, Signer, read_keypair_file};
use solana_sdk::transaction::VersionedTransaction;

use crate::client::{DflowClient, DflowError};
use crate::types::QuoteRequest;

/// Live-execution errors.
#[derive(Debug, thiserror::Error)]
pub enum LiveError {
    #[error("dflow: {0}")]
    Dflow(#[from] DflowError),
    #[error("keypair: {0}")]
    Keypair(String),
    #[error("transaction decode: {0}")]
    Decode(String),
    #[error("signing: {0}")]
    Sign(String),
    #[error("rpc: {0}")]
    Rpc(String),
}

/// Signs and submits DFlow orders with a local keypair.
pub struct LiveExecutor {
    client: DflowClient,
    keypair: Keypair,
    rpc_url: String,
    http: reqwest::Client,
}

impl LiveExecutor {
    /// Load the signer from a Solana CLI JSON keypair file.
    pub fn from_keypair_file(
        client: DflowClient,
        path: &str,
        rpc_url: impl Into<String>,
    ) -> Result<Self, LiveError> {
        let keypair =
            read_keypair_file(path).map_err(|e| LiveError::Keypair(e.to_string()))?;
        Ok(Self {
            client,
            keypair,
            rpc_url: rpc_url.into(),
            http: reqwest::Client::new(),
        })
    }

    /// The signer's public key (base58).
    pub fn pubkey(&self) -> String {
        self.keypair.pubkey().to_string()
    }

    /// Execute one sell tranche: `/order` → sign → `sendTransaction`.
    /// Returns the transaction signature.
    pub async fn sell(&self, req: &QuoteRequest) -> Result<String, LiveError> {
        let order = self.client.order(req, &self.pubkey()).await?;
        let raw = B64
            .decode(&order.transaction)
            .map_err(|e| LiveError::Decode(e.to_string()))?;
        let unsigned: VersionedTransaction =
            bincode::deserialize(&raw).map_err(|e| LiveError::Decode(e.to_string()))?;
        let signed = VersionedTransaction::try_new(unsigned.message, &[&self.keypair])
            .map_err(|e| LiveError::Sign(e.to_string()))?;
        let signed_b64 = B64.encode(
            bincode::serialize(&signed).map_err(|e| LiveError::Sign(e.to_string()))?,
        );

        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "sendTransaction",
            "params": [signed_b64, {"encoding": "base64", "skipPreflight": false}],
        });
        let resp: serde_json::Value = self
            .http
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| LiveError::Rpc(e.to_string()))?
            .json()
            .await
            .map_err(|e| LiveError::Rpc(e.to_string()))?;
        match resp.get("result").and_then(|v| v.as_str()) {
            Some(sig) => Ok(sig.to_string()),
            None => Err(LiveError::Rpc(resp.to_string())),
        }
    }

    /// Fetch a landed transaction and extract what actually happened.
    ///
    /// The quote is what we were offered; this is what we got. Recording the
    /// former as the latter would make every execution measurement a
    /// restatement of the quote, which is the one error this experiment cannot
    /// survive — so `ConfirmedFill` is derived from post-trade balance deltas
    /// and the fee the network actually charged.
    pub async fn confirm(&self, signature: &str) -> Result<ConfirmedFill, LiveError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "getTransaction",
            "params": [signature, {
                "encoding": "jsonParsed",
                "maxSupportedTransactionVersion": 0,
                "commitment": "confirmed",
            }],
        });
        let resp: serde_json::Value = self
            .http
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| LiveError::Rpc(e.to_string()))?
            .json()
            .await
            .map_err(|e| LiveError::Rpc(e.to_string()))?;
        let result = resp
            .get("result")
            .filter(|v| !v.is_null())
            .ok_or_else(|| LiveError::Rpc(format!("transaction not found: {signature}")))?;
        parse_confirmed(result, &self.pubkey()).ok_or_else(|| {
            LiveError::Rpc(format!("could not parse balances for {signature}"))
        })
    }
}

/// What a landed transaction actually did, as opposed to what it was quoted to
/// do.
#[derive(Debug, Clone)]
pub struct ConfirmedFill {
    pub slot: u64,
    /// Network fee charged, in lamports. Includes the priority fee.
    pub fee_lamports: u64,
    /// Token balance deltas for the signer, keyed by mint. Negative is sent.
    pub deltas: std::collections::BTreeMap<String, f64>,
    /// Native lamport delta for the signer, fee already added back so it
    /// reflects trade flow rather than trade flow plus cost.
    pub lamport_delta: i128,
    /// Present when the transaction landed but reverted. A reverted fill is a
    /// recorded failure, not a missing observation.
    pub err: Option<String>,
}

impl ConfirmedFill {
    /// Effective price as `out per in`, from realised deltas.
    ///
    /// Returns `None` unless both legs moved in the expected direction —
    /// a zero or wrong-signed delta means the parse found the wrong accounts,
    /// and guessing past that would fabricate a fill price.
    pub fn effective_price(&self, input_mint: &str, output_mint: &str) -> Option<f64> {
        let sent = -self.deltas.get(input_mint).copied().unwrap_or(0.0);
        let recv = self.deltas.get(output_mint).copied().unwrap_or(0.0);
        match sent > 0.0 && recv > 0.0 {
            true => Some(recv / sent),
            false => None,
        }
    }
}

/// Extract balance deltas for `owner` from a `getTransaction` result.
///
/// Split out from the RPC call so it can be tested against captured responses
/// without a network.
pub fn parse_confirmed(result: &serde_json::Value, owner: &str) -> Option<ConfirmedFill> {
    let meta = result.get("meta")?;
    let slot = result.get("slot").and_then(|v| v.as_u64()).unwrap_or(0);
    let fee_lamports = meta.get("fee").and_then(|v| v.as_u64()).unwrap_or(0);
    let err = meta
        .get("err")
        .filter(|v| !v.is_null())
        .map(std::string::ToString::to_string);

    let owned = |key: &str| -> std::collections::BTreeMap<String, f64> {
        let mut out = std::collections::BTreeMap::new();
        if let Some(arr) = meta.get(key).and_then(|v| v.as_array()) {
            for b in arr {
                if b.get("owner").and_then(|v| v.as_str()) != Some(owner) {
                    continue;
                }
                let (Some(mint), Some(amt)) = (
                    b.get("mint").and_then(|v| v.as_str()),
                    b.pointer("/uiTokenAmount/uiAmount").and_then(|v| v.as_f64()),
                ) else {
                    continue;
                };
                *out.entry(mint.to_string()).or_insert(0.0) += amt;
            }
        }
        out
    };
    let (pre, post) = (owned("preTokenBalances"), owned("postTokenBalances"));
    let mut deltas: std::collections::BTreeMap<String, f64> = post.clone();
    for (mint, v) in &pre {
        *deltas.entry(mint.clone()).or_insert(0.0) -= v;
    }
    deltas.retain(|_, v| v.abs() > 0.0);

    // Native SOL moves in lamports, not token balances. The fee is added back
    // so the delta reflects the trade rather than the trade plus its cost.
    let idx = result
        .pointer("/transaction/message/accountKeys")
        .and_then(|v| v.as_array())
        .and_then(|keys| {
            keys.iter().position(|k| {
                k.as_str() == Some(owner) || k.get("pubkey").and_then(|p| p.as_str()) == Some(owner)
            })
        });
    let lamport_delta = match idx {
        Some(i) => {
            let get = |key: &str| -> i128 {
                meta.get(key)
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.get(i))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as i128
            };
            get("postBalances") - get("preBalances") + i128::from(fee_lamports)
        }
        None => 0,
    };

    Some(ConfirmedFill {
        slot,
        fee_lamports,
        deltas,
        lamport_delta,
        err,
    })
}
