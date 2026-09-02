//! `EngineEvent` → one plain sentence.
//!
//! The dashboard explains itself with a state diagram, a leaderboard and a
//! gate meter. A chat message has one line and a reader who has never seen
//! the word "bandit". This module is the whole difference, so it is pure and
//! tested rather than formatted inline at the call site.
//!
//! Voice rules, inherited from the project's claim discipline:
//!
//! - Describe what happened, never what it means for the reader's wealth.
//!   "sold 10%" is allowed; "locked in profit" is not, because the engine has
//!   no measured edge and the chat voice must not acquire one.
//! - Every number that appears here is one the dashboard already shows.
//! - Losing windows are reported in the same tone as winning ones.

use afterswap_engine::EngineEvent;

/// How chatty the feed is. A watcher who wanted an exit alert does not want
/// one message per internal tournament.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    /// Fills and the final exit only — what a normal user asked for.
    Quiet,
    /// Everything the dashboard's activity feed shows.
    Loud,
}

/// Render one event, or `None` when this verbosity drops it.
pub fn phrase(ev: &EngineEvent, v: Verbosity, symbol: &str) -> Option<String> {
    match (ev, v) {
        (
            EngineEvent::TrancheFilled {
                price,
                frac,
                remaining,
                state,
                input,
                off_peak,
                ..
            },
            _,
        ) => {
            let saw = match (input, off_peak) {
                (0, true) => "saw a dip, and price is off its peak".to_string(),
                (0, false) => "saw a dip".to_string(),
                (_, true) => "saw a tick up, but price is off its peak".to_string(),
                (_, false) => "saw a tick up".to_string(),
            };
            Some(format!(
                "{saw} → sell-state S{state} → sold {sold:.0}% of your {symbol} at {price:.6}. {left:.0}% left.",
                sold = frac * 100.0,
                left = remaining * 100.0,
            ))
        }

        (
            EngineEvent::PositionClosed {
                final_value_norm, ..
            },
            _,
        ) => {
            let bps = (final_value_norm - 1.0) * 10_000.0;
            let vs = match bps >= 0.0 {
                true => format!("+{bps:.1} bps versus your entry price"),
                false => format!("{bps:.1} bps versus your entry price"),
            };
            Some(format!(
                "Position fully exited. The plan finished at {vs}. /proof for the receipt."
            ))
        }

        (EngineEvent::ArmSelected { name, .. }, Verbosity::Loud) => {
            Some(format!("\"{name}\" is now driving your exit."))
        }

        (EngineEvent::WindowClosed { reward_bps, .. }, Verbosity::Loud) => {
            // Reported in both directions on purpose. A feed that goes quiet
            // when the machine loses is a feed that lies by omission.
            let verdict = match reward_bps >= &0.0 {
                true => format!("beat doing nothing by {reward_bps:.1} bps"),
                false => format!("lost to doing nothing by {:.1} bps", -reward_bps),
            };
            Some(format!(
                "Scored the last window: the driving machine {verdict}."
            ))
        }

        (
            EngineEvent::Evolved {
                parent_name,
                child_name,
                generation,
                ..
            },
            Verbosity::Loud,
        ) => Some(format!(
            "\"{child_name}\" (gen {generation}, bred from \"{parent_name}\") won a seat."
        )),

        (
            EngineEvent::Tournament {
                strategies, arms, ..
            },
            Verbosity::Loud,
        ) => Some(format!(
            "Re-ran the tournament: {strategies} exit machines auditioned, {arms} kept."
        )),

        _ => None,
    }
}

