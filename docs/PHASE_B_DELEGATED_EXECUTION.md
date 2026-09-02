# Phase B — delegated execution: design

Status: **step 2 built and tested; not deployed.** This document began as
design-only and is no longer. `crates/afterswap-policy/src/lib.rs` now exposes
six instructions — `CommitPolicy` (0), `AuthorizeExecution` (1),
`ValidateAndSell` (2), `RevokeAuthorization` (3), `DepositToVault` (4) and
`CloseVault` (5) — covered by 26 LiteSVM tests against the compiled SBF binary
(`tests/policy.rs` 2, `tests/execution.rs` 7, `tests/vault.rs` 17), all green.
**What is deployed to devnet is still the Phase A program: `CommitPolicy`
only.** The rung this closes: the old program could prove a violation *after*
the fact but not *prevent* one, and the live UX cost a wallet prompt per
tranche. Sections below that read as forward-looking describe the shape that
now exists in code; treat the build order in §9 as a record, not a plan.

## 1. What "delegated execution" must achieve

Three properties, in priority order — any design that sacrifices the first
for the last is out of scope:

1. **Enforcement, not just audit.** A sell that does not follow the committed
   policy must be *impossible*, not merely *provable as a violation*. Today
   the policy PDA is a witness; Phase B makes it a gate.
2. **No custody.** The user's tokens never sit in a program-owned vault
   without a withdrawable path. Every balance the program can move must be
   one the owner authorised it to move, and that authorisation must be
   revocable at any time by the owner alone.
3. **No per-tranche signer.** After a one-time approval, tranches execute
   without the owner's key in the loop — permissionless crank, anyone
   may trigger when due.

A note on the primitive, because it shapes the rest of the design — and a
**correction to an earlier draft of this document**, which claimed that SPL
Token "delegates can only transfer into the delegate's own account." *That is
false.* `Approve` sets a delegate and a `delegated_amount` on the source
account; the delegate may then sign `Transfer` / `TransferChecked` to an
**arbitrary destination**, capped by the approved amount. The `authority`
account in `TransferChecked` is "the source account's owner **or** delegate",
and nothing constrains the destination. No design decision should rest on the
false version.

What is actually built is **vault-sourced**: the owner deposits the position
into a program-owned PDA vault (`DepositToVault`), and the **vault PDA** signs
every `TransferChecked` out of it. The cranker never holds the authority — it
merely triggers, and the program is unavoidably in the transfer path, which is
what turns the policy PDA from a witness into a gate.

**The honest trade-off, since the false premise was doing the arguing.** A
delegate on the owner's *own* ATA — with a PDA of this program as the delegate
— would also put the program in the path and would be **strictly better on
non-custody**: the tokens never leave the owner's account, and `Revoke` is a
one-instruction exit that needs no cooperation from us. The vault design
instead takes real custody between deposit and sell, mitigated (not
eliminated) by `CloseVault` being owner-only and always available. The vault
was chosen and built; the delegate variant is the open alternative and is
recorded in §8 rather than dismissed.

Everything in this design is subordinate to those three.

## 2. What already exists (build on, do not rebuild)

- **Policy PDA** (Pinocchio, 18 KB, live on devnet):
  `["policy", owner, position_id_le]` → `{owner, position_id, fingerprint
  (blake3-64), n_states, tranche_bps, committed_at_unix, bump}`.
  Immutable per (owner, position). This is the gate's rulebook.
- **Memo binding pattern** (`afterswap:quote sha-256=…` beside the commit) —
  the pattern the fill anchor reuses, already shipped.
- **LiteSVM test harness** against the real compiled SBF binary — the same
  harness Phase A used; Phase B instructions get tests in the same style.
- **`hydra` + `ephemeral-spl-token`** (MagicBlock, MIT, Pinocchio `no_std`,
  same framework as our program) — recorded in `ROADMAP.md` §4 as the
  permissionless-crank and non-custodial-balance primitives. *Unverified by
  us*; the table records what those repos state. Integration is the point of
  this phase; audit precedes mainnet.

## 3. Account model

Three account kinds, two of them new:

