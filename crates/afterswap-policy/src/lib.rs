//! AfterSwap exit-policy registry + delegated-execution gate (Pinocchio).
//!
//! Seven instructions:
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
//!   tag 6 `AnchorFill`         — publishes the fill as an SPL memo, the
//!                                sell-side counterpart of the shipped
//!                                `afterswap:quote` commit-side memo. Moves
//!                                no tokens, writes no account state; every
//!                                field but the quote digest is read from the
//!                                policy and execution PDAs. Phase B step 3.

//! Phase B step 2 design (vault-sourced):
//!   The owner deposits the position into a program PDA vault (tag 4).
//!   The vault PDA signs every `TransferChecked` (tag 2) — the cranker is
//!   a signer/fee-payer who never holds the vault's signing authority. The
//!   authorized-crank check (tag 1) gates which crankers may trigger sells;
//!   the crank picks *when* and *how much* (within the committed tranche
//!   budget) but never *where* — the payout account is bound at
//!   authorization time (`execution.settlement_ata`) and tag 2 rejects any
//!   other destination. An earlier version of this note said the cranker
//!   "can never move funds on their own", which the code did not support:
//!   the destination was unchecked, so an authorized crank could name its
//!   own ATA.
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
//!
//! Layout constants live in `types`; each instruction's handler lives in the
//! module named after the account it governs. This file is the entrypoint and
//! the tag dispatch, nothing else.

mod execution;
mod memo;
mod policy;
mod sell;
mod token;
mod types;
mod vault;

pub use types::*;

use execution::{authorize_execution, revoke_authorization};
use memo::anchor_fill;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use policy::commit_policy;
use sell::validate_and_sell;
use vault::{close_vault, deposit_to_vault};

pinocchio::program_entrypoint!(process_instruction);

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
        Some(IX_ANCHOR_FILL) => anchor_fill(program_id, accounts, data),
        // tag 4 is `DepositToVault`; tag 5 is `CloseVault`. `AnchorFill`
        // took tag 6 because 4 and 5 were already spent by the vault path.
        Some(_) | None => Err(ProgramError::InvalidInstructionData),
    }
}
