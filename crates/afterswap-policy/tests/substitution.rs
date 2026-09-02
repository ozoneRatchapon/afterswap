//! G7 — the cross-account substitution gate.
//!
//! Every handler in `lib.rs` destructures a fixed account list. Being *on*
//! that list is not a permission: the runtime guarantees only that the
//! accounts exist, never that they are the ones the caller was entitled to
//! name. For each instruction this file walks the account list and swaps one
//! slot for an account the attacker controls, then asserts the program
//! refuses and that no tokens moved.
//!
//! The substitutes are deliberately *genuine*. `common::setup_attacker`
//! builds a second principal through the same instructions as the victim —
//! same program, same lengths, same `position_id`, same mint — so a handler
//! cannot pass these tests by checking a length or an owner field alone. A
//! `Pubkey::new_unique()` would be rejected before any gate logic ran and
//! would prove nothing.
//!
//! Slots already covered elsewhere are listed against their test rather than
//! duplicated here:
//!   * `ValidateAndSell` cranker      → `vault.rs::sell_rejects_unauthorized_cranker`
//!   * `ValidateAndSell` policy       → `vault.rs::sell_rejects_substituted_policy_account`
//!   * `ValidateAndSell` dest_ata     → `vault.rs::sell_rejects_destination_other_than_settlement_ata`
//!   * `ValidateAndSell` token_program→ `vault.rs::sell_rejects_foreign_token_program`
//!   * `DepositToVault`  token_program→ `vault.rs::deposit_rejects_foreign_token_program`
//!   * `DepositToVault`  dest_ata     → `vault.rs::deposit_creates_vault_and_moves_tokens` (authority check)
//!   * `CloseVault`      owner        → `vault.rs::close_vault_rejects_non_owner`
//!   * `CloseVault`      token_program→ `vault.rs::close_vault_rejects_foreign_token_program`
//!   * `AnchorFill`      policy/exec/memo → `anchor.rs::anchor_fill_rejects_a_substituted_*`

mod common;

