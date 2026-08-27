//! Canonical byte encoding — the thing every hash and signature stands on.
//!
//! JSON is the *transport* format (human-auditable, stored, served); it is
//! not hash-stable, so nothing is ever hashed over JSON. Hashing runs over
//! this encoding instead, and a JSON round-trip must reproduce it byte for
//! byte (pinned by test).
//!
//! Rules, chosen so two implementations cannot disagree:
//!
//! * every integer is u64 little-endian (amounts stay *strings*: length-
//!   prefixed UTF-8, exactly as published)
//! * strings and variable byte arrays: u64 length ‖ bytes
//! * fixed arrays ([u8; 32], [u8; 64]): raw, no length
//! * `Option`: 0u8 / 1u8 ‖ value; `Vec`: u64 count ‖ items
//! * enums: u8 tag ‖ fields; structs: fields in declaration order
//! * the whole encoding opens with [`crate::SCHEMA_VERSION`]
//!
//! No floats exist in the schema, no maps exist anywhere (iteration order),
//! and no `usize` reaches the wire (platform width). Those three absences
//! are what make native/wasm parity a property instead of a test burden.

use crate::SCHEMA_VERSION;
use crate::types::{AuditRecord, FillRef, QuoteEvidence, RouteDecision, VenueQuote};

struct Enc(Vec<u8>);

impl Enc {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.u64(v.len() as u64);
        self.0.extend_from_slice(v);
    }
    fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }
    fn fixed(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
    fn opt_u64(&mut self, v: Option<u64>) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                self.u64(x);
            }
        }
    }
}

fn quote(e: &mut Enc, q: &VenueQuote) {
    e.str(&q.venue);
    e.opt_u64(q.context_slot);
    e.u64(q.latency_us);
    e.str(&q.in_mint);
    e.str(&q.out_mint);
    e.str(&q.in_amount);
    e.str(&q.out_amount);
    e.str(&q.route);
    match &q.evidence {
        QuoteEvidence::ProviderSigned { sig_headers, body_sha256 } => {
            e.u8(0);
            e.str(sig_headers);
            e.fixed(body_sha256);
        }
        QuoteEvidence::Observed { body_b64, body_sha256 } => {
            e.u8(1);
            e.str(body_b64);
            e.fixed(body_sha256);
        }
    }
}

fn decision(e: &mut Enc, d: &RouteDecision) {
    e.str(&d.chosen_venue);
    e.u64(d.evaluated.len() as u64);
    for v in &d.evaluated {
        e.str(&v.venue);
        e.str(&v.net_out);
    }
}

fn fill(e: &mut Enc, f: &FillRef) {
    e.str(&f.signature);
    e.u64(f.slot);
    e.str(&f.in_mint);
    e.str(&f.out_mint);
    e.str(&f.in_amount);
    e.str(&f.out_amount);
    e.u64(f.fee_lamports);
}

/// Canonical bytes of everything except the attestation.
fn content_bytes(r: &AuditRecord) -> Vec<u8> {
    let mut e = Enc(Vec::with_capacity(512));
    e.u8(SCHEMA_VERSION);
    e.u64(r.seq);
    e.fixed(&r.prev_hash);
    e.u64(r.t_ms);
    e.str(&r.instrument);
    e.u64(r.quotes.len() as u64);
    for q in &r.quotes {
        quote(&mut e, q);
    }
    e.u64(r.policy_fingerprint);
    decision(&mut e, &r.decision);
    match &r.fill {
        None => e.u8(0),
        Some(f) => {
            e.u8(1);
            fill(&mut e, f);
        }
    }
    e.0
}

/// Digest of the record minus its attestation — the signing preimage's core.
/// A record cannot sign its own signature.
pub fn content_digest(r: &AuditRecord) -> [u8; 32] {
    *blake3::hash(&content_bytes(r)).as_bytes()
}

/// Digest of the record *including* its attestation — what the chain links
/// and Merkle segments commit to, so anchors cover records as published.
pub fn record_hash(r: &AuditRecord) -> [u8; 32] {
    let mut bytes = content_bytes(r);
    bytes.extend_from_slice(&r.attestation);
    *blake3::hash(&bytes).as_bytes()
}
