//! AfterSwap exit-policy registry (Pinocchio build).
//!
//! One instruction: `CommitPolicy` — create an immutable PDA recording
//! which exit machine (blake3-64 fingerprint) governs a position, before
//! any fill follows it. Anyone can later audit every DFlow fill against
//! the committed policy. Phase B (delegated execution) builds on this.
//!
//! PDA: seeds = ["policy", owner, position_id_le], owned by this program.
//! Immutability is the point: commits cannot be overwritten or resized.
//! Byte layout and instruction interface identical to the original
//! solana-program build — the LiteSVM tests are framework-agnostic.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

#[cfg(target_os = "solana")]
pinocchio::program_entrypoint!(process_instruction);

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
