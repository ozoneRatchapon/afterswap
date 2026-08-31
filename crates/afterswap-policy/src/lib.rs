//! AfterSwap exit-policy registry + delegated-execution gate (Pinocchio).
//!
//! Seven instructions:
//!   tag 0 `CommitPolicy`       — immutable PDA recording which exit machine
//!                                (blake3-64 fingerprint) governs a position,
//!                                before any fill follows it. The rulebook.
//!   tag 1 `AuthorizeExecution` — owner authorizes a crank pubkey (a PDA —
//!                                the account that signs each sell) for a
//!                                time-bounded window. Phase B step 1.
//!   tag 3 `RevokeAuthorization` — owner alone clears the authorized crank.
//!                                Idempotent. Phase B step 1.
//!   tag 4 `DepositToVault`     — owner locks tokens into a program PDA
//!                                vault (the source of every sell). Idempotent
//!                                top-up on subsequent calls. Phase B step 2.
//!   tag 2 `ValidateAndSell`    — the gate: validates against the committed
//!                                policy + authorization, then moves tokens
//!                                from the vault to the destination ATA via
//!                                `TransferChecked` signed by the vault PDA.
//!                                Phase B step 2 (the critical path).
//!   tag 5 `CloseVault`         — owner reclaims remaining vault tokens to
//!                                their ATA. Idempotent no-op when empty.
//!                                Phase B step 2.
//!   tag 6 `AnchorFill`         — publishes the fill as an SPL memo, the
//!                                sell-side counterpart of the shipped
//!                                `afterswap:quote` commit-side memo. Moves
//!                                no tokens, writes no account state; every
//!                                field but the quote digest is read from the
//!                                policy and execution PDAs. Phase B step 3.

//! Phase B step 2 design (vault-sourced):
//!   The owner deposits the position into a program PDA vault (tag 4).
//!   The vault PDA signs every `TransferChecked` (tag 2) — the cranker is
//!   a signer/fee-payer who never holds the vault's signing authority. The
//!   authorized-crank check (tag 1) gates which crankers may trigger sells;
//!   the crank picks *when* and *how much* (within the committed tranche
//!   budget) but never *where* — the payout account is bound at
//!   authorization time (`execution.settlement_ata`) and tag 2 rejects any
//!   other destination. An earlier version of this note said the cranker
//!   "can never move funds on their own", which the code did not support:
//!   the destination was unchecked, so an authorized crank could name its
//!   own ATA.
//!   The owner reclaims remaining tokens via `CloseVault` (tag 5).
//!   This **does take custody** between deposit and sell: the tokens sit in
//!   a PDA-owned ATA, and the SPL Token program offers the owner no
//!   unilateral exit from it. `CloseVault` is that exit — owner-only, needs
//!   no cooperation from the crank or the Worker, always available — but
//!   "non-custodial" is not a claim this design may make. The alternative
//!   that would earn it (a program PDA as SPL delegate on the owner's *own*
//!   ATA) is open and unbuilt; see `docs/PHASE_B_DELEGATED_EXECUTION.md` §8.
//!
//! `ValidateAndSell` IS implemented (an earlier version of this header said
//! it was not, and that "nothing in this program can move tokens" — both
//! false since Phase B step 2). Tokens move only through tags 2, 4 and 5.
//! Authorization is revocable by the owner alone at any time; a revoked
//! crank can never sell.
//!
//! PDAs: seeds = ["policy", owner, position_id_le]
//!           ["execution", owner, position_id_le], both owned by this program.
//! Immutability of the rulebook is the point: policy commits cannot be
//! overwritten or resized. The `execution` PDA is the only mutable state
//! (the gate's progress); the `vault` PDA holds the funds and signs for them.
//! Byte layout and instruction interface identical to the original
//! solana-program build — the LiteSVM tests are framework-agnostic.

