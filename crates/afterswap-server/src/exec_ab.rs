//! Paired execution experiment: ~300 cycles measuring net margin against
//! arrival price, with a lag-0 depth control variate.
//!
//! Each cycle is three observations and one order:
//!
//! ```text
//!   arrival quote  ──(decision delay)──▶  submit quote  ──▶  order  ──▶  fill
//!   price, impact_bps, slot, route        price, route        sig       realised
//!   └─ the control variate lives here ─┘
//! ```
//!
//! `impact_bps` comes from the arrival response, so it shares that response's
//! `context_slot` with the arrival price — lag-0 by construction, which is the
//! property bench 038 showed is worth 34.6% and that a separately-sampled depth
//! history is not.
//!
//! **What is held fixed, and why.** Every uncontrolled quantity here is a
//! confound rather than noise, because each moves the outcome systematically:
//!
//! * **Notional** — tip cost in bps is tip lamports over notional, so a varying
//!   clip size silently varies the cost being measured.
//! * **Slippage tolerance** — changes which routes the aggregator will accept.
//! * **Priority fee policy** — the tip is a treatment, not a nuisance; an
//!   adaptive fee makes every cycle a different experiment.
//! * **Route** — recorded at arrival and at submission. A change moves venue,
//!   fee tier and depth profile together, so those cycles are a separate
//!   stratum rather than outliers to average over.
//! * **Decision delay** — fixed, because drift scales with it and drift is a
//!   term in the margin identity.
//!
//! Cycles that fail any of these are recorded and excluded at analysis, never
//! dropped at capture: an exclusion the record does not show is a filter on the
//! sample nobody can audit.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use afterswap_dflow::{DflowClient, PricePoller, QuoteRequest, QuoteSnapshot};
use afterswap_engine::execution::ExecutionCycle;
use log::{info, warn};

/// Experiment settings. Everything here is fixed for the run.
pub struct ExecConfig {
    /// Pair and clip size. Held constant across every cycle.
    pub request: QuoteRequest,
    pub cycles: u64,
    /// Gap between cycles.
    pub interval_ms: u64,
    /// Arrival-to-submission delay. Fixed, because drift scales with it.
    pub decision_delay_ms: u64,
    /// Where cycle records are appended, one JSON object per line.
    pub out: PathBuf,
    /// SOL price in the quote currency, for converting lamports to bps.
    pub sol_price: f64,
    /// Notional per cycle in quote-currency units, for the same conversion.
    pub notional: f64,
    /// Execution backend.
    pub executor: Executor,
    /// When set, take a second quote at this clip size to derive an executable
    /// depth spread.
    ///
    /// Needed because the dev endpoint returns `priceImpactPct: "0"` on every
    /// pair tested, so the lag-0 control variate is present but identically
    /// zero — and a constant covariate has no variance to regress on. The probe
    /// costs a second request and can straddle slots, which is worse than a
    /// same-response reading and is why it is a fallback rather than the
    /// default. `QuoteSnapshot::freshness` reports the gap so the cost is
    /// visible per cycle instead of assumed away.
    pub probe_amount: Option<u64>,
}

/// How a cycle's order is filled.
pub enum Executor {
    /// No order is sent. The fill price is the submission quote, so the record
    /// exercises the whole pipeline while measuring nothing about execution.
    /// Rows are flagged `simulated`.
    Paper,
    #[cfg(feature = "live")]
    /// Real orders, real fills, real fees.
    Live(Box<afterswap_dflow::LiveExecutor>),
}

