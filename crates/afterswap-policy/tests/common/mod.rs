//! Shared LiteSVM harness for the Phase B step 2 tests.
//!
//! Extracted from `vault.rs` when `anchor.rs` needed the same environment —
//! `setup_full` builds thirteen linked accounts and duplicating it would let
//! the two copies drift, which is exactly the failure this harness already
//! suffered once when its arity changed under its call sites.
//!
//! Harness style mirrors `tests/policy.rs` / `tests/execution.rs`:
//! plain `LiteSVM::new()` + `add_program` + `airdrop`, no devnet. SPL token
//! accounts are created manually (the `litesvm_token` helpers only accept
//! `Pubkey` signers, which is not enough here).
//!
//! Each test binary links this module separately, so items only one of them
//! uses look dead to the other. That is what `#![allow(dead_code)]` is for
//! here; it is not covering unused production code.
#![allow(dead_code)]

use afterswap_policy::{
    AUTHORIZE_IX_LEN, COMMIT_IX_LEN, DEPOSIT_IX_LEN, EXECUTION_SEED, IX_AUTHORIZE_EXECUTION,
    IX_COMMIT_POLICY, IX_DEPOSIT_TO_VAULT, IX_VALIDATE_SELL, POLICY_SEED, VALIDATE_SELL_IX_LEN,
    VAULT_SEED,
};
use litesvm::LiteSVM;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::system_program;
use solana_sdk::transaction::Transaction;
use spl_token::instruction;

pub const TOKEN_PROGRAM_ID: Pubkey = spl_token::id();
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = spl_associated_token_account::id();

pub fn program_so() -> Vec<u8> {
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

pub fn send(svm: &mut LiteSVM, signers: &[&Keypair], ix: &Instruction) -> Result<(), String> {
    let payer = signers[0].pubkey();
    let blockhash = svm.latest_blockhash();
    let mut tx = Transaction::new_unsigned(Message::new(&[(*ix).clone()], Some(&payer)));
    tx.message.recent_blockhash = blockhash;
    for s in signers {
        tx.partial_sign(&[*s], blockhash);
    }
    match svm.send_transaction(tx) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("err={:?} logs={:?}", e.err, e.meta.logs)),
    }
}

/// Like `send`, but hands back the transaction logs on success — the only
/// way to observe an `AnchorFill` memo, which writes no account state.
pub fn send_logs(
    svm: &mut LiteSVM,
    signers: &[&Keypair],
    ix: &Instruction,
) -> Result<Vec<String>, String> {
    let payer = signers[0].pubkey();
    let blockhash = svm.latest_blockhash();
    let mut tx = Transaction::new_unsigned(Message::new(&[(*ix).clone()], Some(&payer)));
    tx.message.recent_blockhash = blockhash;
    for s in signers {
        tx.partial_sign(&[*s], blockhash);
    }
    match svm.send_transaction(tx) {
        Ok(meta) => Ok(meta.logs),
        Err(e) => Err(format!("err={:?} logs={:?}", e.err, e.meta.logs)),
    }
}

/// Build an `AuthorizeExecution` (tag 1) instruction.
///
/// `settlement_ata` is the payout account the authorization binds — the only
/// destination `ValidateAndSell` will pay into afterwards.
pub fn authorize_ix(
    program_id: Pubkey,
    owner: &Pubkey,
    execution: Pubkey,
    crank: &Pubkey,
    position_id: u64,
    expires_unix: u64,
    settlement_ata: &Pubkey,
) -> Instruction {
    let mut d = Vec::with_capacity(AUTHORIZE_IX_LEN);
    d.push(IX_AUTHORIZE_EXECUTION);
    d.extend_from_slice(&position_id.to_le_bytes());
    d.extend_from_slice(crank.as_ref());
    d.extend_from_slice(&expires_unix.to_le_bytes());
    d.extend_from_slice(settlement_ata.as_ref());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(execution, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: d,
    }
}

pub fn pda(seed: &[u8], program_id: &Pubkey, owner: &Pubkey, position_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[seed, owner.as_ref(), &position_id.to_le_bytes()],
        program_id,
    )
}

