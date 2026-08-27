//! Verify a rail log the way an auditor would: chain, attestations, decision
//! reproduction, evidence digests — plus the R1 falsifier, the cross-venue
//! slot gap on real ticks.
//!
//! ```sh
//! cargo run -p afterswap-rail --example rail_verify --release -- \
//!     data/rail/sol_usdc.jsonl --attest-pubkey-hex <64 hex>
//! # dev runs: --attest-seed-hex a5a5…  derives the pubkey for you
//! ```
//!
//! Exits non-zero on any verification failure, so this gates CI the same way
//! `fetch_tx` gates live runs.

use afterswap_rail::{AttestKey, AuditRecord, QuoteEvidence, verify_chain, verify_record};

fn arg(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        eprintln!("usage: rail_verify <LOG.jsonl> (--attest-pubkey-hex H | --attest-seed-hex H)");
        std::process::exit(2);
    };
    let public = match (arg(&args, "--attest-pubkey-hex"), arg(&args, "--attest-seed-hex")) {
        (Some(h), _) => hex32(&h),
        (None, Some(h)) => hex32(&h).map(|seed| AttestKey::from_seed(seed).public()),
        (None, None) => Some(AttestKey::from_seed([0xA5; 32]).public()), // dev default
    };
    let Some(public) = public else {
        eprintln!("bad key hex");
        std::process::exit(2);
    };

    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("cannot read {path}");
        std::process::exit(1);
    };
    let records: Vec<AuditRecord> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if records.is_empty() {
        eprintln!("no records parsed from {path}");
        std::process::exit(1);
    }

    let mut failures = 0usize;

    // §3.4 step 4: the chain.
    match verify_chain(&records, None) {
        Ok(report) => {
            println!("chain    : {} records, {} gap(s) {:?}", report.records, report.gaps.len(), report.gaps);
        }
        Err(e) => {
            println!("chain    : BROKEN — {e}");
            failures += 1;
        }
    }

    // §3.4 steps 1 and 6, per record.
    let mut bad = 0usize;
    for r in &records {
        if let Err(e) = verify_record(r, &public) {
            println!("record {} : FAIL — {e}", r.seq);
            bad += 1;
        }
    }
    println!("records  : {} verified, {bad} failed", records.len() - bad);
    failures += bad;

    // Evidence census + the R1 falsifier: cross-venue slot gaps on real ticks.
    let (mut signed, mut observed) = (0usize, 0usize);
    let mut gaps: Vec<u64> = Vec::new();
    let mut shadowless = 0usize;
    let mut chosen: std::collections::BTreeMap<&str, usize> = Default::default();
    for r in &records {
        for q in &r.quotes {
            match &q.evidence {
                QuoteEvidence::ProviderSigned { .. } => signed += 1,
                QuoteEvidence::Observed { .. } => observed += 1,
            }
        }
        *chosen.entry(r.decision.chosen_venue.as_str()).or_insert(0) += 1;
        let slots: Vec<u64> = r.quotes.iter().filter_map(|q| q.context_slot).collect();
        match slots.len() {
            2.. => gaps.push(slots.iter().max().unwrap_or(&0) - slots.iter().min().unwrap_or(&0)),
            _ => shadowless += 1,
        }
    }
    println!("evidence : {signed} provider-signed, {observed} observed");
    println!("decisions: {chosen:?}");
    gaps.sort_unstable();
    match gaps.is_empty() {
        true => println!("slot gaps: none measurable ({shadowless} record(s) without both slots)"),
        false => {
            let pct = |p: f64| gaps[((p * (gaps.len() - 1) as f64) as usize).min(gaps.len() - 1)];
            let within = gaps.iter().filter(|g| **g <= 2).count();
            println!(
                "slot gaps: n={} median={} p90={} max={} — {}/{} within the 2-slot bound ({shadowless} shadowless)",
                gaps.len(), pct(0.5), pct(0.9), gaps.last().unwrap_or(&0), within, gaps.len()
            );
        }
    }

    match failures {
        0 => println!("\nVERIFIED — every record and the chain check out."),
        n => {
            println!("\nFAILED — {n} verification failure(s).");
            std::process::exit(1);
        }
    }
}