use pinocchio::{
    account_info::AccountInfo,
    cpi::{slice_invoke, slice_invoke_signed},
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

#[cfg(target_os = "solana")]
pinocchio::program_entrypoint!(process_instruction);

/// PDA seed prefixes.
pub const POLICY_SEED: &[u8] = b"policy";
pub const EXECUTION_SEED: &[u8] = b"execution";
pub const VAULT_SEED: &[u8] = b"vault";

/// Committed policy account layout (fixed size, no realloc ever):
/// owner(32) + position_id(8) + fingerprint(8) + n_states(1) +
/// tranche_bps(2) + committed_at_unix(8) + bump(1)
pub const POLICY_LEN: usize = 32 + 8 + 8 + 1 + 2 + 8 + 1;

/// Instruction data: tag(1=0) + position_id u64 LE + fingerprint u64 LE +
/// n_states u8 + tranche_bps u16 LE
pub const IX_COMMIT_POLICY: u8 = 0;
pub const COMMIT_IX_LEN: usize = 1 + 8 + 8 + 1 + 2;

/// Execution PDA layout (fixed size, no realloc ever):
/// owner(32) + position_id(8) + authorized_crank(32) +
/// tranches_filled(1) + executed_lamports(8) + last_tick_slot(8) +
/// expires_unix(8) + bump(1) + settlement_ata(32)
///
/// `settlement_ata` is appended last so every offset above it is unchanged
/// from the pre-binding layout.
pub const EXECUTION_LEN: usize = 32 + 8 + 32 + 1 + 8 + 8 + 8 + 1 + 32;

/// Offsets inside the execution PDA layout (see `EXECUTION_LEN`).
const EXEC_OFFSET_POSITION_ID: usize = 32;
const EXEC_OFFSET_CRANK: usize = 40;
const EXEC_OFFSET_TRANCHES_FILLED: usize = 72;
const EXEC_OFFSET_EXECUTED_LAMPORTS: usize = 73;
const EXEC_OFFSET_LAST_TICK_SLOT: usize = 81;
const EXEC_OFFSET_EXPIRES_UNIX: usize = 89;
const EXEC_OFFSET_BUMP: usize = 97;
const EXEC_OFFSET_SETTLEMENT_ATA: usize = 98;

/// Instruction data: tag(1=1) + position_id u64 LE + crank pubkey(32) +
/// expires_unix u64 LE + settlement_ata(32).
/// Expiry is mandatory: an authorization with no deadline is rejected.
/// position_id is required on the first call to create the execution PDA
/// (the seeds need it); subsequent calls read it from the PDA and ignore
/// the data value.
///
/// `settlement_ata` is the single token account `ValidateAndSell` may pay
/// into. The owner names it here, at authorization time, alongside the
/// crank. Without it the crank chooses the destination per fill, which
/// means an authorized crank can drain the vault to itself — the account
/// list is not a permission model. Must be non-zero.
pub const IX_AUTHORIZE_EXECUTION: u8 = 1;
pub const AUTHORIZE_IX_LEN: usize = 1 + 8 + 32 + 8 + 32;

/// Instruction data: tag(1=3), no payload.
pub const IX_REVOKE_AUTHORIZATION: u8 = 3;
pub const REVOKE_IX_LEN: usize = 1;

/// Vault PDA layout (fixed size, no realloc ever):
/// owner(32) + position_id(8) + mint(32) + amount(u64 LE) + bump(1) +
/// deposited(u64 LE)
///
/// `amount` is the live balance; `deposited` is the monotone sum of every
/// deposit ever made. The tranche bound needs a fixed denominator — using
/// the live balance would let a 10% tranche be re-applied to the shrinking
/// remainder, so the *n*-th sell is 10% of what is left rather than 10% of
/// the position. `deposited` is that fixed denominator.
pub const VAULT_LEN: usize = 32 + 8 + 32 + 8 + 1 + 8;

/// Offsets inside the vault PDA layout (see `VAULT_LEN`).
const VAULT_OFFSET_POSITION_ID: usize = 32;
const VAULT_OFFSET_MINT: usize = 40;
const VAULT_OFFSET_AMOUNT: usize = 72;
const VAULT_OFFSET_BUMP: usize = 80;
const VAULT_OFFSET_DEPOSITED: usize = 81;

/// Instruction data: tag(1=4) + position_id u64 LE + amount u64 LE.
/// amount = 0 means "transfer the owner's entire balance" (read from the
/// source ATA on-chain). The vault is created by this instruction (first
/// call); subsequent calls top up the existing vault.
pub const IX_DEPOSIT_TO_VAULT: u8 = 4;
pub const DEPOSIT_IX_LEN: usize = 1 + 8 + 8;

/// Instruction data: tag(1=5), no payload.
pub const IX_CLOSE_VAULT: u8 = 5;
pub const CLOSE_IX_LEN: usize = 1;

/// Instruction data: tag(1=6) + quote_digest(32).
/// `AnchorFill` publishes the third link of the verifiable chain — a memo on
/// the *sell* side, matching the shipped `afterswap:quote` pattern on the
/// commit side. Every field except the quote digest is read from on-chain
/// state (the policy PDA's fingerprint, the execution PDA's tranche counter
/// and tick slot), so a caller cannot anchor a fill that did not happen or
/// misreport which tranche it was. The quote digest is necessarily
/// caller-supplied — it names an off-chain quote the chain never saw — so
/// the signer is restricted to the authorized crank or the owner, the same
/// two parties `ValidateAndSell` accepts. Anyone else paying the fee could
/// otherwise bind a false quote to a real fill.
pub const IX_ANCHOR_FILL: u8 = 6;
pub const ANCHOR_FILL_IX_LEN: usize = 1 + 32;

/// SPL Memo v2 — `MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr`.
const MEMO_PROGRAM_ID: Pubkey = [
    5, 74, 83, 90, 153, 41, 33, 6, 77, 36, 232, 113, 96, 218, 56, 124, 124, 53, 181, 221, 188, 146,
    187, 129, 228, 31, 168, 64, 65, 5, 68, 141,
];

/// Upper bound on the rendered memo body (see `render_fill_memo`):
/// 18 + 16 + 5 + 20 + 9 + 3 + 6 + 20 + 14 + 64 = 175.
const FILL_MEMO_CAP: usize = 192;

/// Offset of the blake3-64 fingerprint inside the policy PDA layout.
const POLICY_OFFSET_FINGERPRINT: usize = 40;

/// Instruction data (fixed length):
/// tag(1=2)
///   position_id u64 LE      // must match the policy PDA
///   expected_state u8       // the FSM state the cranker claims (audit binding)
///   tranche_index u8        // which tranche this is (0-based, monotonic)
///   tick_slot u64 LE        // the DFlow context_slot this decision used
///   quote_digest [u8;32]    // sha-256 of the signed quote body (audit binding)
///   amount u64 LE           // raw base units to move (bounded on-chain)
pub const IX_VALIDATE_SELL: u8 = 2;
pub const VALIDATE_SELL_IX_LEN: usize = 1 + 8 + 1 + 1 + 8 + 32 + 8;

/// SPL Token program instruction discriminator for `TransferChecked`
/// (variant 12 of `TokenInstruction`). The instruction body is
/// amount(u64 LE) + decimals(u8) = 9 bytes after the discriminator.
const SPL_TOKEN_TRANSFER_CHECKED_DISCRIMINATOR: u8 = 12;

/// SPL Token — `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`.
///
/// Every `TransferChecked` CPI must target exactly this program. The caller
/// supplies the token-program account, and `slice_invoke_signed` extends the
/// vault PDA's signature into whatever program it names — so an unpinned
/// token program hands vault-PDA signing authority to caller-chosen code.
/// Token-2022 is deliberately not accepted: its transfer-hook and
/// transfer-fee extensions change what a `TransferChecked` means, and this
/// gate's bounds are written against the classic semantics.
const SPL_TOKEN_PROGRAM_ID: Pubkey = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    match data.first().copied() {
        Some(IX_COMMIT_POLICY) => commit_policy(program_id, accounts, data),
        Some(IX_AUTHORIZE_EXECUTION) => authorize_execution(program_id, accounts, data),
        Some(IX_REVOKE_AUTHORIZATION) => revoke_authorization(program_id, accounts, data),
        Some(IX_DEPOSIT_TO_VAULT) => deposit_to_vault(program_id, accounts, data),
        Some(IX_CLOSE_VAULT) => close_vault(program_id, accounts, data),
        Some(IX_VALIDATE_SELL) => validate_and_sell(program_id, accounts, data),
        Some(IX_ANCHOR_FILL) => anchor_fill(program_id, accounts, data),
        // tag 4 is `DepositToVault`; tag 5 is `CloseVault`. `AnchorFill`
        // took tag 6 because 4 and 5 were already spent by the vault path.
        Some(_) | None => Err(ProgramError::InvalidInstructionData),
    }
}