fn route_of(s: &QuoteSnapshot) -> String {
    format!("{}|{}", s.venue.as_deref().unwrap_or("?"), s.hops)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Outcome of one order attempt.
struct Filled {
    price: f64,
    slot: Option<u64>,
    fee_lamports: u64,
    signature: Option<String>,
    revert: Option<String>,
    simulated: bool,
}

impl Executor {
    // `req` is only read on the live path; without that feature the paper arm
    // ignores it.
    #[cfg_attr(not(feature = "live"), allow(unused_variables))]
    async fn execute(&self, req: &QuoteRequest, submit: &QuoteSnapshot) -> Option<Filled> {
        match self {
            Self::Paper => Some(Filled {
                // A modelled fill is the quote restated. Recording it as a fill
                // is only safe because `simulated` travels with it.
                price: submit.price,
                slot: submit.context_slot,
                fee_lamports: 0,
                signature: None,
                revert: None,
                simulated: true,
            }),
            #[cfg(feature = "live")]
            Self::Live(exec) => {
                let sig = match exec.sell(req).await {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("order failed: {e}");
                        return None;
                    }
                };
                // Confirmation is not optional. Without it the "fill price" is
                // the quote, and the experiment measures its own input.
                let confirmed = match exec.confirm(&sig).await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("confirm failed for {sig}: {e}");
                        return None;
                    }
                };
                let price = confirmed.effective_price(&req.input_mint, &req.output_mint);
                Some(Filled {
                    price: price.unwrap_or(0.0),
                    slot: Some(confirmed.slot),
                    fee_lamports: confirmed.fee_lamports,
                    signature: Some(sig),
                    revert: confirmed.err,
                    simulated: false,
                })
            }
        }
    }
}

