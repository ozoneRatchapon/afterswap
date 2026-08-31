//! LiteSVM tests for `AnchorFill` (tag 6) against the real compiled SBF
//! binary — test-plan case 9 of `docs/PHASE_B_DELEGATED_EXECUTION.md` §7.
//!
//! `AnchorFill` writes no account state; its entire output is a memo, so
//! every assertion here reads transaction logs. The environment builder
//! lives in `common/mod.rs`.

mod common;

use afterswap_policy::{
    ANCHOR_FILL_IX_LEN, COMMIT_IX_LEN, EXECUTION_SEED, IX_ANCHOR_FILL, IX_COMMIT_POLICY,
    POLICY_SEED,
};
use common::{happy_setup, send, send_logs, validate_sell_ix};
use litesvm::LiteSVM;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_program;

/// SPL Memo v3 — bundled with LiteSVM, so no `add_program` is needed.
const MEMO_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");

/// The fingerprint `common::setup_full` commits, rendered the way the program
/// renders it: lowercase hex, zero-padded to 16 digits.
const COMMITTED_FP_HEX: &str = "165ef4aabbccddee";

fn anchor_fill_ix(
    program_id: Pubkey,
    signer: &Pubkey,
    policy: Pubkey,
    execution: Pubkey,
    memo_program: Pubkey,
    quote_digest: &[u8; 32],
) -> Instruction {
    let mut d = Vec::with_capacity(ANCHOR_FILL_IX_LEN);
    d.push(IX_ANCHOR_FILL);
    d.extend_from_slice(quote_digest);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(*signer, true),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new_readonly(execution, false),
            AccountMeta::new_readonly(memo_program, false),
        ],
        data: d,
    }
}

/// Hex-render a digest the way the program does, for log comparison.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Drive `happy_setup` through one real sell so there is a fill to anchor.
/// Returns the full setup plus the tick slot the sell recorded.
#[allow(clippy::type_complexity)]
fn setup_with_one_fill() -> (LiteSVM, Pubkey, Keypair, Pubkey, Pubkey, Keypair, u64, u64) {
    let (
        mut svm,
        pid,
        owner,
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
    let tick_slot = 1_000_000u64;
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
        tick_slot,
        &[7u8; 32],
        // One legal tranche: 10% of the 10_000 `happy_setup` deposits.
        1_000,
    );
    send(&mut svm, &[&crank], &ix).expect("sell succeeds");
    (svm, pid, owner, policy, execution, crank, pos, tick_slot)
}

#[test]
fn anchor_fill_emits_the_memo_after_a_sell() {
    let (mut svm, pid, _owner, policy, execution, crank, pos, tick_slot) = setup_with_one_fill();
    let digest = [0xabu8; 32];

    let logs = send_logs(
        &mut svm,
        &[&crank],
        &anchor_fill_ix(
            pid,
            &crank.pubkey(),
            policy,
            execution,
            MEMO_PROGRAM_ID,
            &digest,
        ),
    )
    .expect("anchor succeeds");

    // Every field but the quote digest is read from on-chain state: the
    // fingerprint from the policy PDA, the tranche index and tick slot from
    // the execution PDA the sell just advanced.
    let expected = format!(
        "afterswap:fill fp={COMMITTED_FP_HEX} pos={pos} tranche=0 slot={tick_slot} quote=sha256:{}",
        hex(&digest)
    );
    assert!(
        logs.iter().any(|l| l.contains(&expected)),
        "memo body not found in logs.\nexpected: {expected}\nlogs: {logs:#?}"
    );
}

#[test]
fn anchor_fill_reports_the_second_tranche_after_a_second_sell() {
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

    // Two sells, two different tick slots. The memo must follow state.
    for (tranche, tick) in [(0u8, 1_000_000u64), (1u8, 2_000_000u64)] {
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
            tranche,
            tranche,
            tick,
            &[7u8; 32],
            1_000,
        );
        send(&mut svm, &[&crank], &ix).expect("sell succeeds");
    }

    let logs = send_logs(
        &mut svm,
        &[&crank],
        &anchor_fill_ix(
            pid,
            &crank.pubkey(),
            policy,
            execution,
            MEMO_PROGRAM_ID,
            &[1u8; 32],
        ),
    )
    .expect("anchor succeeds");

    // tranche 1, not 0, and the *second* sell's tick slot — the index is
    // `tranches_filled - 1`, never a caller-supplied value.
    let expected = format!("pos={pos} tranche=1 slot=2000000 ");
    assert!(
        logs.iter().any(|l| l.contains(&expected)),
        "expected {expected} in logs: {logs:#?}"
    );
}

#[test]
fn anchor_fill_rejects_before_any_fill() {
    let (
        mut svm,
        pid,
        _owner,
        policy,
        execution,
        crank,
        _vault,
        _mint,
        _owner_ata,
        _vault_ata,
        _buyer,
        _buyer_ata,
        _pos,
    ) = happy_setup();

    let err = send_logs(
        &mut svm,
        &[&crank],
        &anchor_fill_ix(
            pid,
            &crank.pubkey(),
            policy,
            execution,
            MEMO_PROGRAM_ID,
            &[0u8; 32],
        ),
    )
    .expect_err("nothing has been sold — there is no fill to anchor");
    assert!(err.contains("Custom(3)"), "expected Custom(3), got {err}");
}

