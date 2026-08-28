//! Precomputed behavioural-dedup tables for the FSM strategy space.
//!
//! # Why this exists
//!
//! `FsmEnumerator::enumerate(3)` produces 1,054 behaviourally distinct exit
//! machines out of 5,832 raw ones, and it earns that number the expensive
//! way: every raw machine is replayed over 2^11 input sequences and the
//! 22,528-byte trace blake3-hashed. Measured 2026-08-28 at 219.6 ms natively
//! (`--release`), several times that under wasm. The Workers *free* plan caps
//! a request at 2,010 ms of CPU, so a cold `/decide` sat right on the ceiling
//! and roughly half of them were killed mid-enumeration.
//!
//! The enumeration is pure and deterministic, so its *result* can be computed
//! once, at development time, and shipped. Storing the machines themselves
//! would be wasteful: every survivor is an element of the raw enumeration, so
//! the only information the dedup adds is *which raw indices survived, in what
//! order*. That is a u16 each — 2,108 bytes for the whole n=3 space.
//!
//! Reconstruction replays `FsmEnumerator::enumerate_raw`'s mixed-radix index
//! arithmetic per surviving index, which is a handful of divisions plus the
//! blake3-of-10-bytes that `FsmStrategy::new` does for its cached id.
//!
//! # Exactness
//!
//! This is a cache, not an approximation. `tests/fsm_table.rs` asserts the
//! decoded vector is field-for-field identical to a live
//! `FsmEnumerator::enumerate(n)` for every tabulated `n` — including the
//! residual `state()` that enumeration's fingerprint loop leaves behind,
//! which is why the packing carries two extra bits. If `katgpt-ruliology`'s
//! enumeration order, dedup rule or raw indexing ever changes, that test
//! fails and the table is regenerated with
//! `cargo run -p afterswap-engine --example gen_fsm_table --release`.

use katgpt_ruliology::{FsmStrategy, MAX_STATES};

/// Tables are indexed by `n_states`; index 0 is unused.
const TABLES: [&[u8]; 4] = [
    &[],
    include_bytes!("fsm_table_1.bin"),
    include_bytes!("fsm_table_2.bin"),
    include_bytes!("fsm_table_3.bin"),
];

/// Raw index occupies the low 14 bits of each packed `u16`.
const INDEX_MASK: u16 = (1 << 14) - 1;

/// Rebuild the enumerated strategy set for `n_states` from the shipped table.
///
/// Returns `None` when no table covers `n_states`, leaving the caller to fall
/// back to live enumeration rather than silently returning a wrong-sized set.
pub fn decode(n_states: u8) -> Option<Vec<FsmStrategy>> {
    let packed = TABLES.get(n_states as usize).filter(|t| !t.is_empty())?;
    let n = n_states as usize;
    let configs_per_state = n * n * 2;

    Some(
        packed
            .chunks_exact(2)
            .map(|pair| {
                let word = u16::from_le_bytes([pair[0], pair[1]]);
                let mut idx = (word & INDEX_MASK) as usize;
                let state = (word >> 14) as u8;

                let mut transitions = [[0u8; 2]; MAX_STATES];
                let mut outputs = [0u8; MAX_STATES];
                for s in transitions.iter_mut().zip(outputs.iter_mut()).take(n) {
                    let (transition, output) = s;
                    let remainder = idx % configs_per_state;
                    idx /= configs_per_state;
                    transition[0] = (remainder % n) as u8;
                    transition[1] = ((remainder / n) % n) as u8;
                    *output = ((remainder / (n * n)) % 2) as u8;
                }

                FsmStrategy::new(transitions, outputs, n_states, state)
            })
            .collect(),
    )
}
