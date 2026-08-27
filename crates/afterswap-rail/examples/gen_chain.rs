//! Extend a rail log with synthetic-but-honestly-attested records — chain
//! fuel for exercising segment closing and ingest at volume without waiting
//! on live ticks.
//!
//! The records are cryptographically real (valid attestation, valid rule-v1
//! decision, true evidence digests) and *content*-synthetic, and they say so:
//! instrument "TEST/SYNTH". Never write these into a production log; the
//! falsifier copies the real log aside first.
//!
//! ```sh
//! cargo run -p afterswap-rail --example gen_chain --release -- \
//!     /tmp/rail_falsifier.jsonl 60
//! ```

use afterswap_rail::{
    AttestKey, AuditRecord, EvaluatedVenue, QuoteEvidence, RouteDecision, VenueQuote, attest,
    link, rule_v1_fingerprint,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(path), Some(n)) = (args.get(1), args.get(2).and_then(|a| a.parse::<u64>().ok()))
    else {
        eprintln!("usage: gen_chain <LOG.jsonl> <N>");
        std::process::exit(2);
    };
    let key = AttestKey::from_seed([0xA5; 32]); // the dev seed, deliberately
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut tip: Option<AuditRecord> = text.lines().rev().find_map(|l| serde_json::from_str(l).ok());
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open log");

    for i in 0..n {
        let body = format!("{{\"synthetic\":true,\"i\":{i}}}");
        let out_a = 100_000_000 + i; // varies so decisions are not constant
        let record = AuditRecord {
            seq: 0,
            prev_hash: [0; 32],
            t_ms: 1_787_900_000_000 + i,
            instrument: "TEST/SYNTH".into(),
            quotes: vec![VenueQuote {
                venue: "dflow".into(),
                context_slot: Some(500_000_000 + i),
                latency_us: 1,
                in_mint: "TESTIN".into(),
                out_mint: "TESTOUT".into(),
                in_amount: "1000000000".into(),
                out_amount: out_a.to_string(),
                route: "synthetic|1".into(),
                evidence: QuoteEvidence::observed(body.as_bytes()),
            }],
            policy_fingerprint: rule_v1_fingerprint(),
            decision: RouteDecision {
                chosen_venue: "dflow".into(),
                evaluated: vec![EvaluatedVenue { venue: "dflow".into(), net_out: out_a.to_string() }],
            },
            fill: None,
            attestation: [0; 64],
        };
        let record = attest(link(record, tip.as_ref()), &key);
        use std::io::Write as _;
        writeln!(out, "{}", serde_json::to_string(&record).expect("json")).expect("write");
        tip = Some(record);
    }
    eprintln!("appended {n} synthetic records, tip seq {}", tip.map_or(0, |t| t.seq));
}
