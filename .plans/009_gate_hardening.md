# 009 — Gate hardening: making `ValidateAndSell` enforce its own docstring

Status: **done for the program (55 tests green, G7 substitution gate closed)
and for the signing oracle (per-visitor cap, DEPLOYED to prod 2026-08-31 as
version `cb047d28`); the quote-digest overclaim is reworded to match
behaviour. No open decisions. The worker is deployed; the program is not.**
Date: 2026-09-01.

## Why this exists

Every bench in this repo landed on the same conclusion: there is no durable
edge (Romano–Wolf 0 survivors, PBO 0.05–0.20, live soak +0.10 bps at
t = +0.36). What survives is *verifiability* — the claim that a fill followed
a policy committed in advance, at a price the venue really offered.

The engine defends that claim with six gates (G1–G6). `afterswap-policy` —
the only code in the repo that moves tokens — had none. A read of
`validate_and_sell` against its own docstring found four places where the
code did less than the comment above it said.

Phase B is built but **not deployed** (ROADMAP: devnet deploy is gated on
settling vault-vs-delegate). So the layout changes below cost no migration.

## What was wrong, and what it is now

### 1. The destination was never checked — `lib.rs`, `validate_and_sell`

`dest_ata` appeared only as an argument to `invoke_transfer_checked`. The
vault PDA signs that transfer, so an authorized crank could name its own ATA
and drain the vault at a perfectly legal tranche size and cadence. The file
header claimed the cranker "can never move funds on their own"; that was
false as written.

Note the asymmetry: `deposit_to_vault` *did* validate its destination
properly (owner is the token program, authority is the vault PDA). The
money-*in* path was guarded and the money-*out* path was not.

**Now:** the owner names the payout account when they name the crank.
`execution.settlement_ata` (32 bytes, appended to the PDA so every existing
offset is unchanged), non-zero required, and `validate_and_sell` rejects any
other destination. The crank picks *when* and *how much*; never *where*.

### 2. The token program was not pinned — all three transfer paths

`invoke_transfer_checked` passed `token_program.key()` straight into
`Instruction.program_id` while `slice_invoke_signed` signed with vault PDA
seeds. Signer privilege extends into the CPI, so a caller-supplied token
program received vault-PDA signing authority. Real escalation in
`validate_and_sell` (crank-callable); self-harm only in `close_vault` and
`deposit_to_vault` (owner-signed), but pinned in all three.

**Now:** `SPL_TOKEN_PROGRAM_ID` const; tags 2, 4 and 5 all check it. Token-2022
is deliberately not accepted — transfer hooks and transfer fees change what a
`TransferChecked` means, and these bounds are written against classic
semantics.

### 3. `tranche_bps` was committed, documented, and discarded

The docstring promised `amount <= tranche_bps / 10_000 * vault_balance`. The
code read the field into `_tranche_bps` and threw it away; an inline comment
admitted only `amount <= vault_amount` was enforced. So the "bounded loss"
property did not exist.

**Now:** enforced as `amount * 10_000 <= tranche_bps * vault.deposited`, in
u128 (`amount * 10_000` overflows u64 above ~1.8e15 base units, which a
9-decimal mint reaches at 1.8M tokens).

`vault.deposited` is a new monotone field — the sum of every deposit ever.
The denominator has to be fixed: against the live balance, a 10% tranche is
10% of the *remainder*, so the position asymptotes and never exits. Against
deposits, ten 10% tranches exit it exactly. There is a test for precisely
this.

### 4. The tranche count was bounded by the FSM's state count

`tranche_index < n_states`. These are unrelated quantities, and
`commit_policy` caps `n_states` at 4 — so a 10% tranche policy could sell at
most 40% of a position and the gate had **no path to a full exit**. This was
a functional bug, not only a documentation one.

**Now:** `tranche_index < ceil(10_000 / tranche_bps)`, clamped to 255
(`tranches_filled` is a u8). Still derived entirely from committed policy
data — just from the field that means it.

### 5. Revocation was a one-way door — `authorize_execution`

The docstring said the owner may re-authorize after a revoke. The code
returned `AccountAlreadyInitialized` for *any* existing account, and revoke
keeps the account alive — so revoking once permanently ended the owner's
ability to delegate that position. The documented path was unreachable.

