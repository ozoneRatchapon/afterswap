# Roadmap — good ideas, deliberately not built yet

Everything here was considered during the buildathon and parked on purpose.
Each item carries its evidence and why it waits. Ordering = value ÷ effort.

> **Status, 2026-08-27.** Not clear. Twelve items are closed with evidence
> (shipped, measured, or refuted); six remain open. Two of those six — #7 and
> #7b, both monetisations of decision quality — are now **contradicted by our
> own benches** and are marked below rather than silently left standing.
> `benches/035_asset_vs_machine` measures the tournament's out-of-sample
> selection differential at 1 of 11 assets above its own detection floor, which
> is the thing both items propose to sell. See "What the evidence did to this
> roadmap" at the end.

## 1. Input alphabet v2 — "distance from peak" ✅ SHIPPED (v2.0, bench `005_goat`)

**Outcome:** implemented via input unrolling (two binary steps per tick:
direction, then off-peak ≥30 bps) — zero upstream changes needed. Result:
trailing-stop gap closed (−24.5 → **+2.0 bps**), TWAP floor +60.0 → +87.4,
TP-ladder +95.4 → +122.8, bracket +53.3 → +80.7. G1–G6 re-validated.
(Those are the bench-`005_goat` endpoint values, recorded here as the
result of *this* change. The shipped default now measures TWAP **+75.73**,
trailing **+10.26**, ladder **+113.74**, bracket **+79.93** on the
6-corpus mean — bench `039_goat` — and loses on the 2 recorded DFlow
corpora alone; see §3a and Retraction 1.)
Magnitude quantization as a third bit: ❌ TRIED & REVERTED (bench `008_goat`)
— at 5 bps threshold every floor degraded and G2b FAILED (−7.3: the bit
sits in the noise band and burns 3-state capacity); at 10 bps G2b
improved (+5.7) but every other floor still degraded vs the 2-bit
baseline. The 2-bit protocol (direction + off-peak) remains optimal at
n=3 states. Revisit only together with a state-budget increase (n=4
enumeration or evolution-heavy pools) — more alphabet needs more states
to spend it on.

### (original plan, for the record)

**Evidence:** Bench 004 — the engine beats TP-ladders (+95.4 bps) and
TP/SL brackets (+53.3 bps) but loses −24.5 bps to Jupiter-style trailing
stops, entirely in up-trends. Trailing stops read *drawdown from the
running peak* — information the binary up/down input cannot express.

**Plan:** extend the FSM input alphabet from 2 symbols to 4–6:
`{big-up, small-up, small-down, big-down}` (volatility-quantized) plus a
`peak-drop` symbol (price ≥ X bps below running peak). Exhaustively
enumerate 2-state machines over the richer alphabet (the space stays
enumerable), and let the existing evolution loop explore 3–4 states.
A machine that reacts to `peak-drop` *contains* the trailing stop as a
special case — plus every hybrid Jupiter doesn't offer.

**Blocker:** `katgpt-ruliology`'s FSM table is binary-input
(`[[u8; 2]]`, third-party crate). Needs either an upstream PR or a local
multi-symbol FSM type with the same fingerprint/dedup discipline.
**Gate:** re-run ecosystem floors; target = parity or better vs trailing
on trend-up while keeping the other wins.

## 2. Closed-form latents (the honest "latent-first") — ✅ both halves resolved

**Surprise trigger ✅ SHIPPED (v2.5, bench 012):** dual fast/slow EMA of
signed returns, vol-normalized; a spike forces a full re-tournament and
overrides the gate's "skip" (fresh regime evidence beats "dynamics
unchanged"). All external floors improved (TWAP +71.9→+72.3, trailing
+6.5→+7.0); the vs-random gap narrowed because the random floor shares
the smarter cadence — absolute performance is what shipped.
(Input-bit half: off-peak ✅ v2.0; magnitude ❌ reverted, bench 008.)

Derived signals that need **no training** — temporal-derivative surprise
(dual fast/slow EMA, katgpt-rs Plan-style), volatility ratio, time-in-
position — quantized into input symbols or used as extra re-tournament
triggers. Keeps every property that matters: full enumeration,
auditability, bit-determinism. Learned embeddings are explicitly out —
they would break the product's core honesty claim.

## 3a. Off-policy credit assignment ✅ SHIPPED (v2.6, bench `013_goat`)