fn commit_policy(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() != COMMIT_IX_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let position_id_le: [u8; 8] = data[1..9].try_into().unwrap();
    let fingerprint: [u8; 8] = data[9..17].try_into().unwrap();
    let n_states = data[17];
    let tranche_bps = u16::from_le_bytes(data[18..20].try_into().unwrap());
    if n_states == 0 || n_states > 4 || tranche_bps == 0 || tranche_bps > 10_000 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let [owner, policy, _system] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !owner.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (expected, bump) = pubkey::find_program_address(
        &[POLICY_SEED, owner.key().as_ref(), &position_id_le],
        program_id,
    );
    if &expected != policy.key() {
        return Err(ProgramError::InvalidSeeds);
    }
    // Immutable: a policy for this (owner, position) may only exist once.
    if policy.lamports() > 0 || policy.data_len() > 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let rent = Rent::get()?.minimum_balance(POLICY_LEN);
    let bump_arr = [bump];
    let seeds = [
        Seed::from(POLICY_SEED),
        Seed::from(owner.key().as_ref()),
        Seed::from(position_id_le.as_ref()),
        Seed::from(bump_arr.as_ref()),
    ];
    CreateAccount {
        from: owner,
        to: policy,
        lamports: rent,
        space: POLICY_LEN as u64,
        owner: program_id,
    }
    .invoke_signed(&[Signer::from(&seeds[..])])?;

    let now = Clock::get()?.unix_timestamp;
    let mut out = policy.try_borrow_mut_data()?;
    out[0..32].copy_from_slice(owner.key().as_ref());
    out[32..40].copy_from_slice(&position_id_le);
    out[40..48].copy_from_slice(&fingerprint);
    out[48] = n_states;
    out[49..51].copy_from_slice(&tranche_bps.to_le_bytes());
    out[51..59].copy_from_slice(&now.to_le_bytes());
    out[59] = bump;
    Ok(())
}