/// The message sent when a watch starts. States the honest bound up front —
/// the same sentence the README leads its evidence section with — because a
/// user who found this in a chat will never read the README.
pub fn watch_started(symbol: &str, size: f64, entry: f64) -> String {
    format!(
        "Watching {size} {symbol} from {entry:.6}.\n\n\
         From here I do the selling to a fixed rule, tick by tick, and tell you \
         every time I act. Paper mode: quotes are real DFlow prices, fills are \
         simulated — nothing of yours moves.\n\n\
         What this is not: an edge. We tested it hard enough to disprove our own \
         backtest, and it does not beat holding. What it does is exit on a plan \
         instead of on a mood, and leave a receipt.\n\n\
         /status any time. /stop to end it."
    )
}

/// Reply to `/status`: position, plan progress, and the hold counterfactual.
pub fn status(
    symbol: &str,
    finished: Option<f64>,
    last_price: Option<f64>,
    remaining_frac: Option<f64>,
    value_norm: Option<f64>,
    hold_norm: Option<f64>,
    driver: Option<&str>,
) -> String {
    // A completed run answers first: the position is gone, but the result of
    // it is what the user is asking about.
    if let Some(final_value_norm) = finished {
        let bps = (final_value_norm - 1.0) * 10_000.0;
        let sign = match bps >= 0.0 {
            true => "+",
            false => "",
        };
        return format!(
            "That {symbol} plan is finished — fully exited at {sign}{bps:.1} bps versus \
             your entry price.\n/proof for what is checkable. /watch to start another."
        );
    }
    let Some(remaining) = remaining_frac else {
        return "Not watching anything right now. /watch SOL to start.".to_string();
    };
    let price = match last_price {
        Some(p) => format!("{symbol} is at {p:.6}."),
        None => format!("No {symbol} quote yet."),
    };
    let plan = format!("{:.0}% of the position still held.", remaining * 100.0);
    let driving = match driver {
        Some(name) => format!("\"{name}\" is driving."),
        None => "No machine seated yet — still filling the first window.".to_string(),
    };
    // Both numbers or neither: the engine's value is only interpretable next
    // to the counterfactual, and showing it alone is how a demo flatters
    // itself.
    let versus = match (value_norm, hold_norm) {
        (Some(v), Some(h)) => {
            let d = (v - h) * 10_000.0;
            match d >= 0.0 {
                true => format!("\nAgainst just holding: +{d:.1} bps."),
                false => format!("\nAgainst just holding: {d:.1} bps."),
            }
        }
        _ => String::new(),
    };
    format!("{price}\n{plan}\n{driving}{versus}")
}

/// `/help` and `/start` body.
pub fn help() -> String {
    "AfterSwap — what happens after the swap.\n\n\
     You swapped into a token. Now something has to decide when to sell it. \
     Normally that is you, at 3am, in a mood. Here it is a rule that was fixed \
     before you were in the trade, and it reports what it did.\n\n\
     /watch SOL 1.0 — watch 1.0 SOL from the current price\n\
     /watch BONK 500000 — same for BONK\n\
     /status — where the position stands, next to just holding\n\
     /proof — what is checkable, and what we could not prove\n\
     /stop — end the watch\n\n\
     Paper mode: real DFlow quotes, simulated fills. Nothing of yours moves."
        .to_string()
}

/// `/proof`. Ends on the negative result on purpose — it is the most
/// unusual true thing the project can say, and burying it would be the
/// dishonesty the rest of the harness exists to prevent.
pub fn proof() -> String {
    "What you can check yourself:\n\n\
     • The exit rule is committed on Solana before the first sale, so \"it \
     followed the plan\" is verifiable rather than claimed.\n\
     • Every price came from a DFlow quote the venue signed; the signature is \
     verified in-browser on the dashboard.\n\
     • Replays are bit-reproducible, and the browser and native engines agree \
     byte for byte.\n\n\
     What we could not prove: that any of this makes you money. We enumerated \
     every 3-state exit machine, ran the tournament, and then built a harness \
     strong enough to catch our own result — it says no machine survives \
     correcting for having looked at a thousand candidates. We publish that \
     instead of the green backtest.\n\n\
     Full evidence: https://github.com/ozoneRatchapon/afterswap"
        .to_string()
}
