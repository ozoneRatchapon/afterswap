//! DFlow Trading API client.
//!
//! Paper mode needs only `GET /quote` (real prices, simulated fills).
//! Live mode (`live` feature, D4) adds `GET /order` → sign → submit.

pub mod client;
#[cfg(feature = "live")]
pub mod live;
pub mod price;
pub mod snapshot;
pub mod types;

pub use client::{DEV_BASE, DflowClient, DflowError};
#[cfg(feature = "live")]
pub use live::{ConfirmedFill, LiveError, LiveExecutor, parse_confirmed};
pub use price::{PricePoller, mints};
pub use snapshot::{DepthProbe, Freshness, QuoteSnapshot};
pub use types::{OrderResponse, QuoteRequest, QuoteResponse, RoutePlanStep};
