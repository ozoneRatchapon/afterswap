//! LiteSVM tests for the Phase B step 2 vault-sourced flow against the real
//! compiled SBF binary: `DepositToVault` (tag 4), `ValidateAndSell` (tag 2,
//! the gate), `CloseVault` (tag 5).
//!
//! The environment builder lives in `common/mod.rs` — see the note there.

mod common;

use afterswap_policy::{
    COMMIT_IX_LEN, DEPOSIT_IX_LEN, IX_COMMIT_POLICY, IX_DEPOSIT_TO_VAULT, POLICY_SEED,
    VAULT_LEN, VAULT_SEED,
};
use common::{
    happy_setup, pda, program_so, send, setup_full, token_balance, validate_sell_ix,
    ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use litesvm::LiteSVM;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_program;

// ──────────────────────────────────────────────────────────────────────────
// DepositToVault (tag 4)
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn deposit_creates_vault_and_moves_tokens() {
    let (
        svm,
        _pid,
        _owner,
        _policy,
        _exec,
        _crank,
        vault,
        _mint,
        owner_ata,
        vault_ata,
        _buyer,
        _buyer_ata,
        _pos,
    ) = happy_setup();
    let v = svm.get_account(&vault).expect("vault exists");
    assert_eq!(v.owner, _pid);
    assert_eq!(v.data.len(), VAULT_LEN);

    // Tokens moved: owner ATA 0, vault ATA 10_000.
    assert_eq!(token_balance(&svm, &owner_ata), 0);
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);

    // Vault state: amount = 10_000.
    let amount = u64::from_le_bytes(v.data[72..80].try_into().unwrap());
    assert_eq!(amount, 10_000);
}

#[test]
fn deposit_rejects_non_signer_owner() {
    // A stranger signs, but the owner account is listed non-signer.
    let mut svm = LiteSVM::new();
    let program_id = Pubkey::new_unique();
    svm.add_program(program_id, &program_so());
    let owner = Keypair::new();
    let stranger = Keypair::new();
    svm.airdrop(&owner.pubkey(), 1_000_000_000)
        .expect("airdrop");
    svm.airdrop(&stranger.pubkey(), 1_000_000_000)
        .expect("airdrop");
    let position_id = 42u64;
    let (vault, _) = pda(VAULT_SEED, &program_id, &owner.pubkey(), position_id);
    let mint = Pubkey::new_unique();
    let owner_ata = Pubkey::new_unique();
    let vault_ata = Pubkey::new_unique();

    let mut d = Vec::with_capacity(DEPOSIT_IX_LEN);
    d.push(IX_DEPOSIT_TO_VAULT);
    d.extend_from_slice(&position_id.to_le_bytes());
    d.extend_from_slice(&1u64.to_le_bytes());
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), false), // non-signer!
            AccountMeta::new(vault, false),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        ],
        data: d,
    };
    assert!(
        send(&mut svm, &[&stranger], &ix).is_err(),
        "non-signer owner must fail"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// ValidateAndSell (tag 2) — the gate
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn sell_by_authorized_crank_moves_tokens() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();
    let dest = buyer_ata;
    let digest = [7u8; 32];

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        dest,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        // A full tranche: tranche_bps = 1000 (10%) of the 10_000 deposited.
        1_000,
    );
    send(&mut svm, &[&crank], &ix).expect("sell succeeds");

    // 1_000 moved from vault ATA to dest.
    assert_eq!(token_balance(&svm, &vault_ata), 9_000);
    assert_eq!(token_balance(&svm, &dest), 1_000);

    // Execution state advanced.
    let e = svm.get_account(&execution).expect("execution exists");
    assert_eq!(e.data[72], 1, "tranches_filled == 1");
    assert_eq!(
        u64::from_le_bytes(e.data[73..81].try_into().unwrap()),
        1_000,
        "executed_lamports"
    );
    assert_eq!(
        u64::from_le_bytes(e.data[81..89].try_into().unwrap()),
        1_000_000,
        "last_tick_slot"
    );

    // Vault state decremented.
    let v = svm.get_account(&vault).expect("vault exists");
    assert_eq!(
        u64::from_le_bytes(v.data[72..80].try_into().unwrap()),
        9_000
    );
    // `deposited` is the tranche denominator and does not move on a sell.
    assert_eq!(
        u64::from_le_bytes(v.data[81..89].try_into().unwrap()),
        10_000,
        "deposited stays at the position as deposited"
    );
}

