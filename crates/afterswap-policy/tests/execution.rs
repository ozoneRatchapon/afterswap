//! LiteSVM tests for `AuthorizeExecution` (tag 1) and
//! `RevokeAuthorization` (tag 3) against the real compiled SBF binary.
//!
//! Harness style mirrors `tests/policy.rs`: plain `LiteSVM::new()` +
//! `add_program` + `airdrop`, no devnet, no mocks of our own program.

use afterswap_policy::{
    AUTHORIZE_IX_LEN, COMMIT_IX_LEN, EXECUTION_LEN, EXECUTION_SEED, IX_AUTHORIZE_EXECUTION,
    IX_COMMIT_POLICY, IX_REVOKE_AUTHORIZATION, POLICY_SEED, REVOKE_IX_LEN,
};
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

/// Commit the policy PDA. Returns the policy PDA and a fresh execution PDA
/// (derived from the same seeds, but never created by the program — the
/// program's `commit_policy` helper does not create the execution PDA).
/// The execution PDA is then created by the first `authorize_execution`
/// call in each test, which is the path the production flow uses.
fn setup() -> (LiteSVM, Pubkey, Keypair, Pubkey, Pubkey) {
    let mut svm = LiteSVM::new();
    let program_id = Pubkey::new_unique();
    svm.add_program(program_id, &program_so());
    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 1_000_000_000)
        .expect("airdrop");

    let position_id = 7u64;
    let (policy, _) = Pubkey::find_program_address(
        &[
            POLICY_SEED,
            owner.pubkey().as_ref(),
            &position_id.to_le_bytes(),
        ],
        &program_id,
    );
    let (execution, _) = Pubkey::find_program_address(
        &[
            EXECUTION_SEED,
            owner.pubkey().as_ref(),
            &position_id.to_le_bytes(),
        ],
        &program_id,
    );

    // Commit the policy first (tag 0) — this is the rulebook.
    let mut commit_data = Vec::with_capacity(COMMIT_IX_LEN);
    commit_data.push(IX_COMMIT_POLICY);
    commit_data.extend_from_slice(&position_id.to_le_bytes());
    commit_data.extend_from_slice(&0x165e_f4aa_bbcc_ddee_u64.to_le_bytes());
    commit_data.push(3); // n_states
    commit_data.extend_from_slice(&1000u16.to_le_bytes()); // tranche_bps
    let commit_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(policy, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: commit_data,
    };
    send(&mut svm, &owner, &commit_ix).expect("commit succeeds");

    (svm, program_id, owner, policy, execution)
}

/// Build an `AuthorizeExecution` ix with the position_id in the data
/// (required on the first call to create the execution PDA).
fn authorize_ix(
    program_id: Pubkey,
    owner: Pubkey,
    execution: Pubkey,
    crank: Pubkey,
    position_id: u64,
    expires_unix: u64,
    settlement_ata: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(AUTHORIZE_IX_LEN);
    data.push(IX_AUTHORIZE_EXECUTION);
    data.extend_from_slice(&position_id.to_le_bytes());
    data.extend_from_slice(crank.as_ref());
    data.extend_from_slice(&expires_unix.to_le_bytes());
    // The settlement destination the authorization binds. These tests only
    // exercise tags 1 and 3, which never pay out, so any non-zero key works;
    // the zero key is rejected on purpose (it is revoke's cleared marker).
    data.extend_from_slice(settlement_ata.as_ref());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(execution, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

/// Build a `RevokeAuthorization` ix.
fn revoke_ix(program_id: Pubkey, owner: Pubkey, execution: Pubkey) -> Instruction {
    let mut data = Vec::with_capacity(REVOKE_IX_LEN);
    data.push(IX_REVOKE_AUTHORIZATION);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(execution, false),
        ],
        data,
    }
}

/// Send a transaction and return the error (with logs) if it failed.
///
/// Note: LiteSVM records the *signature* of each successful transaction in
/// its history, and a second transaction with the same signature is
/// rejected with `AlreadyProcessed`. Each `Transaction::new_signed_with_payer`
/// call produces a fresh signature (the blockhash is included in the
/// signing input), so this is not a problem in practice — the idempotent
/// revoke test below sends two *different* transactions (different
/// signatures) that happen to target the same account state.
///
/// For the idempotent-revoke case (same instruction, same signer,
/// same blockhash → same signature), we send the second transaction
/// with a fresh blockhash so the signature differs.
fn send(svm: &mut LiteSVM, owner: &Keypair, ix: &Instruction) -> Result<(), String> {
    send_with_blockhash(svm, owner, ix, svm.latest_blockhash())
}

