//! The per-instrument hash chain.
//!
//! Each record's `prev_hash` is the [`record_hash`](crate::record_hash) of
//! the previous *published* record — including its attestation, so the chain
//! commits to records exactly as the world saw them. `seq` must strictly
//! increase; it need not be dense. A gap is a visible fact about capture
//! (dropped cycle, crashed executor), and hiding it would let the log
//! condition on success — so gaps are *reported*, never rejected.

use crate::canonical::record_hash;
use crate::types::{AuditRecord, RailError, ZERO_HASH};

/// What a chain verification found. Errors abort; gaps do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainReport {
    pub records: usize,
    /// `(after_seq, before_seq)` for every non-contiguous step.
    pub gaps: Vec<(u64, u64)>,
    /// Hash of the final record — the value the next `link` must cite, and
    /// the tail a segment root will cover.
    pub tip: [u8; 32],
}

/// Prepare `record` to follow `prev` (or genesis when `None`): sets `seq`
/// and `prev_hash`. The attestation must be applied *after* linking — the
/// content digest covers both fields.
pub fn link(mut record: AuditRecord, prev: Option<&AuditRecord>) -> AuditRecord {
    match prev {
        None => {
            record.prev_hash = ZERO_HASH;
        }
        Some(p) => {
            record.prev_hash = record_hash(p);
            if record.seq <= p.seq {
                // Sequencing is the caller's (the DO's) job; linking merely
                // refuses to construct an out-of-order chain silently.
                record.seq = p.seq + 1;
            }
        }
    }
    record
}

/// Verify a slice of records as one chain segment.
///
/// Checks: first record either cites `expected_prev` (when resuming from a
/// known tip) or is genesis; every later record cites its predecessor's
/// hash; `seq` strictly increases throughout.
pub fn verify_chain(
    records: &[AuditRecord],
    expected_prev: Option<[u8; 32]>,
) -> Result<ChainReport, RailError> {
    let first = records.first().ok_or(RailError::Empty)?;
    let expected = expected_prev.unwrap_or(ZERO_HASH);
    if first.prev_hash != expected {
        return Err(RailError::ChainLink(first.seq));
    }
    let mut gaps = Vec::new();
    for pair in records.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if b.seq <= a.seq {
            return Err(RailError::SeqOrder(b.seq));
        }
        if b.seq != a.seq + 1 {
            gaps.push((a.seq, b.seq));
        }
        if b.prev_hash != record_hash(a) {
            return Err(RailError::ChainLink(b.seq));
        }
    }
    Ok(ChainReport {
        records: records.len(),
        gaps,
        tip: record_hash(records.last().unwrap_or(first)),
    })
}
