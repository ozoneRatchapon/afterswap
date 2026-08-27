//! AfterSwap engine — "what happens after the swap".
//!
//! Exhaustively enumerated FSM exit strategies (via `katgpt-ruliology`)
//! compete as UCB1 bandit arms over rolling windows of live DFlow quotes.
//! The winning machine drives tranche exits on the open position.
//!
//! Pure algorithms, no IO — the server layer feeds ticks in and executes
//! the `EngineEvent`s that come out.

pub mod bandit;
pub mod cuped;
pub mod engine;
pub mod execution;
pub mod pbo;
pub mod power;
pub mod prereg;
pub mod stepdown;
pub mod rating;
pub mod sim;
pub mod types;
pub mod windows;

pub use bandit::ExitBandit;
pub use engine::{EngineEvent, ExitEngine};
pub use types::{EngineConfig, Position, TrancheFill};
pub use windows::WindowStore;
