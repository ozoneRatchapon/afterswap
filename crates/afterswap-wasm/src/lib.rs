//! WASM bindings: the exit engine running client-side. The dashboard feeds
//! DFlow quotes in (browser fetch — the dev API allows CORS) and renders
//! the same `EngineSnapshot` JSON the server build emits.

use afterswap_engine::{EngineConfig, ExitEngine};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmEngine {
    inner: ExitEngine,
}

#[wasm_bindgen]
impl WasmEngine {
    /// Build an engine with demo-relevant knobs; everything else defaults.
    #[wasm_bindgen(constructor)]
    pub fn new(window_len: usize, n_states: u8, tranche_frac: f64, max_arms: usize) -> WasmEngine {
        let config = EngineConfig {
            window_len,
            window_stride: (window_len / 2).max(1),
            n_fsm_states: n_states,
            tranche_frac,
            max_arms,
            ..EngineConfig::default()
        };
        WasmEngine {
            inner: ExitEngine::new(config),
        }
    }

    /// Feed one price tick; returns the events as a JSON array string.
    pub fn on_tick(&mut self, price: f64) -> String {
        let events = self.inner.on_tick(price);
        serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_string())
    }

    /// Open a paper position; returns the entry price or NaN.
    pub fn open_position(&mut self, size: f64) -> f64 {
        self.inner
            .open_position(size)
            .map_or(f64::NAN, |p| p.entry_price)
    }

    /// Force-close; returns final value (× entry) or NaN.
    pub fn close_position(&mut self) -> f64 {
        self.inner.close_position().unwrap_or(f64::NAN)
    }

    /// Full dashboard snapshot as JSON (same shape as the server's SSE).
    pub fn snapshot(&self, n_prices: usize) -> String {
        serde_json::to_string(&self.inner.snapshot(n_prices)).unwrap_or_else(|_| "{}".to_string())
    }
}

/// GOAT G6 (wasm parity): run the exact `sim::simulate` used by the native
/// gates and return its JSON — compared byte-for-byte against the native
/// output by `tests/goat.rs` + the parity page.
#[wasm_bindgen]
pub fn parity_run(prices_json: &str, open_at: usize) -> String {
    let prices: Vec<f64> = serde_json::from_str(prices_json).unwrap_or_default();
    let config = EngineConfig {
        window_len: 12,
        window_stride: 6,
        n_fsm_states: 3,
        tranche_frac: 0.1,
        max_arms: 24,
        ..EngineConfig::default()
    };
    let result = afterswap_engine::sim::simulate(config, &prices, open_at, 1.0);
    serde_json::to_string(&result).unwrap_or_default()
}
