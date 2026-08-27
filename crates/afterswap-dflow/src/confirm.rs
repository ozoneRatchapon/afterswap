//! Post-trade confirmation: what a transaction actually did.
//!
//! Deliberately free of any Solana SDK dependency and not gated behind the
//! `live` feature. The parser's job is to stop a quote being recorded as a
//! fill, and that check should be runnable — and testable against captured
//! mainnet responses — without a keypair, a signer, or the ability to spend
//! anything. `examples/fetch_tx.rs` exercises it against real transactions.

/// What a landed transaction actually did, as opposed to what it was quoted to
/// do.
#[derive(Debug, Clone)]
pub struct ConfirmedFill {
    pub slot: u64,
    /// Network fee charged, in lamports. Includes the priority fee.
    pub fee_lamports: u64,
    /// Token balance deltas for the signer, keyed by mint. Negative is sent.
    pub deltas: std::collections::BTreeMap<String, f64>,
    /// The same deltas in raw smallest units — exact integers from the
    /// chain's own `amount` strings, no division ever applied. This is the
    /// only representation allowed to reach a `FillRef`: a float that has
    /// been divided and re-multiplied can differ in the last digits, and the
    /// audit record is precisely where the last digits matter.
    pub raw_deltas: std::collections::BTreeMap<String, i128>,
    /// Decimals per mint, as reported by the balance entries.
    pub decimals: std::collections::BTreeMap<String, u8>,
    /// Native lamport delta for the signer, fee already added back so it
    /// reflects trade flow rather than trade flow plus cost.
    pub lamport_delta: i128,
    /// Present when the transaction landed but reverted. A reverted fill is a
    /// recorded failure, not a missing observation.
    pub err: Option<String>,
}

impl ConfirmedFill {
    /// Signed delta for a mint, falling back to the native lamport balance when
    /// the mint is native SOL and no wrapped-SOL token account moved.
    ///
    /// A SOL leg can appear either way: routed through a wrapped-SOL account it
    /// shows in `preTokenBalances`/`postTokenBalances`, but paid natively it
    /// shows only in `preBalances`/`postBalances`. Looking in one place would
    /// make every native SOL swap parse as unfilled — and SOL/USDC is the pair
    /// the experiment targets.
    pub fn delta_for(&self, mint: &str) -> f64 {
        match self.deltas.get(mint) {
            Some(d) => *d,
            None if mint == WRAPPED_SOL => self.lamport_delta as f64 / LAMPORTS_PER_SOL,
            None => 0.0,
        }
    }

    /// Raw integer legs for an audit record: `(in_raw, out_raw)`, both
    /// positive, in each mint's smallest units. Falls back to the native
    /// lamport delta for a SOL leg that never touched a wrapped account.
    /// `None` unless both legs moved in the expected direction — the audit
    /// trail records "no fill" over a guessed one.
    pub fn raw_legs(&self, input_mint: &str, output_mint: &str) -> Option<(u128, u128)> {
        let raw_of = |mint: &str| -> i128 {
            match self.raw_deltas.get(mint) {
                Some(d) => *d,
                None if mint == WRAPPED_SOL => self.lamport_delta,
                None => 0,
            }
        };
        let sent = -raw_of(input_mint);
        let recv = raw_of(output_mint);
        match sent > 0 && recv > 0 {
            true => Some((sent as u128, recv as u128)),
            false => None,
        }
    }

    /// Effective price as `out per in`, from realised deltas.
    ///
    /// Returns `None` unless both legs moved in the expected direction —
    /// a zero or wrong-signed delta means the parse found the wrong accounts,
    /// and guessing past that would fabricate a fill price.
    pub fn effective_price(&self, input_mint: &str, output_mint: &str) -> Option<f64> {
        let sent = -self.delta_for(input_mint);
        let recv = self.delta_for(output_mint);
        match sent > 0.0 && recv > 0.0 {
            true => Some(recv / sent),
            false => None,
        }
    }
}

/// Native SOL's mint address, as it appears in token balance entries.
pub const WRAPPED_SOL: &str = "So11111111111111111111111111111111111111112";
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

/// Raw integer amount and decimals from a balance entry — the authoritative
/// fields, untouched.
fn raw_amount(entry: &serde_json::Value) -> Option<(i128, u8)> {
    let amt = entry.pointer("/uiTokenAmount/amount")?;
    let raw: i128 = match amt {
        serde_json::Value::String(s) => s.parse().ok()?,
        v => v.as_i64().map(i128::from)?,
    };
    let decimals = entry
        .pointer("/uiTokenAmount/decimals")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u8;
    Some((raw, decimals))
}

/// Exact token amount from a balance entry.
///
/// Prefers the raw integer `amount` with `decimals` over the pre-divided
/// `uiAmount` float. At the 0.1 bps resolution this experiment targets, an f64
/// that has already lost the low digits of a large token quantity is not good
/// enough — and `uiAmount` is additionally `null` for a zero balance, which the
/// float path silently drops.
fn ui_amount(entry: &serde_json::Value) -> Option<f64> {
    let amt = entry.pointer("/uiTokenAmount/amount")?;
    let decimals = entry
        .pointer("/uiTokenAmount/decimals")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let raw: f64 = match amt {
        serde_json::Value::String(s) => s.parse().ok()?,
        v => v.as_f64()?,
    };
    Some(raw / 10f64.powi(decimals as i32))
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
                let (Some(mint), Some(amt)) = (b.get("mint").and_then(|v| v.as_str()), ui_amount(b))
                else {
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

    // Raw integer deltas, computed independently of the float path.
    let mut raw_deltas: std::collections::BTreeMap<String, i128> = Default::default();
    let mut decimals: std::collections::BTreeMap<String, u8> = Default::default();
    for (key, sign) in [("postTokenBalances", 1i128), ("preTokenBalances", -1i128)] {
        if let Some(arr) = meta.get(key).and_then(|v| v.as_array()) {
            for b in arr {
                if b.get("owner").and_then(|v| v.as_str()) != Some(owner) {
                    continue;
                }
                let (Some(mint), Some((raw, dec))) =
                    (b.get("mint").and_then(|v| v.as_str()), raw_amount(b))
                else {
                    continue;
                };
                *raw_deltas.entry(mint.to_string()).or_insert(0) += sign * raw;
                decimals.insert(mint.to_string(), dec);
            }
        }
    }
    raw_deltas.retain(|_, v| *v != 0);

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
        raw_deltas,
        decimals,
        lamport_delta,
        err,
    })
}