```
policy        PDA ["policy", owner, position_id]      (existing)
                rulebook: fingerprint, n_states, tranche_bps

execution     PDA ["execution", owner, position_id]   (new)
                the gate's mutable state:
                { authorized_crank (32), tranches_filled (u8),
                  executed_lamports (u64), last_tick_slot (u8),
                  expires_unix (u8), bump, settlement_ata (32) }
                `settlement_ata` is the one account a sell may pay into,
                named by the owner at authorization time.

owner_ata     SPL Token ATA for (owner, mint)         (existing pattern)
                source of every transfer; the owner's tokens, not the
                program's

crank_pda     the `hydra` crank PDA (or equivalent)       (new, external)
                signs each `TransferChecked` — the account that moves
                funds to the arbitrary DFlow destination; this is the
                authority, recorded in the `execution` PDA
```

The `execution` PDA is the only mutable state. The `policy` PDA stays
immutable — the rulebook is never edited, only superseded by a new commit
for a new position. This separation is deliberate: an auditor reads one
account to get the rule and another to get progress, and neither can be
rewritten.

## 4. Instruction surface

Tags continue the existing byte layout (tag is `data[0]`, fixed-length data,
no borsh, no realloc — identical discipline to `CommitPolicy`).

**What the program actually exposes today** — this list is the authority; the
subsections below it were written before the build and are annotated where
they drifted:

| Tag | Instruction | Status |
|---|---|---|
| 0 | `CommitPolicy` | built (Phase A), deployed to devnet |
| 1 | `AuthorizeExecution` | built + tested, **not deployed** |
| 2 | `ValidateAndSell` | built + tested, **not deployed** |
| 3 | `RevokeAuthorization` | built + tested, **not deployed** |
| 4 | `DepositToVault` | built + tested, **not deployed** |
| 5 | `CloseVault` | built + tested, **not deployed** |
| 6 | `AnchorFill` | built + tested (2026-08-29), **not deployed** |

`DepositToVault` and `CloseVault` are the vault-sourced custody path (§1) and
have no subsection below because they postdate this document's §4.

### 4.1 `AuthorizeExecution` — tag 1

Accounts: `owner (signer)`, `execution PDA`, `crank pubkey (readonly)`,
`system`.

Sets `execution.authorized_crank = <pubkey>`,
`execution.expires_unix`, and `execution.settlement_ata` — the single
token account `ValidateAndSell` may pay into. The owner names the payout
destination in the same act that names the crank, so the crank chooses
*when* and *how much*, never *where*. A zero `settlement_ata` is rejected
(it is the value revocation writes to the crank field). Called once per
position, and again after a revocation to rebind. The authorised crank
is the `hydra` crank PDA (or equivalent), **not** a hot wallet — and it
is an *authorization* our program records, not an SPL Token delegation.
In the built design the **vault PDA**, not the crank, signs each
`TransferChecked`; the crank only triggers. (An earlier draft justified
avoiding the Token program's `Delegate` ix by claiming a delegate "cannot
transfer to an arbitrary destination" — that is false, see §1. `Delegate`
is unused because the vault path was chosen, not because it could not
work.) Revocation is a second instruction (§4.3) or expiry — there is no
silent path.

**Why a dedicated instruction rather than folding into `CommitPolicy`:**
committing a policy and authorizing execution are independent decisions.
A user may commit a policy for paper mode (no authorization) and authorize
execution later, or commit for a different position with the same crank.
Coupling them would force re-commit on crank change, which breaks the
immutability guarantee we built Phase A on.

### 4.2 `ValidateAndSell` — tag 2 (the gate)

This is the instruction that closes the missing rung. It is the **only**
instruction that moves tokens, and it is the only place the program
enforces the policy.

Accounts: `cranker (signer, any)`, `policy PDA (readonly)`, `execution PDA`,
`owner ATA (writable)`, `owner (writable, for fee/lamport settle)`,
`system`, `token program`.

Instruction data (fixed length, `VALIDATE_SELL_LEN`):

```
tag(1=2)
| position_id (8, LE)          // must match policy.position_id
| expected_state (1)           // the FSM state the cranker claims
| tranche_index (1)            // which tranche this is (0-based)
| tick_slot (8, LE)            // the DFlow context_slot this decision used
| quote_digest (32)            // sha-256 of the signed quote body
```

