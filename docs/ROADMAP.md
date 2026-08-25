# Roadmap — good ideas, deliberately not built yet

Everything here was considered during the buildathon and parked on purpose.
Each item carries its evidence and why it waits. Ordering = value ÷ effort.

## 1. Input alphabet v2 — "distance from peak" ✅ SHIPPED (v2.0, bench 005)

**Outcome:** implemented via input unrolling (two binary steps per tick:
direction, then off-peak ≥30 bps) — zero upstream changes needed. Result:
trailing-stop gap closed (−24.5 → **+2.0 bps**), TWAP floor +60.0 → +87.4,
TP-ladder +95.4 → +122.8, bracket +53.3 → +80.7. G1–G6 re-validated.
Remaining from the original plan (still parked): magnitude quantization
(big/small moves) as a third bit.

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

## 2. Closed-form latents (the honest "latent-first")

Derived signals that need **no training** — temporal-derivative surprise
(dual fast/slow EMA, katgpt-rs Plan-style), volatility ratio, time-in-
position — quantized into input symbols or used as extra re-tournament
triggers. Keeps every property that matters: full enumeration,
auditability, bit-determinism. Learned embeddings are explicitly out —
they would break the product's core honesty claim.

## 3. ELO / Plackett–Luce arm ratings ❌ TRIED & REVERTED (bench 006)

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
**Phase B — delegated execution:** user `approve`s an SPL delegate to the
program PDA once (bounded amount); a permissionless crank triggers sells
that the program validates against the committed policy. Kills the
per-tranche wallet popup without custody. Phase B is real security
surface — audit before mainnet.
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

## 7. Machine marketplace / copy-trading

Machines are ~16-byte genomes with stable fingerprints and public track
records (realized bps, pulls). Publish → others' positions can hire your
machine → royalty per fill. Requires #4 Phase A for verifiable
provenance. Social loop: "Eager Puffin has exited 4,120 positions at
+31 bps mean."

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
