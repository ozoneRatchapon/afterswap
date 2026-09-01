//! SPL Token helpers shared by the vault, sell and close paths.

use crate::types::{ASSOCIATED_TOKEN_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
    SPL_TOKEN_TRANSFER_CHECKED_DISCRIMINATOR};
use pinocchio::{
    account_info::AccountInfo,
    cpi::slice_invoke_signed,
    instruction::{AccountMeta, Instruction, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    ProgramResult,
};

/// Bind a vault-side token account to *the* associated token account of
/// (vault PDA, mint).
///
/// Being owned by the Token program and having the vault PDA as authority is
/// not enough: anyone can create an auxiliary token account with the vault as
/// its authority, and the vault PDA signs whatever `TransferChecked` it is
/// handed. Without this, a crank could point `src_ata` at a dust account it
/// funded itself — the transfer succeeds, the program still debits
/// `vault.amount`, and the owner's real balance is stranded in the canonical
/// ATA with `amount` already burnt down to zero. Deriving the address is the
/// only check that says *which* account, so deposit, sell and close all agree
/// on one.
pub(crate) fn require_vault_ata(vault: &Pubkey, mint: &Pubkey, ata: &Pubkey) -> ProgramResult {
    let (expected, _bump) = pubkey::find_program_address(
        &[vault.as_ref(), SPL_TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    match &expected == ata {
        true => Ok(()),
        false => Err(ProgramError::InvalidAccountData),
    }
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
pub(crate) fn invoke_transfer_checked(
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