**Validation sequence (all-or-nothing, in this order):**

1. `policy` PDA exists, is owned by this program, and `policy.position_id`
   equals `data.position_id`. Fingerprint is read, not re-derived — the
   rulebook was committed before any fill; we verify *against* it.
2. `execution.authorized_crank` equals the cranker's pubkey (the crank
   PDA), or the cranker is the owner themselves (owner override, always
   allowed). This is the authorization check.
3. `now < execution.expires_unix` (if set). Expired = no more sells.
4. `tranche_index == execution.tranches_filled` (monotonic; no skipping,
   no replay).
5. **State consistency:** the cranker supplies `expected_state`; the program
   does not re-run the FSM (it cannot — the machine genome is not on-chain).
   Instead it enforces the *invariant* the committed policy implies:
   `amount * 10_000 ≤ tranche_bps * vault.deposited` (tranche size never
   exceeds the committed budget, measured against the position as
   deposited — a fixed denominator, so ten 10% tranches exit the position
   exactly rather than chasing a shrinking remainder), and
   `tranche_index < ceil(10_000 / tranche_bps)` (no more fills than the
   committed tranche size divides the position into). The exact transition
   table is the auditor's job, verified off-chain against the fingerprint.
   The on-chain guarantee is bounded loss + monotonic progress; the
   off-chain guarantee is policy fidelity. This split is stated because
   it is the honest limit of what 18 KB of program can enforce.

   *Corrected 2026-09-01.* The tranche index was bounded by `n_states`,
   the FSM's state count — an unrelated quantity that `CommitPolicy` caps
   at 4, so a 10% tranche policy could sell at most 40% of a position and
   the gate had no path to a full exit. And the `tranche_bps` bound was
   documented but never enforced: the code read the field into `_tranche_bps`
   and discarded it, leaving `amount ≤ vault_balance` as the only size
   limit. Both now come from the field that means them.
6. **Destination binding:** the destination ATA must equal
   `execution.settlement_ata`. The vault PDA signs the transfer, so without
   this check an authorized crank could name its own ATA and drain the
   vault one legal tranche at a time — being on the account list is not a
   permission. *Added 2026-09-01; the destination was previously
   unverified.*
7. Move the amount from the `vault ATA` to that destination via
   `TransferChecked` on the SPL Token program, signed by the vault PDA.
   The token program account is pinned to
   `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`: the CPI carries the vault
   PDA's signature, so an unpinned token program would extend the vault's
   signing authority into caller-chosen code. Token-2022 is not accepted —
   its transfer hooks and fees change what `TransferChecked` means, and
   these bounds are written against the classic semantics. *Pin added
   2026-09-01; applies to tags 2, 4 and 5.*
   (`TransferChecked`, not `Transfer`, because amounts are raw integers
   with decimals — the same ground rule as `RAIL.md` §3.1.)

On success: `execution.tranches_filled += 1`, `execution.executed_lamports +=
<amount>`, `execution.last_tick_slot = tick_slot`.

On any failure: no tokens move, `tranches_filled` unchanged, the failed
attempt is the cranker's loss of fee. No partial state.

**This is the enforcement.** A sell that violates steps 5–7 cannot
succeed, because the program refuses to sign the `Transfer`. The witness
becomes the gate.

### 4.3 `RevokeAuthorization` — tag 3

Accounts: `owner (signer)`, `execution PDA`, `system`.

Zeros `execution.authorized_crank`. Idempotent. A zeroed crank is also
the re-authorizable state: revoke keeps the account alive, so
`AuthorizeExecution` rebinds crank, expiry and settlement destination in
place. (It previously returned `AccountAlreadyInitialized` for any
existing account, which made revocation a one-way door — the owner could
never delegate that position again. Fixed 2026-09-01.) The owner is the
only party who can call it — the authorised crank cannot revoke itself, and
no other cranker can. Once revoked, `ValidateAndSell` fails at step 2
for any cranker; only the owner override (step 2, second branch) can
still move tokens, and only if the owner re-authorizes.

### 4.4 `AnchorFill` — tag 6 (built + tested 2026-08-29)

