//! Claim decay — a result is a measurement with a date, not a fact.
//!
//! Markets are non-stationary, so a number measured in August is not a
//! property of the system in November. External review prescribed binding each
//! documented claim to the bench that produced it and expiring it on a TTL.
//!
//! This test enforces the mechanical half: every benchmark referenced from the
//! docs must exist, and every bench directory must be reachable from the docs.
//! A claim whose evidence has been deleted fails the build; evidence nobody
//! cites is flagged so it is either used or removed deliberately.

use std::collections::HashSet;
use std::fs;

fn doc_files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for p in ["../../README.md"] {
        if let Ok(t) = fs::read_to_string(p) {
            out.push((p.to_string(), t));
        }
    }
    if let Ok(dir) = fs::read_dir("../../docs") {
        for e in dir.filter_map(Result::ok) {
            let f = e.path();
            if f.extension().is_some_and(|x| x == "md")
                && let Ok(t) = fs::read_to_string(&f)
            {
                out.push((f.to_string_lossy().to_string(), t));
            }
        }
    }
    out
}

fn existing_benches() -> HashSet<String> {
    fs::read_dir("../../benches")
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_cited_bench_exists() {
    let benches = existing_benches();
    let mut missing = Vec::new();
    for (name, text) in doc_files() {
        // Plan files are numbered the same way benches are
        // (`.plans/001_execution_edge.md`), so a citation of the
        // pre-registration would otherwise be read as a citation of a bench
        // that never existed. Drop those paths before tokenizing.
        let text: String = text
            .split_whitespace()
            .filter(|w| !w.contains(".plans/"))
            .collect::<Vec<_>>()
            .join(" ");
        for token in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            // Bench directories are numbered: 016_goat, 025_multiplicity, …
            let looks_like_bench = token.len() > 4
                && token.chars().take(3).all(|c| c.is_ascii_digit())
                && token.chars().nth(3) == Some('_');
            if looks_like_bench && !benches.contains(token) {
                missing.push(format!("{name} cites missing bench `{token}`"));
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "documented claims whose evidence is gone:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn evidence_is_not_orphaned() {
    let benches = existing_benches();
    let all_docs: String = doc_files().into_iter().map(|(_, t)| t).collect();
    let orphans: Vec<&String> = benches
        .iter()
        .filter(|b| !all_docs.contains(b.as_str()))
        .collect();
    // Orphans are allowed — superseded runs are kept for provenance — but the
    // count is capped so the directory cannot silently fill with unused runs.
    assert!(
        orphans.len() <= 12,
        "{} bench directories are cited nowhere: {:?}",
        orphans.len(),
        orphans
    );
}