use afterswap_policy::{
    CLOSE_IX_LEN, COMMIT_IX_LEN, DEPOSIT_IX_LEN, IX_CLOSE_VAULT, IX_COMMIT_POLICY,
    IX_DEPOSIT_TO_VAULT, IX_REVOKE_AUTHORIZATION, REVOKE_IX_LEN,
};
use common::{
    authorize_ix, create_aux_token_account, create_second_mint, happy_setup, send, setup_attacker,
    token_balance,
    validate_sell_ix, Attacker, ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use litesvm::LiteSVM;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_program;

/// Assert a rejected transaction failed for the stated reason.
///
/// `is_err()` alone would also pass if the transaction died in the runtime
/// before reaching the handler — an unfunded payer, a missing account — which
/// is precisely the false green a substitution test must not give.
fn assert_rejected(result: Result<(), String>, expected: &str, what: &str) {
    match result {
        Ok(()) => panic!("{what}: the substitution was ACCEPTED"),
        Err(e) => assert!(
            e.contains(expected),
            "{what}: expected `{expected}`, got {e}"
        ),
    }
}

/// The victim environment plus a second principal holding the same token.
///
/// `tranche_bps = 10_000` on the attacker's policy is the payload the
/// substitution would carry if it worked: one fill for the whole position.
/// Named for the same reason `common::FullSetup` is: thirteen positional
/// slots destructured at every call site, where a bare tuple lets the arity
/// drift out of sync silently.
type SubstitutionSetup = (
    LiteSVM,
    Pubkey,  // program_id
    Keypair, // victim owner
    Pubkey,  // victim policy
    Pubkey,  // victim execution
    Keypair, // victim crank
    Pubkey,  // victim vault
    Pubkey,  // mint
    Pubkey,  // victim owner ATA
    Pubkey,  // victim vault ATA
    Pubkey,  // buyer ATA (settlement)
    u64,     // position_id
    Attacker,
);

fn victim_and_attacker() -> SubstitutionSetup {
    let (mut svm, pid, owner, policy, execution, crank, vault, mint, owner_ata, vault_ata, _buyer, buyer_ata, pos) =
        happy_setup();
    let attacker = setup_attacker(&mut svm, pid, &owner, mint, pos, 4, 10_000, 5_000, 5_000);
    (
        svm, pid, owner, policy, execution, crank, vault, mint, owner_ata, vault_ata, buyer_ata,
        pos, attacker,
    )
}

// ──────────────────────────────────────────────────────────────────────────
// ValidateAndSell (tag 2)
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn sell_rejects_a_substituted_execution_account() {
    // The execution PDA *is* the authorization: `authorized_crank`,
    // `expires_unix`, `settlement_ata` and — through `exec_owner` — the seeds
    // every other account is checked against all come out of it. An attacker
    // who authorizes their own crank against their own execution PDA and then
    // names the victim's vault would, without the owner cross-check, be an
    // authorized crank spending someone else's position.
    let (mut svm, pid, _owner, policy, _execution, _crank, vault, mint, _owner_ata, vault_ata, buyer_ata, pos, atk) =
        victim_and_attacker();

    let ix = validate_sell_ix(
        pid,
        &atk.pubkey(), // the attacker is a legitimate crank — on their OWN execution
        policy,
        atk.execution, // <- substituted
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
    // The policy is derived from `exec_owner`, so the victim's policy no
    // longer matches the substituted execution's owner: rejected at the
    // step-1b seed check, before authorization is even considered.
    assert_rejected(
        send(&mut svm, &[&atk.keypair], &ix),
        "InvalidSeeds",
        "an execution account belonging to another owner",
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &buyer_ata), 0);
}

#[test]
fn sell_rejects_a_matched_foreign_policy_and_execution_pair() {
    // Sharper than the test above: substitute policy AND execution together,
    // so they agree with each other and both seed checks pass. What must stop
    // it now is the vault owner cross-check — `vault_owner != exec_owner` —
    // which is the only thing tying the account holding the tokens to the
    // authorization being presented.
    let (mut svm, pid, _owner, _policy, _execution, _crank, vault, mint, _owner_ata, vault_ata, buyer_ata, pos, atk) =
        victim_and_attacker();

    let ix = validate_sell_ix(
        pid,
        &atk.pubkey(),
        atk.policy,    // <- substituted, tranche_bps = 10_000
        atk.execution, // <- substituted, authorizes the attacker
        vault,         // the VICTIM's vault
        vault_ata,
        atk.ata, // the attacker's own settlement ATA, as their execution binds
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[21u8; 32],
        1_000,
    );
    assert_rejected(
        send(&mut svm, &[&atk.keypair], &ix),
        "InvalidAccountData",
        "a self-consistent foreign policy+execution pair against the victim's vault",
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &atk.ata), 0);
    assert_eq!(token_balance(&svm, &buyer_ata), 0, "nor did the victim's own settlement ATA move");
}

#[test]
fn sell_rejects_a_substituted_vault_account() {
    // Mirror image: the victim's own crank, authorization and policy, but the
    // attacker's vault in the `vault` slot. `vault_owner != exec_owner`
    // catches it — otherwise a crank authorized on a dust position could
    // drain any vault whose PDA it could name.
    let (mut svm, pid, _owner, policy, execution, crank, _vault, mint, _owner_ata, vault_ata, buyer_ata, pos, atk) =
        victim_and_attacker();

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        atk.vault, // <- substituted
        atk.vault_ata,
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[21u8; 32],
        1_000,
    );
    assert_rejected(
        send(&mut svm, &[&crank], &ix),
        "InvalidAccountData",
        "a vault belonging to another owner",
    );
    assert_eq!(token_balance(&svm, &atk.vault_ata), 5_000);
    assert_eq!(token_balance(&svm, &vault_ata), 10_000, "the victim's vault is untouched");
    assert_eq!(token_balance(&svm, &buyer_ata), 0);
}

