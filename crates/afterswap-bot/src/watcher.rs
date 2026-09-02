//! One watch = one chat, one pair, one engine.
//!
//! Structurally this is `paper.rs` with the dashboard replaced by a chat: same
//! `PricePoller`, same `ExitEngine`, same `EngineEvent` stream. Nothing about
//! the strategy changes because the user arrived through Telegram, which is
//! the point — the bot is a front door, not a second product.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use afterswap_dflow::{DflowClient, PricePoller};
use afterswap_engine::{EngineConfig, EngineEvent, ExitEngine};
use tokio::sync::Mutex;

use crate::phrase::{self, Verbosity};
use crate::session::Pair;
use crate::telegram::Sink;

/// Engine settings for a chat watch.
///
/// These are the demo's shipped numbers (window 12–24, 10% tranches, 3-state
/// machines), not new ones. Every constant in this project is a hypothesis
/// that has been swept; inventing bot-specific values would put an unswept
/// number in front of users.
pub fn chat_engine_config() -> EngineConfig {
    EngineConfig {
        window_len: 12,
        window_stride: 6,
        tranche_frac: 0.1,
        n_fsm_states: 3,
        ..EngineConfig::default()
    }
}

/// What `/status` reads. Kept as a small snapshot updated per tick so a status
/// reply never has to contend for the engine lock mid-poll.
#[derive(Debug, Clone, Default)]
pub struct StatusView {
    /// Set once the plan has run to completion, to the final normalized value.
    /// A finished watch is not the same as no watch: the user who just read
    /// the exit message and typed `/status` is asking about the run that just
    /// ended, and answering "nothing here" would discard the result.
    pub finished: Option<f64>,
    pub last_price: Option<f64>,
    pub remaining_frac: Option<f64>,
    pub value_norm: Option<f64>,
    pub hold_norm: Option<f64>,
    pub driver: Option<String>,
}

/// Handle to a running watch.
pub struct Watch {
    pub pair: Pair,
    stop: Arc<AtomicBool>,
    view: Arc<Mutex<StatusView>>,
}

impl Watch {
    /// Current view for `/status`.
    pub async fn status(&self) -> StatusView {
        self.view.lock().await.clone()
    }

    /// Ask the loop to finish after its current tick.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Start a watch: poll DFlow, drive the engine, narrate to `sink`.
///
/// Returns as soon as the loop is spawned; the position opens on the first
/// tick that produces a price.
pub fn spawn<S: Sink + 'static>(
    chat_id: i64,
    pair: Pair,
    size: f64,
    interval_ms: u64,
    verbosity: Verbosity,
    sink: Arc<S>,
) -> Watch {
    let stop = Arc::new(AtomicBool::new(false));
    let view = Arc::new(Mutex::new(StatusView::default()));

    let task_stop = stop.clone();
    let task_view = view.clone();
    tokio::spawn(async move {
        let client = DflowClient::dev();
        let poller = match pair {
            Pair::Sol => PricePoller::sol_usdc(client),
            Pair::Bonk => PricePoller::bonk_usdc(client),
        };
        let symbol = pair.symbol();
        let mut engine = ExitEngine::new(chat_engine_config());
        let mut opened = false;
        let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));

        while !task_stop.load(Ordering::Relaxed) {
            ticker.tick().await;
            let price = match poller.poll().await {
                Ok(p) => p,
                // A dropped quote is a gap in the feed, not the end of the
                // watch — the engine is tick-driven and tolerates a miss.
                Err(e) => {
                    log::warn!("chat {chat_id}: quote failed, skipping tick: {e}");
                    continue;
                }
            };

            let events = engine.on_tick(price);

            if !opened && let Some(pos) = engine.open_position(size) {
                let entry = pos.entry_price;
                opened = true;
                let _ = sink
                    .send(chat_id, phrase::watch_started(symbol, size, entry))
                    .await;
            }

            for ev in &events {
                if let Some(line) = phrase::phrase(ev, verbosity, symbol)
                    && let Err(e) = sink.send(chat_id, line).await
                {
                    log::warn!("chat {chat_id}: send failed: {e}");
                }
            }

            let snap = engine.snapshot(0);
            let driver = snap
                .live_arm
                .and_then(|i| snap.arms.iter().find(|a| a.index == i))
                .map(|a| a.name.clone());
            let closed = events.iter().find_map(|e| match e {
                EngineEvent::PositionClosed {
                    final_value_norm, ..
                } => Some(*final_value_norm),
                _ => None,
            });
            *task_view.lock().await = StatusView {
                finished: closed,
                last_price: snap.last_price,
                remaining_frac: snap.position.as_ref().map(|p| p.remaining_frac),
                value_norm: snap.position_value_norm,
                hold_norm: snap.hold_value_norm,
                driver,
            };

            // The plan ran to completion; there is nothing left to sell. The
            // watch stays in the registry so `/status` can still report how it
            // finished.
            if closed.is_some() {
                break;
            }
        }
        log::info!("chat {chat_id}: watch on {} ended", pair.symbol());
    });

    Watch { pair, stop, view }
}
