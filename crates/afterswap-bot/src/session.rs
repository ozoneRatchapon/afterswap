//! Command grammar. Pure — parsing is separated from Telegram so the whole
//! command surface is testable without a token or a network.

/// Which pair a watch runs on. Deliberately small: the two pairs the project
/// has recorded corpora for. Offering a token we have never quoted would be a
/// promise the evidence does not cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pair {
    Sol,
    Bonk,
}

impl Pair {
    /// Parse a user-typed symbol, case-insensitively.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "SOL" => Some(Self::Sol),
            "BONK" => Some(Self::Bonk),
            _ => None,
        }
    }

    /// Ticker as shown back to the user.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Sol => "SOL",
            Self::Bonk => "BONK",
        }
    }

    /// Default position size when the user omits one.
    pub fn default_size(self) -> f64 {
        match self {
            Self::Sol => 1.0,
            Self::Bonk => 500_000.0,
        }
    }
}

/// A parsed user command.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
    Proof,
    Status,
    Stop,
    Watch {
        pair: Pair,
        size: f64,
    },
    /// Recognised as a command but unusable; the string is shown to the user.
    Rejected(String),
    /// Not a command at all — ignored rather than answered, so the bot stays
    /// usable in a group chat.
    Ignored,
}

/// Parse one message body.
///
/// Accepts the `/cmd@BotName` form Telegram appends in group chats.
pub fn parse(text: &str) -> Command {
    let mut parts = text.split_whitespace();
    let Some(head) = parts.next() else {
        return Command::Ignored;
    };
    if !head.starts_with('/') {
        return Command::Ignored;
    }
    let verb = head
        .trim_start_matches('/')
        .split('@')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    match verb.as_str() {
        "start" | "help" => Command::Help,
        "proof" => Command::Proof,
        "status" => Command::Status,
        "stop" => Command::Stop,
        "watch" => parse_watch(parts.next(), parts.next()),
        _ => Command::Rejected(format!("I don't know /{verb}. Try /help.")),
    }
}

fn parse_watch(symbol: Option<&str>, size: Option<&str>) -> Command {
    let Some(symbol) = symbol else {
        return Command::Rejected(
            "Which token? Try /watch SOL, or /watch SOL 2.5 to set the size.".to_string(),
        );
    };
    let Some(pair) = Pair::parse(symbol) else {
        return Command::Rejected(format!(
            "I only have recorded quote history for SOL and BONK, so those are the \
             two I will watch. \"{symbol}\" is not one of them."
        ));
    };
    let size = match size {
        None => pair.default_size(),
        // A size that does not parse must not silently become the default —
        // the user would be watching a position they did not ask for.
        Some(raw) => match raw.parse::<f64>() {
            Ok(v) if v.is_finite() && v > 0.0 => v,
            _ => {
                return Command::Rejected(format!(
                    "\"{raw}\" is not a size I can use. Try /watch {} 1.0.",
                    pair.symbol()
                ));
            }
        },
    };
    Command::Watch { pair, size }
}