#[test]
fn sell_rejects_a_source_ata_the_vault_does_not_control() {
    // `src_ata` is the account the tokens actually leave. The vault PDA signs
    // the `TransferChecked`, so the only accounts it can move from are those
    // it is the authority of — and the handler now also pins the address to
    // the vault's ATA for the mint, so this slot no longer rests on the SPL
    // Token program alone. The vault-controlled look-alike that the authority
    // check cannot see is covered by
    // `sell_rejects_a_vault_controlled_account_that_is_not_the_vault_ata`.
    let (mut svm, pid, _owner, policy, execution, crank, vault, mint, owner_ata, vault_ata, buyer_ata, pos, _atk) =
        victim_and_attacker();

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        owner_ata, // <- substituted: the victim owner's own ATA, same mint
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
        "the vault PDA is not the authority of the owner's ATA — the transfer must fail"
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &buyer_ata), 0);
}

#[test]
fn sell_rejects_a_foreign_vaults_source_ata() {
    // Same slot, harder substitute: a token account whose authority is a
    // *vault PDA of this very program*, just a different owner's. The victim
    // vault still signs, so the authority still mismatches.
    let (mut svm, pid, _owner, policy, execution, crank, vault, mint, _owner_ata, vault_ata, buyer_ata, pos, atk) =
        victim_and_attacker();

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        atk.vault_ata, // <- substituted
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
        "another vault's ATA must not be spendable by this vault's signature"
    );
    assert_eq!(token_balance(&svm, &atk.vault_ata), 5_000);
    assert_eq!(token_balance(&svm, &vault_ata), 10_000, "and the real source is untouched");
    assert_eq!(token_balance(&svm, &buyer_ata), 0);
}

#[test]
fn sell_rejects_a_vault_controlled_account_that_is_not_the_vault_ata() {
    // The substitute the authority field cannot catch: a token account for
    // the same mint whose authority *is* the victim's vault PDA, so the
    // vault's own signature moves it and SPL Token is satisfied. Anyone can
    // create one and fund it with dust.
    //
    // What it buys an authorized crank: the transfer succeeds against the
    // decoy, the handler still debits `vault.amount` by the full tranche, and
    // after ten legal tranches the accounting reads zero while every real
    // token is still sitting in the canonical vault ATA — where `CloseVault`
    // will no-op on `amount == 0` and strand it. Only the address derivation
    // says *which* account, so only the address derivation stops this.
    let (mut svm, pid, owner, policy, execution, crank, vault, mint, _owner_ata, vault_ata, buyer_ata, pos, _atk) =
        victim_and_attacker();

    let decoy = create_aux_token_account(&mut svm, &owner, &owner, &vault, &mint, 5_000);

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        decoy, // <- substituted: vault is the authority, but it is not the ATA
        buyer_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[21u8; 32],
        1_000,
    );
    assert_rejected(
        send(&mut svm, &[&crank], &ix),
        "InvalidAccountData",
        "a vault-controlled look-alike source",
    );
    assert_eq!(token_balance(&svm, &decoy), 5_000, "the decoy is untouched");
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &buyer_ata), 0);

    // And the canonical source still works, so the binding is not a blanket
    // refusal of the slot.
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
    send(&mut svm, &[&crank], &ix).expect("the vault's own ATA succeeds");
    assert_eq!(token_balance(&svm, &buyer_ata), 1_000);
}

#[test]
fn sell_rejects_a_substituted_mint() {
    // The `mint` slot is read for one byte — `decimals`, which feeds
    // `TransferChecked`. A substituted mint with different decimals would
    // make the checked transfer mean something other than what the caller
    // signed for, so the token program's own mint/decimals agreement is the
    // backstop. Assert it holds rather than assuming it.
    let (mut svm, pid, owner, policy, execution, crank, vault, _mint, _owner_ata, vault_ata, buyer_ata, pos, _atk) =
        victim_and_attacker();
    let other_mint = create_second_mint(&mut svm, &owner, 3);

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        vault_ata,
        buyer_ata,
        other_mint, // <- substituted
        pos,
        0,
        0,
        1_000_000,
        &[21u8; 32],
        1_000,
    );
    assert!(
        send(&mut svm, &[&crank], &ix).is_err(),
        "a mint that is not the token accounts' mint must be rejected"
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &buyer_ata), 0);
}

