//! The audit record schema.
//!
//! Ground rules, each bought by a bug this project already hit:
//!
//! * **Amounts are raw integer strings.** Floats are not hash-stable across
//!   serialisers, and the pre-divided `uiAmount` float already burned the
//!   fill parser (dropped low digits, null on zero). There is no `f64`
//!   anywhere in this schema — that is a compile-time property, not a
//!   convention.
//! * **`seq` gaps are load-bearing.** A dropped cycle must be visible in the
//!   stream, or the log silently conditions on capture success.
//! * **Hashes serialise as hex.** These records are meant to be read by
//!   auditors, and a 32-element JSON number array is not a fingerprint
//!   anyone can eyeball against an anchor memo.

use serde::{Deserialize, Serialize};

/// Genesis predecessor: the chain starts from all zeroes.
pub const ZERO_HASH: [u8; 32] = [0; 32];

/// Everything that can be wrong with a record, stated rather than panicked.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RailError {
    #[error("unknown schema version {0}")]
    SchemaVersion(u8),
    #[error("amount is not a base-10 integer: {0:?}")]
    Amount(String),
    #[error("attestation does not verify")]
    Attestation,
    #[error("evidence digest mismatch on venue {0}")]
    Evidence(String),
    #[error("decision cites venue {0} with no quote in the record")]
    UnknownVenue(String),
    #[error("decision does not reproduce under rule {0}: chose {1}, rule says {2}")]
    Decision(String, String, String),
    #[error("policy fingerprint {0:#018x} is not a known rule")]
    UnknownRule(u64),
    #[error("chain broken at seq {0}: prev_hash does not match predecessor")]
    ChainLink(u64),
    #[error("seq not strictly increasing at {0}")]
    SeqOrder(u64),
    #[error("empty input")]
    Empty,
    #[error("merkle index out of range")]
    MerkleIndex,
}

/// One venue's quote, with evidence of the strength the venue supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueQuote {
    /// Venue identifier, e.g. `"dflow"` or `"jupiter"`. A string rather than
    /// an enum so that adding a venue is data, not a schema-version bump.
    pub venue: String,
    /// Slot the quote was computed against — the cross-venue alignment key.
    pub context_slot: Option<u64>,
    /// Round-trip latency of the quote request, microseconds.
    pub latency_us: u64,
    pub in_mint: String,
    pub out_mint: String,
    /// Offered amounts in each mint's smallest unit, as base-10 strings.
    pub in_amount: String,
    pub out_amount: String,
    /// Route fingerprint at quote time, `venue|hops`.
    pub route: String,
    pub evidence: QuoteEvidence,
}

/// What warrants a quote. The two variants are evidentially unequal and the
/// record keeps them distinct rather than flattening both to "a quote".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuoteEvidence {
    /// The venue signed the response (RFC 9421). Re-verifiable against the
    /// venue's published key by anyone, forever. The headers carry the
    /// signature material; the digest binds them to the body we saw.
    ProviderSigned {
        sig_headers: String,
        #[serde(with = "hex32")]
        body_sha256: [u8; 32],
    },
    /// The venue signs nothing; we retain the body and attest we observed
    /// it. Worth exactly as much as our attestation key — stated, not hidden.
    Observed {
        /// Full response body, base64. Kept so a recorded quote can be
        /// re-parsed if its interpretation is ever questioned — the
        /// `impact_raw` lesson applied to a whole body.
        body_b64: String,
        #[serde(with = "hex32")]
        body_sha256: [u8; 32],
    },
}

impl QuoteEvidence {
    /// Evidence for a venue that signs nothing: retain the body, digest it.
    /// Encoding and digest live here so capture and verify cannot drift.
    pub fn observed(body: &[u8]) -> Self {
        use sha2::Digest as _;
        Self::Observed {
            body_b64: crate::b64::encode(body),
            body_sha256: sha2::Sha256::digest(body).into(),
        }
    }

    /// Evidence for an RFC 9421-signing venue: the signature headers plus the
    /// digest of the body they cover.
    pub fn provider_signed(sig_headers: String, body: &[u8]) -> Self {
        use sha2::Digest as _;
        Self::ProviderSigned {
            sig_headers,
            body_sha256: sha2::Sha256::digest(body).into(),
        }
    }
}

