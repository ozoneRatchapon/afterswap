//! Gate: the shipped FSM tables must reconstruct the live enumeration exactly.
//!
//! `fsm_table::decode` is a cache for `FsmEnumerator::enumerate`, and the
//! engine's determinism claims (G1/G6) rest on the two being interchangeable.
//! If `katgpt-ruliology` changes its enumeration order, dedup rule or raw
//! index arithmetic, this fails and the table must be regenerated:
//!
//!   cargo run -p afterswap-engine --example gen_fsm_table --release

use afterswap_engine::fsm_table;
use katgpt_ruliology::{FsmEnumerator, SimpleProgram};

/// Every state count the shipped tables cover.
const TABULATED: [u8; 3] = [1, 2, 3];

#[test]
fn decoded_tables_match_live_enumeration() {
    for n_states in TABULATED {
        let decoded = fsm_table::decode(n_states)
            .unwrap_or_else(|| panic!("no shipped table for n_states={n_states}"));
        let live = FsmEnumerator::enumerate(n_states);

        assert_eq!(
            decoded.len(),
            live.len(),
            "n_states={n_states}: table has {} machines, enumeration produced {}",
            decoded.len(),
            live.len()
        );

        for (i, (got, want)) in decoded.iter().zip(live.iter()).enumerate() {
            // Compared field-by-field rather than via PartialEq, which is
            // id-only: an id collision or a differing residual state would
            // otherwise pass silently.
            assert_eq!(got.transitions(), want.transitions(), "n={n_states} i={i}");
            assert_eq!(got.outputs(), want.outputs(), "n={n_states} i={i}");
            assert_eq!(got.n_states(), want.n_states(), "n={n_states} i={i}");
            assert_eq!(got.state(), want.state(), "n={n_states} i={i}");
            assert_eq!(got.id(), want.id(), "n={n_states} i={i}");
            assert_eq!(
                got.complexity().to_bits(),
                want.complexity().to_bits(),
                "n={n_states} i={i}"
            );
        }
    }
}

/// The production configuration is n=3; its size is a published claim.
#[test]
fn production_table_holds_1054_machines() {
    assert_eq!(fsm_table::decode(3).expect("table for n=3").len(), 1054);
}

/// State counts outside the tables must fall back, not return a wrong set.
#[test]
fn untabulated_state_counts_return_none() {
    assert!(fsm_table::decode(0).is_none());
    assert!(fsm_table::decode(4).is_none());
    assert!(fsm_table::decode(9).is_none());
}
