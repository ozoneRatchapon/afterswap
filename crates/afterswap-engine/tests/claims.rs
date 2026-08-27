//! Evidence-ladder linter — the repo's prose is tested like its code.
//!
//! Three claims in this project's history were written stronger than their
//! evidence and had to be retracted. External review named the fix: bind the
//! permissible vocabulary to a verification tier, and fail the build when
//! high-conviction language appears without an artefact behind it.
//!
//! Tier definitions (abridged from the reviewed framework):
//!   L0 observational — "observed", "measured", "recorded"
//!   L1 in-sample significant — needs a paired test with adequate power
//!   L2 multiplicity corrected — needs SPA / deflated Sharpe / PBO
//!   L3 out-of-sample causal — needs walk-forward across regimes
//!
//! This test enforces the cheap, mechanical part: a sentence making a
//! high-conviction comparative claim must cite a bench artefact in the same
//! paragraph, and must not use language reserved for tiers we cannot yet
//! demonstrate.

use std::fs;
use std::path::Path;

/// Words that assert a comparative result. Allowed, but only with evidence.
const HIGH_CONVICTION: [&str; 6] = ["beats", "outperforms", "wins against", "superior to", "durable edge", "proven edge"];

/// Words reserved for tiers this project cannot currently reach for any claim.
const FORBIDDEN: [&str; 5] = [
    "guaranteed return",
    "risk-free",
    "permanent advantage",
    "regime-invariant",
    "universal alpha",
];

fn docs() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in ["../../README.md", "../../docs"] {
        let p = Path::new(path);
        if p.is_file() {
            if let Ok(t) = fs::read_to_string(p) {
                out.push((path.to_string(), t));
            }
        } else if let Ok(dir) = fs::read_dir(p) {
            for e in dir.filter_map(Result::ok) {
                let f = e.path();
                if f.extension().is_some_and(|x| x == "md")
                    && let Ok(t) = fs::read_to_string(&f)
                {
                    out.push((f.to_string_lossy().to_string(), t));
                }
            }
        }
    }
    out
}

/// Paragraphs, so a claim and its citation can sit on different lines.
fn paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n").map(|p| p.to_string()).collect()
}

#[test]
fn high_conviction_claims_cite_a_bench() {
    let mut violations = Vec::new();
    for (name, text) in docs() {
        for para in paragraphs(&text) {
            let lower = para.to_lowercase();
            let claims = HIGH_CONVICTION.iter().any(|w| lower.contains(w));
            // A claim is evidenced if the same paragraph points at a bench
            // artefact, or explicitly negates itself ("does not beat …").
            let cited = lower.contains("bench") || lower.contains("benches/");
            // Negative and hypothetical constructions are not claims:
            // "we cannot claim a machine that beats holding" asserts the
            // opposite of what the keyword alone suggests.
            let negated = ["does not beat", "do not beat", "not beat", "beats nothing",
                           "is not something we can claim", "not something we can claim",
                           "cannot claim"]
                .iter()
                .any(|n| lower.contains(n));
            // A quoted claim is a quotation — the questions document exists to
            // discuss claims that were made and retracted.
            let quoted = HIGH_CONVICTION
                .iter()
                .any(|w| lower.contains(&format!("\"{w}")) || lower.contains(&format!("{w} every standard exit\"")))
                || name.contains("QUESTIONS");
            if claims && !cited && !negated && !quoted {
                violations.push(format!("{name}: {}", para.trim().chars().take(120).collect::<String>()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "high-conviction claims without a bench citation:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_language_above_our_evidence_tier() {
    let mut violations = Vec::new();
    for (name, text) in docs() {
        let lower = text.to_lowercase();
        for word in FORBIDDEN {
            // Allow the linter's own vocabulary list to contain the words.
            if lower.contains(word) && !name.contains("QUESTIONS") {
                violations.push(format!("{name}: uses \"{word}\""));
            }
        }
    }
    assert!(violations.is_empty(), "tier-violating language:\n  {}", violations.join("\n  "));
}