**Now:** a zeroed crank is the re-authorizable state; crank, expiry and
settlement destination rebind in place. Overwriting a *live* authorization is
still rejected (the existing test that asserts this still passes unchanged).

## Tests

34 → **42, all green** against the compiled SBF binary. Eight new, under a
new "account-substitution gate" heading in `tests/vault.rs`:

- sell to any destination other than the bound settlement ATA (asserts the
  vault is untouched, then that the bound destination still works)
- foreign token program on each of the three transfer paths
- one base unit over the tranche budget rejected; exactly the budget allowed
- ten 10% tranches exit the position exactly — the deposits-not-remainder proof
- zero settlement ATA rejected at authorize time
- re-authorize after revoke rebinds crank, expiry and destination

Three existing tests changed meaning and were retargeted rather than
loosened: `sell_rejects_tranche_beyond_n_states` →
`..._beyond_the_committed_tranche_count`, `full_sequence_three_states_then_fourth_fails`
→ `full_sequence_runs_the_schedule_then_stops`, and the two close-vault tests
now sell a legal tranche. The sell amounts in `vault.rs` and `anchor.rs`
dropped from 4_000 to 1_000 because 40% of a position under a 10% tranche
policy is exactly what the gate now refuses.

Binary 42,368 → **45,600 bytes** (44.5 KB), still inside the 60 KB budget.
Rent not re-queried; nothing is deployed.

## Worker / web (same review, different layer)

Two fixed locally, **not deployed**:

- **Demo slot burned on signing failure.** `worker/index.ts` took a slot from
  the DO *before* signing, so a 502 from `commitPolicy` consumed one of 380
  against a transaction that never existed. Added `/slot/release`. It is
  LIFO-guarded: `cycles` is both the count and the next PDA index, so
  releasing when a later slot has already gone out would re-issue a PDA
  someone else is committing, and `CommitPolicy` is immutable per PDA. A
  silently burnt slot beats a collision, so anything but the top of the stack
  is declined.
- **`ok === null` counted as verified.** `verifySignedResponse` returns
  `ok: null` when the browser has no WebCrypto Ed25519 (digest checked,
  signature not). `fetchPrice` incremented `verifiedCount` anyway, and the
  chip read "N DFlow-signed quotes verified". Now counted and displayed
  separately; README §5 updated to match. Behaviour is unchanged — those
  ticks still trade, since failing closed would break older browsers
  entirely — only the claim is now accurate.

## The signing oracle — closed 2026-09-01, still needs a deploy

`/api/commit-policy` had no auth, no rate limit and no per-visitor cap, over
a global `MAX_DEMO_COMMITS = 380`. Anyone who found the endpoint could
exhaust the demo budget before demo day. It was the highest-probability real
attack on the project.

The endpoint stays unauthenticated — that *is* the demo: a real on-chain
commitment with no wallet. What it now has is a per-visitor cap.

- **`Scoreboard.MAX_PER_IP = 3` per `IP_WINDOW_MS` (1h)**, held in a new
  `ip_quota` table in the DO that already owns the budget. No new binding, no
  new dependency, nothing the free plan rejects.
- **The visitor key is mandatory at `/slot`.** An absent `ip` param is a 400,
  not an uncapped request — otherwise the bypass is just omitting it.
- **The key is 8 bytes of SHA-256 over `cf-connecting-ip`**, which the
  Cloudflare edge overwrites, so it is not client-forgeable. Raw addresses
  never reach storage. This is a key, not anonymisation — IPv4 is small
  enough to enumerate against the digest — and the comment in
  `worker/index.ts` says so rather than claiming otherwise.
- **Fixed window, not sliding**: a visitor can get 2 × cap across a boundary.
  That costs a handful of the 380 and keeps this to one row and one
  statement, which is what a counter-only DO on the free CPU budget deserves.
- **Every take is refunded when nothing reaches devnet** — signing failure,
  slot-out-of-range, and the global-budget refusal that happens *after* the
  quota is taken. The three release paths were DRYed into one
  `release_slot()` helper that carries both the slot and the visitor.

Found while testing this: **`pkcs8()` silently zero-padded a short seed**
into a valid, wrong Ed25519 key. A misconfigured `DEMO_KEYPAIR` therefore
produced a real signature that devnet rejects for not matching the fee payer
— burning one slot per request until the budget was gone, with a 200 on every
one. Now `pkcs8` refuses a seed under 32 bytes, and `/api/commit-policy`
checks *before taking a slot* that the secret is 64 bytes and that its public
half equals `pda_table.owner`. A bad secret is a 503 that costs nothing.

