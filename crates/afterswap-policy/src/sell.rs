//! tag 2 — `ValidateAndSell`, the gate's critical path.

use crate::token::{invoke_transfer_checked, require_vault_ata};
use crate::types::*;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

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
pub(crate) fn validate_and_sell(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
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
    // Read the vault balance to bound the amount. This account must be one
    // *this* program created: `vault_amount`, `vault_deposited` and
    // `vault_mint` are read straight out of it below, and every step-5 bound
    // is computed from them. The step-7 seed derivation does prove the
    // address, but it runs sixty lines after these reads — the same reason
    // step 1b pins the policy and execution owners, applied to the account
    // that decides how much may leave.
    if vault.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
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
    let vault_mint: [u8; 32] = vault_out[VAULT_OFFSET_MINT..VAULT_OFFSET_MINT + 32]
        .try_into()
        .unwrap();
    drop(vault_out);

    if vault_owner.as_ref() != exec_owner.as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }

    // ── Step 5b: source binding ─────────────────────────────────────────
    // Step 6 says where the tokens land; this says where they come from.
    // The mint is pinned to the one the vault was opened with, and the
    // source to that pair's associated token account — so a crank cannot
    // debit `vault.amount` against a look-alike account it funded itself
    // and strand the real balance. See `require_vault_ata`.
    if vault_mint.as_ref() != mint.key().as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }
    require_vault_ata(vault.key(), mint.key(), src_ata.key())?;

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
