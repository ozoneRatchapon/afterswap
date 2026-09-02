//! tags 1 and 3 — `AuthorizeExecution` and `RevokeAuthorization`.

use crate::types::*;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

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
pub(crate) fn authorize_execution(
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
pub(crate) fn revoke_authorization(
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