// ──────────────────────────────────────────────────────────────────────────
// DepositToVault (tag 4)
// ──────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn deposit_ix(
    program_id: Pubkey,
    owner: &Pubkey,
    vault: Pubkey,
    source_ata: Pubkey,
    mint: Pubkey,
    dest_ata: Pubkey,
    position_id: u64,
    amount: u64,
) -> Instruction {
    let mut d = Vec::with_capacity(DEPOSIT_IX_LEN);
    d.push(IX_DEPOSIT_TO_VAULT);
    d.extend_from_slice(&position_id.to_le_bytes());
    d.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(vault, false),
            AccountMeta::new(source_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(dest_ata, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        ],
        data: d,
    }
}

#[test]
fn deposit_rejects_a_vault_belonging_to_another_owner() {
    // The vault PDA is derived from the *signer*, so naming someone else's
    // vault cannot type-check against the seeds. Without that derivation a
    // deposit could credit another owner's vault balance — funding a position
    // whose exits that owner controls.
    let (mut svm, pid, owner, _policy, _execution, _crank, _vault, mint, owner_ata, _vault_ata, _buyer_ata, pos, atk) =
        victim_and_attacker();

    let ix = deposit_ix(
        pid,
        &owner.pubkey(),
        atk.vault, // <- substituted
        owner_ata,
        mint,
        atk.vault_ata,
        pos,
        100,
    );
    assert_rejected(
        send(&mut svm, &[&owner], &ix),
        "InvalidSeeds",
        "a vault PDA derived from a different owner",
    );
    assert_eq!(token_balance(&svm, &atk.vault_ata), 5_000);
}

#[test]
fn deposit_rejects_a_source_ata_owned_by_another_party() {
    // The attacker deposits into their own vault — every seed derivation
    // agrees — but names the victim's ATA as the source. The owner signs the
    // `TransferChecked`, and they are not the victim's token authority.
    let (mut svm, pid, _owner, _policy, _execution, _crank, _vault, mint, owner_ata, _vault_ata, _buyer_ata, pos, atk) =
        victim_and_attacker();

    let ix = deposit_ix(
        pid,
        &atk.pubkey(),
        atk.vault,
        owner_ata, // <- substituted: the victim's tokens
        mint,
        atk.vault_ata,
        pos,
        100,
    );
    assert!(
        send(&mut svm, &[&atk.keypair], &ix).is_err(),
        "a deposit must not be able to pull from an ATA the signer does not control"
    );
    assert_eq!(token_balance(&svm, &owner_ata), 0, "victim already deposited");
    assert_eq!(token_balance(&svm, &atk.vault_ata), 5_000);
}

// ──────────────────────────────────────────────────────────────────────────
// CloseVault (tag 5)
// ──────────────────────────────────────────────────────────────────────────

fn close_ix(
    program_id: Pubkey,
    owner: &Pubkey,
    vault: Pubkey,
    src_ata: Pubkey,
    dest_ata: Pubkey,
    mint: Pubkey,
) -> Instruction {
    let mut d = Vec::with_capacity(CLOSE_IX_LEN);
    d.push(IX_CLOSE_VAULT);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(vault, false),
            AccountMeta::new(src_ata, false),
            AccountMeta::new(dest_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: d,
    }
}

#[test]
fn close_rejects_a_vault_belonging_to_another_owner() {
    // `close_vault` reads the owner out of the vault account and compares it
    // to the signer, then re-derives the seeds. The attacker holds a real
    // vault of their own, so this is not "an account of the wrong shape" —
    // it is the right shape with the wrong owner.
    let (mut svm, pid, owner, _policy, _execution, _crank, _vault, mint, owner_ata, _vault_ata, _buyer_ata, _pos, atk) =
        victim_and_attacker();

    let ix = close_ix(
        pid,
        &owner.pubkey(),
        atk.vault, // <- substituted
        atk.vault_ata,
        owner_ata,
        mint,
    );
    assert_rejected(
        send(&mut svm, &[&owner], &ix),
        "InvalidAccountData",
        "closing a vault the signer does not own",
    );
    assert_eq!(token_balance(&svm, &atk.vault_ata), 5_000);
}

