//! Phrasing. These are claim tests, not formatting tests: the chat voice is
//! the one surface where the project's evidence discipline could silently be
//! dropped, because nobody reads a README on their phone.

use afterswap_bot::phrase::{Verbosity, help, phrase, proof, status, watch_started};
use afterswap_engine::EngineEvent;

fn fill(input: u8, off_peak: bool) -> EngineEvent {
    EngineEvent::TrancheFilled {
        tick: 7,
        arm: 1,
        price: 150.25,
        frac: 0.1,
        remaining: 0.9,
        state: 2,
        input,
        off_peak,
    }
}

#[test]
fn a_fill_says_what_it_saw_what_it_did_and_what_is_left() {
    let line = phrase(&fill(0, false), Verbosity::Quiet, "SOL").expect("fills always render");
    assert!(line.contains("saw a dip"), "{line}");
    assert!(line.contains("sold 10%"), "{line}");
    assert!(line.contains("SOL"), "{line}");
    assert!(line.contains("90% left"), "{line}");
}

#[test]
fn the_off_peak_bit_is_visible_to_the_reader() {
    // It is the input bit that closed the trailing-stop gap (bench 005), so
    // when it fires the user should be able to see that it did.
    let line = phrase(&fill(1, true), Verbosity::Quiet, "SOL").unwrap();
    assert!(line.contains("off its peak"), "{line}");
    let line = phrase(&fill(1, false), Verbosity::Quiet, "SOL").unwrap();
    assert!(!line.contains("off its peak"), "{line}");
}

#[test]
fn quiet_mode_keeps_fills_and_the_exit_and_drops_the_machinery() {
    let kept = [
        fill(0, false),
        EngineEvent::PositionClosed {
            tick: 40,
            final_value_norm: 1.01,
        },
    ];
    for ev in &kept {
        assert!(phrase(ev, Verbosity::Quiet, "SOL").is_some(), "{ev:?}");
    }

    let dropped = [
        EngineEvent::ArmSelected {
            arm: 0,
            fsm_id: 12,
            name: "Eager Puffin".into(),
        },
        EngineEvent::WindowClosed {
            arm: 0,
            reward_bps: 3.0,
            pulls: 2,
        },
        EngineEvent::Tournament {
            route: "sol".into(),
            windows_used: 4,
            strategies: 1054,
            arms: 24,
            compression_ratio: 43.9,
        },
        EngineEvent::Evolved {
            parent_name: "Eager Puffin".into(),
            child_name: "Calm Otter".into(),
            generation: 1,
            sim_edge_bps: 2.0,
        },
    ];
    for ev in &dropped {
        assert!(phrase(ev, Verbosity::Quiet, "SOL").is_none(), "{ev:?}");
        assert!(phrase(ev, Verbosity::Loud, "SOL").is_some(), "{ev:?}");
    }
}

/// A feed that narrates its wins and goes silent on its losses lies by
/// omission. Both directions must render, and the losing one must say so.
#[test]
fn losing_windows_are_reported_as_losses() {
    let lost = EngineEvent::WindowClosed {
        arm: 0,
        reward_bps: -4.2,
        pulls: 3,
    };
    let line = phrase(&lost, Verbosity::Loud, "SOL").unwrap();
    assert!(line.contains("lost to doing nothing"), "{line}");
    assert!(line.contains("4.2"), "{line}");
    // No stray minus in front of an already-negative phrasing.
    assert!(!line.contains("-4.2"), "{line}");
}

#[test]
fn a_negative_exit_is_stated_as_negative() {
    let ev = EngineEvent::PositionClosed {
        tick: 40,
        final_value_norm: 0.994,
    };
    let line = phrase(&ev, Verbosity::Quiet, "SOL").unwrap();
    assert!(line.contains("-60.0 bps"), "{line}");
    assert!(!line.contains("+"), "{line}");
}