Accounts as built: `signer (fee payer)`, **`policy PDA (readonly)`**,
`execution PDA (readonly)`, `memo program`. Instruction data: tag(1) +
`quote_digest`(32) = 33 bytes.

Emits `afterswap:fill fp=<blake3-64> pos=<position_id> tranche=<index>
slot=<tick_slot> quote=sha256:<digest>` — the identical pattern to the
shipped `afterswap:quote` memo, now on the *sell* side. This is the third
link of the verifiable chain, on the fill transaction itself, which
`OPPORTUNITIES.md` §3.2 lists as "remaining to make it end-to-end for
mainnet fills."

**Two departures from the sketch above, both deliberate:**

- **The policy PDA is an account, and the caller supplies almost nothing.**
  The sketch passed only the execution PDA, which implies the fingerprint
  came from the instruction data — i.e. from the caller. A memo whose
  contents the caller chooses proves nothing. As built, the fingerprint is
  read from the policy PDA, and the tranche index (`tranches_filled - 1`)
  and tick slot are read from the execution PDA. The **quote digest is the
  only caller-supplied field**, and it has to be: it names an off-chain
  quote the chain never saw.
- **Not "any signer."** Because the quote digest *is* caller-supplied, an
  open instruction would let any fee-payer bind a false quote to a real
  fill. The signer is restricted to the authorized crank or the owner — the
  same two parties `ValidateAndSell` accepts.

Both PDAs are derived from `exec_owner + position_id` and owner-checked
before their contents are trusted, for the reason recorded in §7b finding 1:
a look-alike account would otherwise let a crank publish a chain link naming
a machine that never governed the position. `anchor_fill_rejects_a_
substituted_policy_account` is that attack, aimed at the memo.

**Measured cost (LiteSVM, 2026-08-29):** 57,041 CU for the whole
transaction, of which **51,717 CU is SPL Memo itself** — the AfterSwap half
is ~5.3k CU, and the memo program dominates. Well inside the 200k default,
so no compute-budget instruction is needed; but note that batching a memo
into the `ValidateAndSell` transaction rather than sending it separately
would put a 52k-CU dependency on the critical path, which is why this is a
separate instruction.

Rejections: no fill yet → `Custom(3)` (also prevents `tranches_filled - 1`
wrapping to 255); signer neither crank nor owner → `Custom(1)`; memo program
not SPL Memo v3 → `IncorrectProgramId`; substituted PDA → `InvalidSeeds`.

**Rendering caveat for verifiers.** The on-chain `fp=` field is zero-padded
to a fixed 16 hex digits; the off-chain renderers (`worker/index.ts`, the
demo page) print the same value via `BigInt(...).toString(16)`, which strips
leading zeros. Compare the two **numerically, not as strings.**

### 4.5 `ClosePosition` — superseded by `CloseVault` (tag 5, built)

Written as deferrable on the reasoning that "the owner can always recover
tokens via the Token program `Revoke` + `Transfer` path regardless of what
this program says, so this is a convenience, not a security control."
**That reasoning holds only for the delegate design, which is not what was
built.** Under vault custody the owner's tokens are inside a PDA-owned ATA
and the Token program offers no unilateral exit — so the reclaim path is a
*security control*, and it was built as `CloseVault` (tag 5): owner-only,
idempotent when empty, and covered by three tests. This subsection is kept
only to record why the original framing was wrong.

## 5. Crank model

`hydra` provides the permissionless-crank primitive: scheduled instructions
live in a crank PDA; anyone may trigger when due. The AfterSwap flow:

1. Owner commits policy (`CommitPolicy`, existing).
2. Owner authorizes execution (`AuthorizeExecution`), crank = crank PDA.
3. The Worker (or any node) watches the execution PDA. When the engine's
   decision says "sell tranche N," it submits a `hydra` scheduled
   instruction carrying the `ValidateAndSell` payload, signed by the
   crank PDA.