#[test]
fn close_rejects_a_source_ata_the_vault_does_not_control() {
    // The victim closes their own vault but names the attacker's vault ATA as
    // the source. Their vault PDA signs, so the authority mismatches — the
    // same unchecked slot as on the sell path, asserted here too.
    let (mut svm, pid, owner, _policy, _execution, _crank, vault, mint, owner_ata, vault_ata, _buyer_ata, _pos, atk) =
        victim_and_attacker();

    let ix = close_ix(
        pid,
        &owner.pubkey(),
        vault,
        atk.vault_ata, // <- substituted
        owner_ata,
        mint,
    );
    assert!(
        send(&mut svm, &[&owner], &ix).is_err(),
        "one vault's signature must not move another vault's tokens"
    );
    assert_eq!(token_balance(&svm, &atk.vault_ata), 5_000);
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &owner_ata), 0);
}

#[test]
fn close_rejects_a_vault_controlled_account_that_is_not_the_vault_ata() {
    // Same look-alike on the close path. The owner signs here, so this is not
    // theft — it is the accounting drifting from the tokens, which is exactly
    // what makes the stranding above possible. Both paths bind the same way.
    let (mut svm, pid, owner, _policy, _execution, _crank, vault, mint, owner_ata, vault_ata, _buyer_ata, _pos, _atk) =
        victim_and_attacker();

    let decoy = create_aux_token_account(&mut svm, &owner, &owner, &vault, &mint, 5_000);

    let ix = close_ix(pid, &owner.pubkey(), vault, decoy, owner_ata, mint);
    assert_rejected(
        send(&mut svm, &[&owner], &ix),
        "InvalidAccountData",
        "a vault-controlled look-alike source on close",
    );
    assert_eq!(token_balance(&svm, &decoy), 5_000);
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &owner_ata), 0);

    // The canonical source reclaims the position as before.
    let ix = close_ix(pid, &owner.pubkey(), vault, vault_ata, owner_ata, mint);
    send(&mut svm, &[&owner], &ix).expect("the vault's own ATA succeeds");
    assert_eq!(token_balance(&svm, &owner_ata), 10_000);
}