/// Everything `setup_full` hands back. Named because it appears in two
/// signatures and thirteen positional destructures — a bare tuple here is what
/// let the arity drift out of sync with the call sites in the first place.
pub type FullSetup = (
    LiteSVM,
    Pubkey,  // program_id
    Keypair, // owner
    Pubkey,  // policy PDA
    Pubkey,  // execution PDA
    Keypair, // crank
    Pubkey,  // vault PDA
    Pubkey,  // mint
    Pubkey,  // owner ATA
    Pubkey,  // vault ATA
    Keypair, // buyer (sell destination)
    Pubkey,  // buyer ATA
    u64,     // position_id
);

/// Build the full Phase B step 2 environment:
/// commit policy + authorize crank + mint + ATAs + deposit into the vault.
pub fn setup_full(
    n_states: u8,
    tranche_bps: u16,
    mint_amount: u64,
    deposit_amount: u64,
) -> FullSetup {
    let mut svm = LiteSVM::new();
    let program_id = Pubkey::new_unique();
    svm.add_program(program_id, &program_so());
    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 1_000_000_000)
        .expect("airdrop");
    let crank = Keypair::new();
    // The crank is the fee payer on every `ValidateAndSell` it signs, so it
    // needs lamports of its own — an unfunded signer fails as `AccountNotFound`
    // before the program is ever reached.
    svm.airdrop(&crank.pubkey(), 1_000_000_000)
        .expect("airdrop crank");
    let buyer = Keypair::new();
    let position_id = 42u64;

    let (policy, _) = pda(POLICY_SEED, &program_id, &owner.pubkey(), position_id);
    let (execution, _) = pda(EXECUTION_SEED, &program_id, &owner.pubkey(), position_id);
    let (vault, _) = pda(VAULT_SEED, &program_id, &owner.pubkey(), position_id);

    // ── Commit policy (tag 0) ───────────────────────────────────────────
    let mut d = Vec::with_capacity(COMMIT_IX_LEN);
    d.push(IX_COMMIT_POLICY);
    d.extend_from_slice(&position_id.to_le_bytes());
    d.extend_from_slice(&0x165e_f4aa_bbcc_ddee_u64.to_le_bytes());
    d.push(n_states);
    d.extend_from_slice(&tranche_bps.to_le_bytes());
    send(
        &mut svm,
        &[&owner],
        &Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(policy, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: d,
        },
    )
    .expect("commit policy");

    // ── ATA addresses ───────────────────────────────────────────────────
    // Derived before `AuthorizeExecution` because that instruction now binds
    // the settlement destination: the owner names the payout ATA when they
    // name the crank. Derivation is pure — none of these accounts exist yet.
    let mint_kp = Keypair::new();
    let mint_pk = mint_kp.pubkey();
    let owner_ata = Pubkey::find_program_address(
        &[
            owner.pubkey().as_ref(),
            TOKEN_PROGRAM_ID.as_ref(),
            mint_pk.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0;
    let vault_ata = Pubkey::find_program_address(
        &[vault.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint_pk.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0;
    let buyer_ata = Pubkey::find_program_address(
        &[
            buyer.pubkey().as_ref(),
            TOKEN_PROGRAM_ID.as_ref(),
            mint_pk.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0;

    // ── Authorize crank (tag 1) ─────────────────────────────────────────
    // Settlement destination = the buyer's ATA: the one account tag 2 may
    // pay into for this authorization.
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);
    send(
        &mut svm,
        &[&owner],
        &authorize_ix(
            program_id,
            &owner.pubkey(),
            execution,
            &crank.pubkey(),
            position_id,
            expires,
            &buyer_ata,
        ),
    )
    .expect("authorize crank");

    // ── SPL token: mint + ATAs + mint tokens ────────────────────────────
    // Tx 1a: create mint account (owner pays rent, mint PDA signs).
    const MINT_LEN: u64 = 82;
    let ix = system_instruction::create_account(
        &owner.pubkey(),
        &mint_pk,
        svm.minimum_balance_for_rent_exemption(MINT_LEN as usize),
        MINT_LEN,
        &TOKEN_PROGRAM_ID,
    );
    send(
        &mut svm,
        &[&owner, &mint_kp],
        &Instruction {
            program_id: system_program::id(),
            accounts: ix.accounts.clone(),
            data: ix.data.clone(),
        },
    )
    .expect("create mint account");

    // Tx 1b: initialize the mint (owner = mint authority).
    let init_ix =
        instruction::initialize_mint2(&TOKEN_PROGRAM_ID, &mint_pk, &owner.pubkey(), None, 8)
            .expect("init mint ix");
    send(
        &mut svm,
        &[&owner],
        &Instruction {
            program_id: TOKEN_PROGRAM_ID,
            accounts: init_ix.accounts.clone(),
            data: init_ix.data.clone(),
        },
    )
    .expect("initialize mint");

    // Tx 2: owner ATA (idempotent create).
    {
        let ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &owner.pubkey(),
                &owner.pubkey(),
                &mint_pk,
                &TOKEN_PROGRAM_ID,
            );
        send(
            &mut svm,
            &[&owner],
            &Instruction {
                program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
                accounts: ix.accounts.clone(),
                data: ix.data.clone(),
            },
        )
        .expect("create owner ata");
    }

    // Tx 3: vault ATA (owned by the vault PDA; the owner just pays rent).
    {
        let ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &owner.pubkey(),
                &vault,
                &mint_pk,
                &TOKEN_PROGRAM_ID,
            );
        send(
            &mut svm,
            &[&owner],
            &Instruction {
                program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
                accounts: ix.accounts.clone(),
                data: ix.data.clone(),
            },
        )
        .expect("create vault ata");
    }

    // Tx 3b: buyer ATA (the sell destination for `ValidateAndSell`).
    {
        let ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &owner.pubkey(),
                &buyer.pubkey(),
                &mint_pk,
                &TOKEN_PROGRAM_ID,
            );
        send(
            &mut svm,
            &[&owner],
            &Instruction {
                program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
                accounts: ix.accounts.clone(),
                data: ix.data.clone(),
            },
        )
        .expect("create buyer ata");
    }

    // Tx 4: mint tokens to the owner ATA (mint authority = owner).
    {
        let ix = instruction::mint_to(
            &TOKEN_PROGRAM_ID,
            &mint_pk,
            &owner_ata,
            &owner.pubkey(),
            &[],
            mint_amount,
        )
        .expect("mint_to ix");
        send(
            &mut svm,
            &[&owner],
            &Instruction {
                program_id: TOKEN_PROGRAM_ID,
                accounts: ix.accounts.clone(),
                data: ix.data.clone(),
            },
        )
        .expect("mint tokens");
    }

    // ── Deposit to vault (tag 4) ────────────────────────────────────────
    let mut d = Vec::with_capacity(DEPOSIT_IX_LEN);
    d.push(IX_DEPOSIT_TO_VAULT);
    d.extend_from_slice(&position_id.to_le_bytes());
    d.extend_from_slice(&deposit_amount.to_le_bytes());
    let deposit_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        ],
        data: d,
    };
    send(&mut svm, &[&owner], &deposit_ix).expect("deposit to vault");

    (
        svm,
        program_id,
        owner,
        policy,
        execution,
        crank,
        vault,
        mint_pk,
        owner_ata,
        vault_ata,
        buyer,
        buyer_ata,
        position_id,
    )
}

