//! Verifiable execution rail — R0 of `docs/RAIL.md`.
//!
//! This crate is the record format and nothing else: canonical bytes, the
//! hash chain, Merkle segments, attestation, and standalone verification.
//! No I/O, no async, no clock, no entropy — every function is a pure map
//! from inputs to outputs, which is what lets the same bytes compile for the
//! native executor, the Worker, and the browser verifier (the G6 parity
//! discipline; determinism here is not a style preference, it is the
//! product).
//!
//! Two digests per record, deliberately distinct:
//!
//! * [`canonical::content_digest`] — the record *minus* its attestation.
//!   This is what gets signed; a record cannot sign its own signature.
//! * [`canonical::record_hash`] — the record *including* its attestation.
//!   This is what chains and what Merkle segments commit to, so the anchor
//!   covers the record exactly as published.
//!
//! What this crate does **not** verify: the RFC 9421 provider signature on a
//! DFlow leg. That verifier shipped in v4.2 and runs where the headers are
//! (browser / server); the rail stores the material and leaves step 2 of the
//! audit procedure (§3.4) to it.

pub mod attest;
pub mod b64;
pub mod canonical;
pub mod chain;
pub mod merkle;
pub mod types;
pub mod verify;

pub use attest::{AttestKey, attest, verify_attestation};
pub use canonical::{content_digest, record_hash};
pub use chain::{ChainReport, link, verify_chain};
pub use merkle::{merkle_proof, merkle_root, merkle_verify};
pub use types::{
    AuditRecord, EvaluatedVenue, FillRef, QuoteEvidence, RailError, RouteDecision, VenueQuote,
    ZERO_HASH,
};
pub use verify::{RULE_V1_ID, rule_v1_fingerprint, verify_record};

/// Domain tag for attestation preimages. A record's signature is over
/// `blake3(DOMAIN ‖ content_digest)`, never over raw bytes — so no audit
/// record can collide with a Solana transaction or any other signable blob.
pub const DOMAIN: &[u8] = b"afterswap-rail:v1";

/// Canonical schema version, first byte of every encoding. Bump on any
/// change to the byte layout; verification refuses versions it does not know.
pub const SCHEMA_VERSION: u8 = 1;