The sample-efficiency problem the PL experiment was aiming at, solved a
different way: only the seated arm used to be rewarded per window (1 of
24). Now every non-seated arm is replayed on the same realized window and
credited with its counterfactual edge — the replay cost was already being
paid by the tournament. Every floor improved: TWAP +72.3→**+76.0**,
random-arm +1.8→**+5.4**, trailing +7.0→**+10.5**, ladder +110.3→+114.0,
bracket +76.5→+80.2. **These endpoint values were measured with the
surprise trigger ON** (the default at the time); Retraction 1 below turned it
off, and the shipped default now measures TWAP **+75.73** / random-arm
**+6.90** (bench `039_goat`). The *deltas* stand — the absolute figures do
not describe what ships. Side effect worth noting: with all arms credited
every window, UCB1's exploration bonus equalizes and selection tends
toward follow-the-leader — appropriate, since this is now a
full-information setting rather than a bandit one.

## 3b. ELO / Plackett–Luce arm ratings ❌ TRIED & REVERTED (bench `006_goat`)

**Outcome:** implemented (`rating.rs`, Hunter-MM PL, unit-tested,
deterministic) and wired into survivor ranking — **every floor got worse**
(G2a +87.4→+84.3, G2b +3.8→+0.5, trailing +2.0→−1.1). Mechanism: PL
rewards rank *consistency*, but the objective is bps *magnitude* — big
wins in trend windows matter more than consistently edging flat windows.
Reverted to mean-payoff ranking; the module stays available for contexts
where consistency is the objective (e.g. future marketplace reputation).
A negative result, measured and kept.

### (original rationale, for the record)

**Evidence:** G2b margin is thin (+5.0 bps vs random) because most arms
get 0–2 pulls — raw mean reward is a weak ranker at low sample counts.
Tournament win-matrix → PL/ELO ratings ranks arms by *pairwise wins*,
much more sample-efficient. (katgpt-rs Issue 686 proposes primitives;
nothing implemented upstream yet — checked 2026-08-26.)

## 4. On-chain exit-policy program (PDA) + delegated execution

**Phase A — policy registry:** PDA `["policy", wallet, position_id]`
storing `{machine blake3 fingerprint, n_states, tranche_frac, size,
opened_at}`. Committed before the first live fill → every DFlow order is
auditable against a pre-committed policy. Turns "provably uninformed
flow" from a pitch line into a verifiable on-chain fact.
**Phase B — delegated execution.** Original plan: user `approve`s an SPL
delegate to the program PDA once (bounded amount); a permissionless crank
triggers sells that the program validates against the committed policy.
Kills the per-tranche wallet popup without custody. Real security surface —
audit before mainnet.

**Phase B, revised (2026-08-27): most of it already exists, MIT, and on the
same framework we chose.** MagicBlock published a leverage template
(`magicblock-labs/leveraged-prediction`) whose supporting primitives map
onto our missing pieces almost one-to-one:

| Our gap | Their primitive | Verified from |
|---|---|---|
| "who presses the button without holding keys" | **`hydra`** — permissionless crank: scheduled instructions live in a crank PDA, *anyone* may trigger when due; **Pinocchio `no_std`**, same framework as our policy program | repo README |
| non-custodial balances the program can move | **`ephemeral-spl-token`** — ephemeral ATAs + per-mint global vault, deposit/withdraw, delegation to data-layer programs (MIMD 0013) | repo README |
| no wallet popup per tranche | **session keys** (Lever template) | project announcement |
| affordable per-tick on-chain evaluation | `ephemeral-rollups-sdk` + `magicblock-validator` (see Phase C) | repo list |
| prices inside the rollup | `real-time-pricing-oracle` | repo list |

Sketch: user deposits into an ephemeral vault (withdrawable, non-custodial)
→ our policy PDA commits the machine fingerprint → Hydra schedules the
evaluation instruction → any cranker triggers it → the program executes only
sells the committed policy authorises. No popup per tranche, no bot holding
keys, no custody — the exact UX failure observed in live testing
(a wallet prompt on every tranche, plus Phantom's new-domain heuristics
blocking the request) dissolves.