/// The voice rule, enforced. The engine has no measured edge; the chat must
/// not imply one.
#[test]
fn no_message_promises_the_reader_money() {
    let banned = [
        "profit",
        "gain",
        "win big",
        "guaranteed",
        "safe",
        "risk-free",
        "moon",
    ];
    let events = [
        fill(0, true),
        fill(1, false),
        EngineEvent::PositionClosed {
            tick: 40,
            final_value_norm: 1.03,
        },
        EngineEvent::WindowClosed {
            arm: 0,
            reward_bps: 9.9,
            pulls: 2,
        },
        EngineEvent::ArmSelected {
            arm: 0,
            fsm_id: 1,
            name: "Eager Puffin".into(),
        },
    ];
    let mut corpus: Vec<String> = vec![
        watch_started("SOL", 1.0, 150.0),
        proof(),
        status(
            "SOL",
            None,
            Some(150.0),
            Some(0.8),
            Some(1.01),
            Some(1.0),
            Some("Eager Puffin"),
        ),
    ];
    for ev in &events {
        corpus.extend(phrase(ev, Verbosity::Loud, "SOL"));
    }
    for text in &corpus {
        // Word-boundary match: "gain" is a substring of "against", which the
        // status line legitimately uses.
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        for banned_phrase in banned {
            let needle: Vec<&str> = banned_phrase
                .split(|c: char| !c.is_ascii_alphanumeric())
                .collect();
            assert!(
                !words.windows(needle.len()).any(|w| w == needle.as_slice()),
                "banned word {banned_phrase:?} in: {text}"
            );
        }
    }
}

/// The onboarding message is the only text most users will read. The honest
/// bound has to survive in it.
#[test]
fn onboarding_states_the_negative_result_and_that_nothing_moves() {
    let text = watch_started("SOL", 1.0, 150.0);
    assert!(text.contains("does not beat holding"), "{text}");
    assert!(text.contains("Paper mode"), "{text}");
    assert!(text.contains("nothing of yours moves"), "{text}");
}

#[test]
fn proof_ends_on_what_could_not_be_proved() {
    let text = proof();
    let claim = text.find("could not prove").expect("must disclose");
    let checkable = text
        .find("can check yourself")
        .expect("must offer evidence");
    assert!(checkable < claim, "evidence first, then the limit");
}

/// The engine's value is only interpretable next to the counterfactual, so it
/// is shown with the hold comparison or not at all.
#[test]
fn status_never_shows_value_without_the_hold_baseline() {
    let with_both = status(
        "SOL",
        None,
        Some(150.0),
        Some(0.7),
        Some(1.02),
        Some(1.0),
        None,
    );
    assert!(
        with_both.contains("Against just holding: +200.0 bps"),
        "{with_both}"
    );

    let missing_baseline = status("SOL", None, Some(150.0), Some(0.7), Some(1.02), None, None);
    assert!(!missing_baseline.contains("holding"), "{missing_baseline}");
    assert!(!missing_baseline.contains("bps"), "{missing_baseline}");
}

#[test]
fn status_with_no_position_tells_you_how_to_start() {
    let text = status("", None, None, None, None, None, None);
    assert!(text.contains("/watch SOL"), "{text}");
}

/// Regression from the first live dry run: `/status` immediately after the
/// plan finished replied "Not watching anything", throwing away the result the
/// user had just been given.
#[test]
fn status_after_a_finished_run_reports_the_result() {
    let text = status("SOL", Some(0.99992), None, None, None, None, None);
    assert!(text.contains("finished"), "{text}");
    assert!(text.contains("-0.8 bps"), "{text}");
    assert!(!text.contains("Not watching"), "{text}");
}

/// Source strings are wrapped across lines; a lost `\` continuation turns
/// source indentation into a visible gap in the chat. Caught in a live dry
/// run once already.
#[test]
fn no_message_contains_collapsed_indentation() {
    let corpus = [
        watch_started("SOL", 1.0, 150.0),
        proof(),
        help(),
        status("SOL", Some(0.99992), None, None, None, None, None),
        status(
            "SOL",
            None,
            Some(150.0),
            Some(0.7),
            Some(1.02),
            Some(1.0),
            Some("Eager Puffin"),
        ),
        status("", None, None, None, None, None, None),
        phrase(&fill(0, true), Verbosity::Quiet, "SOL").unwrap(),
    ];
    for text in &corpus {
        assert!(!text.contains("  "), "double space in: {text:?}");
    }
}