Verified live against `wrangler dev --local`, not by inspection:
3 commits succeed and the 4th and 5th are refused; two addresses hold
independent quotas; issued slots stay contiguous (0,1,2…) so refusals leak
nothing; four forced post-slot failures burn zero slots and zero quota, and
the next good call from the same address still succeeds. Test scaffolding
(`.dev.vars`, a throwaway keypair, a patched `pda_table.owner`, a forced
`throw`) was reverted — `git diff` covers `commit.ts`, `index.ts`,
`scoreboard.ts` only.

**Still open here:** this bounds one address, not one attacker — a
distributed caller gets 3 per address it controls. Turnstile is the next rung
and composes with this rather than replacing it. And `MAX_DEMO_COMMITS` still
has no reset path: once 380 are spent the demo is over until the DO is
redeployed. Both are the user's call. **None of this is deployed** — it is a
`--dry-run` clean bundle in the working tree.

## The quote-digest claim — reworded 2026-09-01, not re-engineered

`quote_digest` is client-asserted and never server-verified:
`worker/index.ts` → `worker/commit.ts` regex-check its *shape* only. So the
memo binds the commitment to a quote the client *says* it verified.

Decision taken: align the claim with the behaviour rather than change the
architecture the day before submission. README §5 and the §1 "dashcam"
paragraph, and `docs/DFLOW_PARTNER_ASK.md` §1, now say which links hold
unconditionally (the PDA is immutable and chain-timestamped; DFlow's
signature over a held quote re-verifies against its published key) and which
one rests on the client (that the memo names the quote the engine actually
traded), with the reason: the engine *is* the client. "Three cryptographic
facts, not a claim" is gone from both.

The real fix — **carry DFlow's ed25519 signature in the memo**, not just the
digest, so a third party can verify the venue's price offline from the
receipt — closes it without introducing a backend. Not built: it grows the
transaction and wants devnet testing that the deadline does not leave room
for. This is the top post-submission item on this page.

## Open — decisions, not implementations

*(none)*

## Also worth doing

**G7, an account-substitution gate — built 2026-09-01.** See the section
below; this entry is kept only to record what it was before it was built.

**Verified sound during the same review** (recorded so it is not re-audited):
`afterswap-rail/src/canonical.rs` — length-prefixed throughout, no floats,
maps or `usize`, domain-tagged blake3, and `content_digest` correctly excludes
its own signature. No encoding ambiguity found. `rail/lib.rs:19` honestly
declares it does not verify provider signatures. `validate_and_sell` step 1b
already defended policy-account substitution, with a test at `vault.rs:675`.

## G7 — the account-substitution gate, closed 2026-09-01

`tests/substitution.rs`, **13 tests**, plus an `Attacker` harness in
`tests/common/mod.rs`. 42 → **55 green**, clippy clean, no production code
changed: this is a gate over behaviour that already existed, and it found no
new defect.

The design decision worth recording is what counts as a substitute. A
`Pubkey::new_unique()` in an account slot is rejected on length or ownership
before any gate logic runs, so a matrix built from junk accounts would be ~50
green tests proving nothing. `setup_attacker` instead builds a **second real
principal** in the same `LiteSVM` — same program, same `position_id`, same
mint, its policy/execution/vault committed through the same instructions the
victim used, holding the same token. Every field a handler reads is genuine;
only the seeds differ. That is the substitution that actually threatens the
gate, and it is the one the matrix now enumerates.

What each case pins, by the check that stops it:

- **sell / execution** — the execution PDA *is* the authorization; an attacker
  authorized on their own position is rejected by step 1b's seed derivation.
- **sell / policy+execution as a matched pair** — the sharper case: both
  substituted together so they agree and both seed checks pass. Only
  `vault_owner != exec_owner` stands, which is the sole link between the
  account holding the tokens and the authorization presented.
- **sell / vault** — mirror image, victim's authorization against the
  attacker's vault. Same owner cross-check.