/// Run the experiment, appending one `ExecutionCycle` per line.
///
/// Returns the cycles recorded. Failures at any stage are logged and skipped —
/// a cycle that never produced an arrival quote is an absent observation, not a
/// failed one, and inventing a row for it would corrupt the sequence.
pub async fn run(cfg: ExecConfig) -> anyhow::Result<Vec<ExecutionCycle>> {
    let client = DflowClient::dev();
    let poller = PricePoller::new(client, cfg.request.clone());
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.out)?;
    let mut recorded = Vec::new();

    if matches!(cfg.executor, Executor::Paper) {
        warn!(
            "PAPER mode: fills are modelled from the submission quote. Rows are flagged \
`simulated` and cannot measure execution quality — this verifies the harness only."
        );
    }

    let mut interval = tokio::time::interval(Duration::from_millis(cfg.interval_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    for cycle in 0..cfg.cycles {
        interval.tick().await;

        let arrival = match match cfg.probe_amount {
            Some(amount) => poller.poll_snapshot_probed(cycle, amount).await,
            None => poller.poll_snapshot(cycle).await,
        } {
            Ok(s) => s,
            Err(e) => {
                warn!("cycle {cycle}: arrival quote failed: {e}");
                continue;
            }
        };
        tokio::time::sleep(Duration::from_millis(cfg.decision_delay_ms)).await;
        let submit = match poller.poll_snapshot(cycle).await {
            Ok(s) => s,
            Err(e) => {
                warn!("cycle {cycle}: submission quote failed: {e}");
                continue;
            }
        };

        let filled = cfg.executor.execute(&cfg.request, &submit).await;
        let record = ExecutionCycle {
            cycle,
            t_ms: now_ms(),
            arrival_slot: arrival.context_slot,
            arrival_price: arrival.price,
            // Opting into a probe is an explicit statement that the
            // same-response figure is unusable on this endpoint, so the probe
            // wins when it was asked for. Preferring `impact_bps` here would
            // silently pick `Some(0.0)` — present, useless, and impossible to
            // regress on — over the reading that was paid for.
            arrival_impact_bps: match cfg.probe_amount {
                Some(_) => arrival.probe.as_ref().map(|p| p.depth_bps),
                None => arrival.impact_bps,
            },
            fill_slot: filled.as_ref().and_then(|f| f.slot),
            submit_price: submit.price,
            fill_price: filled.as_ref().map_or(0.0, |f| f.price),
            notional: cfg.notional,
            tip_lamports: 0,
            l1_fee_lamports: filled.as_ref().map_or(0, |f| f.fee_lamports),
            sol_price: cfg.sol_price,
            arrival_route: route_of(&arrival),
            submit_route: route_of(&submit),
            // A landed-but-reverted transaction has not filled.
            filled: filled
                .as_ref()
                .is_some_and(|f| f.price > 0.0 && f.revert.is_none()),
            simulated: filled.as_ref().is_some_and(|f| f.simulated),
            signature: filled.as_ref().and_then(|f| f.signature.clone()),
            revert: filled.as_ref().and_then(|f| f.revert.clone()),
        };

        writeln!(out, "{}", serde_json::to_string(&record)?)?;
        out.flush()?;
        if cycle % 25 == 0 {
            let ok = recorded.iter().filter(|c: &&ExecutionCycle| c.filled).count();
            info!("cycle {cycle}/{}: {ok} filled so far", cfg.cycles);
        }
        recorded.push(record);
    }
    Ok(recorded)
}


/// CLI entry for `--exec-ab`.
///
/// Paper (verifies the harness, no capital):
/// ```sh
/// cargo run -p afterswap-server --release -- --exec-ab \
///     --cycles 300 --interval-ms 4000 --decision-delay-ms 400 \
///     --pair sol --notional 1000 --sol-price 100 \
///     --out data/execution/dryrun.jsonl
/// ```
///
/// Live (real orders — requires the `live` feature and a funded keypair):
/// ```sh
/// cargo run -p afterswap-server --features live --release -- --exec-ab \
///     --cycles 300 --interval-ms 4000 --decision-delay-ms 400 \
///     --pair sol --notional 1000 --sol-price 100 \
///     --keypair ~/.config/solana/id.json --rpc <archival-rpc> \
///     --out data/execution/run_001.jsonl
/// ```
///
/// Then: `cargo run -p afterswap-engine --example cuped_analysis --release --
/// data/execution/run_001.jsonl`
pub async fn run_cli(args: &[String]) -> anyhow::Result<()> {
    fn arg<T: std::str::FromStr>(args: &[String], name: &str) -> Option<T> {
        let idx = args.iter().position(|a| a == name)?;
        args.get(idx + 1)?.parse().ok()
    }

    let pair = arg::<String>(args, "--pair").unwrap_or_else(|| "sol".to_string());
    let amount: u64 = arg(args, "--amount").unwrap_or(1_000_000_000);
    let request = match pair.as_str() {
        "bonk" => QuoteRequest {
            input_mint: afterswap_dflow::mints::BONK.to_string(),
            output_mint: afterswap_dflow::mints::USDC.to_string(),
            amount,
            slippage_bps: arg(args, "--slippage-bps").unwrap_or(100),
        },
        _ => QuoteRequest {
            input_mint: afterswap_dflow::mints::SOL.to_string(),
            output_mint: afterswap_dflow::mints::USDC.to_string(),
            amount,
            slippage_bps: arg(args, "--slippage-bps").unwrap_or(50),
        },
    };

    let out: PathBuf = arg::<String>(args, "--out")
        .unwrap_or_else(|| "data/execution/run.jsonl".to_string())
        .into();
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }

    #[cfg(feature = "live")]
    let executor = match arg::<String>(args, "--keypair") {
        Some(path) => {
            let rpc = arg::<String>(args, "--rpc")
                .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
            let exec = afterswap_dflow::LiveExecutor::from_keypair_file(
                DflowClient::dev(),
                &path,
                rpc,
            )?;
            log::warn!(
                "LIVE execution as {} — {} cycles will send real orders",
                exec.pubkey(),
                arg::<u64>(args, "--cycles").unwrap_or(300)
            );
            Executor::Live(Box::new(exec))
        }
        None => Executor::Paper,
    };
    #[cfg(not(feature = "live"))]
    let executor = Executor::Paper;

    let cfg = ExecConfig {
        request,
        cycles: arg(args, "--cycles").unwrap_or(300),
        interval_ms: arg(args, "--interval-ms").unwrap_or(4_000),
        decision_delay_ms: arg(args, "--decision-delay-ms").unwrap_or(400),
        out: out.clone(),
        sol_price: arg(args, "--sol-price").unwrap_or(100.0),
        notional: arg(args, "--notional").unwrap_or(1_000.0),
        executor,
        probe_amount: arg(args, "--probe-amount"),
    };

    let cycles = run(cfg).await?;
    let filled = cycles.iter().filter(|c| c.filled).count();
    let admissible = cycles.iter().filter(|c| c.admissible(1)).count();
    log::info!(
        "recorded {} cycles to {} — {filled} filled, {admissible} admissible at lag <= 1 slot",
        cycles.len(),
        out.display()
    );
    log::info!(
        "analyse: cargo run -p afterswap-engine --example cuped_analysis --release -- {}",
        out.display()
    );
    Ok(())
}