/// One venue's standing in the routing decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatedVenue {
    pub venue: String,
    /// The objective the rule compared, in output smallest units net of
    /// recorded fees, as a base-10 string.
    pub net_out: String,
}

/// The routing decision and the inputs it was made from. §3.4 step 6: an
/// auditor re-runs the committed rule over `evaluated` and must land on
/// `chosen_venue`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteDecision {
    pub chosen_venue: String,
    pub evaluated: Vec<EvaluatedVenue>,
}

/// The realised fill — `parse_confirmed` output, never the quote restated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillRef {
    /// Transaction signature; the on-chain half of the evidence.
    pub signature: String,
    pub slot: u64,
    pub in_mint: String,
    pub out_mint: String,
    /// Realised amounts from balance deltas, smallest units, base-10 strings.
    pub in_amount: String,
    pub out_amount: String,
    pub fee_lamports: u64,
}

/// One execution's audit record. The unit of publication, chaining, and
/// anchoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Per-instrument, monotonic. Gaps are visible and load-bearing.
    pub seq: u64,
    /// `record_hash` of the previous published record; `ZERO_HASH` at genesis.
    #[serde(with = "hex32")]
    pub prev_hash: [u8; 32],
    pub t_ms: u64,
    pub instrument: String,
    /// Every venue quoted — the multi-venue discovery Article 78 asks for.
    pub quotes: Vec<VenueQuote>,
    /// blake3-64 fingerprint of the routing rule that governed the decision.
    ///
    /// Serialised as a 16-hex string in JSON, never a number: a full-range
    /// u64 exceeds 2^53, and a JavaScript consumer that parses and
    /// re-stringifies the record would silently alter it — which shifts the
    /// canonical bytes and voids the attestation. Found by the browser
    /// verifier failing on every record; the schema rule is that no
    /// full-range integer crosses a JSON boundary as a number. Legacy
    /// numeric values still deserialise.
    #[serde(with = "fp_serde")]
    pub policy_fingerprint: u64,
    pub decision: RouteDecision,
    /// `None` when no order was sent or it did not land; an absent fill is a
    /// recorded outcome, not a missing row.
    pub fill: Option<FillRef>,
    /// ed25519 over `blake3(DOMAIN ‖ content_digest)`.
    #[serde(with = "hex64")]
    pub attestation: [u8; 64],
}

/// A base-10 integer amount, validated. `u128` bounds every SPL amount.
pub fn parse_amount(s: &str) -> Result<u128, RailError> {
    match !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        true => s.parse().map_err(|_| RailError::Amount(s.to_string())),
        false => Err(RailError::Amount(s.to_string())),
    }
}

/// u64 as 16-hex in JSON; accepts legacy plain numbers on read.
pub(crate) mod fp_serde {
    use serde::{Deserializer, Serializer, de::Error as _};

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:016x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = u64;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("16-hex string or legacy u64")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<u64, E> {
                Ok(v)
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<u64, E> {
                u64::from_str_radix(v, 16).map_err(|_| E::custom("bad fingerprint hex"))
            }
        }
        d.deserialize_any(V).map_err(|e| D::Error::custom(e.to_string()))
    }
}

macro_rules! hex_serde {
    ($mod_name:ident, $len:expr) => {
        pub(crate) mod $mod_name {
            use serde::{Deserialize, Deserializer, Serializer, de::Error};

            pub fn serialize<S: Serializer>(v: &[u8; $len], s: S) -> Result<S::Ok, S::Error> {
                let mut out = String::with_capacity($len * 2);
                for b in v {
                    out.push_str(&format!("{b:02x}"));
                }
                s.serialize_str(&out)
            }

            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; $len], D::Error> {
                let s = String::deserialize(d)?;
                if s.len() != $len * 2 {
                    return Err(D::Error::custom("bad hex length"));
                }
                let mut out = [0u8; $len];
                for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
                    let hi = (chunk[0] as char).to_digit(16).ok_or_else(|| D::Error::custom("bad hex"))?;
                    let lo = (chunk[1] as char).to_digit(16).ok_or_else(|| D::Error::custom("bad hex"))?;
                    out[i] = (hi * 16 + lo) as u8;
                }
                Ok(out)
            }
        }
    };
}

hex_serde!(hex32, 32);
hex_serde!(hex64, 64);