- **sell / src_ata**, **close / src_ata** — the one slot no handler validates.
  The vault PDA signs, so safety here rests entirely on SPL Token's authority
  check rather than on the gate. Asserted with two substitutes: an ordinary
  ATA, and a *different vault PDA's* ATA. Both rejected — but by the token
  program, which is worth knowing rather than assuming.
- **sell / mint** — read only for `decimals`, which is what `TransferChecked`
  is checked against; a second mint is rejected by the token program.
- **deposit / vault** — seeds derive from the signer, so crediting another
  owner's vault cannot type-check.
- **deposit / source_ata** — pulling from an ATA the signer does not control.
- **close / vault** — right shape, wrong owner.
- **commit / policy** — both arms: over the victim's *existing* PDA (must fail
  on seeds, not merely on `AccountAlreadyInitialized`) and under an
  *uncommitted* one, where seeds are the only thing that can stop it.
- **authorize / execution** — both arms, and this one corrected a wrong
  expectation rather than the program: an initialized PDA is caught by the
  stored-owner comparison, which runs *before* the seed derivation; only the
  uninitialized arm reaches `InvalidSeeds`. The test asserts the victim's
  crank and settlement bytes are unchanged, since that arm is one write away
  from rebinding the whole gate.
- **revoke / execution** — revoking someone else's authorization is a DoS on
  their exit; the stored-owner check fires, and the victim's crank still works
  afterwards.

Assertions are on the *specific* error, not `is_err()` — `assert_rejected`
exists because `is_err()` also passes when a transaction dies in the runtime
before reaching the handler, which is exactly the false green this kind of
test invites. Where the SPL Token program is the thing rejecting, the
assertion is `is_err()` plus unchanged balances on both sides, because the
error is a `Custom(n)` that is not ours to pin.

Slots already covered in `vault.rs` / `anchor.rs` are listed in the file
header against their test rather than duplicated.

**Not closed by this:** the matrix covers cross-*principal* substitution.
Substituting a sysvar, a program slot with a look-alike, or reordering the
account list is not enumerated. `sell / src_ata` and `close / src_ata` remain
unvalidated in the program itself — currently harmless because the vault PDA's
signature is what bounds them, but a handler-level check would make that
independent of SPL Token's behaviour.

## Static-linter pass (`program_autofixer`) — run 2026-09-01

Deferred across three prior sessions; run now against the whole of
`crates/afterswap-policy/src/lib.rs` (1,249 lines, framework detected as
`pinocchio`, matching the crate).

**Result: one medium finding, no critical, no high, no syntax errors, and
`require_another_tool_call_after_fixing: false` on the first pass** — the
linter did not consider the program to need another round.

The single finding was `unchecked-arithmetic` ("Unchecked integer mul") on
the tranche-bound check, `lib.rs:947`:

```rust
if (amount as u128) * 10_000 > (tranche_bps as u128) * (vault_deposited as u128) {
```

