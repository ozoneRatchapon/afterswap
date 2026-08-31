//! Command → effect. Holds the per-chat watch registry.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::phrase::{self, Verbosity};
use crate::session::{Command, parse};
use crate::telegram::{Incoming, Sink};
use crate::watcher::{self, Watch};

/// Live watches, keyed by chat. One watch per chat: a second `/watch` replaces
/// the first rather than running two narrations into the same conversation.
#[derive(Default)]
pub struct Registry {
    watches: Mutex<HashMap<i64, Watch>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `chat_id` currently has a watch running.
    pub async fn is_watching(&self, chat_id: i64) -> bool {
        self.watches.lock().await.contains_key(&chat_id)
    }
}

/// Handle one incoming message end to end.
pub async fn handle<S: Sink + 'static>(
    msg: &Incoming,
    registry: &Registry,
    interval_ms: u64,
    verbosity: Verbosity,
    sink: Arc<S>,
) -> anyhow::Result<()> {
    let reply = match parse(&msg.text) {
        Command::Ignored => return Ok(()),
        Command::Help => phrase::help(),
        Command::Proof => phrase::proof(),
        Command::Rejected(why) => why,

        Command::Status => {
            let watches = registry.watches.lock().await;
            match watches.get(&msg.chat_id) {
                None => phrase::status("", None, None, None, None, None, None),
                Some(w) => {
                    let symbol = w.pair.symbol();
                    let v = w.status().await;
                    phrase::status(
                        symbol,
                        v.finished,
                        v.last_price,
                        v.remaining_frac,
                        v.value_norm,
                        v.hold_norm,
                        v.driver.as_deref(),
                    )
                }
            }
        }

        Command::Stop => {
            let mut watches = registry.watches.lock().await;
            match watches.remove(&msg.chat_id) {
                Some(w) => {
                    w.stop();
                    "Stopped. Nothing of yours moved — this was paper mode.".to_string()
                }
                None => "Nothing to stop. /watch SOL to start one.".to_string(),
            }
        }

        Command::Watch { pair, size } => {
            let mut watches = registry.watches.lock().await;
            // Replace rather than stack: two engines narrating one chat would
            // read as contradictory advice.
            if let Some(old) = watches.remove(&msg.chat_id) {
                old.stop();
            }
            let watch = watcher::spawn(
                msg.chat_id,
                pair,
                size,
                interval_ms,
                verbosity,
                sink.clone(),
            );
            watches.insert(msg.chat_id, watch);
            format!(
                "Getting a {} quote from DFlow — the position opens on the first \
                 price that arrives.",
                pair.symbol()
            )
        }
    };

    sink.send(msg.chat_id, reply).await
}
