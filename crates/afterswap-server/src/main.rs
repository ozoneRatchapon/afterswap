//! AfterSwap server entrypoint.
//!
//! D2: terminal paper-trading loop (live DFlow quotes → engine).
//! D3: axum + SSE dashboard wraps the same loop.
//!
//! Args (all optional): --ticks N --interval-ms M --open-after K
//!                      --size SOL --states S --window W

mod paper;
mod server;
mod shadow;

use std::sync::Arc;

use afterswap_engine::{EngineConfig, ExitEngine};
use paper::PaperConfig;
use tokio::sync::{Mutex, broadcast};

fn arg<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
    let idx = args.iter().position(|a| a == name)?;
    args.get(idx + 1)?.parse().ok()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().collect();

    let mut engine = EngineConfig::default();
    if let Some(s) = arg::<u8>(&args, "--states") {
        engine.n_fsm_states = s;
    }
    if let Some(w) = arg::<usize>(&args, "--window") {
        engine.window_len = w;
        engine.window_stride = (w / 2).max(1);
    }
    if let Some(t) = arg::<f64>(&args, "--tranche") {
        engine.tranche_frac = t.clamp(0.01, 1.0);
    }

    let cfg = PaperConfig {
        interval_ms: arg(&args, "--interval-ms").unwrap_or(2_000),
        max_ticks: arg(&args, "--ticks"),
        open_after_ticks: arg(&args, "--open-after").unwrap_or(30),
        size: arg(&args, "--size").unwrap_or(0.5),
        engine,
        replay: match arg::<String>(&args, "--replay") {
            Some(path) => Some(paper::load_recording(&path)?),
            None => None,
        },
        record: arg::<String>(&args, "--record").map(Into::into),
        paired: arg::<String>(&args, "--paired").map(Into::into),
        #[cfg(feature = "live")]
        live: match arg::<String>(&args, "--keypair") {
            Some(path) => {
                let rpc = arg::<String>(&args, "--rpc")
                    .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
                let exec = afterswap_dflow::LiveExecutor::from_keypair_file(
                    afterswap_dflow::DflowClient::dev(),
                    &path,
                    rpc,
                )?;
                log::info!("LIVE mode: selling real tranches as {}", exec.pubkey());
                Some(exec)
            }
            None => None,
        },
    };

    match arg::<u16>(&args, "--serve") {
        Some(port) => {
            let engine = Arc::new(Mutex::new(ExitEngine::new(cfg.engine.clone())));
            let (tx, _rx) = broadcast::channel(64);
            let state = server::AppState {
                engine: engine.clone(),
                snapshots: tx.clone(),
            };
            // In serve mode the position opens via the dashboard, not a timer.
            let loop_cfg = PaperConfig {
                open_after_ticks: 0,
                max_ticks: None,
                ..cfg
            };
            tokio::select! {
                r = paper::run_shared(loop_cfg, engine, tx) => r?,
                r = server::serve(state, port) => r?,
            }
        }
        None => {
            paper::run(cfg).await?;
        }
    }
    Ok(())
}
