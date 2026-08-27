//! Standalone record verification — §3.4 steps 1, 3-adjacent, and 6.
//!
//! What an auditor can check from the record alone: the attestation, the
//! internal consistency of the evidence, the amounts, and whether the
//! decision reproduces under the committed rule. What needs the outside
//! world stays outside: the DFlow RFC 9421 signature (v4.2 verifier), the
//! fill transaction on-chain (RPC), the Merkle path to an anchor
//! ([`crate::merkle_verify`] plus the anchor transaction).

use sha2::{Digest as _, Sha256};

use crate::attest::verify_attestation;
use crate::types::{AuditRecord, QuoteEvidence, RailError, parse_amount};

/// The v1 routing rule, versioned by name. The fingerprint of this string is
/// what the policy PDA commits and what every record cites.
pub const RULE_V1_ID: &str = "afterswap-route:argmax_net_out:v1";

/// blake3-64 fingerprint of the v1 rule — first 8 bytes, little-endian, the
/// same convention as the policy program's machine fingerprints.
pub fn rule_v1_fingerprint() -> u64 {
    let h = blake3::hash(RULE_V1_ID.as_bytes());
    u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap_or_default())
}

/// The v1 rule itself: highest net output wins; ties break toward the first
/// evaluated venue (the primary, by construction of the capture order).
fn rule_v1_choose(record: &AuditRecord) -> Result<&str, RailError> {
    let mut best: Option<(&str, u128)> = None;
    for e in &record.decision.evaluated {
        let net = parse_amount(&e.net_out)?;
        if best.as_ref().is_none_or(|(_, b)| net > *b) {
            best = Some((&e.venue, net));
        }
    }
    best.map(|(v, _)| v).ok_or(RailError::Empty)
}

/// Base64 decode (standard alphabet, padded) without a dependency: the only
/// use is re-hashing observed bodies, and pulling a crate in for one decoder
/// would be the larger surface.
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = ALPHABET.iter().position(|&a| a == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// Full standalone verification of one record.
///
/// 1. attestation signature against `attest_public`
/// 2. every amount parses as a base-10 integer
/// 3. observed evidence: the retained body re-hashes to its digest
/// 4. the decision cites only venues the record quotes
/// 5. the policy fingerprint is a rule this verifier knows
/// 6. re-running that rule over `evaluated` lands on `chosen_venue`
pub fn verify_record(record: &AuditRecord, attest_public: &[u8; 32]) -> Result<(), RailError> {
    verify_attestation(record, attest_public)?;

    for q in &record.quotes {
        parse_amount(&q.in_amount)?;
        parse_amount(&q.out_amount)?;
        if let QuoteEvidence::Observed { body_b64, body_sha256 } = &q.evidence {
            let body = b64_decode(body_b64).ok_or_else(|| RailError::Evidence(q.venue.clone()))?;
            let digest: [u8; 32] = Sha256::digest(&body).into();
            if digest != *body_sha256 {
                return Err(RailError::Evidence(q.venue.clone()));
            }
        }
    }
    if let Some(f) = &record.fill {
        parse_amount(&f.in_amount)?;
        parse_amount(&f.out_amount)?;
    }

    for e in &record.decision.evaluated {
        if !record.quotes.iter().any(|q| q.venue == e.venue) {
            return Err(RailError::UnknownVenue(e.venue.clone()));
        }
    }

    if record.policy_fingerprint != rule_v1_fingerprint() {
        return Err(RailError::UnknownRule(record.policy_fingerprint));
    }
    let should = rule_v1_choose(record)?;
    match should == record.decision.chosen_venue {
        true => Ok(()),
        false => Err(RailError::Decision(
            RULE_V1_ID.to_string(),
            record.decision.chosen_venue.clone(),
            should.to_string(),
        )),
    }
}