/// Happy-path setup: 3-state machine, 10% tranches, 10_000 tokens deposited.
pub fn happy_setup() -> FullSetup {
    setup_full(3, 1000, 10_000, 0)
}

// Fourteen arguments because that is the `ValidateAndSell` account list plus
// its data fields, in wire order. Keeping them positional here keeps each
// test's call readable against the layout in `lib.rs`.
#[allow(clippy::too_many_arguments)]
pub fn validate_sell_ix(
    program_id: Pubkey,
    cranker: &Pubkey,
    policy: Pubkey,
    execution: Pubkey,
    vault: Pubkey,
    vault_ata: Pubkey,
    dest_ata: Pubkey,
    mint: Pubkey,
    position_id: u64,
    expected_state: u8,
    tranche_index: u8,
    tick_slot: u64,
    quote_digest: &[u8; 32],
    amount: u64,
) -> Instruction {
    let mut d = Vec::with_capacity(VALIDATE_SELL_IX_LEN);
    d.push(IX_VALIDATE_SELL);
    d.extend_from_slice(&position_id.to_le_bytes());
    d.push(expected_state);
    d.push(tranche_index);
    d.extend_from_slice(&tick_slot.to_le_bytes());
    d.extend_from_slice(quote_digest);
    d.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(*cranker, true),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(execution, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_ata, false),
            AccountMeta::new(dest_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data: d,
    }
}

