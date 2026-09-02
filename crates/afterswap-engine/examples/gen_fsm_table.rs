//! Regenerate the precomputed behavioural-dedup table consumed by
//! `fsm_table::decode`.
//!
//! `FsmEnumerator::enumerate(n)` is pure and deterministic, but it is not
//! cheap: for n=3 it builds 5,832 raw machines and behaviourally fingerprints
//! each over 2^11 input sequences, blake3-hashing 22,528 bytes per machine.
//! That is ~0.26 s natively and 1-2 s under wasm — right at the Workers free
//! plan's 2,010 ms CPU ceiling, which is why cold `/decide` calls were killed.
//!
//! The *result* is a subset of the raw enumeration, so it compresses to the
//! raw indices that survived. This example emits that index list; the engine
//! replays `enumerate_raw`'s index arithmetic to rebuild the same machines in
//! milliseconds. `tests/fsm_table.rs` gates the two against each other.
//!
//! Run: `cargo run -p afterswap-engine --example gen_fsm_table --release`

use katgpt_ruliology::{FsmEnumerator, FsmStrategy, MAX_STATES};
use std::time::Instant;

/// Invert `FsmEnumerator::enumerate_raw`'s mixed-radix index arithmetic.
fn raw_index(fsm: &FsmStrategy) -> usize {
    let n = fsm.n_states() as usize;
    let configs_per_state = n * n * 2;
    let transitions = fsm.transitions();
    let outputs = fsm.outputs();
    let mut idx = 0usize;
    for s in (0..n).rev() {
        let digit = transitions[s][0] as usize
            + (transitions[s][1] as usize) * n
            + (outputs[s] as usize) * n * n;
        idx = idx * configs_per_state + digit;
    }
    idx
}

fn main() {
    // n=4 is out of reach by construction: 1,048,576 raw machines each
    // fingerprinted over 2^18 sequences. The engine never runs it (config
    // caps at 3) and it would not fit the u16 packing anyway.
    const MAX_TABULATED: u8 = 3;
    for n_states in 1..=MAX_TABULATED {
        let started = Instant::now();
        let distinct = FsmEnumerator::enumerate(n_states);
        let elapsed = started.elapsed();

        let n = n_states as usize;
        let total = usize::pow(n * n * 2, n as u32);
        // Packing: raw index in the low 14 bits, the machine's residual state
        // (left dirty by the fingerprint loop, so reproduced exactly) in the
        // top 2 bits. Only emitted while both fit a u16.
        let fits = total <= (1 << 14) && (MAX_STATES <= 4);
        let bytes: Vec<u8> = distinct
            .iter()
            .flat_map(|fsm| {
                let packed = (raw_index(fsm) as u16) | ((fsm.state() as u16) << 14);
                packed.to_le_bytes()
            })
            .collect();

        println!(
            "n={n_states}: {total} raw -> {} distinct in {elapsed:?} ({} bytes, packable={fits})",
            distinct.len(),
            bytes.len()
        );

        if fits {
            let path = format!("crates/afterswap-engine/src/fsm_table_{n_states}.bin");
            std::fs::write(&path, &bytes).expect("write table");
            println!("  wrote {path}");
        }
    }
}
