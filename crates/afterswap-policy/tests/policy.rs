//! LiteSVM tests against the real compiled SBF binary.

use afterswap_policy::{COMMIT_IX_LEN, IX_COMMIT_POLICY, POLICY_LEN, POLICY_SEED};
use litesvm::LiteSVM;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_program;
use solana_sdk::transaction::Transaction;

fn program_so() -> Vec<u8> {
    let mut candidates = vec![];
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        candidates.push(format!("{dir}/deploy/afterswap_policy.so"));
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{home}/.cargo/target/deploy/afterswap_policy.so"));
    }
    candidates.push("target/deploy/afterswap_policy.so".to_string());
    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            return bytes;
        }
    }
    panic!(
        "afterswap_policy.so not found in {candidates:?} — run \
         `cargo-build-sbf --manifest-path crates/afterswap-policy/Cargo.toml` first"
    );
}

fn commit_ix(
    program_id: Pubkey,
    owner: Pubkey,
    policy: Pubkey,
    position_id: u64,
    fingerprint: u64,
    n_states: u8,
    tranche_bps: u16,
) -> Instruction {
    let mut data = Vec::with_capacity(COMMIT_IX_LEN);
    data.push(IX_COMMIT_POLICY);
    data.extend_from_slice(&position_id.to_le_bytes());
    data.extend_from_slice(&fingerprint.to_le_bytes());
    data.push(n_states);
    data.extend_from_slice(&tranche_bps.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(policy, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

fn setup() -> (LiteSVM, Pubkey, Keypair) {
    let mut svm = LiteSVM::new();
    let program_id = Pubkey::new_unique();
    svm.add_program(program_id, &program_so());
    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 1_000_000_000)
        .expect("airdrop");
    (svm, program_id, owner)
}

fn send(svm: &mut LiteSVM, owner: &Keypair, ix: &Instruction) -> Result<(), String> {
    let tx = Transaction::new_signed_with_payer(
        &[(*ix).clone()],
        Some(&owner.pubkey()),
        &[owner],
        svm.latest_blockhash(),
    );
    match svm.send_transaction(tx) {
        Ok(_) => Ok(()),
        // `meta.logs` carries the program's `msg!` / panic-handler output —
        // include it so a failed assert shows *where* the program stopped.
        Err(e) => Err(format!("err={:?} logs={:?}", e.err, e.meta.logs)),
    }
}

#[test]
fn commit_writes_immutable_policy() {
    let (mut svm, program_id, owner) = setup();
    let position_id = 42u64;
    let fingerprint = 0x165e_f4aa_bbcc_ddee_u64;
    let (policy, bump) = Pubkey::find_program_address(
        &[
            POLICY_SEED,
            owner.pubkey().as_ref(),
            &position_id.to_le_bytes(),
        ],
        &program_id,
    );

    let ix = commit_ix(
        program_id,
        owner.pubkey(),
        policy,
        position_id,
        fingerprint,
        3,
        1000,
    );
    send(&mut svm, &owner, &ix).expect("commit succeeds");

    let acct = svm.get_account(&policy).expect("policy exists");
    assert_eq!(acct.owner, program_id);
    assert_eq!(acct.data.len(), POLICY_LEN);
    assert_eq!(&acct.data[0..32], owner.pubkey().as_ref());
    assert_eq!(
        u64::from_le_bytes(acct.data[32..40].try_into().unwrap()),
        position_id
    );
    assert_eq!(
        u64::from_le_bytes(acct.data[40..48].try_into().unwrap()),
        fingerprint
    );
    assert_eq!(acct.data[48], 3);
    assert_eq!(
        u16::from_le_bytes(acct.data[49..51].try_into().unwrap()),
        1000
    );
    assert_eq!(acct.data[59], bump);

    // Immutability: same (owner, position) cannot commit twice.
    let ix2 = commit_ix(program_id, owner.pubkey(), policy, position_id, 999, 2, 500);
    assert!(
        send(&mut svm, &owner, &ix2).is_err(),
        "double commit must fail"
    );
}

#[test]
fn rejects_wrong_pda_and_bad_params() {
    let (mut svm, program_id, owner) = setup();
    // Wrong PDA (random account) must be rejected.
    let bogus = Pubkey::new_unique();
    let ix = commit_ix(program_id, owner.pubkey(), bogus, 7, 1, 3, 1000);
    assert!(send(&mut svm, &owner, &ix).is_err(), "wrong PDA must fail");

    // n_states out of range must be rejected.
    let (policy, _) = Pubkey::find_program_address(
        &[POLICY_SEED, owner.pubkey().as_ref(), &7u64.to_le_bytes()],
        &program_id,
    );
    let ix = commit_ix(program_id, owner.pubkey(), policy, 7, 1, 9, 1000);
    assert!(send(&mut svm, &owner, &ix).is_err(), "n_states=9 must fail");
}
