//! tag 0 — `CommitPolicy`.

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

pub(crate) fn commit_policy(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
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