4. Any cranker (the Worker, a MEV-free relay, the user's browser) triggers
   it. The program validates; the crank PDA signs the `TransferChecked`; the
   cranker pays fees.
5. The Worker observes the fill, records the `AnchorFill` memo, and
   continues the next window.

**No hot key.** The Worker never signs a sell. The owner never signs a
sell after step 2. The crank PDA is the signer, via its own schedule.
This is the UX failure, dissolved: the wallet prompt per tranche is gone.

**If `hydra` integration is infeasible** (framework mismatch, audit
blocker): the fallback is the existing Worker-signed path, which is what
ships today. Phase B without `hydra` still delivers the *enforcement*
rung (the program validates before any `TransferChecked`) — it just keeps
the per-tranche prompt. The authorization rung is the optional bonus.

## 6. Security surface and audit notes

This is a real security surface. The audit scope, stated explicitly:

- **`ValidateAndSell` is the critical path.** Every branch is a potential
  token-out. The invariant set (steps 1–6 above) must be audited as a
  unit. The off-chain limit (step 5: the program enforces bounds, not the
  full transition table) is a stated design boundary, not a bug — but the
  auditor must confirm the bounds are tight enough that no sequence of
  valid `ValidateAndSell` calls can extract more than `n_states ×
  tranche_bps` of the position.
- **Crank authorization.** The authorised crank pubkey is set once and
  revoked once. There is no delegation of the delegation, no re-
  authorization without owner signature. The authority mechanism is the
  `TransferChecked` signer (the crank PDA), recorded by our program —
  not the SPL Token `Delegate` ix, which cannot route to an arbitrary
  destination. Our program does not invent a new authority primitive; it
  records and enforces the crank identity.
- **No external calls beyond the Token program.** The program does not
  invoke the DFlow route, does not call an oracle, does not trust a price
  passed in `data`. The `quote_digest` is a binding for the auditor, not a
  price the program acts on. This is the smallest possible trust surface.
- **`hydra` and `ephemeral-spl-token` are unverified by us.** MIT,
  Pinocchio, same framework — but the audit must cover the integration
  boundary, not just our program. If either primitive is not audited by
  its author, mainnet is blocked regardless of our program's quality.
- **Devnet → mainnet.** Mainnet deploy ≈ 0.13 SOL rent at 18 KB (the new
  instructions add a few KB; re-measure after build). The devnet history
  resets periodically (`RAIL.md` §8), so the audit trail on devnet is not
  durable evidence. Mainnet is a precondition for the "verifiable" claim
  to mean anything to a third party.

## 7. Test plan (LiteSVM, same style as `tests/policy.rs`)

**Status: done, and exceeded.** The plan below asked for 10 cases; the build
landed **42 tests, all green** against the compiled SBF binary —
`tests/policy.rs` 2, `tests/execution.rs` 9, `tests/vault.rs` 23,
`tests/anchor.rs` 8. **All 10 cases are now covered**; case 9 closed on
2026-08-29 when `AnchorFill` was built. The shared LiteSVM environment moved
to `tests/common/mod.rs` at the same time — `vault.rs` had already outgrown
the 1024-line limit, and a second copy of a thirteen-account builder is
exactly the drift that broke its call sites once before.
The vault path (`DepositToVault` / `CloseVault`) is tested beyond this plan:
deposit creates and tops up, non-signer and wrong-PDA rejects, sell rejects
amounts exceeding the vault balance and zero amounts, close reclaims, close
is idempotent when empty, and close rejects a non-owner.

**Account-substitution cases (added 2026-09-01).** The plan above tested
what the caller *says* (instruction data) far more thoroughly than what the
caller *supplies* (accounts), which is where all four gate bugs lived. Each
case below substitutes one account and asserts rejection *and* that no
tokens moved: a sell to any destination other than the bound settlement
ATA, and a foreign token program on each of the three transfer paths (sell,
deposit, close). Two more cover the tranche budget — one base unit over is
rejected while exactly the budget is allowed, and ten 10% tranches exit the
position exactly, which is what proves the denominator is deposits and not
the remainder. Two on the authorize side: a zero settlement ATA is
rejected, and re-authorizing after a revoke rebinds crank, expiry and
destination.

The generalization worth building next: for every instruction × every
account it takes, a case that substitutes an attacker-controlled account
and asserts rejection. That matrix is enumerable the same way the FSM space
is (~7 instructions × 6–8 accounts), and it would have caught all four of
these without anyone reading the code.

The original plan, kept for the record:

1. `AuthorizeExecution` sets the authorised crank; `RevokeAuthorization`
   clears it; double-authorization by a non-owner fails.
2. `ValidateAndSell` with a correct crank, tranche index, and authorized
   crank succeeds and moves exactly `tranche_bps` of the position.
3. `ValidateAndSell` with a wrong `position_id` (mismatch with policy PDA)
   fails, no tokens move.
4. `ValidateAndSell` with `tranche_index ≠ tranches_filled` (skip or
   replay) fails, no tokens move, `tranches_filled` unchanged.
5. `ValidateAndSell` with `tranche_index ≥ n_states` fails.
6. `ValidateAndSell` after `RevokeAuthorization` fails for any non-owner
   cranker; succeeds for the owner (owner override).
7. `ValidateAndSell` after expiry (`now ≥ expires_unix`) fails.
8. Full sequence: commit → enable → sell 3 tranches (3-state machine) →
   4th sell fails (exceeds `n_states`). `executed_lamports` equals
   `3 × tranche_bps` of position.
9. `AnchorFill` emits the memo; the memo contains the correct fingerprint,
   position id, tranche index, slot, and quote digest. **Covered** by
   `tests/anchor.rs`, plus five rejection cases the plan did not ask for.
10. **Immutability regression:** `CommitPolicy` still refuses a second
    commit for the same (owner, position) — the existing test in
    `tests/policy.rs` must still pass unchanged.

## 7b. Security findings fixed before this was written down

Three defects were found reviewing the built code, all in the token-moving
path, all fixed with tests. Recorded because "built and tested" said nothing
about *reviewed*.

1. **Critical — the gate could be bypassed with a substituted policy account.**
   `ValidateAndSell` validated the `policy` account only by its length and by
   the `position_id` stored *inside* it. It never derived the policy PDA and
   never checked the account's owner. `n_states`, read from that account, is
   the only on-chain bound on how many tranches may be sold. An authorized
   crank could therefore commit *its own* policy for the same `position_id`
   with a larger `n_states` — a genuine, program-owned, correctly-sized
   account — pass it in place of the victim's, and keep selling past the
   committed machine size. That is the exact guarantee the program exists to
   provide. Fixed by deriving both the policy and execution PDAs from
   `exec_owner + position_id` and rejecting a mismatch, plus explicit
   `owner() != program_id` checks. Regression test:
   `sell_rejects_substituted_policy_account` in `tests/vault.rs`, which was
   confirmed to **fail against the pre-fix binary** — the substituted policy
   was accepted and the sell went through — and pass after.
2. **Medium — unchecked balance addition.** `DepositToVault` did
   `current + transfer_amount` on the vault balance: a debug panic, a silent
   wrap in release. Now `checked_add(..).ok_or(ArithmeticOverflow)?`.
3. **Compute — `CloseVault` derived the same PDA twice**, discarding the key
   it had just validated on the second call. `find_program_address` is one of
   the more expensive things a Pinocchio program can do. Now derived once.

The Solana MCP `program_autofixer` flagged 2 and 3 and reports **no remaining
issues**. It did **not** flag 1 — it detects a PDA derived-but-unvalidated,
and the bug was a PDA never derived at all. Worth knowing before treating a
clean autofixer run as a security review; it is not one, and neither is this.

## 8. What this does not do (stated, not implied)

- It does not run the FSM on-chain. The transition table is not in the
  program. The on-chain guarantee is bounded, monotonic, and
  revocable. The policy-fidelity guarantee is off-chain, verified by an
  auditor who has the fingerprint and the quote digest. This is the
  honest limit of 18 KB and is a feature (small surface) as much as a
  constraint.
- **It does custody tokens, between deposit and sell.** An earlier draft of
  this list claimed the opposite ("the owner's ATA is the source of every
  `TransferChecked`"); that describes a design that was not the one built.
  The source is the **vault PDA's** ATA, and tokens sit there from
  `DepositToVault` until they are sold or reclaimed. The mitigation is that
  `CloseVault` is owner-only, needs no cooperation from the crank or the
  Worker, and is always available — but "no custody" is not a claim this
  design may make, and property 2 in §1 is therefore only partially met.
- **The open alternative: delegate-on-owner-ATA.** Approving a PDA of this
  program as the SPL delegate on the owner's own ATA would keep the program
  in the transfer path — so enforcement is unchanged — while leaving the
  tokens in the owner's account and making `Revoke` a one-instruction exit.
  That would satisfy property 2 fully. It was not built, and the earlier
  draft ruled it out on a factually false premise (see §1). It should be
  evaluated on its merits before Phase B is deployed, not inherited.
- It does not replace the Worker. The Worker still runs the engine, makes
  the decision, and submits the `hydra` schedule. The program is the
  enforcement point, not the decision point.
- **`AnchorFill` is built as of 2026-08-29** (an earlier version of this
  line said it was not). The shipped tags are 0 `CommitPolicy`, 1
  `AuthorizeExecution`, 2 `ValidateAndSell`, 3 `RevokeAuthorization`, 4
  `DepositToVault`, 5 `CloseVault`, 6 `AnchorFill` — tag 4 is the vault
  deposit, *not* `AnchorFill` as an earlier draft stated. All seven are
  built and tested; **none of tags 1–6 is deployed.**
- It does not close the mainnet anchor gap by itself. The `RAIL.md` §3.3
  Merkle-root anchoring is a separate mechanism on the Worker side. Both are
  needed for the end-to-end claim; this document covers the program-side
  half.

## 9. Build order

1. ~~Implement `AuthorizeExecution` + `RevokeAuthorization` (tags 1, 3). Test.~~ **DONE.**
2. ~~Implement `ValidateAndSell` (tag 2). Test (cases 2–8 above).~~ **DONE**,
   plus the unplanned `DepositToVault` (4) and `CloseVault` (5).
3. ~~Implement `AnchorFill` (tag 6 — tags 4 and 5 went to the vault path).
   Test (case 9).~~ **DONE 2026-08-29** — 8 tests in `tests/anchor.rs`.
   Every instruction in the table above is now built.
4. ~~Re-run the existing `tests/policy.rs` suite (case 10).~~ **DONE** — 2/2,
   and the whole workspace is green.
5. ~~Re-measure binary size; confirm < 60 KB (rent budget < 0.4 SOL).~~
   **DONE, measured 2026-08-29:** `afterswap_policy.so` was **35,736 bytes
   (34.9 KB)** at 0.2496 SOL rent before `AnchorFill`, and **42,368 bytes
   (41.4 KB)** after it. `getMinimumBalanceForRentExemption(42368)` on devnet
   returns **295,772,160 lamports = 0.29577216 SOL** (queried 2026-08-29).
   The 2026-09-01 gate fixes (destination binding, token-program pin, real
   tranche bound) take it to **45,600 bytes (44.5 KB)**; rent scales with
   size and has not been re-queried, since nothing is deployed yet.
   Both targets still met: 44.5 KB < 60 KB, ~0.32 SOL < 0.4 SOL. The
   0.2496 SOL figure above is the pre-`AnchorFill` measurement.
6. Devnet deploy; run the demo against the new program. **NOT DONE** — devnet
   still runs the Phase A `CommitPolicy`-only program.
7. Audit (external). Mainnet deploy post-audit. **NOT DONE.**
8. `hydra` integration: separate workstream, blocked on `hydra` audit
   status. Fallback path (Worker-signed) works without it. **NOT DONE.**

**Before step 6, settle the vault-vs-delegate question in §8.** Deploying the
vault design to devnet makes it the thing demoed and written about, and the
non-custody property it gives up is one the project has claimed elsewhere.

## 10. Relation to the DFlow partner ask

The DFlow ask (`docs/DFLOW_PARTNER_ASK.md`) asks for the declarative-swap
API access (`/intent`, `/submit-intent`) and production rate limits. Phase B
is the program-side complement: the same verifiable chain, closed on the
execution side. Together they form the "Verifiable Execution Rail" that
`RAIL.md` specifies — the program enforces, DFlow signs, the Worker
records, and the auditor verifies all three without calling any of us.