/// `AuthorizeExecution` (tag 1): owner authorizes a crank pubkey (a PDA —
/// the account that will sign each `TransferChecked` in the Phase B step 2
/// gate) for a time-bounded window. The authority mechanism is not the SPL
/// Token `Delegate` ix — but *not* for the reason an earlier version of this
/// comment gave. It claimed "a delegate cannot route to an arbitrary
/// destination"; that is false, an SPL delegate may transfer to any
/// destination up to the approved amount. `Delegate` is unused because the
/// vault-sourced design was chosen instead
/// (docs/PHASE_B_DELEGATED_EXECUTION.md §1 and §4.1).
///
/// The execution PDA is created by this instruction (first call) and is
/// immutable thereafter: re-authorizing with a different crank is
/// deliberately not supported — the owner must `RevokeAuthorization`
/// first, then `AuthorizeExecution` again with the new crank.
fn authorize_execution(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if data.len() != AUTHORIZE_IX_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let crank_key =
        Pubkey::try_from(data[9..41].to_vec()).map_err(|_| ProgramError::InvalidInstructionData)?;
    let expires_unix = u64::from_le_bytes(data[41..49].try_into().unwrap());
    let settlement_ata = Pubkey::try_from(data[49..81].to_vec())
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let now = Clock::get()?.unix_timestamp;
    if expires_unix <= now as u64 {
        return Err(ProgramError::InvalidInstructionData);
    }
    // The all-zero key is `RevokeAuthorization`'s cleared marker; accepting
    // it here would authorize a crank with an unbound destination.
    if settlement_ata == [0u8; 32] {
        return Err(ProgramError::InvalidInstructionData);
    }

    let [owner, execution, _system] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !owner.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // The position_id is supplied in the instruction data (not read from
    // the execution PDA, which may not exist yet on the first call).
    let position_id_le: [u8; 8] = data[1..9].try_into().unwrap();

    // If the execution PDA is already initialized, it must be owned by
    // this program, match the owner, and have no tranches filled.
    // Overwriting a *live* authorization is rejected: the owner must
    // `RevokeAuthorization` first. A revoked PDA (crank zeroed) is
    // re-authorizable in place — the docstring has always promised that,
    // and returning `AccountAlreadyInitialized` unconditionally made the
    // owner's only path back to delegation permanently unreachable, since
    // revoke keeps the account alive.
    if execution.data_len() > 0 {
        if execution.lamports() == 0 || execution.data_len() != EXECUTION_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if execution.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        let out = execution.try_borrow_data()?;
        if out[0..32] != *owner.key().as_ref() {
            return Err(ProgramError::InvalidAccountData);
        }
        if out[EXEC_OFFSET_TRANCHES_FILLED] != 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        // The PDA's own position_id is authoritative once it exists; the
        // data field only seeds the *first* call.
        let stored_position_id: [u8; 8] = out[EXEC_OFFSET_POSITION_ID..EXEC_OFFSET_POSITION_ID + 8]
            .try_into()
            .unwrap();
        let revoked = out[EXEC_OFFSET_CRANK..EXEC_OFFSET_CRANK + 32] == [0u8; 32];
        drop(out);
        let (expected, _) = pubkey::find_program_address(
            &[EXECUTION_SEED, owner.key().as_ref(), &stored_position_id],
            program_id,
        );
        if &expected != execution.key() {
            return Err(ProgramError::InvalidSeeds);
        }
        if !revoked {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        let mut out = execution.try_borrow_mut_data()?;
        out[EXEC_OFFSET_CRANK..EXEC_OFFSET_CRANK + 32].copy_from_slice(crank_key.as_ref());
        out[EXEC_OFFSET_EXPIRES_UNIX..EXEC_OFFSET_EXPIRES_UNIX + 8]
            .copy_from_slice(&expires_unix.to_le_bytes());
        out[EXEC_OFFSET_SETTLEMENT_ATA..EXEC_OFFSET_SETTLEMENT_ATA + 32]
            .copy_from_slice(settlement_ata.as_ref());
        return Ok(());
    }

    // Uninitialized: first call — create the PDA and set crank + expiry.
    let (expected, bump) = pubkey::find_program_address(
        &[EXECUTION_SEED, owner.key().as_ref(), &position_id_le],
        program_id,
    );
    if &expected != execution.key() {
        return Err(ProgramError::InvalidSeeds);
    }

    let rent = Rent::get()?.minimum_balance(EXECUTION_LEN);
    let bump_arr = [bump];
    let seeds = [
        Seed::from(EXECUTION_SEED),
        Seed::from(owner.key().as_ref()),
        Seed::from(position_id_le.as_ref()),
        Seed::from(bump_arr.as_ref()),
    ];
    CreateAccount {
        from: owner,
        to: execution,
        lamports: rent,
        space: EXECUTION_LEN as u64,
        owner: program_id,
    }
    .invoke_signed(&[Signer::from(&seeds[..])])?;

    let mut out = execution.try_borrow_mut_data()?;
    out[0..32].copy_from_slice(owner.key().as_ref());
    out[EXEC_OFFSET_POSITION_ID..EXEC_OFFSET_POSITION_ID + 8].copy_from_slice(&position_id_le);
    out[EXEC_OFFSET_CRANK..EXEC_OFFSET_CRANK + 32].copy_from_slice(crank_key.as_ref());
    out[EXEC_OFFSET_TRANCHES_FILLED] = 0;
    out[EXEC_OFFSET_EXECUTED_LAMPORTS..EXEC_OFFSET_EXECUTED_LAMPORTS + 8]
        .copy_from_slice(&0u64.to_le_bytes());
    out[EXEC_OFFSET_LAST_TICK_SLOT..EXEC_OFFSET_LAST_TICK_SLOT + 8]
        .copy_from_slice(&0u64.to_le_bytes());
    out[EXEC_OFFSET_EXPIRES_UNIX..EXEC_OFFSET_EXPIRES_UNIX + 8]
        .copy_from_slice(&expires_unix.to_le_bytes());
    out[EXEC_OFFSET_BUMP] = bump;
    out[EXEC_OFFSET_SETTLEMENT_ATA..EXEC_OFFSET_SETTLEMENT_ATA + 32]
        .copy_from_slice(settlement_ata.as_ref());
    Ok(())
}

/// `RevokeAuthorization` (tag 3): owner alone clears the authorized crank.
/// Idempotent: revoking a never-authorized or already-revoked execution PDA
/// is a no-op. This is the "no silent path" half of the design: the only
/// ways an authorization stops working are revocation (owner) or expiry
/// (time), and the owner always controls revocation.
///
/// After a revoke, the owner may `AuthorizeExecution` again with a new
/// crank and a new settlement destination: `authorize_execution` treats a
/// zeroed crank as the re-authorizable state and rebinds in place. Revoke
/// keeps the account alive, so without that path revoking once would end
/// the owner's ability to delegate this position for good.
///
/// `settlement_ata` is deliberately left as-is: a zeroed crank already
/// rejects every sell, and re-authorization overwrites it.
fn revoke_authorization(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if data.len() != REVOKE_IX_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let [owner, execution] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !owner.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let out = execution.try_borrow_data()?;
    if out.len() != EXECUTION_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if out[0..32] != *owner.key().as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }
    let position_id_le: [u8; 8] = out[EXEC_OFFSET_POSITION_ID..EXEC_OFFSET_POSITION_ID + 8]
        .try_into()
        .unwrap();
    let already_zero = out[EXEC_OFFSET_CRANK..EXEC_OFFSET_CRANK + 32] == [0u8; 32];
    drop(out);

    if already_zero {
        return Ok(());
    }
    let (expected, _) = pubkey::find_program_address(
        &[EXECUTION_SEED, owner.key().as_ref(), &position_id_le],
        program_id,
    );
    if &expected != execution.key() {
        return Err(ProgramError::InvalidSeeds);
    }
    let mut out = execution.try_borrow_mut_data()?;
    out[EXEC_OFFSET_CRANK..EXEC_OFFSET_CRANK + 32].fill(0);
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────
// Phase B step 2 — vault-sourced delegated execution
// ──────────────────────────────────────────────────────────────────────────

/// `DepositToVault` (tag 4): owner locks tokens into a program PDA vault.
///
/// Accounts: `owner (signer)`, `vault PDA`, `source ATA (writable)`,
/// `mint (readonly)`, `destination ATA (writable)` — the vault's ATA for
/// (vault PDA, mint), `token program (readonly)`, `system (readonly)`,
/// `associated-token program (readonly)`.
///
/// Instruction data: `position_id u64 LE` + `amount u64 LE`.
/// `amount = 0` means "transfer the owner's entire balance" (read from
/// the source ATA on-chain). First call creates the vault PDA; subsequent
/// calls top up the existing vault.
fn deposit_to_vault(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() != DEPOSIT_IX_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let position_id_le: [u8; 8] = data[1..9].try_into().unwrap();
    let amount = u64::from_le_bytes(data[9..17].try_into().unwrap());

    let [owner, vault, source_ata, mint, dest_ata, token_program, _system_program, _ata_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !owner.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.key() != &SPL_TOKEN_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive the expected vault PDA.
    let (expected_vault, bump) = pubkey::find_program_address(
        &[VAULT_SEED, owner.key().as_ref(), &position_id_le],
        program_id,
    );
    if expected_vault != *vault.key().as_ref() {
        return Err(ProgramError::InvalidSeeds);
    }

    // The destination ATA must be the vault PDA's token account: it is
    // owned by the SPL Token program, and its `owner` field (data[32..64])
    // is the vault PDA. (The ATA's `owner` *account* is the Token program,
    // not the vault PDA — the vault PDA is its token-account *authority*.)
    if *dest_ata.owner() != token_program.key().as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }
    {
        let dest_data = dest_ata.try_borrow_data()?;
        if dest_data[32..64] != *vault.key().as_ref() {
            return Err(ProgramError::InvalidAccountData);
        }
        drop(dest_data);
    }

    // First call: create the vault PDA account.
    if vault.lamports() == 0 {
        let rent = Rent::get()?.minimum_balance(VAULT_LEN);
        let bump_arr = [bump];
        let seeds = [
            Seed::from(VAULT_SEED),
            Seed::from(owner.key().as_ref()),
            Seed::from(position_id_le.as_ref()),
            Seed::from(bump_arr.as_ref()),
        ];
        pinocchio_system::instructions::CreateAccount {
            from: owner,
            to: vault,
            lamports: rent,
            space: VAULT_LEN as u64,
            owner: program_id,
        }
        .invoke_signed(&[Signer::from(&seeds[..])])?;

        let mut out = vault.try_borrow_mut_data()?;
        out[0..32].copy_from_slice(owner.key().as_ref());
        out[VAULT_OFFSET_POSITION_ID..VAULT_OFFSET_POSITION_ID + 8]
            .copy_from_slice(&position_id_le);
        out[VAULT_OFFSET_MINT..VAULT_OFFSET_MINT + 32].copy_from_slice(mint.key().as_ref());
        out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8].copy_from_slice(&0u64.to_le_bytes());
        out[VAULT_OFFSET_BUMP] = bump;
        out[VAULT_OFFSET_DEPOSITED..VAULT_OFFSET_DEPOSITED + 8]
            .copy_from_slice(&0u64.to_le_bytes());
    } else {
        // Subsequent call: verify the existing vault.
        if vault.data_len() != VAULT_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        let out = vault.try_borrow_data()?;
        if out[0..32] != *owner.key().as_ref() {
            return Err(ProgramError::InvalidAccountData);
        }
        if out[VAULT_OFFSET_MINT..VAULT_OFFSET_MINT + 32] != *mint.key().as_ref() {
            return Err(ProgramError::InvalidAccountData);
        }
        drop(out);
    }

    // Read the source ATA's balance to determine the transfer amount.
    let src = source_ata.try_borrow_data()?;
    let src_amount = u64::from_le_bytes(src[64..72].try_into().unwrap());
    let transfer_amount = if amount == 0 {
        src_amount
    } else {
        amount.min(src_amount)
    };
    if transfer_amount == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }
    drop(src);

    // Read the mint's decimals for TransferChecked.
    let mint_data = mint.try_borrow_data()?;
    let decimals = mint_data[44];
    drop(mint_data);

    // CPI: TransferChecked from source_ata to dest_ata, signed by the owner.
    invoke_transfer_checked(
        token_program.key(),
        source_ata,
        mint,
        dest_ata,
        owner,
        transfer_amount,
        decimals,
        &[],
    )?;

    // Update the vault balance.
    let mut out = vault.try_borrow_mut_data()?;
    let current = u64::from_le_bytes(
        out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8]
            .try_into()
            .unwrap(),
    );
    let updated = current
        .checked_add(transfer_amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8].copy_from_slice(&updated.to_le_bytes());

    // `deposited` only ever grows — it is the tranche bound's denominator,
    // so a top-up widens each tranche but a sell never narrows it.
    let deposited = u64::from_le_bytes(
        out[VAULT_OFFSET_DEPOSITED..VAULT_OFFSET_DEPOSITED + 8]
            .try_into()
            .unwrap(),
    );
    let deposited = deposited
        .checked_add(transfer_amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    out[VAULT_OFFSET_DEPOSITED..VAULT_OFFSET_DEPOSITED + 8]
        .copy_from_slice(&deposited.to_le_bytes());
    Ok(())
}