pub fn token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let acct = svm.get_account(ata).expect("token account exists");
    // SPL Token account layout: mint(32) + owner(32) + amount(u64 LE @ 64..72)
    u64::from_le_bytes(acct.data[64..72].try_into().unwrap())
}

// ──────────────────────────────────────────────────────────────────────────
// G7 — cross-account substitution
// ──────────────────────────────────────────────────────────────────────────

/// A second, fully-formed principal inside the same `LiteSVM` as
/// `setup_full`'s victim.
///
/// The point of a substitution test is *not* that the program rejects
/// garbage — a random `Pubkey::new_unique()` fails on length or ownership
/// before any gate logic runs, and proves nothing. The point is that it
/// rejects a **genuine** account of exactly the right shape that simply
/// belongs to someone else: same program, same length, same `position_id`,
/// same mint, committed through the same instructions. Every field the
/// handlers read is real here; only the seeds differ.
pub struct Attacker {
    pub keypair: Keypair,
    pub policy: Pubkey,
    pub execution: Pubkey,
    /// `Pubkey::default()`-valued account is never created when `deposit == 0`;
    /// the address is still derived so substitution tests can name it.
    pub vault: Pubkey,
    pub ata: Pubkey,
    pub vault_ata: Pubkey,
}

impl Attacker {
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
}

