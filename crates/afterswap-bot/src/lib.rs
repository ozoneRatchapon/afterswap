//! AfterSwap Telegram front door.
//!
//! The dashboard answers "what is this engine doing?". This answers "what
//! should I do about my bag?" — same engine, same DFlow quotes, same events,
//! rendered as sentences instead of a state diagram.
//!
//! Split so the parts that decide *what is said* ([`phrase`]) and *what was
//! asked* ([`session`]) are pure and tested, and only [`telegram`] and
//! [`watcher`] touch the network.

pub mod dispatch;
pub mod phrase;
pub mod session;
pub mod telegram;
pub mod watcher;

pub use phrase::Verbosity;
pub use session::{Command, Pair};
pub use telegram::{Incoming, Sink, StdoutSink, Telegram};
pub use watcher::{StatusView, Watch};