#[test]
fn sell_by_owner_override_moves_tokens() {
    let (
        mut svm,
        pid,
        owner,
        policy,
        execution,
        _crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();
    let digest = [8u8; 32];

    // The owner signs as cranker (owner override, step 2 second branch).
    let ix = validate_sell_ix(
        pid,
        &owner.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        1_000,
    );
    send(&mut svm, &[&owner], &ix).expect("owner override sell succeeds");

    assert_eq!(token_balance(&svm, &vault_ata), 9_000);
    assert_eq!(token_balance(&svm, &buyer_ata), 1_000);
}

#[test]
fn sell_rejects_wrong_position_id() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        _pos,
    ) = happy_setup();
    let digest = [9u8; 32];

    // position_id 99 does not match the policy PDA's position_id (42).
    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        99,
        0,
        0,
        1_000_000,
        &digest,
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "wrong position_id must fail"
    );
}

#[test]
fn sell_rejects_unauthorized_cranker() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        _crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000)
        .expect("airdrop");
    let digest = [10u8; 32];

    let ix = validate_sell_ix(
        pid,
        &stranger.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        1_000,
    );
    assert!(
        send(&mut svm, &[&stranger], &ix).is_err(),
        "unauthorized cranker must fail"
    );
}

#[test]
fn sell_rejects_after_expiry() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();

    // Warp the clock past the expiry (now + 3_600).
    let clock = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let mut new_clock = clock;
    new_clock.unix_timestamp = (new_clock.unix_timestamp as u64).saturating_add(7_200) as i64;
    svm.set_sysvar(&new_clock);

    let digest = [11u8; 32];
    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "expired authorization must fail"
    );
}

#[test]
fn sell_rejects_tranche_skip() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();
    let digest = [12u8; 32];

    // tranches_filled is 0; supplying tranche_index 1 must be rejected.
    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        1,
        1_000_000,
        &digest,
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "tranche skip must fail"
    );
}

#[test]
fn sell_rejects_tranche_replay() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();
    let digest_a = [13u8; 32];
    let digest_b = [14u8; 32];

    // First sell (tranche 0) succeeds.
    let ix0 = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest_a,
        1_000,
    );
    send(&mut svm, &[&crank], &ix0).expect("first sell succeeds");

    // Replay of tranche 0 must be rejected (tranches_filled is now 1).
    let ix_replay = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest_b,
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix_replay).is_err(),
        "tranche replay must fail"
    );
}

#[test]
fn sell_rejects_tranche_beyond_the_committed_tranche_count() {
    // tranche_bps = 10_000 (100%): the position is one tranche, so only
    // tranche 0 is valid. The bound is ceil(10_000 / tranche_bps), not
    // n_states — n_states sizes the FSM, not the sell schedule.
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = setup_full(1, 10_000, 10_000, 0);
    let digest = [15u8; 32];

    // Fill tranche 0.
    let ix0 = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        1_000,
    );
    send(&mut svm, &[&crank], &ix0).expect("tranche 0 succeeds");

    // Tranche 1 >= the committed tranche count (1) must be rejected.
    let ix1 = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        1,
        1_000_000,
        &digest,
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix1).is_err(),
        "tranche beyond the committed tranche count must fail"
    );
}

#[test]
fn sell_rejects_amount_exceeding_vault() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();
    let digest = [16u8; 32];

    // Vault holds 10_000; asking for 10_001 must fail.
    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        10_001,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "amount > vault balance must fail"
    );
}