/// Build an `Attacker` sharing the victim's `mint` and `position_id`.
///
/// `mint_authority` is the victim's owner keypair — the attacker holds the
/// same token because that is the interesting case: the substituted ATAs are
/// mint-compatible, so `TransferChecked` cannot be what rejects them.
///
/// `deposit == 0` skips the vault entirely (`DepositToVault` reads
/// `amount == 0` as "move the whole balance", so it has no "create an empty
/// vault" mode).
#[allow(clippy::too_many_arguments)]
pub fn setup_attacker(
    svm: &mut LiteSVM,
    program_id: Pubkey,
    mint_authority: &Keypair,
    mint: Pubkey,
    position_id: u64,
    n_states: u8,
    tranche_bps: u16,
    balance: u64,
    deposit: u64,
) -> Attacker {
    let keypair = Keypair::new();
    let attacker = keypair.pubkey();
    svm.airdrop(&attacker, 1_000_000_000).expect("airdrop attacker");

    let (policy, _) = pda(POLICY_SEED, &program_id, &attacker, position_id);
    let (execution, _) = pda(EXECUTION_SEED, &program_id, &attacker, position_id);
    let (vault, _) = pda(VAULT_SEED, &program_id, &attacker, position_id);

    let ata = Pubkey::find_program_address(
        &[attacker.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0;
    let vault_ata = Pubkey::find_program_address(
        &[vault.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0;

    // ── Commit the attacker's own policy (tag 0) ────────────────────────
    let mut d = Vec::with_capacity(COMMIT_IX_LEN);
    d.push(IX_COMMIT_POLICY);
    d.extend_from_slice(&position_id.to_le_bytes());
    d.extend_from_slice(&0x165e_f4aa_bbcc_ddee_u64.to_le_bytes());
    d.push(n_states);
    d.extend_from_slice(&tranche_bps.to_le_bytes());
    send(
        svm,
        &[&keypair],
        &Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attacker, true),
                AccountMeta::new(policy, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: d,
        },
    )
    .expect("attacker commits their own policy");

    // ── Attacker ATA + tokens ───────────────────────────────────────────
    create_ata(svm, &keypair, &attacker, &mint);
    if balance > 0 {
        let ix = instruction::mint_to(
            &TOKEN_PROGRAM_ID,
            &mint,
            &ata,
            &mint_authority.pubkey(),
            &[],
            balance,
        )
        .expect("mint_to ix");
        send(
            svm,
            &[mint_authority],
            &Instruction {
                program_id: TOKEN_PROGRAM_ID,
                accounts: ix.accounts.clone(),
                data: ix.data.clone(),
            },
        )
        .expect("mint tokens to attacker");
    }

    // ── Authorize the attacker's own crank, paying into their own ATA ───
    let now = svm.get_sysvar::<solana_sdk::clock::Clock>();
    let expires = (now.unix_timestamp as u64).saturating_add(3_600);
    send(
        svm,
        &[&keypair],
        &authorize_ix(
            program_id,
            &attacker,
            execution,
            &attacker,
            position_id,
            expires,
            &ata,
        ),
    )
    .expect("attacker authorizes their own crank");

    // ── Attacker vault ──────────────────────────────────────────────────
    if deposit > 0 {
        create_ata(svm, &keypair, &vault, &mint);
        let mut d = Vec::with_capacity(DEPOSIT_IX_LEN);
        d.push(IX_DEPOSIT_TO_VAULT);
        d.extend_from_slice(&position_id.to_le_bytes());
        d.extend_from_slice(&deposit.to_le_bytes());
        send(
            svm,
            &[&keypair],
            &Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(attacker, true),
                    AccountMeta::new(vault, false),
                    AccountMeta::new(ata, false),
                    AccountMeta::new_readonly(mint, false),
                    AccountMeta::new(vault_ata, false),
                    AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                    AccountMeta::new_readonly(system_program::id(), false),
                    AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
                ],
                data: d,
            },
        )
        .expect("attacker deposits into their own vault");
    }

    Attacker {
        keypair,
        policy,
        execution,
        vault,
        ata,
        vault_ata,
    }
}

/// Idempotent ATA creation for `(authority, mint)`, rent paid by `payer`.
pub fn create_ata(svm: &mut LiteSVM, payer: &Keypair, authority: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
        &payer.pubkey(),
        authority,
        mint,
        &TOKEN_PROGRAM_ID,
    );
    send(
        svm,
        &[payer],
        &Instruction {
            program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
            accounts: ix.accounts.clone(),
            data: ix.data.clone(),
        },
    )
    .expect("create ata");
    Pubkey::find_program_address(
        &[authority.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

/// Mint a second, unrelated mint into the same `LiteSVM` — the substitute for
/// the `mint` account slot.
pub fn create_second_mint(svm: &mut LiteSVM, payer: &Keypair, decimals: u8) -> Pubkey {
    const MINT_LEN: u64 = 82;
    let mint_kp = Keypair::new();
    let mint_pk = mint_kp.pubkey();
    let ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint_pk,
        svm.minimum_balance_for_rent_exemption(MINT_LEN as usize),
        MINT_LEN,
        &TOKEN_PROGRAM_ID,
    );
    send(
        svm,
        &[payer, &mint_kp],
        &Instruction {
            program_id: system_program::id(),
            accounts: ix.accounts.clone(),
            data: ix.data.clone(),
        },
    )
    .expect("create second mint account");
    let init_ix =
        instruction::initialize_mint2(&TOKEN_PROGRAM_ID, &mint_pk, &payer.pubkey(), None, decimals)
            .expect("init mint ix");
    send(
        svm,
        &[payer],
        &Instruction {
            program_id: TOKEN_PROGRAM_ID,
            accounts: init_ix.accounts.clone(),
            data: init_ix.data.clone(),
        },
    )
    .expect("initialize second mint");
    mint_pk
}