Status: steps 1 and 2 built and tested (2026-08-29); `AnchorFill`, devnet
deploy and audit remain. The table records what those repos state they do.
Integration is post-buildathon work and Phase B still needs an audit before
mainnet.
**Phase A status: ✅ DEPLOYED ON DEVNET (v2.4).** Rewritten in Pinocchio
(Anza) after evaluating Quasar (beta, unaudited — parked) and Anchor
(Phase B candidate): binary 74 KB → **18 KB**, autofixer 0 issues, same
LiteSVM tests byte-for-byte. Live artifacts:
- Program: `GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8`
  (https://explorer.solana.com/address/GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8?cluster=devnet)
- First committed policy PDA `5LRDFS9WckZUA1BNoBmt6N3A6r2Pzie3TcULADSKEXiA`
  (fingerprint 0x165ef4aabbcc, 3 states, 10% tranches — decoded and
  verified on-chain), tx `2WHpDfMD3K5DNMheEdHKGm8djxKZPGeRLiYHyMmrVkzQKoykjz9iAQUBFEPquk8F3fdSRwow4BeDVqKgXqp4RLA5`
- Mainnet deploy ≈ 0.13 SOL rent at this size — post-audit.

**Phase B step 1 status: ✅ BUILT AND TESTED (2026-08-29).**
`AuthorizeExecution` (tag 1) and `RevokeAuthorization` (tag 3) implemented
in `crates/afterswap-policy/src/lib.rs`, 7 new LiteSVM tests in
`tests/execution.rs` (authorize sets crank + expiry, double-authorize
rejected, non-owner / wrong-PDA / non-signer / past-expiry all rejected,
revoke clears crank, revoke idempotent). Binary 18 KB → **23.8 KB** (still
< 60 KB rent budget). `CommitPolicy` (tag 0) unchanged; existing tests in
`tests/policy.rs` still pass.

**Phase B step 2 status: ✅ BUILT AND TESTED (2026-08-29).**
`ValidateAndSell` (tag 2) — the gate, and the only instruction that moves
tokens — plus the vault path it needs: `DepositToVault` (tag 4) and
`CloseVault` (tag 5). 16 further LiteSVM tests in `tests/vault.rs`, so the
crate now runs **26 tests, all green** (`policy.rs` 2, `execution.rs` 7,
`vault.rs` 17). Binary **35,736 bytes (34.9 KB)**, rent-exempt minimum **0.2496 SOL** — both inside the < 60 KB / < 0.4 SOL budget.

Two caveats, stated because they change what may be claimed. The design is
**vault-sourced**, so the program *does* take custody between deposit and
sell — `CloseVault` is the owner-only exit, but "no custody" is not
available as a claim for this design. And an earlier draft of the design doc
justified the vault over an SPL delegate by asserting that a delegate cannot
transfer to an arbitrary destination; **that is false**, and the
delegate-on-owner-ATA alternative is still open. See
`PHASE_B_DELEGATED_EXECUTION.md` §1 and §8. Next: `AnchorFill` (tag 6), then
devnet deploy — but settle vault-vs-delegate first.

**Phase C — real-time on-chain execution (MagicBlock ephemeral rollups):**
the endgame of the trust ladder. Delegate the position/policy PDA into an
ephemeral rollup session (1ms blocks, zero fees, <50ms e2e) and run the
FSM itself as on-chain state — every input bit and transition verifiable,
settled back to L1 with fraud proofs. Uniquely feasible for us: an FSM
step is a 16-byte table lookup, likely the cheapest "strategy on-chain"
workload that exists. Their TEE (Intel TDX) additionally enables selling
machine decisions without revealing the genome — the privacy primitive
the marketplace (#7) needs. Trust ladder: Memo commitment (shipped) →
PDA-enforced policy (Phase A/B) → machine-runs-on-chain (this).

**Cheap precursor:** ✅ SHIPPED (v2.1) — before a position's first live
fill, the dashboard publishes a Memo tx: `afterswap:policy fp=<blake3-64>
machine="<name>" gen=<n> states=<n> tranche=10%`, signed by the user's
wallet. The exit policy is now committed on-chain before any sale follows
it — commitment only; program-enforced verification remains Phase A/B.

## 5. Shared world — Durable Object mode

Today each tab is a private universe (localStorage persists per-browser).
A Durable Object per market = one persistent population learning from
everyone, plus a DO per user position for the multi-tenant product. Also
retires the "two tabs show different charts" question. The WASM port
makes this cheap: the same engine runs in a DO unchanged.

## 6. Prediction-market outcome tokens ("after the bet")

DFlow has Kalshi rails. Outcome tokens force an exit decision by
construction (terminal resolution + time decay) — a *more* natural home
for exit machines than spot. Yield framing: deposit outcome tokens,
machines ladder out as probability drifts, edge measured vs
hold-to-resolution.

## 6b. Perps exits + maker-side exits (Phoenix / CLOB venues)

Phoenix (Ellipsis Labs) pivoted to perpetuals — crankless atomic
settlement, ~0.035% fees, sub-1 bps impact to $8M, $500M OI (Aug 2026).
Two extensions it motivates:
- **Perps exit machines:** leveraged positions add funding rates and a
  liquidation line — both quantize into closed-form input bits (same
  pattern that closed the trailing-stop gap in bench 005: "funding
  expensive" bit, "near liquidation" bit). Bigger pain, bigger
  willingness-to-pay than spot exits.
- **Maker-side exits (output-alphabet v2):** on an orderbook the sell
  action can be {market-sell, rest-a-limit-above} — capture spread
  instead of paying it. Output expansion stays enumerable, mirroring the
  input expansion already shipped.
- **Partnership note:** Ellipsis' MM-protection ethos = DFlow's
  conditional-liquidity thesis; our on-chain-committed policies are
  *provably benign flow* — one story, two teams.

## 7. Machine marketplace / copy-trading ⚠️ PREMISE CONTRADICTED (bench 035)

**Blocked on evidence, not effort.** The product is a machine's public track
record, hired by others for a royalty. `benches/035_asset_vs_machine` measures
what a machine's selection actually contributes out of sample — Δ over the
population median, drift removed — and finds it exceeds its own detection floor
on **1 of 11 assets**. `benches/025_multiplicity` reached the same conclusion by
a different route: zero machines survive Romano-Wolf correction.

Publishing "Eager Puffin has exited 4,120 positions at +31 bps mean" would be
publishing a number the project's own statistics say is indistinguishable from
the population median. Do not build this until Δ is shown non-zero on a corpus
that could detect it.


Machines are ~16-byte genomes with stable fingerprints and public track
records (realized bps, pulls). Publish → others' positions can hire your
machine → royalty per fill. Requires #4 Phase A for verifiable
provenance. Social loop: "Eager Puffin has exited 4,120 positions at
+31 bps mean."

## 7b. Monetization rail ⚠️ RE-SCOPED → Verifiable Execution Rail (R0–R3 built)

**2026-08-28:** the re-scope below is now implemented through R3 — see
`docs/RAIL.md` (spec + status + deployment runbook) and the rail plan in
`.plans/` (plan 002). Locally verified end to end; deployment,
the production attestation key, and the first funded anchor are owner
actions listed in RAIL.md §7.

### (original re-scope note)

**The rail is sound; the thing sold over it is not.** Everything below about
pay.sh, 402 and the deployment remains accurate. What has changed is the pitch:
selling a *decision* prices the machine's edge, and bench 035 cannot show that
edge is non-zero on ten of eleven assets. Round three states the same
conclusion from the cost side — "positioning an execution engine primarily as
an alpha-generating trading system is commercially fragile".

The surviving version sells **verifiability, not alpha**: the signed quote →
on-chain policy commitment → verified fill chain (#7h, shipped) as a
best-execution compliance artifact **aligned with MiCA Article 78**. Per the
claims discipline in `docs/RAIL.md` §0, "aligned with" is the ceiling: whether
an artifact *is* compliant is a determination for a regulator and counsel, not
for a codebase. That product does not depend on the machine being good — only
on the record being provable, which it is.


**Preview ✅ built (v2.2), deployed:** `POST /decide` runs
the same wasm binary server-side, roster + full simulated exit, verified
deterministic under `wrangler dev`.

**Correction (2026-08-28):** an earlier revision of this entry claimed the
plan upgrade "has happened." It has not — the account is on the Workers
**Free** plan, confirmed by the API rejecting a `limits` block (error
100328). The earlier "1 of 3 calls" figure also understated the problem,
and the failures were misattributed to CPU-exceeded 1102 responses when
most were in fact 1101 — uncaught Rust panics.

**How it used to fail.** Measured over 40 consecutive requests: **20 ok, 20
failed** (a second 40-call run the same day gave 25 ok / 15 failed — the rate
had no single stable value). The free-plan CPU ceiling is **2,010 ms**; the
cold 1,054-machine enumeration cost **1.0–2.0 s** under wasm, so cold starts
were killed. Enumeration was process-cached (`enumerate_cached`), so warm
calls cost **1 ms**. A killed request left the wasm instance trapped, and the
instance is cached per isolate, so every later call there aborted until
the isolate was recycled — the failures clustered in runs.

Fixed on that path: `enumerate_cached` aborted on a poisoned mutex
(`.expect`) instead of recovering into it, and the worker leaked its
`WasmEngine` handle every request. Both `/decide` modes now degrade to a
clean 503 rather than a raw 1101 crash page. Success went ~40% → ~50%.

**Ceiling removed at the source (2026-08-28, re-measured in prod below).**
The precompute option above is done. `FsmEnumerator::enumerate(3)` spends
its time behaviourally fingerprinting 5,832 raw machines over 2^11 input
sequences to find the 1,054 distinct ones; since it is pure and
deterministic, the *survivors* are computed once at development time and
shipped. Only their identity is new information, so the table stores the
raw enumeration indices that survived, in order — 2 bytes each, 2,108
bytes total (`src/fsm_table_3.bin`, regenerated by
`examples/gen_fsm_table.rs`) — and the engine replays the raw index
arithmetic to rebuild them.

- native: **224.9 ms → 132.5 µs** (1,698x)
- cold `/decide` in local `workerd`, same harness before and after:
  **752 ms / 730 ms → 7 ms** (~280x under the 2,010 ms ceiling)
- wasm: 473 KB → **476 KB** (155 KB gzipped); the indices do not compress
- `tests/fsm_table.rs` asserts the decoded set is field-for-field
  identical to a live enumeration — transitions, outputs, residual state,
  blake3 id and complexity bits — so the table is a cache, not a second
  source of truth. Live enumeration remains the fallback for any state
  count no table covers.

Re-measured on the hosted endpoint after deploy, 2026-08-28, same 40-call
procedure (`scripts/decide_measure.sh`), run twice: **80 ok, 0 failed**, p50
76/69 ms, p95 134/125 ms. It was run twice on purpose — the pre-fix build did
not have one stable rate (two 40-call runs the same day gave **20 ok / 20
failed** and **25 ok / 15 failed**), so a single clean post-fix run would not
have been enough to distinguish a fix from a lucky draw. The comparison is
against that pre-fix range, not a single number.

Remaining for revenue: pay.sh registry onboarding, 402 challenge.

pay.sh (Solana Foundation) lets AI agents pay per API call with no
accounts — 74-provider registry, MCP tools, Solana-wallet funding. The
fit: trading agents are proliferating and share the human weakness —
good at entries, undisciplined at exits. Expose `POST /decide`
(price window + position → machine decision + policy fingerprint) priced
per call over 402. Everything an agent-facing decision API needs, we
already have: determinism (reproducible), µs latency, on-chain-auditable
policies, and the GOAT report as the sales page. The browser demo stays
free (marketing); agents pay for the hosted endpoint + evolved-machine
leaderboards + corpora. This is the most concrete revenue path on this
list — pairs with #5 (the DO host becomes the paid endpoint).

## 7c. Paired online evaluation ✅ SHIPPED (v2.6)

The soak measured absolute edge vs hold — per-cycle SD ≈ 6.6 bps against
a sub-1 bps effect, so 534 cycles still gave t = 0.37 (see SOAK.md).
Fix: `afterswap-server/src/shadow.rs` drives TWAP, trailing stop,
TP-ladder and TP/SL bracket from the *same* entry on the *same* ticks, so
each cycle yields paired differences where the price path cancels — the
online analogue of what the GOAT bench does offline. Run with
`--paired <file>`; the soak monitor reports per-floor means with
t-values hourly. This is the instrument that makes every future
improvement measurable in hours instead of weeks.

## 7d. Horizon sweep + null control ✅ RUN (v2.7, bench 014)

Tested the standing hypothesis that live edges look tiny only because the
demo exit horizon is ~1 minute. Method: block-bootstrap bars from the
1,224 recorded DFlow ticks at aggregation factors 1–60 (bar ≈ 2 s → 2 min),
40 seeds per scale, means reported ±SE.

- **Null control (de-meaned, ≈ random walk): no significant edge at any
  horizon** — every cell within ~1–2 SE of zero. The engine does not
  manufacture alpha from noise. This is the overfitting check.
- **Structured (drift preserved): advantage over other exit strategies is
  real and scales with horizon** — +469 ± 87 bps vs TWAP and +496 ± 86 vs
  trailing at ~80-minute horizons (>5 SE). Versus *holding* it loses in a
  compounding bull sample, consistent with G2c.
- Hypothesis verdict: confirmed for effect *magnitude*, but the sign
  depends on market structure. Demo-scale measurement sits in the least
  favorable corner.
- Caveat kept in the report: bootstrapping preserves only within-block
  autocorrelation, so it understates real longer-range structure.

**Follow-up ✅ RUN (v2.8, bench 015_params) — negative result, no change
shipped.** Grid-searched window × tranche per horizon with an honest
TRAIN/TEST seed split. On bootstrapped paths window 96 won everywhere with
huge out-of-sample gains (+1917 bps at 2-min bars, >7 SE). On the **real**
corpora it reversed: trend_down +38 → −103, v_shape +54 → −249. Shipping
that tuning would have degraded the product.

The lesson outranks the parameter: *a correct train/test split does not
protect you when the data-generating process is wrong.* Block-bootstrapped
paths have no structure above the block scale, so long windows are optimal
there for the wrong reason. Out-of-**distribution** validation (real
corpora) caught it. Demo parameters stand unchanged.

**Real blocking dependency:** tuning for genuinely long horizons needs
genuinely long *recorded* data. A 6-hour recorder at 4 s intervals is
running to produce `data/recorded_long.jsonl`; the sweep re-runs on it.

## 7e. Per-regime arm statistics ⚠️ BUILT, UNRESOLVED, DEFAULT OFF (v2.9)

Machines' records are pooled across market regimes, so a downtrend
specialist has its average diluted by rallies. Implemented regime-keyed
statistics (closed-form chop / trend-up / trend-down label from the
surprise EMAs) with shrinkage back to pooled when a bucket is thin.

**Result: the bench cannot tell the difference — ON and OFF score
bit-identically (bench 016_goat).** Regime keys only affect re-tournament
*seeding*, and a 300-tick bench run rebuilds the arm set too few times for
that to matter. The feature would bite across long sessions with
persistence, which needs a long-soak A/B to settle. Default OFF: ship the
simpler behavior until an instrument exists that can resolve it.

**Process finding worth more than the feature.** The first three attempts
at this A/B (the discarded runs) all showed a large regression that had
nothing to do with the feature: a background recorder was appending a new
corpus into `data/`, which the bench scans, so the comparison surface
changed underneath the experiment. Corpus sets must be frozen during an
A/B; in-progress recordings now live in `data/incoming/`. The tell was
that disabling the feature did *not* restore the previous numbers — a
control that only exists if you run it.

## 7f. Real long-horizon data ✅ OBTAINED — and it contradicts our headline

The blocking dependency from bench 015 (genuinely long *recorded* data, not
resampled) is closed: 45,000 real 1-minute SOL/USDC bars, 31 days, public
CEX reference prices. Idea distilled from
[trickshot](https://github.com/nathanliow/trickshot) — reconstruct real
history and replay strategies on it — but sourced from a free public
endpoint rather than a paid indexer.

**The result does not flatter us** (bench 017_real_horizon). At 1-minute
bars with 225 non-overlapping windows — far more statistical power than
the 6 GOAT corpora — the engine **trails TWAP by 4 ± 1 bps** (≈4 SE) and is
neutral-to-negative against the other floors. Point estimates turn positive
at 30–60 minute bars (+33 ± 15 vs TWAP) but only 3–7 windows exist there.

**Open question, stated plainly:** the GOAT corpora say we beat every
standard exit; 225 windows of real market data say we do not, at least at
minute resolution. Candidate explanations, none yet tested: (a) the six
corpora are short and half of them are synthetic regimes we designed, so
they may flatter the engine; (b) the engine runs on second-scale DFlow
ticks and minute bars are a different regime it was never tuned for;
(c) TWAP is genuinely hard to beat in a month that trended down 30%.

**Next experiment ✅ RUN (bench 018_train_test) — first significant
positive result in the project.** Chronological train/test on four real
1-minute series (SOL, BONK, WIF, PEPE): tune on the first 60%, score on the
last 40%.
- vs **trailing stops on memecoins**: BONK **+34 ± 10**, PEPE **+26 ± 11**
  (3.4 and 2.4 SE) — significant, out-of-sample, on real bars.
- vs **TWAP**: nothing anywhere (+0 ± 3 SOL, +7 ± 8 BONK, −8 ± 9 PEPE).
- Tuning transferred adequately this time (real data, chronological split),
  unlike the bootstrap tuning that reversed on real corpora.

**Retracted the same day (bench 018 extended to 11 assets).** Adding seven
more real series collapses it: across-asset means are **+5.3 ± 5.7 vs TWAP**
and **+10.5 ± 7.9 vs trailing** — noise. BONK and PEPE really did clear
significance, but two-of-four is what selection looks like when you stop at
four. The lesson repeats a third time: *every increase in statistical power
shrank the apparent edge.*

**Direction this actually sets:** stop hunting alpha with this alphabet at
these horizons — the null control already said the engine will not invent it,
and now the real-data tests agree there is little to find. The defensible
product is disciplined, auditable, zero-cost exit automation with honest
benchmarking attached, and the open research direction is *execution* (DFlow
depth, Plan 001) rather than direction prediction.

## 7g. Execution-cost model ✅ SHIPPED, ❌ not a lever (bench 019_cost)

`EngineConfig.fill_cost_bps` charges a per-fill cost on live fills and inside
the tournament's own replays; the floors gained matching cost-aware variants
(`twap_value_norm_cost`, `trailing_stop_value_norm_cost`) so comparisons stay
paired. Default 0 so historical benchmarks remain comparable.

Measured across 11 real assets at 0/1/2/5 bps per fill: **every comparison
moves less than half a bp.** The expected asymmetry (a tranching exit pays ten
times, a single-shot stop once) does not appear, because the engine often does
not complete ten tranches in a window and the trailing stop often never fires.
One more candidate explanation for the synthetic-vs-real gap, eliminated.

## 7h. Verifiable exit chain ✅ SHIPPED (v4.2)

Three cryptographic links, all live:

1. **Signed quotes, verified client-side.** `x-sign-request: true` returns
   RFC 9421 headers; the browser rebuilds the signature base, checks the
   SHA-256 body digest and verifies ed25519 against DFlow's published key
   (`EZKxYr7bb…`). Every quote, not a sample — measured at 0.055 ms, so the
   sampling that an earlier version did bought 0.006% of a core in exchange
   for the machines acting on unchecked prices. A quote that fails
   verification is discarded.
2. **Policy committed before the first sale** — the Pinocchio program on
   devnet, unchanged.
3. **Binding** — the commitment transaction carries
   `afterswap:quote sha-256=<digest>` in a memo instruction beside the
   commit, so the chain records *which signed quote* the policy was committed
   against. Hand-rolled two-instruction transaction; no program change.

What is still trust-me: the fill itself on mainnet carries no memo yet
(needs the production API), and the demo's devnet commitments are signed by
our throwaway key rather than the visitor's wallet.

## 7i. Constant audit ✅ RUN — two retractions (bench 021_sensitivity)

Prompted by the question *"what else did you assert without measuring?"* —
after a sampling rate for signature verification had already been caught that
way and was wrong. Swept every intuition-chosen constant one at a time over
the six corpora:

| constant | spread | verdict |
|---|---|---|
| `window_len` | **34 bps** | genuinely load-bearing (48 collapses to +10) |
| `tranche_frac` | 3 bps | barely a knob |
| `refresh_every_windows`, `max_arms` | 2 bps each | not knobs |
| `peak_drop_bps` | 1 bps | the off-peak *bit* mattered (bench 005); its *threshold* does not — robustness, not a problem |
| `surprise_ratio` | **0 bps** | **retracted** |

**Retraction 1 — the surprise trigger.** Bench 012 reported that it "improved
every floor". A direct A/B now says otherwise: OFF gives TWAP +75.73 /
random +6.90 / real-vs-trailing −21.05, ON gives +76.00 / +5.40 / −20.24 —
under 1.5 bps, in both directions. The original claim came from one
measurement with no control (bench `001_goat`, superseded by
`039_goat`). **Now off by default**, flag retained.

**Retraction 2 — the learning loop was mostly not running.** The same audit
measured live cycle length: median ~15 ticks against a 24-tick evaluation
window, so most positions closed before a window ever completed. After an hour
of live trading, 19 of 24 machines had never been tried and the seated machine
was chosen by exploration order rather than by realized performance. Fixed by
extending off-policy credit to the position-close path: one completed cycle now
credits every machine. Verified live — 24 of 24 arms carry pulls.

## 7j. Enumeration frontier ✅ MEASURED (bench 028_states)

An architectural choice that had been taken on faith — why three states? —
finally has a number. Complete enumeration at each state count, same objective,
same corpora:

| states | machines | tournament setup | objective |
|---|---|---|---|
| 2 | 26 | 487 µs | +40.4 bps |
| **3** | **1,054** | **202 ms** | **+43.3 bps** |
| 4 | 57,068 | **8,253 s** | +41.0 bps |

Going to four states costs **40,000× the setup time and scores worse**. The
per-corpus figures barely move, so the extra 56,014 machines are not producing
new behaviour, only more ways to select a non-generalising winner — the same
mechanism CSCV measures. Conclusion, now evidence-backed: enumerate
exhaustively to three states, reach beyond by evolution, whose cost is flat in
the size of the space. That is what the engine already did.

## 8. Ops / trust hardening

- Custom domain (workers.dev triggers Phantom's new-domain heuristics);
  email review@phantom.com (template in SUBMISSION.md).
- DFlow production API key (pond.dflow.net) + rate-limit handling.
- Corpus library: record more regimes (rally, crash, weekend chop) for
  fatter GOAT coverage; long-horizon window-64 tuning.

## Explicitly rejected (with reasons)

- **Devnet mode** — DFlow aggregates mainnet venues; devnet has no
  liquidity to quote. Would disconnect the product from DFlow.
- **Learned/ML exit models** — breaks enumeration-completeness,
  auditability, and determinism; the product's differentiation *is* the
  absence of a black box.
- **Custody vault without the policy program** — hot-wallet mode exists
  for demos (`--features live --keypair`) but is custody software the
  moment real users appear; #4 is the correct path.


## What the evidence did to this roadmap

Late benches changed the standing of items that were written before them. Kept
here rather than edited into the items above, so the ordering stays legible.

| item | written assuming | measured since | status |
| --- | --- | --- | --- |
| #7 marketplace | a machine's track record is worth hiring | Δ detectable on 1 of 11 assets (bench 035); zero survive multiplicity (bench 025) | premise contradicted |
| #7b decisions API | decision quality is the sellable good | same, plus round three on commercial fragility | re-scoped to verifiability |
| #7e per-regime stats | regime split might be a lever | still unresolved, default off | unchanged |
| #7j enumeration frontier | more states might help | 4 states costs 40,000x and scores worse (bench 028) | closed |
| — | 1,054 machines is a broad search | effective count 1.2 (bench 032) | description corrected in README |

Two items are blocked on data rather than effort, and no amount of building
moves them:

- **CUPED / sub-bps execution measurement** needs the depth-aware recorder that
  stopped when Plan 001 closed — for the control variate *and* for the paired
  execution outcome. Price-only data yields 1.9% variance reduction against a
  prescribed 30–50% (bench 033).
- **Deciding what makes a series generalise** needs assets with stronger
  autocorrelation, or many more of them. At |ρ₁| ≤ 0.30 and n = 11, our test
  had 4.8% power (bench 037) — it could not have detected the effect if it were
  there.

The honest summary is that the statistical work is well ahead of the product
work, and it has spent the last several benches removing claims rather than
adding them. That is the pipeline functioning as designed, and it is also why
the roadmap is not clear.

## Reconsideration — 2026-08-28

Re-ranked after the submission-kit review, which turned up two things the
ordering above hides. Written as an appended record, not an edit, so the
earlier reasoning stays readable next to what replaced it.

**Correction to "blocked on data rather than effort."** The CUPED / sub-bps
bullet is filed under data-blocked, and that is the wrong label. The thing it
waits on is the depth-aware recorder — a *build* that stopped when Plan 001
closed, not a dataset the market refuses to hand us. It is effort-blocked and
was mislabelled. The second bullet (generalisation across assets) is genuinely
data-blocked: no amount of building produces 11 assets with stronger
autocorrelation.

**The trust ladder has a missing rung.** §4 lays it out as Memo commitment →
PDA-enforced policy → machine-on-chain. What is deployed is the first rung
plus a registry: `afterswap-policy` exposes exactly one instruction,
`CommitPolicy`. Nothing validates a sell against the committed policy, and
there is no delegate authority — so today the chain can prove a violation
after the fact and cannot prevent one. That gap, not the marketplace and not
the frontier, is what the next build should close.

**Retention gap, closed as documentation (2026-08-28).** `RAIL.md` §3.3 and §6
described five-year retention as an R2 bucket policy; no bucket is bound
(§7 step 3, skipped per the free-tier invariant), and the trim `DELETE` in
`sequencer.rs` is gated behind a successful R2 put — so nothing is deleted and
durability is the Durable Object's SQLite. Both passages now say that. The
archive path is written and unexercised; executing step 3 is what makes the
R2 claim true rather than designed.

### Worth continuing, in order

1. **§4 Phase B — delegated execution.** Closes the missing rung and fixes the
   UX failure observed live (a wallet prompt per tranche). MagicBlock's
   `hydra` and `ephemeral-spl-token` are MIT and already Pinocchio, the same
   framework as our program. Untested by us; needs an audit before mainnet.
2. **Mainnet for the program and the anchors.** Devnet history is periodically
   reset (`RAIL.md` §8). An audit trail whose anchors can vanish is not
   durable evidence, which is the only claim that survived the benches.
3. **The depth-aware recorder.** Unblocks CUPED and the paired execution
   outcome — the item above says "blocked on data", and it is not.
4. **Retention**: either execute §7 step 3 or leave the claim scoped to DO
   SQLite as it now is. Both are honest; only one is an archive.

### Dropped

- **§7 marketplace** and **§7j frontier** — premise contradicted (bench 035)
  and closed (bench 028) respectively.
- **§5 shared world**, **§6 outcome tokens**, **§6b perps/maker exits** — all
  three assume finding or selling edge. Benches 025 and 035 removed the edge;
  what survived is verifiability, and none of these sell that.
- **§4 Phase C** — the endgame of a ladder whose second rung is not built.
  Reconsider once Phase B exists.
