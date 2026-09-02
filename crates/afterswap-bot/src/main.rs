//! Bot entrypoint.
//!
//! `--dry-run` runs the whole thing against stdin/stdout: no token, no
//! BotFather, no network except DFlow. That path exists so the demo is
//! reproducible by someone who has neither our token nor a Telegram account.

use std::sync::Arc;

use afterswap_bot::dispatch::{Registry, handle};
use afterswap_bot::phrase::Verbosity;
use afterswap_bot::telegram::{Incoming, StdoutSink, Telegram};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let verbosity = match args.iter().any(|a| a == "--loud") {
        true => Verbosity::Loud,
        false => Verbosity::Quiet,
    };
    let interval_ms = flag(&args, "--interval-ms").unwrap_or(2_000);

    let registry = Registry::new();

    match dry_run {
        true => run_stdin(registry, interval_ms, verbosity).await,
        false => run_telegram(registry, interval_ms, verbosity).await,
    }
}

fn flag(args: &[String], name: &str) -> Option<u64> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1)?.parse().ok()
}

/// Dry run: read commands from stdin, print replies.
async fn run_stdin(
    registry: Registry,
    interval_ms: u64,
    verbosity: Verbosity,
) -> anyhow::Result<()> {
    let sink = Arc::new(StdoutSink);
    println!("AfterSwap bot — dry run. Type commands (e.g. /watch SOL 1.0), Ctrl-D to quit.\n");
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::AsyncBufReadExt::lines(tokio::io::BufReader::new(stdin));
    while let Some(line) = lines.next_line().await? {
        let msg = Incoming {
            chat_id: 0,
            text: line,
        };
        handle(&msg, &registry, interval_ms, verbosity, sink.clone()).await?;
    }
    Ok(())
}

/// Live: long-poll Telegram.
async fn run_telegram(
    registry: Registry,
    interval_ms: u64,
    verbosity: Verbosity,
) -> anyhow::Result<()> {
    let token = std::env::var("TELEGRAM_BOT_TOKEN").map_err(|_| {
        anyhow::anyhow!(
            "TELEGRAM_BOT_TOKEN is not set. Get one from @BotFather, or run with \
             --dry-run to drive the same loop from stdin."
        )
    })?;
    let bot = Arc::new(Telegram::new(&token));
    log::info!("polling telegram (interval {interval_ms}ms per watch tick)");
    loop {
        match bot.poll(30).await {
            Ok(messages) => {
                for msg in messages {
                    if let Err(e) =
                        handle(&msg, &registry, interval_ms, verbosity, bot.clone()).await
                    {
                        log::warn!("chat {}: handler failed: {e}", msg.chat_id);
                    }
                }
            }
            // Telegram outages are transient; backing off beats exiting and
            // dropping every running watch.
            Err(e) => {
                log::warn!("getUpdates failed, retrying in 5s: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}