// ──────────────────────────────────────────────────────────────────────────
// CommitPolicy (tag 0) / AuthorizeExecution (tag 1) / RevokeAuthorization (tag 3)
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn commit_rejects_a_policy_pda_belonging_to_another_owner() {
    // A policy is immutable once committed, so overwriting the victim's is
    // already impossible — but the seed check must reject the attempt on
    // seeds, not merely on `AccountAlreadyInitialized`, or the same call
    // against an *uncommitted* position would plant a policy under the
    // victim's PDA. Both are asserted.
    let (mut svm, pid, _owner, policy, _execution, _crank, _vault, _mint, _owner_ata, _vault_ata, _buyer_ata, pos, atk) =
        victim_and_attacker();

    let commit = |policy_account: Pubkey, position_id: u64| {
        let mut d = Vec::with_capacity(COMMIT_IX_LEN);
        d.push(IX_COMMIT_POLICY);
        d.extend_from_slice(&position_id.to_le_bytes());
        d.extend_from_slice(&0xdead_beef_dead_beef_u64.to_le_bytes());
        d.push(1);
        d.extend_from_slice(&10_000u16.to_le_bytes());
        Instruction {
            program_id: pid,
            accounts: vec![
                AccountMeta::new(atk.pubkey(), true),
                AccountMeta::new(policy_account, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: d,
        }
    };

    assert_rejected(
        send(&mut svm, &[&atk.keypair], &commit(policy, pos)),
        "InvalidSeeds",
        "committing over the victim's existing policy PDA",
    );

    // The same substitution against a position the victim has never
    // committed: nothing to collide with, so only the seeds can stop it.
    let fresh = common::pda(afterswap_policy::POLICY_SEED, &pid, &_owner.pubkey(), pos + 1).0;
    assert_rejected(
        send(&mut svm, &[&atk.keypair], &commit(fresh, pos + 1)),
        "InvalidSeeds",
        "planting a policy under an uncommitted PDA of the victim's",
    );
}

#[test]
fn authorize_rejects_an_execution_pda_belonging_to_another_owner() {
    // `AuthorizeExecution` is what names the crank and the settlement ATA. If
    // the execution slot were not checked against the signer, an attacker
    // could rebind the victim's authorization to their own crank and their
    // own payout account — the whole gate in one instruction.
    //
    // The handler has two arms and each one stops this differently, so both
    // are exercised: an *initialized* PDA is caught by the stored-owner
    // comparison (`out[0..32] != owner`), which runs before the seed
    // derivation; an *uninitialized* one has no stored owner to compare, so
    // only the seed check stands between the attacker and a planted
    // authorization under the victim's address.
    let (mut svm, pid, owner, _policy, execution, _crank, _vault, _mint, _owner_ata, _vault_ata, _buyer_ata, pos, atk) =
        victim_and_attacker();

    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64) + 3_600;
    let rebind = |execution_account: Pubkey, position_id: u64| {
        authorize_ix(
            pid,
            &atk.pubkey(),
            execution_account,
            &atk.pubkey(),
            position_id,
            expires,
            &atk.ata,
        )
    };

    assert_rejected(
        send(&mut svm, &[&atk.keypair], &rebind(execution, pos)),
        "InvalidAccountData",
        "rebinding another owner's live authorization",
    );

    // The victim's authorization is untouched: still their crank, and still
    // a *live* one — a rebind that got as far as the "already initialized"
    // arm without the owner check would have overwritten these bytes.
    let acct = svm.get_account(&execution).expect("execution account");
    assert_ne!(
        acct.data[40..72],
        atk.pubkey().to_bytes(),
        "the crank was not rebound to the attacker"
    );
    assert_ne!(
        acct.data[98..130],
        atk.ata.to_bytes(),
        "the settlement destination was not rebound to the attacker"
    );

    // Uninitialized arm: a position the victim has never authorized, so the
    // PDA does not exist yet and there is no stored owner to catch it.
    let fresh = common::pda(afterswap_policy::EXECUTION_SEED, &pid, &owner.pubkey(), pos + 1).0;
    assert_rejected(
        send(&mut svm, &[&atk.keypair], &rebind(fresh, pos + 1)),
        "InvalidSeeds",
        "planting an authorization under an uninitialized PDA of the victim's",
    );
}

#[test]
fn revoke_rejects_an_execution_pda_belonging_to_another_owner() {
    // Revoking someone else's authorization is a denial-of-service on their
    // exit: the crank stops filling and the position sits through the move it
    // was committed to sell into. `revoke` compares the stored owner to the
    // signer before it clears anything.
    let (mut svm, pid, _owner, _policy, execution, crank, _vault, _mint, _owner_ata, _vault_ata, _buyer_ata, _pos, atk) =
        victim_and_attacker();

    let ix = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(atk.pubkey(), true),
            AccountMeta::new(execution, false), // <- substituted
        ],
        data: {
            let mut d = Vec::with_capacity(REVOKE_IX_LEN);
            d.push(IX_REVOKE_AUTHORIZATION);
            d
        },
    };
    assert_rejected(
        send(&mut svm, &[&atk.keypair], &ix),
        "InvalidAccountData",
        "revoking another owner's authorization",
    );

    // And the victim's crank still works afterwards.
    let acct = svm.get_account(&execution).expect("execution account");
    assert_eq!(
        acct.data[40..72],
        crank.pubkey().to_bytes(),
        "the authorized crank is untouched"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Account-list reordering
// ──────────────────────────────────────────────────────────────────────────
//
// The tests above swap a slot for a *foreign* account. Reordering is the
// cheaper attack: pass the handler's own accounts, in the wrong order. Most
// slot pairs cannot be confused — `policy` (60 bytes), `vault` (89) and
// `execution` (129) are distinguished by length before any seed derivation
// runs, and each is then pinned to its PDA address. The pair that *is*
// interchangeable by shape is the two token accounts, which are both 165
// bytes of the same layout for the same mint. Those are the swaps worth
// pinning: nothing about the account's contents distinguishes them, so only
// the direction of the two bindings does.

#[test]
fn sell_rejects_the_source_and_destination_slots_swapped() {
    // Step 5b binds the source to the vault's ATA and step 6 binds the
    // destination to the settlement ATA the owner named. Swapped, each
    // account is individually legitimate and appears in an account list the
    // handler expects — the transfer would simply run backwards, pulling the
    // buyer's tokens into the vault while `vault.amount` is debited as
    // though a sale had happened.
    let (mut svm, pid, _owner, policy, execution, crank, vault, mint, _owner_ata, vault_ata, buyer_ata, pos, _atk) =
        victim_and_attacker();

    let ix = validate_sell_ix(
        pid,
        &crank.pubkey(),
        policy,
        execution,
        vault,
        buyer_ata, // <- source and destination exchanged
        vault_ata,
        mint,
        pos,
        0,
        0,
        1_000_000,
        &[22u8; 32],
        1_000,
    );
    assert_rejected(
        send(&mut svm, &[&crank], &ix),
        "InvalidAccountData",
        "the sell source and destination slots exchanged",
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000, "nothing moved in");
    assert_eq!(token_balance(&svm, &buyer_ata), 0, "nothing moved out");
}

#[test]
fn close_rejects_the_source_and_destination_slots_swapped() {
    // `close_vault` is owner-signed, so this is drift rather than theft —
    // but the source binding must hold regardless of who is asking. Reversed,
    // the handler would be instructed to pull the owner's own balance into
    // the vault and then zero `vault.amount`, stranding both sides.
    let (mut svm, pid, owner, _policy, _execution, _crank, vault, mint, owner_ata, vault_ata, _buyer_ata, _pos, _atk) =
        victim_and_attacker();

    let before_owner = token_balance(&svm, &owner_ata);
    let ix = close_ix(pid, &owner.pubkey(), vault, owner_ata, vault_ata, mint);
    assert_rejected(
        send(&mut svm, &[&owner], &ix),
        "InvalidAccountData",
        "the close source and destination slots exchanged",
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &owner_ata), before_owner);

    // The correct order still drains the vault, so the binding is directional
    // rather than a refusal of the slot.
    let ix = close_ix(pid, &owner.pubkey(), vault, vault_ata, owner_ata, mint);
    send(&mut svm, &[&owner], &ix).expect("the canonical order succeeds");
    assert_eq!(token_balance(&svm, &vault_ata), 0);
    assert_eq!(token_balance(&svm, &owner_ata), before_owner + 10_000);
}

