//! Merkle segments — what gets anchored on-chain.
//!
//! Leaves are [`record_hash`](crate::record_hash)es. Construction is
//! RFC 6962-style: leaf and interior hashes are domain-separated (0x00 /
//! 0x01 prefixes) so no interior node can be presented as a leaf, and an odd
//! node is *promoted*, never duplicated — duplication is the classic
//! ambiguity where two different leaf sets share a root.

const LEAF: u8 = 0x00;
const NODE: u8 = 0x01;

use crate::types::RailError;

fn leaf_hash(record_hash: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(&[LEAF]);
    h.update(record_hash);
    *h.finalize().as_bytes()
}

fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(&[NODE]);
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

/// Root over a segment of record hashes. Empty segments have no root — an
/// anchor over nothing is not a statement.
pub fn merkle_root(record_hashes: &[[u8; 32]]) -> Result<[u8; 32], RailError> {
    if record_hashes.is_empty() {
        return Err(RailError::Empty);
    }
    let mut level: Vec<[u8; 32]> = record_hashes.iter().map(leaf_hash).collect();
    while level.len() > 1 {
        level = level
            .chunks(2)
            .map(|pair| match pair {
                [l, r] => node_hash(l, r),
                // Odd node promotes unchanged.
                [one] => *one,
                _ => unreachable!("chunks(2)"),
            })
            .collect();
    }
    Ok(level[0])
}

/// One step of an inclusion proof: the sibling hash and which side it is on.
pub type ProofStep = ([u8; 32], bool); // (sibling, sibling_is_left)

/// Inclusion proof for the leaf at `index`.
pub fn merkle_proof(record_hashes: &[[u8; 32]], index: usize) -> Result<Vec<ProofStep>, RailError> {
    if record_hashes.is_empty() {
        return Err(RailError::Empty);
    }
    if index >= record_hashes.len() {
        return Err(RailError::MerkleIndex);
    }
    let mut level: Vec<[u8; 32]> = record_hashes.iter().map(leaf_hash).collect();
    let mut idx = index;
    let mut proof = Vec::new();
    while level.len() > 1 {
        let sibling = match idx % 2 {
            0 => level.get(idx + 1).map(|s| (*s, false)),
            _ => level.get(idx - 1).map(|s| (*s, true)),
        };
        if let Some(step) = sibling {
            proof.push(step);
        } // else: odd node promoting — no sibling this level.
        level = level
            .chunks(2)
            .map(|pair| match pair {
                [l, r] => node_hash(l, r),
                [one] => *one,
                _ => unreachable!("chunks(2)"),
            })
            .collect();
        idx /= 2;
    }
    Ok(proof)
}

/// Verify that `record_hash` is included under `root` via `proof`.
pub fn merkle_verify(record_hash: &[u8; 32], proof: &[ProofStep], root: &[u8; 32]) -> bool {
    let mut acc = leaf_hash(record_hash);
    for (sibling, sibling_is_left) in proof {
        acc = match sibling_is_left {
            true => node_hash(sibling, &acc),
            false => node_hash(&acc, sibling),
        };
    }
    acc == *root
}
