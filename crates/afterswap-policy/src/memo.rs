//! tag 6 — `AnchorFill`, plus the no-alloc memo renderer it uses.

use crate::types::*;
use pinocchio::{
    account_info::AccountInfo,
    cpi::slice_invoke,
    instruction::Instruction,
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    ProgramResult,
};

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
pub(crate) fn anchor_fill(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
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