/// Send a transaction with an explicit blockhash (so the signature
/// differs from a previous identical transaction).
fn send_with_blockhash(
    svm: &mut LiteSVM,
    owner: &Keypair,
    ix: &Instruction,
    blockhash: solana_sdk::hash::Hash,
) -> Result<(), String> {
    let tx = Transaction::new_signed_with_payer(
        &[(*ix).clone()],
        Some(&owner.pubkey()),
        &[owner],
        blockhash,
    );
    match svm.send_transaction(tx) {
        Ok(_) => Ok(()),
        Err(e) => {
            // `meta.logs` carries the program's `msg!` / panic-handler output —
            // include it so a failed assert shows *where* the program stopped.
            Err(format!("err={:?} logs={:?}", e.err, e.meta.logs))
        }
    }
}

#[test]
fn authorize_sets_crank_and_expiry() {
    let (mut svm, program_id, owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);
    let ix = authorize_ix(program_id, owner.pubkey(), execution, crank, 7, expires, settlement);
    send(&mut svm, &owner, &ix).expect("authorize succeeds");

    let acct = svm.get_account(&execution).expect("execution exists");
    assert_eq!(acct.owner, program_id);
    assert_eq!(acct.data.len(), EXECUTION_LEN);
    assert_eq!(&acct.data[0..32], owner.pubkey().as_ref());
    assert_eq!(&acct.data[40..72], crank.as_ref());
    assert_eq!(acct.data[72], 0); // tranches_filled
    assert_eq!(u64::from_le_bytes(acct.data[73..81].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(acct.data[81..89].try_into().unwrap()), 0);
    assert_eq!(
        u64::from_le_bytes(acct.data[89..97].try_into().unwrap()),
        expires
    );

    // Immutability: a second authorize for the same (owner, position) must
    // be rejected — the execution PDA is immutable once created (the owner
    // must revoke first, then re-authorize with a new crank).
    // Expire the blockhash so the second transaction has a different
    // signature (LiteSVM deduplicates by signature).
    svm.expire_blockhash();
    let ix2 = authorize_ix(program_id, owner.pubkey(), execution, crank, 7, expires + 1, settlement);
    let result = send(&mut svm, &owner, &ix2);
    match result {
        Ok(_) => panic!("double authorize must fail (got Ok)"),
        Err(e) => eprintln!("double authorize failed as expected: {e}"),
    }
}

#[test]
fn authorize_rejects_expired_or_past_expiry() {
    let (mut svm, program_id, owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();

    // Expiry in the past must be rejected.
    let ix = authorize_ix(
        program_id,
        owner.pubkey(),
        execution,
        crank,
        7,
        now.unix_timestamp as u64,
        settlement,
    );
    assert!(
        send(&mut svm, &owner, &ix).is_err(),
        "past expiry must fail"
    );

    // Expiry == now must be rejected (strict inequality).
    let ix = authorize_ix(
        program_id,
        owner.pubkey(),
        execution,
        crank,
        7,
        now.unix_timestamp as u64,
        settlement,
    );
    assert!(
        send(&mut svm, &owner, &ix).is_err(),
        "expiry == now must fail"
    );
}

#[test]
fn revoke_clears_crank() {
    let (mut svm, program_id, owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);

    let ix = authorize_ix(program_id, owner.pubkey(), execution, crank, 7, expires, settlement);
    send(&mut svm, &owner, &ix).expect("authorize succeeds");
    let acct = svm.get_account(&execution).expect("execution exists");
    assert_eq!(&acct.data[40..72], crank.as_ref());

    let ix = revoke_ix(program_id, owner.pubkey(), execution);
    send(&mut svm, &owner, &ix).expect("revoke succeeds");
    let acct = svm.get_account(&execution).expect("execution exists");
    assert_eq!(&acct.data[40..72], [0u8; 32]);
}

#[test]
fn revoke_is_idempotent() {
    let (mut svm, program_id, owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);

    let ix = authorize_ix(program_id, owner.pubkey(), execution, crank, 7, expires, settlement);
    send(&mut svm, &owner, &ix).expect("authorize succeeds");

    // First revoke: clears the crank.
    let ix = revoke_ix(program_id, owner.pubkey(), execution);
    send(&mut svm, &owner, &ix).expect("first revoke succeeds");

    // Second revoke: no-op (crank already zero), must still succeed.
    // Expire the blockhash so the second transaction has a different
    // signature (LiteSVM deduplicates by signature).
    svm.expire_blockhash();
    let ix = revoke_ix(program_id, owner.pubkey(), execution);
    send(&mut svm, &owner, &ix).expect("second revoke succeeds");

    // Crank must still be zero.
    let acct = svm.get_account(&execution).expect("execution exists");
    assert_eq!(&acct.data[40..72], [0u8; 32]);
}

#[test]
fn authorize_rejects_non_owner() {
    let (mut svm, program_id, _owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);

    // A different (non-owner) signer must be rejected.
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000)
        .expect("airdrop");
    let ix = authorize_ix(program_id, stranger.pubkey(), execution, crank, 7, expires, settlement);
    assert!(
        send(&mut svm, &stranger, &ix).is_err(),
        "non-owner must fail"
    );
}

#[test]
fn authorize_rejects_wrong_pda() {
    let (mut svm, program_id, owner, _policy, _execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);

    // A random (non-PDA) account must be rejected.
    let bogus = Pubkey::new_unique();
    let ix = authorize_ix(program_id, owner.pubkey(), bogus, crank, 7, expires, settlement);
    assert!(send(&mut svm, &owner, &ix).is_err(), "wrong PDA must fail");
}

#[test]
fn authorize_rejects_non_signer_owner() {
    let (mut svm, program_id, owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);

    // A different (non-owner) keypair signs the transaction, but the
    // instruction lists the owner as non-signer. The program must reject
    // because the owner account in the instruction is not a signer.
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000)
        .expect("airdrop");
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), false),
            AccountMeta::new(execution, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: {
            let mut d = Vec::with_capacity(AUTHORIZE_IX_LEN);
            d.push(IX_AUTHORIZE_EXECUTION);
            d.extend_from_slice(&7u64.to_le_bytes());
            d.extend_from_slice(crank.as_ref());
            d.extend_from_slice(&expires.to_le_bytes());
            // Full-length payload on purpose: a short one would be rejected
            // for its length and the signer check would never be reached.
            d.extend_from_slice(settlement.as_ref());
            d
        },
    };
    assert!(
        send(&mut svm, &stranger, &ix).is_err(),
        "non-signer owner must fail"
    );
}

