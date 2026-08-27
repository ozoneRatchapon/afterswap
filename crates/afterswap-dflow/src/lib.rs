//! DFlow Trading API client.
//!
//! Paper mode needs only `GET /quote` (real prices, simulated fills).
//! Live mode (`live` feature, D4) adds `GET /order` → sign → submit.

pub mod client;
pub mod confirm;
#[cfg(feature = "live")]
pub mod live;
pub mod price;
pub mod snapshot;
pub mod types;
pub mod venues;

pub use client::{DEV_BASE, DflowClient, DflowError};
pub use confirm::{ConfirmedFill, parse_confirmed};
#[cfg(feature = "live")]
pub use live::{LiveError, LiveExecutor};
pub use price::{PricePoller, mints};
pub use snapshot::{DepthProbe, Freshness, QuoteSnapshot};
pub use types::{OrderResponse, QuoteRequest, QuoteResponse, RoutePlanStep};
pub use venues::{CapturedQuote, capture_dflow, capture_jupiter, parse_jupiter};