**Verified a false positive** (linter hint 2, "type cannot overflow the
domain"). Every operand is widened to `u128` *before* the multiply:

| side | bound | max |
|---|---|---|
| lhs | `(u64 as u128) * 10_000` | 1.845e23 |
| rhs | `(u16 as u128) * (u64 as u128)`, and `commit_policy` rejects `tranche_bps > 10_000` | 1.845e23 |

`u128::MAX` is 3.4e38 — about fifteen orders of magnitude of headroom. A
`checked_mul` here would be dead code. The widening is itself the fix for a
*real* u64 overflow that this expression used to have (`amount * 10_000`
wraps above ~1.8e15 base units, which a 9-decimal mint reaches at 1.8M
tokens); the adjacent comment records that.

`rg -n 'as u128' crates/afterswap-policy/src/lib.rs` returns exactly this one
line, so the finding is fully accounted for.

**Caveat on method, recorded honestly.** The tool takes source as an inline
string, not a path, so the analyzed copy was transcribed rather than read
from disk. Two things bound the risk: a transcription slip would almost
certainly have surfaced as a syntax error (none were reported), and the one
finding's location was confirmed against disk with `rg`. It is still a copy,
not the file. If the tool ever grows a path parameter, this pass is worth
re-running for free.

**No production code was changed by this pass**, so the SBF artifact and the
55 green tests remain valid against current source.

## Deploy — done 2026-08-31, version `cb047d28`

The per-visitor cap only protected prod once this ran. It has now run.

**The `10021` / `deploy.sh` blocker did not exist in this repo.** That
constraint is recorded in the global agent rules and describes a different
project: there is no `deploy.sh` here (`fd -H -t f 'deploy'` finds only
`.plans/003_post_deploy_doc_edits.md`), so there was no PUT fallback to fix.
Plain `npx wrangler deploy` accepts the `SCOREBOARD` Durable Object binding
without complaint, and always has — the deploy history shows successful
`wrangler deploy` runs on 08-28 and 08-29 with that binding already in place.
No config change was needed or made.

**The D1 backup rule also did not apply**: `rg 'd1_databases|database_id'`
across all three `wrangler.jsonc` files returns nothing. This worker has no
D1. Its only state is the SQLite-backed Durable Object, which `wrangler` does
not export.

Pre-flight checks, all clean:

| check | result |
|---|---|
| `wrangler deploy --dry-run` | binding accepted, 520.94 KiB / 178.64 KiB gzip |
| `wrangler secret list` | `DEMO_KEYPAIR` present — a secret, not bundled, so it survives the deploy |
| `ip_quota` schema | `CREATE TABLE IF NOT EXISTS` in the DO constructor, so it self-creates; no new migration tag needed |
| DO storage | persists across code deploys; the 380-slot budget does not reset |
| rollback target | `87d73bba-cfba-47da-b589-b51fd6e0aad0` (the 08-29T17:57 version) |

Post-deploy verification:

- live version `cb047d28-f267-4c12-904d-537b56badbf0` at 100%
- `/api/score` byte-identical before and after (878 cycles/floor, `mean_bps`
  matching to full float precision) — **DO state survived the deploy**
- `/` → 200, `GET /api/commit-policy` → 405, malformed blockhash → 400
  (rejected before any slot is taken)

**The cap firing is NOT verified in prod — a deliberate call, 2026-08-31.**
The checks above prove the new code is live and the budget is intact; none of
them exercises `MAX_PER_IP` itself. The cap is only observable by actually
taking slots, and `refund_ip_quota` means any request engineered to fail is
also refunded — so there is no free path to a positive test. A positive test
costs **3 of the 380 slots**, permanently. (It costs no devnet transactions
and no SOL: the worker only *signs* and returns `signed_tx` to the browser,
which submits it, so discarding the response leaves the chain untouched. The
slot counter still advances.)

**Decision: skip the live test, ship on code review** — conserving scarce
demo slots the day before submission beat spending 0.8% of the budget on a
verification that a read of the code supports. Recorded so nobody later reads
"deployed" as "exercised in prod".

The review that decision rests on (`worker/scoreboard.ts`):

- `/slot` requires `ip` matching `^[0-9a-f]{16}$` and 400s without it. This
  is the bypass that mattered: an omitted `ip` used to mean an uncapped
  request. `visitor_key` emits 8 bytes of SHA-256 as hex, so the format
  agrees on both sides.
- `take_ip_quota` runs *before* the global-budget check and is refunded if
  the global budget refuses, so a visitor is never charged for a slot the
  global cap denied.
- `refund_ip_quota` floors at `MAX(n - 1, 0)`, so a duplicate refund cannot
  mint quota.
- `/slot/release` refunds only for the top-of-stack slot (`used === want + 1`),
  which is the same LIFO guard that protects the global counter.
- The `DELETE FROM ip_quota WHERE window_start < ?` prune bounds the table to
  addresses seen in the last hour, so a wide address pool cannot turn the cap
  into unbounded DO storage.
- Durable Objects are single-threaded per instance, so the read-then-write in
  `take_ip_quota` has no race.

Two limits this control has by construction, neither a defect:

1. **It bounds one address, not one attacker.** A distributed caller still
   gets `MAX_PER_IP` per address it controls. Turnstile is the next rung.
2. **Fixed window, not sliding** — a visitor can get 2 × `MAX_PER_IP` across a
   window boundary. Documented in the code; costs at most a handful of 380.

## Relevance to the undeployed-Phase-B decision

ROADMAP says vault-vs-SPL-delegate must be settled before devnet deploy,
since deploying makes the vault design the one demoed. This is evidence for
that decision: the vault path needed four fixes to make its own docstring
true, and three of them (destination binding, token-program pin, custody of
the signing authority) exist because the vault holds the funds and signs for
them. A delegate on the owner's own ATA does not need them in the same form.