#[test]
fn authorize_rejects_zero_settlement_ata() {
    let (mut svm, program_id, owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);

    // The all-zero key is `RevokeAuthorization`'s cleared marker. Accepting
    // it as a settlement destination would authorize a crank whose payout
    // account is the same value the revoke path writes.
    let ix = authorize_ix(
        program_id,
        owner.pubkey(),
        execution,
        crank,
        7,
        expires,
        Pubkey::default(),
    );
    assert!(
        send(&mut svm, &owner, &ix).is_err(),
        "a zero settlement ATA must fail"
    );
}

#[test]
fn reauthorize_after_revoke_rebinds_crank_and_settlement() {
    let (mut svm, program_id, owner, _policy, execution) = setup();
    let crank = Pubkey::new_unique();
    let settlement = Pubkey::new_unique();
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);

    let ix = authorize_ix(
        program_id,
        owner.pubkey(),
        execution,
        crank,
        7,
        expires,
        settlement,
    );
    send(&mut svm, &owner, &ix).expect("first authorize succeeds");

    svm.expire_blockhash();
    send(&mut svm, &owner, &revoke_ix(program_id, owner.pubkey(), execution))
        .expect("revoke succeeds");

    // Revoke keeps the account alive, so re-authorizing has to work in
    // place — otherwise revoking would permanently end the owner's ability
    // to delegate this position, which is not what revoke is for.
    svm.expire_blockhash();
    let crank2 = Pubkey::new_unique();
    let settlement2 = Pubkey::new_unique();
    let expires2 = expires + 60;
    let ix = authorize_ix(
        program_id,
        owner.pubkey(),
        execution,
        crank2,
        7,
        expires2,
        settlement2,
    );
    send(&mut svm, &owner, &ix).expect("re-authorize after revoke succeeds");

    let acct = svm.get_account(&execution).expect("execution exists");
    assert_eq!(&acct.data[40..72], crank2.as_ref(), "crank rebound");
    assert_eq!(
        u64::from_le_bytes(acct.data[89..97].try_into().unwrap()),
        expires2,
        "expiry rebound"
    );
    assert_eq!(
        &acct.data[98..130],
        settlement2.as_ref(),
        "settlement destination rebound"
    );
    assert_eq!(acct.data[72], 0, "tranches_filled untouched");
}
