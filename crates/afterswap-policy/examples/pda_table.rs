//! Precompute policy PDAs for the demo signer.
//!
//! The Worker signs demo commitments but cannot derive PDAs (that needs an
//! ed25519 on-curve check). Owner is fixed (the demo keypair) and
//! position_id is a counter, so the whole table is deterministic and can be
//! generated once, here.
//!
//! Usage: pda_table <owner_pubkey> <program_id> <count>

use afterswap_policy::POLICY_SEED;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let owner = Pubkey::from_str(&args[1]).expect("owner");
    let program_id = Pubkey::from_str(&args[2]).expect("program id");
    let count: u64 = args[3].parse().expect("count");

    let entries: Vec<String> = (0..count)
        .map(|position_id| {
            let (pda, _bump) = Pubkey::find_program_address(
                &[POLICY_SEED, owner.as_ref(), &position_id.to_le_bytes()],
                &program_id,
            );
            format!("\"{pda}\"")
        })
        .collect();
    println!(
        "{{\"owner\":\"{owner}\",\"program\":\"{program_id}\",\"pdas\":[{}]}}",
        entries.join(",")
    );
}
