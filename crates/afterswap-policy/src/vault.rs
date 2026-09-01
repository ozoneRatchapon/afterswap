//! tags 4 and 5 — `DepositToVault` and `CloseVault`.

use crate::token::{invoke_transfer_checked, require_vault_ata};
use crate::types::*;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};

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
pub(crate) fn deposit_to_vault(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
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

    // The destination must be the vault PDA's *associated* token account for
    // this mint — not merely some token account the vault can authorize. The
    // address derivation subsumes the old `data[32..64] == vault` authority
    // check: only the ATA program can create an account at that address, and
    // it always sets the authority to the derivation's wallet.
    if *dest_ata.owner() != token_program.key().as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }
    require_vault_ata(vault.key(), mint.key(), dest_ata.key())?;

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
pub(crate) fn close_vault(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
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

    // Same discipline as `validate_and_sell`: the stored owner, position_id
    // and mint below drive the derivation, the ATA binding and `decimals`,
    // so establish that this program wrote them before trusting them.
    if vault.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
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
    let vault_mint: [u8; 32] = out[VAULT_OFFSET_MINT..VAULT_OFFSET_MINT + 32]
        .try_into()
        .unwrap();
    drop(out);

    // The mint is what `decimals` is read from and what the ATA derivation
    // below is keyed on, so it has to be the one the vault was opened with.
    if vault_mint.as_ref() != mint.key().as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify the PDA seeds match.
    let (expected_vault, bump) = pubkey::find_program_address(
        &[VAULT_SEED, owner.key().as_ref(), &position_id_le],
        program_id,
    );
    if &expected_vault != vault.key() {
        return Err(ProgramError::InvalidSeeds);
    }

    // Draw from the one account the vault is accounted in.
    require_vault_ata(vault.key(), mint.key(), src_ata.key())?;

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