/// `CloseVault` (tag 5): owner reclaims remaining vault tokens to their ATA.
///
/// Accounts: `owner (signer)`, `vault PDA`, `source ATA (writable)` — the
/// vault's ATA for (vault PDA, mint), `destination ATA (writable)` — the
/// owner's ATA for (owner, mint), `mint (readonly)`, `token program
/// (readonly)`, `system (readonly)`.
///
/// Instruction data: no payload.
/// Idempotent: if the vault holds 0 tokens, this is a no-op.
fn close_vault(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() != CLOSE_IX_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }

    let [owner, vault, src_ata, dest_ata, mint, token_program, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !owner.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.key() != &SPL_TOKEN_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    if vault.data_len() != VAULT_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let out = vault.try_borrow_data()?;
    if out[0..32] != *owner.key().as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }
    let position_id_le: [u8; 8] = out[VAULT_OFFSET_POSITION_ID..VAULT_OFFSET_POSITION_ID + 8]
        .try_into()
        .unwrap();
    let amount = u64::from_le_bytes(
        out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8]
            .try_into()
            .unwrap(),
    );
    drop(out);

    // Verify the PDA seeds match.
    let (expected_vault, bump) = pubkey::find_program_address(
        &[VAULT_SEED, owner.key().as_ref(), &position_id_le],
        program_id,
    );
    if &expected_vault != vault.key() {
        return Err(ProgramError::InvalidSeeds);
    }

    if amount == 0 {
        return Ok(()); // idempotent no-op
    }

    // Read the mint's decimals.
    let mint_data = mint.try_borrow_data()?;
    let decimals = mint_data[44];
    drop(mint_data);

    // The vault PDA is the authority of src_ata; sign via CPI. The bump comes
    // from the derivation validated above — deriving a second time would cost
    // another `find_program_address` and discard the key it just proved.
    let bump_arr = [bump];
    let seeds = [
        Seed::from(VAULT_SEED),
        Seed::from(owner.key().as_ref()),
        Seed::from(position_id_le.as_ref()),
        Seed::from(bump_arr.as_ref()),
    ];
    let signers = [Signer::from(&seeds[..])];

    invoke_transfer_checked(
        token_program.key(),
        src_ata,
        mint,
        dest_ata,
        vault,
        amount,
        decimals,
        &signers,
    )?;

    // Zero the vault balance (the PDA account is kept for future re-deposit).
    let mut out = vault.try_borrow_mut_data()?;
    out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8].copy_from_slice(&0u64.to_le_bytes());
    Ok(())
}

