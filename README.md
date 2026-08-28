# AfterSwap

> **Exhaustively enumerated exit machines, fighting over your position — live on DFlow.**
>
> DFlow × Superteam Thailand Buildathon 2026 — *"build what happens after the swap."*
>
> **Live demo (no install, no wallet): https://afterswap.solana-thailand.workers.dev**
> — the entire engine runs in your browser as WASM; quotes come straight
> from DFlow. Switch the pair to **BONK** to watch it in the market where the
> out-of-sample evidence says exit discipline pays. Add `?replay` for the
> recorded deterministic segment.

You swapped into SOL. Now what? Every wallet goes silent at exactly the moment
that decides whether you make money: **the exit**. AfterSwap picks up where the
swap ends — it watches live DFlow quotes and lets a population of tiny machines
compete for the right to scale you out.

![dashboard](docs/dashboard.png)

## What makes this different, in five facts

1. **Nobody designed these strategies.** We enumerate *every* deterministic
   3-state exit machine that can exist — 1,054 after behavioral dedup — then
   evolution breeds 4-state machines the enumeration cannot reach. No model,
   no training, no prompt: a decision costs **$0 and ~1.2 microseconds**.
   Read 1,054 as coverage, not as diversity: their returns are correlated
   tightly enough that the effective count is **1.2**
   ([bench 032](benches/032_policy_degeneracy/report.md)). The enumeration is
   exhaustive over the alphabet; the alphabet says close to one thing.
   Three states is a measured frontier, not a guess: completing the 4-state
   space takes **2.3 hours per tournament instead of 202 ms and scores worse**
   ([`028_states`](benches/028_states/report.md)).
2. **No durable edge over standard exits — established by our own harness,
   the hard way.** Tuned on the first 60% of each series and scored on the
   last 40%, never overlapping, across **11 real assets**
   ([`018_train_test`](benches/018_train_test/report.md)):
   - Across assets: **+5.3 ± 5.7 bps vs TWAP**, **+10.5 ± 7.9 vs trailing
     stops** — both inside the noise.
   - Two assets *did* clear significance (BONK +34 ± 10, PEPE +26 ± 11 vs
     trailing). With four assets tested that looked like a finding; with
     eleven it looks like selection. We report the aggregate, not the two.
   - Our synthetic regimes flatter the engine badly (+174 vs TP-ladder) while
     recorded DFlow data reads **−7.9**; the benchmark now splits real from
     synthetic automatically so that gap cannot hide.

   Every time we increased statistical power, the apparent edge shrank. That
   is the result, and it is stated here rather than buried. A formal
   overfitting test ([`024_overfit`](benches/024_overfit/report.md), CSCV/PBO,
   calibrated against synthetic noise) then separated two things everything
   else had conflated: **the selection is sound — PBO 0.05–0.20 on 7 of 11
   assets, with the in-sample winner ranking 0.70–0.88 out of sample — while
   the profitability is absent**, in-sample +2.5…+16.8 bps collapsing to
   −6…+2 out of sample. We can reliably pick the best machine; the best
   machine is not profitable. That points away from our search and our
   statistics, and at the strategy space itself. Decomposing that collapse
   ([`026_diagnosis`](benches/026_diagnosis/report.md)) shows why, and
   incidentally refutes the tidy explanation we were offered: with **zero**
   simulated friction the selected machine still scores −6.35 bps out of
   sample, so there is no gross edge for fees to have eaten. What the split
   does reveal is that "edge versus hold" is a badly-conditioned metric — its
   variance is dominated by realised drift, identical for all 1,054 machines —
   while the part actually attributable to the search is a consistent but tiny
   **+2.59 bps over the population median**. The last test in the
   recommended pipeline closes it: Romano–Wolf stepdown across all 1,054
   machines on all 11 assets returns **zero survivors** after familywise
   correction ([`025_multiplicity`](benches/025_multiplicity/report.md)),
   with the minimum detectable effect stated beside every null so the
   absence is informative rather than merely empty. Execution cost is
   not the explanation either — charging 0→5 bps per fill to every strategy
   alike moves the comparisons by less than half a bp
   ([`019_cost`](benches/019_cost/report.md)).
