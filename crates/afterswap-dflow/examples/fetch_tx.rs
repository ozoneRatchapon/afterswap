//! Validate `parse_confirmed` against a real mainnet transaction.
//!
//! The confirmation parser exists to stop a quote being recorded as a fill. It
//! was written against hand-built fixtures, which means it was verified against
//! its author's idea of the `getTransaction` format — and a parser wrong in
//! that way fails silently, in exactly the direction that reintroduces the
//! problem it guards. This runs it against a transaction the chain actually
//! produced.
//!
//! ```sh
//! cargo run -p afterswap-dflow --example fetch_tx --release -- <SIGNATURE>
//! cargo run -p afterswap-dflow --example fetch_tx --release -- <SIG> \
//!     --rpc https://your-endpoint --owner <PUBKEY> --save fixtures/orca_swap.json
//! ```
//!
//! Exit status is 0 only if the parser found a two-sided swap for the owner and
//! produced a finite price — so this is usable as a pre-flight gate before a
//! live run, not just as a viewer.
//!
//! Public RPC serves recent transactions only; older ones need an archival
//! endpoint. `--save` writes the raw response so it can be pinned in
//! `tests/confirm.rs` and the parse becomes a regression test rather than a
//! one-off check.

use afterswap_dflow::parse_confirmed;

fn arg(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let Some(sig) = args.get(1).filter(|s| !s.starts_with("--")) else {
        eprintln!("usage: fetch_tx <SIGNATURE> [--rpc URL] [--owner PUBKEY] [--save PATH]");
        std::process::exit(2);
    };
    let rpc = arg(&args, "--rpc")
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());

    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getTransaction",
        "params": [sig, {
            "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 0,
            "commitment": "confirmed",
        }],
    });
    let resp: serde_json::Value = reqwest::Client::new()
        .post(&rpc)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    if let Some(e) = resp.get("error") {
        eprintln!("RPC error: {e}");
        std::process::exit(1);
    }
    let Some(result) = resp.get("result").filter(|v| !v.is_null()) else {
        eprintln!(
            "transaction not found. Public RPC keeps only recent history — an older \
signature needs an archival endpoint via --rpc."
        );
        std::process::exit(1);
    };

    if let Some(path) = arg(&args, "--save") {
        if let Some(dir) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(result)?)?;
        println!("raw response saved to {path}\n");
    }

    // Default the owner to the fee payer — account index 0 — which is the
    // signer whose balances our own runs would be reading.
    let owner = arg(&args, "--owner").or_else(|| {
        result
            .pointer("/transaction/message/accountKeys/0")
            .and_then(|k| {
                k.as_str()
                    .map(str::to_string)
                    .or_else(|| k.get("pubkey").and_then(|p| p.as_str()).map(str::to_string))
            })
    });
    let Some(owner) = owner else {
        eprintln!("could not determine an owner; pass --owner");
        std::process::exit(1);
    };
    println!("signature : {sig}\nowner     : {owner}");

    // Show the raw balance entries first. If the parser and the eye disagree,
    // that disagreement is the finding.
    for key in ["preTokenBalances", "postTokenBalances"] {
        let n = result
            .pointer(&format!("/meta/{key}"))
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let mine = result
            .pointer(&format!("/meta/{key}"))
            .and_then(|v| v.as_array())
            .map_or(0, |a| {
                a.iter()
                    .filter(|b| b.get("owner").and_then(|v| v.as_str()) == Some(owner.as_str()))
                    .count()
            });
        println!("{key:<18}: {n} entries, {mine} owned by us");
    }

    let Some(fill) = parse_confirmed(result, &owner) else {
        eprintln!("\nPARSE FAILED — meta missing or unreadable");
        std::process::exit(1);
    };
    println!(
        "\nslot      : {}\nfee       : {} lamports\nrevert    : {}\nlamport Δ : {}",
        fill.slot,
        fill.fee_lamports,
        fill.err.as_deref().unwrap_or("none"),
        fill.lamport_delta
    );

    println!("\ntoken deltas for this owner:");
    for (mint, d) in &fill.deltas {
        println!("  {d:>22.9}  {mint}");
    }

    // Identify the legs by largest movement in each direction, then ask the
    // parser for the price it would have recorded.
    let sent = fill
        .deltas
        .iter()
        .filter(|(_, d)| **d < 0.0)
        .min_by(|a, b| a.1.total_cmp(b.1));
    let recv = fill
        .deltas
        .iter()
        .filter(|(_, d)| **d > 0.0)
        .max_by(|a, b| a.1.total_cmp(b.1));

    match (sent, recv) {
        (Some((in_mint, _)), Some((out_mint, _))) => {
            match fill.effective_price(in_mint, out_mint) {
                Some(p) if p.is_finite() && p > 0.0 => {
                    println!("\ninferred legs: {in_mint} -> {out_mint}");
                    println!("EFFECTIVE PRICE: {p:.9} out per in");
                    println!(
                        "\nParser reproduces a two-sided swap from realised deltas. Pin the saved \
response in tests/confirm.rs to keep it that way."
                    );
                }
                _ => {
                    eprintln!("\nFAILED: legs found but no finite price");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!(
                "\nFAILED: could not find one negative and one positive leg for this owner.\n\
This is the blindspot worth knowing about — if real swaps route through an\n\
intermediate account rather than the fee payer's own token accounts, the parser\n\
sees nothing and a live run would record every fill as unfilled. Re-run with\n\
--owner set to the account that actually holds the tokens."
            );
            std::process::exit(1);
        }
    }
    Ok(())
}
