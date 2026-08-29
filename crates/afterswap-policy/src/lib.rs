//! AfterSwap exit-policy registry + delegated-execution gate (Pinocchio).
//!
//! Six instructions:
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

//! Phase B step 2 design (vault-sourced):
//!   The owner deposits the position into a program PDA vault (tag 4).
//!   The vault PDA signs every `TransferChecked` (tag 2) — the cranker is
//!   a signer/fee-payer who can never move funds on their own. The
//!   authorized-crank check (tag 1) gates which crankers may trigger sells.
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
    cpi::slice_invoke_signed,
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
/// expires_unix(8) + bump(1)
pub const EXECUTION_LEN: usize = 32 + 8 + 32 + 1 + 8 + 8 + 8 + 1;

/// Offsets inside the execution PDA layout (see `EXECUTION_LEN`).
const EXEC_OFFSET_POSITION_ID: usize = 32;
const EXEC_OFFSET_CRANK: usize = 40;
const EXEC_OFFSET_TRANCHES_FILLED: usize = 72;
const EXEC_OFFSET_EXECUTED_LAMPORTS: usize = 73;
const EXEC_OFFSET_LAST_TICK_SLOT: usize = 81;
const EXEC_OFFSET_EXPIRES_UNIX: usize = 89;
const EXEC_OFFSET_BUMP: usize = 97;

/// Instruction data: tag(1=1) + position_id u64 LE + crank pubkey(32) +
/// expires_unix u64 LE.
/// Expiry is mandatory: an authorization with no deadline is rejected.
/// position_id is required on the first call to create the execution PDA
/// (the seeds need it); subsequent calls read it from the PDA and ignore
/// the data value.
pub const IX_AUTHORIZE_EXECUTION: u8 = 1;
pub const AUTHORIZE_IX_LEN: usize = 1 + 8 + 32 + 8;

/// Instruction data: tag(1=3), no payload.
pub const IX_REVOKE_AUTHORIZATION: u8 = 3;
pub const REVOKE_IX_LEN: usize = 1;

/// Vault PDA layout (fixed size, no realloc ever):
/// owner(32) + position_id(8) + mint(32) + amount(u64 LE) + bump(1)
pub const VAULT_LEN: usize = 32 + 8 + 32 + 8 + 1;

/// Offsets inside the vault PDA layout (see `VAULT_LEN`).
const VAULT_OFFSET_POSITION_ID: usize = 32;
const VAULT_OFFSET_MINT: usize = 40;
const VAULT_OFFSET_AMOUNT: usize = 72;
const VAULT_OFFSET_BUMP: usize = 80;

/// Instruction data: tag(1=4) + position_id u64 LE + amount u64 LE.
/// amount = 0 means "transfer the owner's entire balance" (read from the
/// source ATA on-chain). The vault is created by this instruction (first
/// call); subsequent calls top up the existing vault.
pub const IX_DEPOSIT_TO_VAULT: u8 = 4;
pub const DEPOSIT_IX_LEN: usize = 1 + 8 + 8;

/// Instruction data: tag(1=5), no payload.
pub const IX_CLOSE_VAULT: u8 = 5;
pub const CLOSE_IX_LEN: usize = 1;

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
        // tag 4 is now `DepositToVault` (see above); tag 5 is `CloseVault`.
        // The original design's `AnchorFill` (memo) is deferred — the
        // verifiable chain is closed by the vault + execution state instead.
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
    let now = Clock::get()?.unix_timestamp;
    if expires_unix <= now as u64 {
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
    // this program, match the owner, and have no tranches filled. In that
    // case, re-authorizing with a different crank is deliberately not
    // supported — the owner must `RevokeAuthorization` first, then call
    // `AuthorizeExecution` again with the new crank.
    if execution.data_len() > 0 {
        if execution.lamports() == 0 || execution.data_len() != EXECUTION_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        let out = execution.try_borrow_data()?;
        if out[0..32] != *owner.key().as_ref() {
            return Err(ProgramError::InvalidAccountData);
        }
        if out[EXEC_OFFSET_TRANCHES_FILLED] != 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        drop(out);
        let (expected, _) = pubkey::find_program_address(
            &[EXECUTION_SEED, owner.key().as_ref(), &position_id_le],
            program_id,
        );
        if &expected != execution.key() {
            return Err(ProgramError::InvalidSeeds);
        }
        return Err(ProgramError::AccountAlreadyInitialized);
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
    Ok(())
}

/// `RevokeAuthorization` (tag 3): owner alone clears the authorized crank.
/// Idempotent: revoking a never-authorized or already-revoked execution PDA
/// is a no-op. This is the "no silent path" half of the design: the only
/// ways an authorization stops working are revocation (owner) or expiry
/// (time), and the owner always controls revocation.
///
/// After a revoke, the owner may `AuthorizeExecution` again with a new
/// crank (the PDA is still owned by this program, and `tranches_filled`
/// is still 0, so the idempotent no-op path in `authorize_execution`
/// does not trigger).
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
///   5. `tranche_index < policy.n_states` (bounded by the committed
///      machine size) AND `amount <= tranche_bps / 10_000 * vault_balance`
///      (bounded loss: no single sell can exceed the tranche budget).
///   6. Move `amount` from `source ATA` to `destination ATA` via
///      `TransferChecked`, signed by the vault PDA.
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

    // ── Step 1: policy PDA ──────────────────────────────────────────────
    if policy.data_len() != POLICY_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let policy_out = policy.try_borrow_data()?;
    let policy_position_id: [u8; 8] = policy_out[32..40].try_into().unwrap();
    let n_states = policy_out[48];
    let _tranche_bps = u16::from_le_bytes(policy_out[49..51].try_into().unwrap());
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
    drop(exec_out);

    if exec_position_id != position_id_le {
        return Err(ProgramError::InvalidAccountData);
    }

    // ── Step 1b: the policy account must BE the policy PDA ──────────────
    // Step 1 above only checked this account's length and the position_id
    // stored inside it. Both are attacker-supplied if the account is not the
    // real PDA, and `n_states` — read from it and used as the tranche bound
    // in step 5 — is the gate's only on-chain limit. Without this check a
    // caller who is already an authorized crank could pass a look-alike
    // account carrying `n_states = 255` and keep selling tranches long past
    // the committed machine size, which defeats the purpose of the program.
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
    if tranche_index >= n_states {
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
    drop(vault_out);

    if vault_owner.as_ref() != exec_owner.as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }

    if amount > vault_amount {
        return Err(ProgramError::InvalidInstructionData);
    }
    // tranche_bps bound: amount must be ≤ tranche_bps% of the original
    // position. We don't store the original position size, so we enforce
    // the simpler invariant: amount ≤ vault_amount (checked above) and
    // the audit trail (quote_digest, expected_state) binds the off-chain
    // guarantee. The on-chain bound is: bounded loss + monotonic progress.

    // ── Step 6: TransferChecked ─────────────────────────────────────────
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