3. **It passes a null control.** On de-meaned (random-walk) paths it shows
   **no** significant edge at any horizon — it does not manufacture alpha from
   noise. Most strategy searches never publish this test; ours is in the repo.
4. **The exit policy is committed on-chain before any sale follows it** — an
   18 KB Pinocchio program [live on
   devnet](https://explorer.solana.com/address/GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8?cluster=devnet)
   writes the machine's blake3 fingerprint to an immutable PDA (**3,285
   compute units** per commit), so every DFlow fill can be audited against a
   pre-committed policy. **Open the demo and it happens to you**: the machine
   that takes your position commits its policy to devnet automatically, no
   wallet required — the signing key lives in the Worker, your browser only
   relays the transaction.
5. **The whole chain is verifiable, and the last link closes on-chain.**
   DFlow signs its API responses (RFC 9421, ed25519); **every** quote is
   checked in your own tab — body digest *and* signature against DFlow's
   published key — and a quote that fails verification is discarded rather
   than traded on (measured cost: 0.055 ms, 0.006% of a core). The machine's
   policy is then committed to Solana **bound to that exact signed quote**:
   the commitment transaction carries `afterswap:quote sha-256=…` in a memo
   beside it. So "this fill followed a policy committed in advance, at a price
   the venue really offered" is three cryptographic facts, not a claim.
6. **There is no backend.** The whole engine compiles to a 476 KB WASM binary
   that runs in the visitor's tab and polls DFlow directly — byte-identical to
   the native build (gate G6), self-custodial, free to run, impossible to
   rug-pull.

**So what is the product, then?** Not alpha — we looked hard for it with a
harness good enough to catch ourselves, and it is not there at these horizons.
What is left is worth having anyway: an exit that runs **without you**, follows
a policy **committed on-chain before it sells**, costs **nothing per decision**,
and reports its result against doing nothing and against every standard
alternative — honestly, including when that is a loss. Most retail exits are
not benchmarked against anything at all; that is the bar we actually clear.

**We also changed the objective, on external advice, and it still says no.**
Directional edge being dead at these horizons, we rebuilt around Almgren–Chriss
arrival-price implementation shortfall and asked a different question: can the
machines make execution more *predictable*, if not cheaper? Against TWAP the
answer looked like yes — 30% lower shortfall variance, significant on 8 of 11
assets, and it survived adding a price-impact model. Then the second control
ran: the machines simply liquidate four times sooner, and a plain TWAP
compressed to the same urgency matches them
([`027_shortfall`](benches/027_shortfall/report.md), SD ratio 1.12,
significant on 0 of 11). We had moved along the efficient frontier and briefly
mistaken it for beating it.

**The pipeline is now tested end to end, and it says no.** Reproducible (G1),
browser-native byte-identical (G6), selection generalises rather than mines
noise (PBO 0.05–0.20), and no individual machine survives correcting for having
looked at a thousand candidates (0 survivors, α = 0.05). Each stage was built
because the previous one could not answer the question — and the honest end
state is a negative result with its power stated, which is worth more than the
positive one we could have shipped by stopping earlier.

**And we audit our own defaults, not just our results.** Asked "what else did
you assert without measuring?", we swept every constant we had chosen by
intuition ([`021_sensitivity`](benches/021_sensitivity/report.md)). Two of them
turned out not to be knobs at all, and one shipped feature — a
surprise-triggered re-tournament we had claimed "improved every floor" — moves
the floors by under 1.5 bps in both directions once a control is run. It is now
off by default with the retraction recorded. A defect the same audit exposed:
the median live position closed in ~15 ticks against a 24-tick learning window,
so 19 of 24 machines had never been tried after an hour of live trading. Fixed
by crediting every machine at position close — 24 of 24 now learn from every
cycle.