#[test]
fn sell_rejects_zero_amount() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();
    let digest = [17u8; 32];

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        0,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "zero amount must fail"
    );
}

#[test]
fn full_sequence_runs_the_schedule_then_stops() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    // tranche_bps = 2_500 (25%): the committed schedule is 4 tranches.
    ) = setup_full(3, 2_500, 10_000, 0);
    let base = [18u8; 32];

    // Sells 0..3 all succeed — 2_500 each, the full position.
    for i in 0..4u8 {
        let mut digest = base;
        digest[0] = i;
        let ix = validate_sell_ix(
            pid,
            &crank.pubkey(),
            policy,
            execution,
            vault,
            vault_ata,
            buyer_ata,
            mint,
            pos,
            0,
            i,
            1_000_000 + u64::from(i),
            &digest,
            2_500,
        );
        send(&mut svm, &[&crank], &ix)
            .unwrap_or_else(|e| panic!("sell {i} succeeds: {e:?}"));
    }

    // The whole position exited; vault ATA empty.
    assert_eq!(token_balance(&svm, &vault_ata), 0);
    assert_eq!(token_balance(&svm, &buyer_ata), 10_000);

    // The 5th sell (tranche 4) must fail: the committed schedule has
    // ceil(10_000 / 2_500) = 4 tranches, and 4 is past the last one.
    let mut digest = base;
    digest[0] = 4;
    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        4,
        2_000_000,
        &digest,
        2_500,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "a sell past the committed tranche count must fail"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// CloseVault (tag 5)
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn sell_rejects_substituted_policy_account() {
    // The gate's only on-chain bound is `n_states`, read out of the account
    // passed as `policy`. Before the PDA check was added, that account was
    // validated only by length and by the `position_id` stored inside it —
    // so an authorized crank could hand the program a *different real policy
    // account* that happens to share the position_id and carry a larger
    // `n_states`, and keep selling past the committed machine size.
    //
    // Here the attacker's own policy is genuine: same program, same length,
    // same position_id, owned by the program, and legitimately committed with
    // n_states = 4 against the victim's 3. Only the seeds differ.
    let (
        mut svm,
        pid,
        _owner,
        _policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();

    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 1_000_000_000)
        .expect("airdrop attacker");
    let (attacker_policy, _) = pda(POLICY_SEED, &pid, &attacker.pubkey(), pos);

    let mut d = Vec::with_capacity(COMMIT_IX_LEN);
    d.push(IX_COMMIT_POLICY);
    d.extend_from_slice(&pos.to_le_bytes());
    d.extend_from_slice(&0x165e_f4aa_bbcc_ddee_u64.to_le_bytes());
    d.push(4); // n_states = 4, larger than the victim's committed 3
    d.extend_from_slice(&1000u16.to_le_bytes());
    send(
        &mut svm,
        &[&attacker],
        &Instruction {
            program_id: pid,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(attacker_policy, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: d,
        },
    )
    .expect("attacker commits their own policy");

    // Sanity: the substitute really does look like the victim's policy on
    // every dimension the old code checked.
    let a = svm.get_account(&attacker_policy).expect("attacker policy");
    assert_eq!(a.owner, pid, "owned by the same program");
    assert_eq!(a.data.len(), 60, "same POLICY_LEN");
    assert_eq!(
        u64::from_le_bytes(a.data[32..40].try_into().unwrap()),
        pos,
        "same position_id"
    );
    assert_eq!(a.data[48], 4, "but a larger n_states");

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        attacker_policy, // <- substituted
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[21u8; 32],
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "a policy account that is not the position's own PDA must be rejected"
    );

    // And nothing moved.
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &buyer_ata), 0);
}

