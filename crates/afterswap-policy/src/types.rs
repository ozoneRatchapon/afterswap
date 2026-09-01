//! Byte layouts, PDA seeds, instruction tags and pinned program ids.
//!
//! Every offset here is load-bearing: the accounts are fixed-size and never
//! reallocated, so a layout constant is the only thing naming a field.

use pinocchio::pubkey::Pubkey;

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
pub(crate) const EXEC_OFFSET_POSITION_ID: usize = 32;
pub(crate) const EXEC_OFFSET_CRANK: usize = 40;
pub(crate) const EXEC_OFFSET_TRANCHES_FILLED: usize = 72;
pub(crate) const EXEC_OFFSET_EXECUTED_LAMPORTS: usize = 73;
pub(crate) const EXEC_OFFSET_LAST_TICK_SLOT: usize = 81;
pub(crate) const EXEC_OFFSET_EXPIRES_UNIX: usize = 89;
pub(crate) const EXEC_OFFSET_BUMP: usize = 97;
pub(crate) const EXEC_OFFSET_SETTLEMENT_ATA: usize = 98;

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
pub(crate) const VAULT_OFFSET_POSITION_ID: usize = 32;
pub(crate) const VAULT_OFFSET_MINT: usize = 40;
pub(crate) const VAULT_OFFSET_AMOUNT: usize = 72;
pub(crate) const VAULT_OFFSET_BUMP: usize = 80;
pub(crate) const VAULT_OFFSET_DEPOSITED: usize = 81;

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
pub(crate) const MEMO_PROGRAM_ID: Pubkey = [
    5, 74, 83, 90, 153, 41, 33, 6, 77, 36, 232, 113, 96, 218, 56, 124, 124, 53, 181, 221, 188, 146,
    187, 129, 228, 31, 168, 64, 65, 5, 68, 141,
];

/// Upper bound on the rendered memo body (see `render_fill_memo`):
/// 18 + 16 + 5 + 20 + 9 + 3 + 6 + 20 + 14 + 64 = 175.
pub(crate) const FILL_MEMO_CAP: usize = 192;

/// Offset of the blake3-64 fingerprint inside the policy PDA layout.
pub(crate) const POLICY_OFFSET_FINGERPRINT: usize = 40;

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
pub(crate) const SPL_TOKEN_TRANSFER_CHECKED_DISCRIMINATOR: u8 = 12;

/// SPL Token — `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`.
///
/// Every `TransferChecked` CPI must target exactly this program. The caller
/// supplies the token-program account, and `slice_invoke_signed` extends the
/// vault PDA's signature into whatever program it names — so an unpinned
/// token program hands vault-PDA signing authority to caller-chosen code.
/// Token-2022 is deliberately not accepted: its transfer-hook and
/// transfer-fee extensions change what a `TransferChecked` means, and this
/// gate's bounds are written against the classic semantics.
pub(crate) const SPL_TOKEN_PROGRAM_ID: Pubkey = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];

/// Associated Token Account program — `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`.
pub(crate) const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = [
    140, 151, 37, 143, 78, 36, 137, 241, 187, 61, 16, 41, 20, 142, 13, 131, 11, 90, 19, 153, 218,
    255, 16, 132, 4, 142, 123, 216, 219, 233, 248, 89,
];
