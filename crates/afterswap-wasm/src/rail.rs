//! Rail verification exports — the same `afterswap-rail` bytes the executor
//! links, exposed to the Worker's Sequencer DO and the browser verifier.
//!
//! One implementation everywhere is the point: the Worker never reimplements
//! canonical encoding or blake3 in TypeScript, so ingest-time verification
//! cannot drift from what `rail_verify` (native) and an auditor's build of
//! the crate would conclude.
//!
//! Interface style matches the engine exports: JSON strings in, JSON or hex
//! strings out, errors as `"err: …"` values rather than thrown exceptions —
//! a DO that throws mid-ingest loses the request context; one that returns
//! an error string logs it and rejects cleanly.

use wasm_bindgen::prelude::*;

use afterswap_rail::{AuditRecord, merkle_proof, merkle_root, record_hash, verify_record};

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Full standalone verification (§3.4 steps 1 + 6): attestation, amounts,
/// evidence digests, decision reproduction. Returns `"ok"` or `"err: …"`.
#[wasm_bindgen]
pub fn rail_verify_record(record_json: &str, attest_pubkey_hex: &str) -> String {
    let Some(public) = hex32(attest_pubkey_hex) else {
        return "err: bad pubkey hex".to_string();
    };
    let record: AuditRecord = match serde_json::from_str(record_json) {
        Ok(r) => r,
        Err(e) => return format!("err: parse: {e}"),
    };
    match verify_record(&record, &public) {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("err: {e}"),
    }
}

/// `record_hash` (content + attestation) as hex — what the chain links and
/// Merkle leaves commit to.
#[wasm_bindgen]
pub fn rail_record_hash(record_json: &str) -> String {
    match serde_json::from_str::<AuditRecord>(record_json) {
        Ok(r) => to_hex(&record_hash(&r)),
        Err(e) => format!("err: parse: {e}"),
    }
}

/// Merkle root over a JSON array of record-hash hex strings.
#[wasm_bindgen]
pub fn rail_merkle_root(hashes_json: &str) -> String {
    let Ok(hex_list) = serde_json::from_str::<Vec<String>>(hashes_json) else {
        return "err: parse".to_string();
    };
    let mut leaves = Vec::with_capacity(hex_list.len());
    for h in &hex_list {
        match hex32(h) {
            Some(x) => leaves.push(x),
            None => return format!("err: bad hash hex {h}"),
        }
    }
    match merkle_root(&leaves) {
        Ok(root) => to_hex(&root),
        Err(e) => format!("err: {e}"),
    }
}

/// Inclusion proof for `index`, as a JSON array of `[sibling_hex, is_left]`.
#[wasm_bindgen]
pub fn rail_merkle_proof(hashes_json: &str, index: usize) -> String {
    let Ok(hex_list) = serde_json::from_str::<Vec<String>>(hashes_json) else {
        return "err: parse".to_string();
    };
    let mut leaves = Vec::with_capacity(hex_list.len());
    for h in &hex_list {
        match hex32(h) {
            Some(x) => leaves.push(x),
            None => return format!("err: bad hash hex {h}"),
        }
    }
    match merkle_proof(&leaves, index) {
        Ok(steps) => {
            let out: Vec<(String, bool)> =
                steps.iter().map(|(s, l)| (to_hex(s), *l)).collect();
            serde_json::to_string(&out).unwrap_or_else(|e| format!("err: {e}"))
        }
        Err(e) => format!("err: {e}"),
    }
}

/// Verify an inclusion proof: record hash (hex), proof (JSON array of
/// `[sibling_hex, is_left]`), root (hex). Returns `"ok"` or `"err: …"` —
/// the browser verifier's final step.
#[wasm_bindgen]
pub fn rail_merkle_verify(record_hash_hex: &str, proof_json: &str, root_hex: &str) -> String {
    let (Some(hash), Some(root)) = (hex32(record_hash_hex), hex32(root_hex)) else {
        return "err: bad hex".to_string();
    };
    let Ok(steps_hex) = serde_json::from_str::<Vec<(String, bool)>>(proof_json) else {
        return "err: parse proof".to_string();
    };
    let mut steps = Vec::with_capacity(steps_hex.len());
    for (s, left) in &steps_hex {
        match hex32(s) {
            Some(x) => steps.push((x, *left)),
            None => return "err: bad sibling hex".to_string(),
        }
    }
    match afterswap_rail::merkle_verify(&hash, &steps, &root) {
        true => "ok".to_string(),
        false => "err: proof does not verify".to_string(),
    }
}
