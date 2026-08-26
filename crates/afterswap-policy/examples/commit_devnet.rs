//! Build+sign a real CommitPolicy tx (prints base64 for JSON-RPC send).
//! Usage: commit_devnet <keypair.json> <program_id> <recent_blockhash>

use afterswap_policy::{COMMIT_IX_LEN, IX_COMMIT_POLICY, POLICY_SEED};
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Signer, read_keypair_file};
use solana_sdk::system_program;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let payer = read_keypair_file(&args[1]).expect("keypair");
    let program_id = Pubkey::from_str(&args[2]).expect("program id");
    let blockhash = Hash::from_str(&args[3]).expect("blockhash");

    let position_id = 1u64;
    // Fingerprint of a real enumerated machine (Eager Puffin lineage).
    let fingerprint = 0x0000_165e_f4aa_bbccu64;
    let (policy, _) = Pubkey::find_program_address(
        &[POLICY_SEED, payer.pubkey().as_ref(), &position_id.to_le_bytes()],
        &program_id,
    );
    let mut data = Vec::with_capacity(COMMIT_IX_LEN);
    data.push(IX_COMMIT_POLICY);
    data.extend_from_slice(&position_id.to_le_bytes());
    data.extend_from_slice(&fingerprint.to_le_bytes());
    data.push(3u8);
    data.extend_from_slice(&1000u16.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(policy, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );
    use base64::Engine as _;
    println!("{}", base64::engine::general_purpose::STANDARD.encode(bincode::serialize(&tx).unwrap()));
    eprintln!("policy_pda={policy}");
}
