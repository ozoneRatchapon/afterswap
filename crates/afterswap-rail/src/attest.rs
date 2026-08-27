//! Attestation — a dedicated key signs each record's content digest.
//!
//! Dedicated, and domain-separated, for one reason: the executor also holds
//! a *trading* keypair, and signing arbitrary blobs with a key that also
//! signs transactions is a signature-confusion footgun. The preimage here is
//! `blake3(DOMAIN ‖ content_digest)`, so nothing this key ever signs can be
//! mistaken for — or replayed as — anything else.
//!
//! No entropy dependency: keys come from seed bytes. Where the seed comes
//! from (keyfile, env, HSM) is the executor's business; this crate stays
//! pure so it compiles for wasm32 unchanged.

use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};

use crate::DOMAIN;
use crate::canonical::content_digest;
use crate::types::{AuditRecord, RailError};

/// The attestation keypair, held by the native executor only. The Worker and
/// the browser verifier see nothing but [`AttestKey::public`].
pub struct AttestKey(SigningKey);

impl AttestKey {
    /// Deterministic construction from 32 seed bytes.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&seed))
    }

    /// The public key auditors verify against, published beside DFlow's.
    pub fn public(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }
}

/// The exact preimage the attestation signs.
fn preimage(digest: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(DOMAIN);
    h.update(digest);
    *h.finalize().as_bytes()
}

/// Sign a record's content, returning it with the attestation filled in.
/// Whatever was in `attestation` before is overwritten — the field is not
/// part of its own preimage.
pub fn attest(mut record: AuditRecord, key: &AttestKey) -> AuditRecord {
    let digest = content_digest(&record);
    let sig = key.0.sign(&preimage(&digest));
    record.attestation = sig.to_bytes();
    record
}

/// Verify a record's attestation against a public key.
pub fn verify_attestation(record: &AuditRecord, public: &[u8; 32]) -> Result<(), RailError> {
    let key = VerifyingKey::from_bytes(public).map_err(|_| RailError::Attestation)?;
    let digest = content_digest(record);
    let sig = Signature::from_bytes(&record.attestation);
    key.verify(&preimage(&digest), &sig)
        .map_err(|_| RailError::Attestation)
}