**So what survives?** The machinery, and it is the part that is hard: a
complete enumeration that provably contains trailing-stop behaviour, an
overfitting null control it passes, byte-identical determinism from browser to
native, an auditable on-chain policy, and decisions that cost nothing and take
a microsecond. What has *not* been demonstrated is a durable price edge on real
SOL/USDC action at these horizons — a machine that beats holding on a month of
real data is not something we can claim today, and we would rather say so than
show you only the synthetic regimes.

And the part most projects leave out: **we publish what did not work.**
Plackett–Luce ratings, a third input bit, horizon-scaled parameters and
per-regime statistics were all built, measured, and reverted; an over-claimed
"learning curve" was retracted when more data killed it. Every number here
survived a gate that was allowed to say no.

## Where to read what

| File | What it holds |
|---|---|
| [`docs/PITCH.md`](docs/PITCH.md) | The same product explained five ways — for traders, judges, the DFlow team, engineers, and social — plus Q&A armor |
| [`docs/SOAK.md`](docs/SOAK.md) | Live-quote soak results, including a retraction of an earlier over-claim |
| [`docs/QUESTIONS.md`](docs/QUESTIONS.md) | The agent's own open questions after its harness killed four of its claims — what it still does not know about not fooling itself |
| [`docs/OPPORTUNITIES.md`](docs/OPPORTUNITIES.md) | The whole DFlow API surface mapped against what we use, the katgpt-rs primitives worth pulling now that alpha is off the table, and the research method this repo converged on |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Everything deliberately *not* built yet, each with its evidence — and the ideas that were tried and reverted |
| [`docs/DEMO.md`](docs/DEMO.md) | The two-minute demo script, beat by beat |
| [`docs/API.md`](docs/API.md) | Running the engine from your own agent, free, no API key |
| [`benches/`](benches/) | Every measurement, numbered and dated: GOAT gates, ecosystem floors, horizon sweep, null control, parameter sweep |
| [`crates/afterswap-policy/`](crates/afterswap-policy/) | The on-chain policy registry (Pinocchio, live on devnet) |

## The idea (Wolfram ruliology, applied to trading)

Instead of hand-designing one exit heuristic (trailing stop, TWAP, take-profit
ladder), AfterSwap **enumerates the entire space of simple exit strategies** and
lets data pick the winner:

1. **Enumerate** — every deterministic finite-state machine up to N states,
   reading two bits per tick (price up/down, then off-peak: ≥30 bps below the
   running high) and emitting sell-a-tranche / hold. 3 states → **1,054
   behaviorally distinct machines** (blake3-fingerprint dedup). No training,
   no gradients — the strategy space is *complete* by construction, and it
   provably contains trailing-stop behavior as a special case.
2. **Tournament** — every machine replays recent live price windows; a Pareto
   filter (edge vs complexity) plus a top-K cap keeps ~24 survivors.
3. **Bandit** — survivors become **UCB1 arms**. When you open a position, the
   bandit picks a machine to drive; every window its realized reward
   (tranche-exit value vs what naive holding would have done) updates its arm.
   Underperformers lose the seat.
4. **Credit everyone** — at each window boundary the seated machine is
   scored on what it actually did, and every other machine is replayed on
   that same realized window and credited with its counterfactual edge.
   ~24× more learning signal per unit time, reusing the tournament's own
   replay machinery (bench 013: every floor improved).