#[test]
fn anchor_fill_rejects_an_unauthorized_signer() {
    let (mut svm, pid, _owner, policy, execution, _crank, _pos, _) = setup_with_one_fill();
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000)
        .expect("airdrop");

    let err = send_logs(
        &mut svm,
        &[&stranger],
        &anchor_fill_ix(
            pid,
            &stranger.pubkey(),
            policy,
            execution,
            MEMO_PROGRAM_ID,
            &[0xffu8; 32],
        ),
    )
    .expect_err("only the authorized crank or the owner may bind a quote digest");
    assert!(err.contains("Custom(1)"), "expected Custom(1), got {err}");
}

#[test]
fn anchor_fill_accepts_the_owner() {
    let (mut svm, pid, owner, policy, execution, _crank, pos, _) = setup_with_one_fill();
    let logs = send_logs(
        &mut svm,
        &[&owner],
        &anchor_fill_ix(
            pid,
            &owner.pubkey(),
            policy,
            execution,
            MEMO_PROGRAM_ID,
            &[2u8; 32],
        ),
    )
    .expect("owner may anchor");
    assert!(logs.iter().any(|l| l.contains(&format!("pos={pos} "))));
}

#[test]
fn anchor_fill_rejects_a_foreign_memo_program() {
    let (mut svm, pid, _owner, policy, execution, crank, _pos, _) = setup_with_one_fill();
    let err = send_logs(
        &mut svm,
        &[&crank],
        &anchor_fill_ix(
            pid,
            &crank.pubkey(),
            policy,
            execution,
            Pubkey::new_unique(),
            &[0u8; 32],
        ),
    )
    .expect_err("the memo program is pinned");
    assert!(
        err.contains("IncorrectProgramId"),
        "expected IncorrectProgramId, got {err}"
    );
}

/// The same attack `sell_rejects_substituted_policy_account` covers, aimed at
/// the memo instead of the gate: the fingerprint in the memo is the whole
/// point of the anchor, so an attacker-supplied policy account would let a
/// crank publish a chain link naming a machine that never governed the
/// position. The PDA derivation is what stops it.
#[test]
fn anchor_fill_rejects_a_substituted_policy_account() {
    let (mut svm, pid, _owner, _policy, execution, crank, pos, _) = setup_with_one_fill();

    // The crank commits its own, genuinely program-owned, correctly sized
    // policy for the same position_id — carrying a different fingerprint.
    let attacker_policy = Pubkey::find_program_address(
        &[POLICY_SEED, crank.pubkey().as_ref(), &pos.to_le_bytes()],
        &pid,
    )
    .0;
    let mut d = Vec::with_capacity(COMMIT_IX_LEN);
    d.push(IX_COMMIT_POLICY);
    d.extend_from_slice(&pos.to_le_bytes());
    d.extend_from_slice(&0xdead_beef_dead_beef_u64.to_le_bytes());
    d.push(3);
    d.extend_from_slice(&1000u16.to_le_bytes());
    send(
        &mut svm,
        &[&crank],
        &Instruction {
            program_id: pid,
            accounts: vec![
                AccountMeta::new(crank.pubkey(), true),
                AccountMeta::new(attacker_policy, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: d,
        },
    )
    .expect("attacker's own policy commits fine — it is their position id space");

    let err = send_logs(
        &mut svm,
        &[&crank],
        &anchor_fill_ix(
            pid,
            &crank.pubkey(),
            attacker_policy,
            execution,
            MEMO_PROGRAM_ID,
            &[0u8; 32],
        ),
    )
    .expect_err("a policy account that is not the position's policy PDA must be refused");
    assert!(
        err.contains("InvalidSeeds"),
        "expected InvalidSeeds, got {err}"
    );
}

/// The execution PDA carries the tranche index and tick slot the memo
/// reports, so it needs the same derivation check as the policy account.
#[test]
fn anchor_fill_rejects_a_substituted_execution_account() {
    let (mut svm, pid, _owner, policy, _execution, crank, pos, _) = setup_with_one_fill();
    let foreign_execution = Pubkey::find_program_address(
        &[EXECUTION_SEED, crank.pubkey().as_ref(), &pos.to_le_bytes()],
        &pid,
    )
    .0;

    let err = send_logs(
        &mut svm,
        &[&crank],
        &anchor_fill_ix(
            pid,
            &crank.pubkey(),
            policy,
            foreign_execution,
            MEMO_PROGRAM_ID,
            &[0u8; 32],
        ),
    )
    .expect_err("an execution account this program never created must be refused");
    assert!(
        err.contains("InvalidAccountData") || err.contains("InvalidSeeds"),
        "expected an account-validation failure, got {err}"
    );
}