/// `ValidateAndSell` (tag 2): the gate.
///
/// Accounts: `cranker (signer, any)`, `policy PDA (readonly)`,
/// `execution PDA (writable)`, `vault PDA (readonly)`, `source ATA
/// (writable)` — the vault's ATA for (vault PDA, mint), `destination ATA
/// (writable)`, `mint (readonly)`, `token program (readonly)`.
///
/// Instruction data:
///   position_id u64 LE
///   expected_state u8
///   tranche_index u8
///   tick_slot u64 LE
///   quote_digest [u8; 32]
///   amount u64 LE   // raw base units to move
///
/// Validation sequence (all-or-nothing, in this order):
///   1. `policy` PDA exists, is owned by this program, and
///      `policy.position_id == data.position_id`.
///   2. `execution.authorized_crank == cranker`, OR `cranker == owner`
///      (owner override, always allowed).
///   3. `now < execution.expires_unix` (not expired).
///   4. `tranche_index == execution.tranches_filled` (monotonic, no
///      skipping, no replay).
///   5. `tranche_index < ceil(10_000 / policy.tranche_bps)` (no more
///      tranches than the committed tranche size divides the position
///      into), `amount <= vault.amount` (cannot overdraw), and
///      `amount * 10_000 <= policy.tranche_bps * vault.deposited`
///      (bounded loss: no single sell exceeds the committed tranche
///      budget, measured against the position as deposited, not against
///      the shrinking remainder).
///   6. `destination ATA == execution.settlement_ata` — the payout account
///      the owner named at authorization time. The crank picks *when* and
///      *how much*, never *where*.
///   7. Move `amount` from `source ATA` to `destination ATA` via
///      `TransferChecked` on the pinned SPL Token program, signed by the
///      vault PDA.
///
/// What is *not* enforced on chain: `expected_state` and `quote_digest` are
/// audit bindings only. The chain cannot replay the FSM or see the quote, so
/// these two fields bind the off-chain claim to this transaction for later
/// verification — they do not gate it.
///
/// On success: `execution.tranches_filled += 1`,
/// `execution.executed_lamports += amount`,
/// `execution.last_tick_slot = tick_slot`,
/// `vault.amount -= amount`.
fn validate_and_sell(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() != VALIDATE_SELL_IX_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let position_id_le: [u8; 8] = data[1..9].try_into().unwrap();
    // expected_state = data[9] (audit binding, not enforced on-chain)
    let tranche_index = data[10];
    let tick_slot = u64::from_le_bytes(data[11..19].try_into().unwrap());
    // quote_digest = data[19..51] (audit binding, not enforced on-chain)
    let amount = u64::from_le_bytes(data[51..59].try_into().unwrap());

    let [cranker, policy, execution, vault, src_ata, dest_ata, mint, token_program] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !cranker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.key() != &SPL_TOKEN_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    // ── Step 1: policy PDA ──────────────────────────────────────────────
    if policy.data_len() != POLICY_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let policy_out = policy.try_borrow_data()?;
    let policy_position_id: [u8; 8] = policy_out[32..40].try_into().unwrap();
    // `n_states` (policy_out[48]) is committed but no longer read here: it
    // sizes the FSM, not the sell schedule. See step 5.
    let tranche_bps = u16::from_le_bytes(policy_out[49..51].try_into().unwrap());
    drop(policy_out);
    if policy_position_id != position_id_le {
        return Err(ProgramError::InvalidAccountData);
    }

    // ── Step 2: authorization ───────────────────────────────────────────
    if execution.data_len() != EXECUTION_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let exec_out = execution.try_borrow_data()?;
    let exec_owner: [u8; 32] = exec_out[0..32].try_into().unwrap();
    let authorized_crank: [u8; 32] = exec_out[EXEC_OFFSET_CRANK..EXEC_OFFSET_CRANK + 32]
        .try_into()
        .unwrap();
    let tranches_filled = exec_out[EXEC_OFFSET_TRANCHES_FILLED];
    let expires_unix = u64::from_le_bytes(
        exec_out[EXEC_OFFSET_EXPIRES_UNIX..EXEC_OFFSET_EXPIRES_UNIX + 8]
            .try_into()
            .unwrap(),
    );
    let exec_position_id: [u8; 8] = exec_out[EXEC_OFFSET_POSITION_ID..EXEC_OFFSET_POSITION_ID + 8]
        .try_into()
        .unwrap();
    let settlement_ata: [u8; 32] = exec_out
        [EXEC_OFFSET_SETTLEMENT_ATA..EXEC_OFFSET_SETTLEMENT_ATA + 32]
        .try_into()
        .unwrap();
    drop(exec_out);

    if exec_position_id != position_id_le {
        return Err(ProgramError::InvalidAccountData);
    }

    // ── Step 1b: the policy account must BE the policy PDA ──────────────
    // Step 1 above only checked this account's length and the position_id
    // stored inside it. Both are attacker-supplied if the account is not the
    // real PDA, and `tranche_bps` — read from it and driving both of step 5's
    // limits — is the gate's only on-chain bound on outflow. Without this
    // check a caller who is already an authorized crank could pass a
    // look-alike account carrying `tranche_bps = 10_000` and sell the whole
    // position in one fill, which defeats the purpose of the program.
    // The owner comes from the execution PDA, so this must run after step 2's
    // read rather than beside step 1.
    if policy.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let (expected_policy, _) = pubkey::find_program_address(
        &[POLICY_SEED, exec_owner.as_ref(), &position_id_le],
        program_id,
    );
    if &expected_policy != policy.key() {
        return Err(ProgramError::InvalidSeeds);
    }
    // Same reasoning for the execution PDA: its `authorized_crank` and
    // `expires_unix` are the authorization itself.
    if execution.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let (expected_execution, _) = pubkey::find_program_address(
        &[EXECUTION_SEED, exec_owner.as_ref(), &position_id_le],
        program_id,
    );
    if &expected_execution != execution.key() {
        return Err(ProgramError::InvalidSeeds);
    }

    let cranker_key = cranker.key().as_ref();
    let is_authorized_crank = authorized_crank.as_ref() == cranker_key;
    let is_owner = exec_owner.as_ref() == cranker_key;
    if !is_authorized_crank && !is_owner {
        return Err(ProgramError::Custom(1));
    }

    // ── Step 3: expiry ──────────────────────────────────────────────────
    let now = Clock::get()?.unix_timestamp as u64;
    if now >= expires_unix {
        return Err(ProgramError::Custom(2));
    }

    // ── Step 4: monotonic tranche ───────────────────────────────────────
    if tranche_index != tranches_filled {
        return Err(ProgramError::InvalidInstructionData);
    }

    // ── Step 5: bounds ──────────────────────────────────────────────────
    // How many tranches the committed policy divides the position into:
    // ceil(10_000 / tranche_bps). This used to be `n_states`, the FSM's
    // state count — an unrelated quantity, and one `commit_policy` caps at
    // 4, so a 10% tranche policy could never sell more than 40% of a
    // position and the gate had no path to a full exit. Both fields are
    // still committed; only the tranche count now comes from the field
    // that means it. Clamped to 255 because `tranches_filled` is a u8.
    let max_tranches = {
        let n = 10_000u32.div_ceil(tranche_bps as u32);
        n.min(255) as u8
    };
    if tranche_index >= max_tranches {
        return Err(ProgramError::InvalidInstructionData);
    }
    if amount == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }
    // Read the vault balance to bound the amount.
    if vault.data_len() != VAULT_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let vault_out = vault.try_borrow_data()?;
    let vault_amount = u64::from_le_bytes(
        vault_out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8]
            .try_into()
            .unwrap(),
    );
    let vault_owner: [u8; 32] = vault_out[0..32].try_into().unwrap();
    let vault_deposited = u64::from_le_bytes(
        vault_out[VAULT_OFFSET_DEPOSITED..VAULT_OFFSET_DEPOSITED + 8]
            .try_into()
            .unwrap(),
    );
    drop(vault_out);

    if vault_owner.as_ref() != exec_owner.as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }

    if amount > vault_amount {
        return Err(ProgramError::InvalidInstructionData);
    }
    // The tranche budget the policy committed to, enforced. `vault_deposited`
    // is the position as deposited — a fixed denominator, so ten 10% tranches
    // sell the whole position and an eleventh has nothing left to draw on.
    // Widened to u128 because `amount * 10_000` overflows u64 above ~1.8e15
    // base units, which a 9-decimal mint reaches at 1.8M tokens.
    if (amount as u128) * 10_000 > (tranche_bps as u128) * (vault_deposited as u128) {
        return Err(ProgramError::InvalidInstructionData);
    }

    // ── Step 6: destination binding ─────────────────────────────────────
    // The owner named the payout account when they authorized the crank.
    // Nothing else in this instruction constrains where the tokens land:
    // the vault PDA signs the transfer, so without this check an authorized
    // crank could name its own ATA and drain the vault one legal tranche at
    // a time. Being on the account list is not a permission.
    if dest_ata.key().as_ref() != settlement_ata.as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }

    // ── Step 7: TransferChecked ─────────────────────────────────────────
    let mint_data = mint.try_borrow_data()?;
    let decimals = mint_data[44];
    drop(mint_data);

    // Derive the vault PDA seeds for signing.
    let (expected_vault, bump) = pubkey::find_program_address(
        &[VAULT_SEED, exec_owner.as_ref(), &position_id_le],
        program_id,
    );
    if &expected_vault != vault.key() {
        return Err(ProgramError::InvalidSeeds);
    }
    let bump_arr = [bump];
    let seeds = [
        Seed::from(VAULT_SEED),
        Seed::from(exec_owner.as_ref()),
        Seed::from(position_id_le.as_ref()),
        Seed::from(bump_arr.as_ref()),
    ];
    let signers = [Signer::from(&seeds[..])];

    invoke_transfer_checked(
        token_program.key(),
        src_ata,
        mint,
        dest_ata,
        vault,
        amount,
        decimals,
        &signers,
    )?;

    // ── Post-transfer state updates ─────────────────────────────────────
    {
        let mut out = vault.try_borrow_mut_data()?;
        let current: [u8; 8] = out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8]
            .try_into()
            .unwrap();
        let new_amount = u64::from_le_bytes(current)
            .saturating_sub(amount)
            .to_le_bytes();
        out[VAULT_OFFSET_AMOUNT..VAULT_OFFSET_AMOUNT + 8].copy_from_slice(&new_amount);
    }
    {
        let mut out = execution.try_borrow_mut_data()?;
        out[EXEC_OFFSET_TRANCHES_FILLED] = out[EXEC_OFFSET_TRANCHES_FILLED].saturating_add(1);
        let executed = u64::from_le_bytes(
            out[EXEC_OFFSET_EXECUTED_LAMPORTS..EXEC_OFFSET_EXECUTED_LAMPORTS + 8]
                .try_into()
                .unwrap(),
        );
        out[EXEC_OFFSET_EXECUTED_LAMPORTS..EXEC_OFFSET_EXECUTED_LAMPORTS + 8]
            .copy_from_slice(&(executed.saturating_add(amount)).to_le_bytes());
        out[EXEC_OFFSET_LAST_TICK_SLOT..EXEC_OFFSET_LAST_TICK_SLOT + 8]
            .copy_from_slice(&tick_slot.to_le_bytes());
    }
    Ok(())
}

