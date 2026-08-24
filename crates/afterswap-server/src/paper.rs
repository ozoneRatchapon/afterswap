//! Paper-trading loop: live DFlow quotes → exit engine → simulated fills.
//!
//! This is the product's core loop; D3 wraps it in axum + SSE. Fills are
//! applied by the engine at the quoted price (paper semantics); live mode
//! will mirror `TrancheFilled` events into real DFlow orders.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use afterswap_dflow::{DflowClient, PricePoller};
#[cfg(feature = "live")]
use afterswap_dflow::{LiveExecutor, QuoteRequest, mints};
#[cfg(feature = "live")]
use afterswap_engine::EngineEvent;
use afterswap_engine::{EngineConfig, ExitEngine};
use log::{info, warn};
use tokio::sync::{Mutex, broadcast};

use crate::server::{SNAPSHOT_PRICES, SharedEngine};

/// Paper loop settings.
pub struct PaperConfig {
    /// Poll interval per tick.
    pub interval_ms: u64,
    /// Stop after this many ticks (None → run forever).
    pub max_ticks: Option<u64>,
    /// Open the paper position after this many ticks (lets windows fill).
    pub open_after_ticks: u64,
    /// Position size in SOL (paper).
    pub size: f64,
    /// Engine tuning.
    pub engine: EngineConfig,
    /// Replay these recorded prices (looping) instead of polling DFlow.
    pub replay: Option<Vec<f64>>,
    /// Append each live tick as a jsonl line for later replay.
    pub record: Option<PathBuf>,
    /// Live executor: mirror paper tranche fills into real DFlow orders.
    #[cfg(feature = "live")]
    pub live: Option<LiveExecutor>,
}

impl Default for PaperConfig {
    fn default() -> Self {
        Self {
            interval_ms: 2_000,
            max_ticks: None,
            open_after_ticks: 30,
            size: 0.5,
            engine: EngineConfig::default(),
            replay: None,
            record: None,
            #[cfg(feature = "live")]
            live: None,
        }
    }
}

/// Price feed: live DFlow polling, or a recorded loop for deterministic demos.
enum PriceFeed {
    Live(PricePoller),
    Replay { prices: Vec<f64>, idx: usize },
}

impl PriceFeed {
    async fn next(&mut self) -> Result<f64, afterswap_dflow::DflowError> {
        match self {
            Self::Live(poller) => poller.poll().await,
            Self::Replay { prices, idx } => {
                let p = prices[*idx % prices.len()];
                *idx += 1;
                Ok(p)
            }
        }
    }
}

/// Load a `{"price": f}` jsonl recording.
pub fn load_recording(path: &str) -> anyhow::Result<Vec<f64>> {
    let text = std::fs::read_to_string(path)?;
    let prices: Vec<f64> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("price").and_then(|p| p.as_f64()))
        .collect();
    anyhow::ensure!(prices.len() >= 2, "recording too short: {}", prices.len());
    Ok(prices)
}

/// CLI wrapper: fresh engine, no broadcast, run to completion.
pub async fn run(cfg: PaperConfig) -> anyhow::Result<SharedEngine> {
    let engine = Arc::new(Mutex::new(ExitEngine::new(cfg.engine.clone())));
    let (tx, _rx) = broadcast::channel(16);
    run_shared(cfg, engine.clone(), tx).await?;
    Ok(engine)
}

/// Core loop over a shared engine; broadcasts a full snapshot every tick.
pub async fn run_shared(
    mut cfg: PaperConfig,
    shared: SharedEngine,
    snapshots: broadcast::Sender<String>,
) -> anyhow::Result<()> {
    let mut feed = match cfg.replay.take() {
        Some(prices) => {
            info!("REPLAY mode: {} recorded ticks (looping)", prices.len());
            PriceFeed::Replay { prices, idx: 0 }
        }
        None => PriceFeed::Live(PricePoller::sol_usdc(DflowClient::dev())),
    };
    let mut recorder = cfg
        .record
        .as_ref()
        .map(|p| std::fs::OpenOptions::new().create(true).append(true).open(p))
        .transpose()?;
    let mut interval = tokio::time::interval(Duration::from_millis(cfg.interval_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut ticks = 0u64;
    let mut opened = false;

    loop {
        interval.tick().await;
        let price = match feed.next().await {
            Ok(p) => p,
            Err(e) => {
                warn!("quote poll failed (skipping tick): {e}");
                continue;
            }
        };
        if let Some(f) = recorder.as_mut() {
            let _ = writeln!(f, "{}", serde_json::json!({"price": price}));
        }

        let mut engine = shared.lock().await;
        let events = engine.on_tick(price);
        ticks += 1;
        info!("tick {ticks}: SOL/USDC {price:.5}");
        for ev in &events {
            info!("event: {}", serde_json::to_string(ev)?);
            #[cfg(feature = "live")]
            if let (Some(exec), EngineEvent::TrancheFilled { frac, .. }) = (&cfg.live, ev) {
                let lamports = (frac * cfg.size * 1e9) as u64;
                let req = QuoteRequest {
                    input_mint: mints::SOL.to_string(),
                    output_mint: mints::USDC.to_string(),
                    amount: lamports,
                    slippage_bps: 50,
                };
                match exec.sell(&req).await {
                    Ok(sig) => info!("LIVE tranche sold: {lamports} lamports, sig {sig}"),
                    Err(e) => warn!("LIVE tranche failed (paper state unchanged): {e}"),
                }
            }
        }
        #[cfg(not(feature = "live"))]
        let _ = &events;

        if !opened && cfg.open_after_ticks > 0 && ticks >= cfg.open_after_ticks {
            match engine.open_position(cfg.size) {
                Some(pos) => {
                    opened = true;
                    info!(
                        "PAPER position opened: {} SOL @ {:.5} USDC",
                        pos.size, pos.entry_price
                    );
                }
                None => warn!("could not open position (no price yet)"),
            }
        }

        let _ = snapshots.send(serde_json::to_string(&engine.snapshot(SNAPSHOT_PRICES))?);
        drop(engine);

        match cfg.max_ticks {
            Some(max) if ticks >= max => break,
            _ => {}
        }
    }

    let engine = shared.lock().await;
    let snap = engine.snapshot(16);
    info!(
        "final: tick={:?} arms={} windows={} value_norm={:?} hold_norm={:?}",
        snap.tick,
        snap.arms.len(),
        snap.completed_windows,
        snap.position_value_norm,
        snap.hold_value_norm
    );
    Ok(())
}