5. **Evolve** — every live window, mutants of the current arms (output
   flips, edge reroutes, and **4-state growth past the enumerable
   frontier** — `katgpt-ruliology`'s co-evolution operators) challenge the
   worst arm on replayed windows; keep-if-better, Wolfram-style. Evolved
   machines wear a ✦gen badge on the leaderboard.
6. **Verify** — a renoise check (perturb the window → re-rank → measure
   drift) scores how stable the live machine's selection is under noise;
   the dashboard shows it as a per-decision confidence badge.
7. **Gate** — a spectral irreducibility test on the win-matrix decides whether
   the next tournament can be **skipped, light, or full** — Wolfram's
   computational-irreducibility argument, used as a scheduler.

The FSM enumeration/mutation primitives come from
[`katgpt-ruliology`](https://github.com/katopz/katgpt-rs) by
[@katopz](https://github.com/katopz) (MIT, third-party — credit where due).
Everything trading-shaped — the exit engine, tournament economics, bandit
rewards, evolution loop, DFlow integration, server, dashboard, and GOAT
proofs — was built for this buildathon on top of real market data.

## How DFlow integrates

DFlow is not a data feed bolted on the side — it is both the **sensor** and the
**actuator**:

- **Sensor**: the engine's only input is the implied price of DFlow
  [`GET /quote`](https://pond.dflow.net) (SOL→USDC, 0.1 SOL probe) polled every
  tick. The FSMs literally *are* functions of DFlow's aggregated liquidity —
  Tessera-routed quotes, not an oracle.
- **Actuator**: every `sell tranche` output maps to a DFlow swap. In **paper
  mode** (default) fills are simulated at the quoted price; in **live mode**
  the same event requests [`GET /order`](https://pond.dflow.net), signs the
  returned transaction with a local keypair, and submits it — declarative,
  sandwich-resistant execution for exactly the flow (small, repeated,
  uninformed tranches) DFlow's conditional liquidity is designed to price well.

```
DFlow /quote ──► tick ──► FSM population ──► UCB1 bandit ──► sell-tranche
     ▲                                                            │
     └────────────── DFlow /order (live mode) ◄───────────────────┘
```

## Run it

**Zero-install:** open the [live demo](https://afterswap.solana-thailand.workers.dev)
— engine compiled to WASM (476 KB, 155 KB gzipped), no server anywhere, your browser polls
DFlow directly (their dev API allows CORS). Falls back to the bundled
recording automatically if DFlow is unreachable.

**Native:**

```bash
cargo run -p afterswap-server -- --serve 8787 \
  --interval-ms 1000 --window 12 --states 3 --tranche 0.1
```

Open http://localhost:8787, press **Open position** (paper — no wallet needed).
You'll watch machines get selected, sell tranches at live DFlow prices, and
accumulate realized edge vs holding on the leaderboard.

Deterministic demo (replay recorded DFlow quotes, looping):

```bash
cargo run -p afterswap-server -- --serve 8787 --interval-ms 1000   --window 12 --states 3 --tranche 0.1 --replay data/recorded.jsonl
```

Add `--record <file>` to any live run to capture your own segment.

Terminal-only e2e (no dashboard):

```bash
cargo run -p afterswap-server -- --ticks 75 --interval-ms 1000 \
  --open-after 15 --window 12 --states 2
```

## GOAT-gated

The engine passes the GOAT gate discipline inherited from katgpt-rs — no
performance claim without a named floor
(full report: [`benches/039_goat/report.md`](benches/039_goat/report.md), re-run 2026-08-28):

| Gate | Result |
|---|---|
| **G1 determinism** | PASS — bit-identical event stream on every corpus, two runs |
| **G2a floor: TWAP** | PASS — **+75.7 bps mean** vs same-cadence TWAP exit across 6 corpora (4 synthetic regimes + 2 recorded DFlow segments) |
| **G2b floor: random arm** | PASS — **+6.9 bps mean** vs seeded random arm selection (8 seeds) |
| **G2c vs hold** (report-only) | **+372.2 bps** in trend-down, +14.9 chop, −137.7 v-shape, +0.0 trend-up — an exit product wins when exiting matters and pays opportunity cost in a rally, as it should |
| **G3 arm-cap ablation** | PASS — 24-arm cap costs at worst **−0.6 bps** vs the uncapped front (chop; 0.0 on four of six corpora), against a −10 bps budget |
| **G4 latency** (release) | PASS — **686 ns** mean `on_tick`; worst tick (1,054-FSM enumeration + tournament) **111 µs**. Budgets 1 ms / 1 s |
| **G5 evolution ablation** | PASS — evolution on ≥ off within tolerance across corpora |
| **Ecosystem floors** (report) | 6-corpus mean: **+113.7 bps vs TP-ladder**, **+79.9 bps vs TP/SL bracket**, **+10.3 bps vs Jupiter-style trailing stop**. **That mean is an upper bound, not a result** — it is carried by the 4 synthetic regimes; on the **2 recorded DFlow corpora alone the engine loses** (trailing **−21.1**, ladder **−7.9**, bracket **−8.9**), though one of the two is a win (+29.3 vs trailing). Bench 004 measured a −24.5 bps loss to trailing stops (machines couldn't see "distance from peak"); adding the off-peak input bit (alphabet v2, roadmap #1) closed it (bench 005, confirmed 007) — trailing-stop behavior now *emerges from enumeration*, plus hybrids |
| **G6 wasm parity** | PASS — the browser (WASM) engine produces **byte-identical** `simulate()` output to the native binary (`scripts/g6_parity.sh`). Caught a real bug: `rng.usize` is platform-width-dependent — now fixed-width everywhere |

Reproduce: `cargo test -p afterswap-engine --test goat` (gates) and
`cargo run -p afterswap-engine --example goat_bench --release` (report).

## Agent API (preview)

The same engine, callable by AI agents — deterministic, stateless:

```bash
curl -X POST https://afterswap.solana-thailand.workers.dev/decide \
  -H 'content-type: application/json' \
  -d '{"prices": [/* ≥30 ticks */], "open_at": 30}'
```

Returns the tournament roster (names, blake3 fingerprints, simulated
edges) or, with `open_at`, a full simulated exit with fills and the
honest edge vs holding. Same input → byte-identical output (G1/G6).

**Status: measured 2026-08-28 over 40 consecutive requests — 40 returned a
real roster, 0 failed (p50 76 ms, p95 134 ms).** The free-plan CPU
ceiling that used to kill cold starts — a 1,694 ms p95 against a 2,010 ms
limit — is no longer reached: the 1,054-machine enumeration is precomputed and
shipped as a 2,108-byte table, so a cold call costs ~7 ms instead of 752 ms.
The local WASM path (`docs/API.md`) remains the route for anything that must
answer without a network hop.

**How it used to fail.** The cause was measured, not guessed. This account is
on the Workers **Free** plan, whose CPU ceiling is **2,010 ms** (a `limits`
block is rejected outright, API error 100328). A cold isolate had to enumerate
the 1,054 machines, which cost **1.0–2.0 s of CPU under wasm** — right at the
ceiling, so cold starts were killed mid-enumeration. Enumeration was
process-cached, so a warm call cost **1 ms**; the endpoint was either nearly
free or dead, with little in between.

A killed request was worse than a slow one: it left the wasm instance
trapped, and because the instance is cached per isolate, every later call
there aborted until Cloudflare recycled it — which is why the failures
arrived in runs rather than spread evenly, and why the pre-fix success rate
had no single stable value (two 40-call runs the same day gave 20/40 and
25/40). Two bugs found along that path
are fixed (a poisoned-mutex abort in `enumerate_cached`, and a leaked
`WasmEngine` handle), and both `/decide` modes now return a clean
`503 {"error":"engine unavailable, retry shortly"}` instead of a raw
Cloudflare 1101 crash page. That improved the success rate from ~40% to
~50%, but it did not touch the CPU ceiling.

**The ceiling itself is addressed at the source; the numbers above are the
post-deploy measurement.** Enumeration is pure and deterministic, so
its result is precomputed and shipped: the 1,054 survivors are stored as
the raw indices that survived behavioural dedup (2,108 bytes,
`crates/afterswap-engine/src/fsm_table_3.bin`), and the engine rebuilds the
identical machines from them. Natively that is **224.9 ms → 132.5 µs**; a
cold `/decide` in local `workerd`, same harness before and after, went from
**752 ms / 730 ms → 7 ms**, which is ~280x under the free-plan ceiling.
`tests/fsm_table.rs` gates the table field-for-field against a live
`FsmEnumerator::enumerate`, so it stays a cache and never becomes a second
source of truth.
Pay-per-decision via pay.sh HTTP-402 is the roadmap (7b).

## Architecture

| Crate | What |
|---|---|
| `afterswap-engine` | FSM enumeration, window tournament, Pareto+cap pruning, UCB1 bandit, spectral simulation gate, tranche executor. Pure — no I/O. |
| `afterswap-dflow` | DFlow Trading API client (`/quote`, `/order`), price poller. Types verified against live captures. |
| `afterswap-server` | Paper loop + axum server, SSE snapshot stream, vanilla-JS/SVG dashboard. |
| `afterswap-wasm` | Browser build of the engine (wasm-bindgen) — powers the serverless live demo on Cloudflare Workers static assets. |
| `afterswap-policy` | On-chain exit-policy registry (Pinocchio, 18 KB) — **live on devnet**: [`GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8`](https://explorer.solana.com/address/GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8?cluster=devnet), autofixer-clean, LiteSVM-tested against the real SBF binary. |

Every window the position is open emits an honest score:
`reward = tranche-exit value ÷ counterfactual hold value` (in bps). The
dashboard's hero number is that same measure over the whole position — if the
machines can't beat doing nothing, the number is red and we say so.

## Live soak

Beyond backtests: a continuous live-quote soak with auto-reopened paper
positions, 535 cycles. Result reported honestly — **no statistically
significant live edge** (mean +0.10 bps, t = +0.36), a thin
positive tilt in chop, a measurable weak spot in a fast rally, and an
observable regime adaptation: [`docs/SOAK.md`](docs/SOAK.md).

## Null control

The engine passes the check that matters most for a strategy search: on
block-bootstrapped **de-meaned** price paths (≈ random walk), it shows **no
significant edge at any horizon** — it does not manufacture alpha from
noise. On the same paths with structure preserved, its advantage over other
exit strategies grows with horizon (+469 ± 87 bps vs TWAP at ~80 min).
Full sweep: [`benches/014_horizon/report.md`](benches/014_horizon/report.md).

## What we stopped doing, and why

Two research tracks were closed by evidence rather than abandoned. The
liquidity-depth hypothesis — the one genuinely DFlow-native signal we found,
27 bps between clip sizes on a long-tail token — is **−13.2 bps net** once the
pool fee (25), priority tip (10) and latency drift (5) are counted, so no
amount of better signal processing reaches it. The same cost table puts
liquid CLMM majors at +0.1 to +0.3 bps instead — positive, but an order of
magnitude below what our sample could detect, so it is not a track either.
And the non-directional
redirect closed itself the moment its benchmark was matched on urgency. Both
are written up with their arithmetic in [`docs/QUESTIONS.md`](docs/QUESTIONS.md)
and [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Roadmap

Parked ideas with their evidence — input-alphabet v2 (the trailing-stop
gap), closed-form latents, ELO ratings, on-chain policy program, shared
Durable-Object world, prediction-market outcome tokens:
[`docs/ROADMAP.md`](docs/ROADMAP.md).

## Status & limits

- Paper mode is the demo: **quotes are real, fills are simulated** at quote
  price (no slippage/fee model beyond DFlow's own quoted amounts).
- Live mode sells real tranches but is deliberately minimal (throwaway keypair,
  no retry logic) — it is a buildathon proof, not custody software.
- Built during the buildathon (Aug 21–31, 2026); not previously released.

MIT.
