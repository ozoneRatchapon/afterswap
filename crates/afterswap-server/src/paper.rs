//! Paper-trading loop: live DFlow quotes → exit engine → simulated fills.
//!
//! This is the product's core loop; D3 wraps it in axum + SSE. Fills are
//! applied by the engine at the quoted price (paper semantics); live mode
//! will mirror `TrancheFilled` events into real DFlow orders.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use afterswap_dflow::{DflowClient, PricePoller, QuoteSnapshot};
#[cfg(feature = "live")]
use afterswap_dflow::{LiveExecutor, QuoteRequest, mints};
use afterswap_engine::{EngineConfig, EngineEvent, ExitEngine};

use crate::shadow::Shadow;
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
    /// Append one paired-comparison line per completed position cycle.
    pub paired: Option<PathBuf>,
    /// Quote pair to soak: "sol" (default) or "bonk".
    pub pair: String,
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
            paired: None,
            pair: "sol".to_string(),
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
    /// One tick as a full snapshot. Replay has no depth reading to offer, so it
    /// yields `None` — a replayed row must never look like a captured one, or
    /// the control-variate column would silently contain nothing on half the
    /// corpus.
    async fn next(&mut self, seq: u64) -> Result<(f64, Option<QuoteSnapshot>), afterswap_dflow::DflowError> {
        match self {
            Self::Live(poller) => {
                let snap = poller.poll_snapshot(seq).await?;
                Ok((snap.price, Some(snap)))
            }
            Self::Replay { prices, idx } => {
                let p = prices[*idx % prices.len()];
                *idx += 1;
                Ok((p, None))
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
        None => {
            info!("live feed: {} / USDC", cfg.pair);
            PriceFeed::Live(match cfg.pair.as_str() {
                "bonk" => PricePoller::bonk_usdc(DflowClient::dev()),
                _ => PricePoller::sol_usdc(DflowClient::dev()),
            })
        }
    };
    let mut recorder = cfg
        .record
        .as_ref()
        .map(|p| std::fs::OpenOptions::new().create(true).append(true).open(p))
        .transpose()?;
    let mut paired_writer = cfg
        .paired
        .as_ref()
        .map(|p| std::fs::OpenOptions::new().create(true).append(true).open(p))
        .transpose()?;
    // Reference exits driven by the same ticks from the same entry.
    let mut shadow: Option<Shadow> = None;
    let mut interval = tokio::time::interval(Duration::from_millis(cfg.interval_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut ticks = 0u64;
    let mut opened = false;
    // Median-of-3 spike filter — a lone bad quote must not reach the
    // engine (corrupts fills and machine rewards). Sustained moves pass.
    let mut raw_tail: Vec<f64> = Vec::with_capacity(3);

    loop {
        interval.tick().await;
        let (raw, snapshot) = match feed.next(ticks).await {
            Ok(p) => p,
            Err(e) => {
                warn!("quote poll failed (skipping tick): {e}");
                continue;
            }
        };
        raw_tail.push(raw);
        if raw_tail.len() > 3 {
            raw_tail.remove(0);
        }
        let price = match raw_tail.len() {
            3 => {
                let mut sorted = raw_tail.clone();
                sorted.sort_by(f64::total_cmp);
                sorted[1]
            }
            _ => raw,
        };
        if let Some(f) = recorder.as_mut() {
            // Record the snapshot when we have one, so the depth reading lands
            // in the same row as the price it shares a slot with. `price_used`
            // is the post-filter value the engine consumed — it differs from
            // `price` whenever the median-of-3 filter fires, and that
            // difference is a one-tick lag the analysis has to know about
            // rather than infer.
            let line = match &snapshot {
                Some(snap) => serde_json::to_string(&snap.clone().with_price_used(price)),
                None => serde_json::to_string(&serde_json::json!({"price": price})),
            };
            if let Ok(line) = line {
                let _ = writeln!(f, "{line}");
            }
        }

        let mut engine = shared.lock().await;
        // Shadow tracks the same ticks as the engine, from the same entry.
        if shadow.is_none()
            && let Some(pos) = engine.position()
        {
            shadow = Some(Shadow::new(pos.entry_price));
        }
        if let Some(sh) = shadow.as_mut() {
            sh.on_tick(price);
        }
        let events = engine.on_tick(price);
        if let Some(EngineEvent::PositionClosed {
            final_value_norm, ..
        }) = events
            .iter()
            .find(|e| matches!(e, EngineEvent::PositionClosed { .. }))
            && let Some(sh) = shadow.take()
        {
            {
                let paired = sh.compare(*final_value_norm, price);
                info!(
                    "paired ({} ticks): vs hold {:+.2} | vs twap {:+.2} | vs trailing {:+.2} | vs ladder {:+.2} | vs bracket {:+.2} bps",
                    paired.ticks,
                    paired.vs_hold_bps,
                    paired.vs_twap_bps,
                    paired.vs_trailing_bps,
                    paired.vs_ladder_bps,
                    paired.vs_bracket_bps
                );
                if let Some(w) = paired_writer.as_mut() {
                    let _ = writeln!(w, "{}", serde_json::to_string(&paired)?);
                }
            }
        }
        ticks += 1;
        // Pair label and precision follow the feed: a memecoin quote is
        // ~3e-6 USDC and rounds to zero at 5 decimals.
        info!("tick {ticks}: {}/USDC {price:.9}", cfg.pair.to_uppercase());
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
