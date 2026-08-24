//! DFlow Trading API client.
//!
//! Paper mode needs only `GET /quote` (real prices, simulated fills).
//! Live mode (`live` feature, D4) adds `GET /order` → sign → submit.

pub mod client;
#[cfg(feature = "live")]
pub mod live;
pub mod price;
pub mod types;

pub use client::{DEV_BASE, DflowClient, DflowError};
#[cfg(feature = "live")]
pub use live::{LiveExecutor, LiveError};
pub use price::{PricePoller, mints};
pub use types::{OrderResponse, QuoteRequest, QuoteResponse, RoutePlanStep};
