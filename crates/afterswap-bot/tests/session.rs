//! Command grammar. Every reply here is something a stranger sees on their
//! first try, so bad input must produce a usable sentence, never a default.

use afterswap_bot::session::{Command, Pair, parse};

#[test]
fn bare_verbs_parse() {
    assert_eq!(parse("/start"), Command::Help);
    assert_eq!(parse("/help"), Command::Help);
    assert_eq!(parse("/status"), Command::Status);
    assert_eq!(parse("/stop"), Command::Stop);
    assert_eq!(parse("/proof"), Command::Proof);
}

#[test]
fn group_chat_suffix_is_stripped() {
    assert_eq!(parse("/status@AfterSwapBot"), Command::Status);
}

#[test]
fn non_commands_are_ignored_not_answered() {
    // A bot that replies to every line is unusable in a group.
    assert_eq!(parse("gm"), Command::Ignored);
    assert_eq!(parse(""), Command::Ignored);
    assert_eq!(parse("   "), Command::Ignored);
}

#[test]
fn watch_takes_symbol_and_optional_size() {
    assert_eq!(
        parse("/watch SOL 2.5"),
        Command::Watch {
            pair: Pair::Sol,
            size: 2.5
        }
    );
    assert_eq!(
        parse("/watch sol"),
        Command::Watch {
            pair: Pair::Sol,
            size: Pair::Sol.default_size()
        }
    );
    assert_eq!(
        parse("/watch bonk"),
        Command::Watch {
            pair: Pair::Bonk,
            size: Pair::Bonk.default_size()
        }
    );
}

#[test]
fn unsupported_pair_says_which_pairs_exist() {
    let Command::Rejected(msg) = parse("/watch DOGE") else {
        panic!("expected rejection");
    };
    assert!(msg.contains("SOL") && msg.contains("BONK"), "{msg}");
}

#[test]
fn missing_symbol_is_rejected_with_an_example() {
    let Command::Rejected(msg) = parse("/watch") else {
        panic!("expected rejection");
    };
    assert!(msg.contains("/watch SOL"), "{msg}");
}

/// The regression that matters most: a size that does not parse must not
/// quietly become the default, or the user watches a position they never
/// asked for.
#[test]
fn unparseable_size_is_rejected_not_defaulted() {
    for bad in [
        "/watch SOL abc",
        "/watch SOL -1",
        "/watch SOL 0",
        "/watch SOL nan",
    ] {
        assert!(
            matches!(parse(bad), Command::Rejected(_)),
            "{bad} should be rejected, not defaulted"
        );
    }
}

#[test]
fn unknown_verb_points_at_help() {
    let Command::Rejected(msg) = parse("/moon") else {
        panic!("expected rejection");
    };
    assert!(msg.contains("/help"), "{msg}");
}
