//! AfterSwap exit-policy registry.
//!
//! One instruction: `CommitPolicy` — create an immutable PDA recording
//! which exit machine (blake3-64 fingerprint) governs a position, before
//! any fill follows it. Anyone can later audit every DFlow fill against
//! the committed policy. Phase B (delegated execution) builds on this.
//!
//! PDA: seeds = ["policy", owner, position_id_le], owned by this program.
//! Immutability is the point: commits cannot be overwritten or resized.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction, system_program,
    sysvar::Sysvar,
};

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// PDA seed prefix.
pub const POLICY_SEED: &[u8] = b"policy";

/// Committed policy account layout (fixed size, no realloc ever):
/// owner(32) + position_id(8) + fingerprint(8) + n_states(1) +
/// tranche_bps(2) + committed_at_unix(8) + bump(1)
pub const POLICY_LEN: usize = 32 + 8 + 8 + 1 + 2 + 8 + 1;

/// Instruction data: tag(1=0) + position_id u64 LE + fingerprint u64 LE +
/// n_states u8 + tranche_bps u16 LE
pub const IX_COMMIT_POLICY: u8 = 0;
pub const COMMIT_IX_LEN: usize = 1 + 8 + 8 + 1 + 2;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if data.len() != COMMIT_IX_LEN || data[0] != IX_COMMIT_POLICY {
        return Err(ProgramError::InvalidInstructionData);
    }
    let position_id = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let fingerprint = u64::from_le_bytes(data[9..17].try_into().unwrap());
    let n_states = data[17];
    let tranche_bps = u16::from_le_bytes(data[18..20].try_into().unwrap());
    if n_states == 0 || n_states > 4 || tranche_bps == 0 || tranche_bps > 10_000 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let accounts_iter = &mut accounts.iter();
    let owner = next_account_info(accounts_iter)?;
    let policy = next_account_info(accounts_iter)?;
    let system = next_account_info(accounts_iter)?;

    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !system_program::check_id(system.key) {
        return Err(ProgramError::IncorrectProgramId);
    }

    let position_id_le = position_id.to_le_bytes();
    let seeds: &[&[u8]] = &[POLICY_SEED, owner.key.as_ref(), &position_id_le];
    let (expected, bump) = Pubkey::find_program_address(seeds, program_id);
    if expected != *policy.key {
        return Err(ProgramError::InvalidSeeds);
    }
    // Immutable: a policy for this (owner, position) may only exist once.
    if policy.lamports() > 0 || !policy.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let rent = Rent::get()?.minimum_balance(POLICY_LEN);
    invoke_signed(
        &system_instruction::create_account(
            owner.key,
            policy.key,
            rent,
            POLICY_LEN as u64,
            program_id,
        ),
        &[owner.clone(), policy.clone(), system.clone()],
        &[&[POLICY_SEED, owner.key.as_ref(), &position_id_le, &[bump]]],
    )?;

    let now = Clock::get()?.unix_timestamp;
    let mut out = policy.try_borrow_mut_data()?;
    out[0..32].copy_from_slice(owner.key.as_ref());
    out[32..40].copy_from_slice(&position_id_le);
    out[40..48].copy_from_slice(&fingerprint.to_le_bytes());
    out[48] = n_states;
    out[49..51].copy_from_slice(&tranche_bps.to_le_bytes());
    out[51..59].copy_from_slice(&now.to_le_bytes());
    out[59] = bump;

    msg!(
        "policy committed: machine {:x} ({} states, {} bps tranches)",
        fingerprint,
        n_states,
        tranche_bps
    );
    Ok(())
}