#[test]
fn close_vault_reclaims_remaining_tokens() {
    let (
        mut svm,
        pid,
        owner,
        policy,
        execution,
        _crank,
        vault,
        mint,
        owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();

    // Sell one 10% tranche first, leaving 9_000 in the vault. The payout
    // goes to the bound settlement ATA even under the owner override — the
    // destination binding is on the account, not on who signs. The owner's
    // route back to their own ATA is `CloseVault`, exercised below.
    let digest = [19u8; 32];
    let ix = validate_sell_ix(
        pid,
        &owner.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        1_000,
    );
    send(&mut svm, &[&owner], &ix).expect("sell one tranche");
    assert_eq!(token_balance(&svm, &vault_ata), 9_000);

    // Close the vault: 9_000 back to the owner ATA.
    let close_ix = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: vec![5u8], // IX_CLOSE_VAULT
    };
    send(&mut svm, &[&owner], &close_ix).expect("close vault");

    assert_eq!(token_balance(&svm, &vault_ata), 0);
    assert_eq!(token_balance(&svm, &owner_ata), 9_000); // 1_000 sold, 9_000 reclaimed

    // Vault state zeroed.
    let v = svm.get_account(&vault).expect("vault exists");
    assert_eq!(u64::from_le_bytes(v.data[72..80].try_into().unwrap()), 0);
}

#[test]
fn close_vault_is_idempotent_when_empty() {
    let (
        mut svm,
        pid,
        owner,
        policy,
        execution,
        _crank,
        vault,
        mint,
        owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
        // tranche_bps = 10_000 (100%): one sell may legally drain the whole
        // position, which is what this test needs to reach an empty vault.
    ) = setup_full(3, 10_000, 10_000, 0);

    // Sell everything first.
    let digest = [20u8; 32];
    let ix = validate_sell_ix(
        pid,
        &owner.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &digest,
        10_000,
    );
    send(&mut svm, &[&owner], &ix).expect("sell all");
    assert_eq!(token_balance(&svm, &vault_ata), 0);

    // Close with an empty vault: no-op, must still succeed.
    let close_ix = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: vec![5u8],
    };
    send(&mut svm, &[&owner], &close_ix).expect("close empty vault (idempotent)");
    // Nothing came back: the vault was already empty and the position was
    // paid out to the bound settlement ATA.
    assert_eq!(token_balance(&svm, &owner_ata), 0);
}

#[test]
fn close_vault_rejects_non_owner() {
    let (
        mut svm,
        pid,
        _owner,
        _policy,
        _exec,
        _crank,
        vault,
        mint,
        owner_ata,
        vault_ata,
        _buyer,
        _buyer_ata,
        _pos,
    ) = happy_setup();
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000)
        .expect("airdrop");

    let close_ix = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(stranger.pubkey(), true), // wrong owner
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: vec![5u8],
    };
    assert!(
        send(&mut svm, &[&stranger], &close_ix).is_err(),
        "non-owner close must fail"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Account-substitution gate
//
// Each test below substitutes one account the caller supplies and asserts the
// program rejects it. Being on the account list is not a permission: the vault
// PDA signs every transfer, so any account the gate reads but does not verify
// is an account the caller gets to choose. These four were all unverified —
// the destination, and the token program in each of the three transfer paths.
// ──────────────────────────────────────────────────────────────────────────

/// A pubkey that is not the SPL Token program. Used as the substituted token
/// program below; the check is on the key, before any CPI, so the account
/// need not be a real program.
fn foreign_token_program() -> Pubkey {
    Pubkey::new_unique()
}

#[test]
fn sell_rejects_destination_other_than_settlement_ata() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();

    // The authorization bound `buyer_ata`. The crank names a different
    // account instead — one that exists and would accept the transfer.
    // Without the binding this is how an authorized crank drains a vault:
    // legal tranche size, legal cadence, wrong destination.
    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        owner_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[21u8; 32],
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "a destination other than the bound settlement ATA must fail"
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000, "vault untouched");
    assert_eq!(token_balance(&svm, &owner_ata), 0, "nothing was paid out");

    // The bound destination still works.
    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[21u8; 32],
        1_000,
    );
    send(&mut svm, &[&crank], &ix).expect("bound destination succeeds");
    assert_eq!(token_balance(&svm, &buyer_ata), 1_000);
}