#[test]
fn sell_rejects_a_vault_pda_this_program_does_not_own() {
    // The seed derivation proves the *address*; it does not prove who wrote
    // the bytes. In practice nothing else can create an account at a PDA of
    // this program, so this is defence in depth rather than a closed hole —
    // but `validate_and_sell` reads `vault_amount`, `vault_deposited` and
    // `vault_mint` out of this account sixty lines before the derivation
    // runs, and every step-5 bound is computed from them. `set_account` lets
    // the test do what the chain will not, and pins the early refusal.
    //
    // Without the owner check the instruction gets as far as the transfer and
    // then dies in the runtime on the post-instruction write-back — no state
    // change either way, but `IllegalOwner` before the CPI is the honest
    // failure.
    let (mut svm, pid, _owner, policy, execution, crank, vault, mint, _owner_ata, vault_ata, buyer_ata, pos, _atk) =
        victim_and_attacker();

    let mut hijacked = svm.get_account(&vault).expect("the vault exists");
    assert_eq!(hijacked.owner, pid, "the vault starts out program-owned");
    hijacked.owner = system_program::id();
    svm.set_account(vault, hijacked)
        .expect("re-owning the vault account");

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
        &[23u8; 32],
        1_000,
    );
    assert_rejected(
        send(&mut svm, &[&crank], &ix),
        "IllegalOwner",
        "a vault PDA owned by another program",
    );
    assert_eq!(token_balance(&svm, &vault_ata), 10_000);
    assert_eq!(token_balance(&svm, &buyer_ata), 0);
}
