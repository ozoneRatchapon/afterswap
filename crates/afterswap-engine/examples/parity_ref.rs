//! Dump the native `simulate()` JSON for the recorded corpus — the G6
//! reference the wasm build must match byte-for-byte.

use afterswap_engine::EngineConfig;
use afterswap_engine::sim::{load_corpus, simulate};

fn main() {
    let prices = load_corpus("data/recorded.jsonl").expect("corpus");
    let config = EngineConfig {
        window_len: 12,
        window_stride: 6,
        n_fsm_states: 3,
        tranche_frac: 0.1,
        max_arms: 24,
        ..EngineConfig::default()
    };
    let result = simulate(config, &prices, 30, 1.0);
    println!("{}", serde_json::to_string(&result).expect("serialize"));
}