#[test]
fn sell_rejects_foreign_token_program() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();

    // Everything else is a valid tranche; only the token program is
    // substituted. The vault PDA signs the CPI, so an unpinned token program
    // would extend the vault's signing authority into caller-chosen code.
    let mut ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[22u8; 32],
        1_000,
    );
    ix.accounts[7] = AccountMeta::new_readonly(foreign_token_program(), false);
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "a token program other than SPL Token must fail"
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000, "vault untouched");
}

#[test]
fn deposit_rejects_foreign_token_program() {
    let (
        mut svm,
        pid,
        owner,
        _policy,
        _exec,
        _crank,
        vault,
        mint,
        owner_ata,
        vault_ata,
        _buyer,
        _buyer_ata,
        pos,
    ) = setup_full(3, 1000, 10_000, 1);
    // 1 base unit was deposited by the setup; the rest is still in owner_ata.

    let mut d = Vec::with_capacity(DEPOSIT_IX_LEN);
    d.push(IX_DEPOSIT_TO_VAULT);
    d.extend_from_slice(&pos.to_le_bytes());
    d.extend_from_slice(&100u64.to_le_bytes());
    let ix = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new_readonly(foreign_token_program(), false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        ],
        data: d,
    };
    assert!(
        send(&mut svm, &[&owner], &ix).is_err(),
        "deposit through a foreign token program must fail"
    );
    assert_eq!(token_balance(&svm, &vault_ata), 1, "no extra deposit landed");
}

#[test]
fn close_vault_rejects_foreign_token_program() {
    let (
        mut svm,
        pid,
        owner,
        _policy,
        _exec,
        _crank,
        vault,
        mint,
        owner_ata,
        vault_ata,
        _buyer,
        _buyer_ata,
        _pos,
    ) = happy_setup();

    let close_ix = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(foreign_token_program(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: vec![5u8],
    };
    assert!(
        send(&mut svm, &[&owner], &close_ix).is_err(),
        "close through a foreign token program must fail"
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000, "vault untouched");
}

// ──────────────────────────────────────────────────────────────────────────
// Tranche budget
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn sell_rejects_amount_above_tranche_budget() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = happy_setup();

    // tranche_bps = 1000 (10%) of 10_000 deposited = 1_000 per tranche.
    // 1_001 is one base unit over budget and well under the vault balance,
    // so only the tranche bound can reject it.
    let over = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[23u8; 32],
        1_001,
    );
    assert!(
        send(&mut svm, &[&crank], &over).is_err(),
        "one base unit over the tranche budget must fail"
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);

    // Exactly the budget is allowed (the bound is inclusive).
    let exact = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[23u8; 32],
        1_000,
    );
    send(&mut svm, &[&crank], &exact).expect("exactly the tranche budget succeeds");
}

#[test]
fn tranche_budget_is_measured_against_deposits_not_the_remainder() {
    // 10% tranches: the committed schedule is ten of them.
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        vault,
        mint,
        _owner_ata,
        vault_ata,
        _buyer,
        buyer_ata,
        pos,
    ) = setup_full(3, 1000, 10_000, 0);

    // Ten 10% tranches drain the position exactly. Against a shrinking
    // denominator the tenth tranche would be 10% of what is left (~387) and
    // the position would never fully exit.
    for tranche in 0..10u8 {
        let ix = validate_sell_ix(
            pid,
            &crank.pubkey(),
            policy,
            execution,
            vault,
            vault_ata,
            buyer_ata,
            mint,
            pos,
            0,
            tranche,
            1_000_000 + tranche as u64,
            &[tranche; 32],
            1_000,
        );
        send(&mut svm, &[&crank], &ix).unwrap_or_else(|e| panic!("tranche {tranche}: {e}"));
    }
    assert_eq!(token_balance(&svm, &vault_ata), 0, "position fully exited");
    assert_eq!(token_balance(&svm, &buyer_ata), 10_000);
}
