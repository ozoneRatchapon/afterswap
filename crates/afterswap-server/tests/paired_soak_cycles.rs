//! A paired soak must collect many cycles, not one.
//!
//! The CLI loop latched `opened` on the first position and never cleared it,
//! so `--paired` recorded a single cycle and then idled for the rest of the
//! run. A 4,500-tick BONK soak produced 1 usable line before this was caught.
//! The failure is silent — the process exits 0 and writes a well-formed file —
//! so it needs a test rather than an eyeball.

use afterswap_server::paper::{self, PaperConfig};

/// Deterministic saw-tooth: enough movement to open and close repeatedly.
fn prices() -> Vec<f64> {
    (0..240)
        .map(|i| {
            let phase = (i % 60) as f64;
            let leg = if phase < 30.0 { phase } else { 60.0 - phase };
            1.0 + leg * 0.002
        })
        .collect()
}

#[tokio::test]
async fn paired_mode_reopens_and_records_many_cycles() {
    let out = std::env::temp_dir().join("afterswap_paired_cycles_test.jsonl");
    let _ = std::fs::remove_file(&out);

    paper::run(PaperConfig {
        interval_ms: 1,
        max_ticks: Some(1_200),
        replay: Some(prices()),
        paired: Some(out.clone()),
        ..Default::default()
    })
    .await
    .expect("paper run");

    let lines = std::fs::read_to_string(&out)
        .expect("paired file")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let _ = std::fs::remove_file(&out);

    assert!(
        lines > 1,
        "paired soak recorded {lines} cycle(s) in 1,200 ticks — the open latch \
         is stuck again, so the run collects no usable sample"
    );
}