/// Write `value` as lowercase hex, `width` digits, zero-padded. Returns the
/// number of bytes written so callers can chain into a cursor.
fn push_hex(out: &mut [u8], at: usize, value: u64, width: usize) -> usize {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for i in 0..width {
        let shift = 4 * (width - 1 - i);
        out[at + i] = HEX[((value >> shift) & 0xf) as usize];
    }
    width
}

/// Write `value` as unqualified decimal (no padding, "0" for zero).
fn push_dec(out: &mut [u8], at: usize, value: u64) -> usize {
    if value == 0 {
        out[at] = b'0';
        return 1;
    }
    // u64::MAX is 20 digits.
    let mut tmp = [0u8; 20];
    let mut n = value;
    let mut len = 0usize;
    while n > 0 {
        tmp[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for i in 0..len {
        out[at + i] = tmp[len - 1 - i];
    }
    len
}

/// Render the fill memo body into `out`, returning its length.
///
/// `afterswap:fill fp=<16 hex> pos=<dec> tranche=<dec> slot=<dec>
/// quote=sha256:<64 hex>`
///
/// The fingerprint is zero-padded to a fixed 16 hex digits. The off-chain
/// renderers (`worker/index.ts`, the demo page) print it via
/// `BigInt(...).toString(16)`, which strips leading zeros — a verifier must
/// compare the two numerically, not as strings. Fixed width is used here
/// because it costs no branches on-chain and never yields an empty field.
fn render_fill_memo(
    out: &mut [u8; FILL_MEMO_CAP],
    fingerprint: u64,
    position_id: u64,
    tranche_index: u8,
    tick_slot: u64,
    quote_digest: &[u8; 32],
) -> usize {
    let mut at = 0usize;
    let prefix = b"afterswap:fill fp=";
    out[at..at + prefix.len()].copy_from_slice(prefix);
    at += prefix.len();
    at += push_hex(out, at, fingerprint, 16);
    out[at..at + 5].copy_from_slice(b" pos=");
    at += 5;
    at += push_dec(out, at, position_id);
    out[at..at + 9].copy_from_slice(b" tranche=");
    at += 9;
    at += push_dec(out, at, tranche_index as u64);
    out[at..at + 6].copy_from_slice(b" slot=");
    at += 6;
    at += push_dec(out, at, tick_slot);
    out[at..at + 14].copy_from_slice(b" quote=sha256:");
    at += 14;
    for byte in quote_digest {
        at += push_hex(out, at, *byte as u64, 2);
    }
    at
}

/// tag 6 — anchor the most recent fill as an SPL memo.
///
/// Accounts: `signer (fee payer, crank or owner)`, `policy PDA (readonly)`,
/// `execution PDA (readonly)`, `memo program`.
///
/// Reads nothing the caller controls except the quote digest, and moves no
/// tokens. Both PDAs are derived and owner-checked before their contents are
/// trusted — the same discipline `ValidateAndSell` needs, for the same
/// reason: an undeserved look-alike account would let a caller anchor a
/// fingerprint or tranche index that was never committed.
fn anchor_fill(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() != ANCHOR_FILL_IX_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let quote_digest: [u8; 32] = data[1..33].try_into().unwrap();

    let [signer, policy, execution, memo_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !signer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if memo_program.key() != &MEMO_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    // ── Execution PDA: the fill record ──────────────────────────────────
    if execution.owner() != program_id || execution.data_len() != EXECUTION_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let exec_out = execution.try_borrow_data()?;
    let exec_owner: [u8; 32] = exec_out[0..32].try_into().unwrap();
    let position_id_le: [u8; 8] = exec_out[EXEC_OFFSET_POSITION_ID..EXEC_OFFSET_POSITION_ID + 8]
        .try_into()
        .unwrap();
    let authorized_crank: [u8; 32] = exec_out[EXEC_OFFSET_CRANK..EXEC_OFFSET_CRANK + 32]
        .try_into()
        .unwrap();
    let tranches_filled = exec_out[EXEC_OFFSET_TRANCHES_FILLED];
    let tick_slot = u64::from_le_bytes(
        exec_out[EXEC_OFFSET_LAST_TICK_SLOT..EXEC_OFFSET_LAST_TICK_SLOT + 8]
            .try_into()
            .unwrap(),
    );
    drop(exec_out);

    let (expected_execution, _) = pubkey::find_program_address(
        &[EXECUTION_SEED, exec_owner.as_ref(), &position_id_le],
        program_id,
    );
    if &expected_execution != execution.key() {
        return Err(ProgramError::InvalidSeeds);
    }

    // Nothing has been sold yet — there is no fill to anchor. Without this,
    // `tranches_filled - 1` would wrap and claim tranche 255.
    if tranches_filled == 0 {
        return Err(ProgramError::Custom(3));
    }
    let tranche_index = tranches_filled - 1;

    // ── Signer: the same two parties `ValidateAndSell` accepts ──────────
    let signer_key = signer.key().as_ref();
    if authorized_crank.as_ref() != signer_key && exec_owner.as_ref() != signer_key {
        return Err(ProgramError::Custom(1));
    }

    // ── Policy PDA: the committed fingerprint ───────────────────────────
    if policy.owner() != program_id || policy.data_len() != POLICY_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let (expected_policy, _) = pubkey::find_program_address(
        &[POLICY_SEED, exec_owner.as_ref(), &position_id_le],
        program_id,
    );
    if &expected_policy != policy.key() {
        return Err(ProgramError::InvalidSeeds);
    }
    let policy_out = policy.try_borrow_data()?;
    let fingerprint = u64::from_le_bytes(
        policy_out[POLICY_OFFSET_FINGERPRINT..POLICY_OFFSET_FINGERPRINT + 8]
            .try_into()
            .unwrap(),
    );
    drop(policy_out);

    // ── Emit ────────────────────────────────────────────────────────────
    let mut buf = [0u8; FILL_MEMO_CAP];
    let len = render_fill_memo(
        &mut buf,
        fingerprint,
        u64::from_le_bytes(position_id_le),
        tranche_index,
        tick_slot,
        &quote_digest,
    );
    let ix = Instruction {
        program_id: &MEMO_PROGRAM_ID,
        accounts: &[],
        data: &buf[..len],
    };
    slice_invoke(&ix, &[])
}

/// Build a `TransferChecked` instruction for the SPL Token program.
///
/// Instruction data: discriminator(1=12) + amount(u64 LE) + decimals(u8).
/// Accounts: source(writable), mint(readonly), destination(writable),
/// authority(readonly signer).
/// Invoke `TransferChecked` on the SPL Token program.
///
/// Builds the instruction inline and calls `slice_invoke_signed` so that no
/// `Instruction` value with lifetime parameters needs to escape this function.
///
/// Accounts in the instruction: source(writable), mint(readonly),
/// destination(writable), authority(readonly-signer).
///
/// # Parameters
/// * `account_infos` – the 4 accounts in order: `source, mint, destination,
///   authority`. The last one is the PDA that signs (for CPI) or the
///   native signer (for the owner's own ATA).
/// * `signers` – PDA signer list (empty for native-signer calls).
// Eight arguments, but each is a distinct account or field of the SPL
// `TransferChecked` layout this wraps. Bundling them into a struct would put
// a second name between the call site and the instruction it mirrors.
#[allow(clippy::too_many_arguments)]
fn invoke_transfer_checked(
    token_program: &Pubkey,
    source: &AccountInfo,
    mint: &AccountInfo,
    destination: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    decimals: u8,
    signers: &[Signer],
) -> ProgramResult {
    let mut data = [0u8; 10];
    data[0] = SPL_TOKEN_TRANSFER_CHECKED_DISCRIMINATOR;
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    data[9] = decimals;

    let accounts = [
        AccountMeta::new(source.key(), true, false),
        AccountMeta::new(mint.key(), false, false),
        AccountMeta::new(destination.key(), true, false),
        AccountMeta::new(authority.key(), false, true),
    ];

    let ix = Instruction {
        program_id: token_program,
        accounts: &accounts,
        data: &data,
    };

    let account_infos = [source, mint, destination, authority];
    slice_invoke_signed(&ix, &account_infos, signers)
}
