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
use crate::confirm::{ConfirmedFill, parse_confirmed};
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

    /// Anchor a rail segment root on-chain: a single-instruction memo
    /// transaction, signed by the executor's keypair.
    ///
    /// Memo body follows the shipped `afterswap:quote sha-256=<digest>`
    /// convention: `afterswap:rail blake3=<root> seq=<from>..<to>`. The
    /// Worker never sees this path — anchors are exclusively built and
    /// submitted here, which is what keeps it keyless.
    pub async fn anchor_memo(&self, memo: &str) -> Result<String, LiveError> {
        const MEMO_PROGRAM: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
        let blockhash_resp: serde_json::Value = self
            .http
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc":"2.0","id":1,"method":"getLatestBlockhash",
                "params":[{"commitment":"finalized"}]
            }))
            .send()
            .await
            .map_err(|e| LiveError::Rpc(e.to_string()))?
            .json()
            .await
            .map_err(|e| LiveError::Rpc(e.to_string()))?;
        let blockhash_str = blockhash_resp
            .pointer("/result/value/blockhash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LiveError::Rpc("no blockhash".to_string()))?;
        let blockhash: solana_sdk::hash::Hash = blockhash_str
            .parse()
            .map_err(|_| LiveError::Rpc("bad blockhash".to_string()))?;

        let program: solana_sdk::pubkey::Pubkey = MEMO_PROGRAM
            .parse()
            .map_err(|_| LiveError::Rpc("bad memo program id".to_string()))?;
        let ix = solana_sdk::instruction::Instruction {
            program_id: program,
            accounts: vec![],
            data: memo.as_bytes().to_vec(),
        };
        let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.keypair.pubkey()),
            &[&self.keypair],
            blockhash,
        );
        let raw = bincode::serialize(&tx).map_err(|e| LiveError::Sign(e.to_string()))?;
        let body = serde_json::json!({
            "jsonrpc":"2.0","id":1,"method":"sendTransaction",
            "params":[B64.encode(raw), {"encoding":"base64","skipPreflight":false}],
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
