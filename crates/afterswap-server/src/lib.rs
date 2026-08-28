//! AfterSwap server library — the CLI modes behind the `afterswap-server` binary.
//!
//! `main.rs` stays dispatch-only. Everything with logic worth asserting on
//! lives here so integration tests under `tests/` can reach it; a binary-only
//! crate would force those assertions into inline `#[cfg(test)]` blocks.

pub mod anchor;
pub mod exec_ab;
pub mod paper;
pub mod rail_ship;
pub mod server;
pub mod shadow;
