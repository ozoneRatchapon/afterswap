# AfterSwap — consolidated research and architecture record

Single-file summary of how this project's claims evolved, why its commercial
framing changed, and how its data capture is designed. Every figure here is
reproduced from a bench report in `benches/` or a test in `crates/`.

## 0. What the system is

An exit-timing engine for Solana spot positions. It enumerates every
deterministic 3-state finite-state exit machine — 1,054 after behavioural
dedup — replays each over rolling 120-tick windows of 1-minute price data, and
selects a winner by tournament. No model, no training. A decision costs ~1.2 µs.

Corpus: 11 assets (BONK, FLOKI, JTO, JUP, ORCA, PEPE, PYTH, RAY, SHIB,
SOL/USDC, WIF), 166–375 windows each. Measured dispersion: paired difference
σ_d = 2.6 bps, unpaired per-cycle σ_u = 6.6 bps. A prior 534-cycle live soak
returned t = 0.37.

**Sample size governs everything below.** At σ_d = 2.6, detecting a 0.25 bps
paired effect at 80% power needs 849 cycles; 0.10 bps needs 5,306. Unpaired
costs ~25× more. The project's central discipline is reporting the minimum
detectable effect (MDE) beside every null, because "we found nothing" is only
informative next to "we could only have found ≥ X".

---

## 1. Strategic pivot: from alpha to verifiable execution

### 1.1 What was retired

**Roadmap #7 — Machine marketplace / copy-trading. Premise contradicted.**
The product was a machine's public track record, hired by others for a royalty
("Eager Puffin has exited 4,120 positions at +31 bps mean"). Two independent
measurements say that number is indistinguishable from the population median:

- Bench 025: **zero machines survive Romano–Wolf familywise correction on any
  of the 11 assets.** Best adjusted p-values: SHIB 0.122, JTO 0.399, FLOKI
  0.818, everything else ≥ 0.895, seven at 1.000.
- Bench 035: the selection differential Δ exceeds its own detection floor on
  **1 of 11 assets**.

**Roadmap #7b — Exit-decisions-as-an-API (HTTP 402 / pay.sh). Re-scoped, not
killed.** The rail is sound and deployed. What changed is the good being sold:
pricing a *decision* prices the machine's edge, which bench 035 cannot show is
non-zero. External research reached the same conclusion from the cost side —
"positioning an execution engine primarily as an alpha-generating trading
system is commercially fragile".

### 1.2 The economics that forced it

Net realisable margin identity:

```
Margin_net = Spread_gross − (Fee_pool + Tip_priority + Drift_latency + Fee_L1)
```

Worked on BONK (long-tail CPMM):

```
27.0 − (25.0 + 10.0 + 5.0 + 0.2) = −13.2 bps
27.0 − (30.0 + 15.0 + 8.0 + 0.5) = −26.5 bps   (pessimistic end)
```

| Component | CPMM long-tail (BONK) | CLMM liquid major (SOL/USDC) |
|---|---|---|
| Gross spread | +20.0 … +27.0 bps | +0.8 … +1.2 bps |
| Liquidity pool fee | −25.0 … −30.0 bps | −0.05 … −0.10 bps |
| Validator priority tip | −10.0 … −15.0 bps | −0.20 … −0.40 bps |
| Latency drift (50–400 ms) | −5.0 … −8.0 bps | −0.10 … −0.30 bps |
| L1 base fee | −0.2 … −0.5 bps | −0.01 … −0.05 bps |
| **Net** | **−13.2 … −26.5 bps** | **+0.10 … +0.35 bps** |

The pool fee alone (25 bps) consumes the entire gross spread on long-tail
routes before any tip or latency cost. This is not an execution-quality problem
to be engineered away; it is negative arithmetic. Liquid CLMM majors are
positive but sit an order of magnitude below the detection floor of any
small-sample live experiment.

**Consequence:** the depth-signal research track (Plan 001) was closed on
economics rather than left pending more data.

### 1.3 What replaced it

Sell **verifiability, not alpha**. The pipeline `signed pre-trade quote →
on-chain policy commitment (PDA hash) → verified settlement fill` is already
shipped. It does not depend on the machine being good — only on the record
being provable.

Regulatory pull, MiCA Article 78 (Crypto-Asset Service Providers):

- Best-execution obligation across price, cost, speed, likelihood of execution,
  settlement and size.
- Pre- and post-trade data published within **30 seconds** of execution
  (vs 60 s under MiFID II).
- Immutable audit trail retained **5 years**.
- Single-broker reliance prohibited — multi-venue price discovery must be
  demonstrated.

Competing aggregators (Jupiter Ultra v3, Titan, Carbium, DFlow) publish
*asserted* best execution: post-hoc telemetry and internal win-rate claims.
None provides cryptographic proof of pre-trade market state. That is the gap
the commitment chain fills, and it is a compliance artifact rather than a
performance claim.

---

## 2. Benchmark findings

### 2.1 Bench 025 — multiplicity correction: zero survivors

Romano–Wolf stepdown over all 1,054 machines against holding, 400 bootstrap
resamples, familywise α = 0.05.

| asset | best mean | best t | adjusted p | MDE |
|---|---|---|---|---|
| SHIB | +4.0 bps | 2.14 | 0.122 | 5.2 bps |
| JTO | +1.2 bps | 2.22 | 0.399 | 1.6 bps |
| BONK | +9.8 bps | 1.28 | 1.000 | 21.4 bps |
| SOL/USDC | +0.2 bps | 1.12 | 1.000 | 0.4 bps |

Total surviving familywise correction across every asset: **0**.

Romano–Wolf was chosen over the Deflated Sharpe Ratio deliberately. DSR and its
paired variant (DPM) rest on an extreme-value-theory threshold that assumes
independent or weakly dependent trials. Bench 032 measured the opposite: the
correlation eigenspectrum of the 1,054 machines is dominated by one component,
λ₁/Σλ = 0.87–0.91 on every asset, giving an **effective trial count N_eff ≈ 1.2
(N_eff/N ≈ 0.0012)**. EVT thresholds are invalid under that concentration;
Romano–Wolf resamples the joint dependence structure and never assumed
independence, so the zero-survivor result stands.

That measurement also corrected a description rather than a result:
"1,054 enumerated machines" reads as breadth. The enumeration is exhaustive
over the alphabet; the alphabet says close to one thing.

### 2.2 Bench 031 — PBO confidence intervals, and a self-inflicted error

Bench 024 reported per-asset Probability of Backtest Overfitting and read three
assets as dissenting (FLOKI 0.623, JTO 0.516, PYTH 0.448) against eight
generalising cleanly. Bench 031 put intervals on those point estimates.

**First attempt was wrong.** A stationary bootstrap resamples windows *with
replacement*, so duplicate rows enter the performance matrix and a duplicated
row can land in both the training and testing half of a CSCV split. That is
precisely the block-exchangeability violation CSCV forbids for overlapping
windows, reintroduced through the resampler. Symptom: PEPE's point estimate
(0.198) fell outside its own interval (0.000–0.127). Intervals came out
0.29–0.72 wide and separated nothing.

**Corrected design: block permutation.** Windows are cut into contiguous blocks
of 5 and the block order is shuffled — every window appears exactly once, so no
row can straddle a split, while within-block dependence is preserved. Widths
halved to 0.09–0.33 and the answer changed:

| asset | PBO | 95% interval | verdict |
|---|---|---|---|
| JTO | 0.516 | 0.417 – 0.647 | **separable** |
| FLOKI | 0.623 | 0.333 – 0.667 | borderline (overlaps by 0.036) |
| PYTH | 0.448 | 0.278 – 0.567 | not separable |
| low group (JUP/ORCA/RAY/SHIB) | 0.075–0.190 | envelope 0.036 – 0.369 | — |

**Three dissenters is an overcount: one dissents measurably, one is borderline,
one does not.** The wider interval was not the safer answer — it was the wrong
one.

Secondary finding from the permutation diagnostic: every asset with ≥250
windows has a permutation median well below its point estimate (PEPE 0.028 vs
0.198, BONK 0.131 vs 0.202). If block order were uninformative the median would
centre on the estimate. Part of measured PBO is produced by temporal *ordering*
rather than by overfitting — the signature of regime non-stationarity.

### 2.3 Bench 035 — what the tournament actually contributes

Decomposition (from external research, adopted here):

```
R_{i*,OOS} = D_t + Δ_{i*}
             │     └─ selection differential: what the tournament adds
             └─ common drift, shared by every machine, swings −52 … +28 bps
```

`D_t` is exogenous market movement and is not the tournament's to claim.
Measuring against it — "edge versus hold" — is an **ill-conditioned objective**
that conflates market beta with execution efficiency. Negative out-of-sample
performance persisted even in zero-friction simulation (−6.35 bps), which rules
out frictional dominance as the explanation.

Bench 035 therefore measures Δ only: machine picked on the first 60% of
windows, scored on the remaining 40%, edge taken over the population median so
drift cancels.

| asset | Δ (bps) | MDE (bps) | detectable |
|---|---|---|---|
| PEPE | +11.748 | 9.127 | **yes** |
| WIF | +21.035 | 28.632 | no |
| BONK | +16.041 | 25.163 | no |
| RAY | +11.211 | 63.695 | no |
| JTO | −15.931 | 44.113 | no |
| PYTH | −9.978 | 77.851 | no |

**Δ exceeds its own detection floor on 1 of 11 assets.** Mean Δ is +3.1 bps and
positive on 8 of 11 — consistent with a small real edge, and equally consistent
with nothing. The MDE column is what distinguishes "we measured a small effect"
from "we could not have measured this effect if it were there", and here it is
the second. This is bench 025's conclusion restated in bps rather than in
significance.

### 2.4 Bench 036 — a hypothesis built, then killed by controlled experiment

Observational benches suggested the machines were extracting **mean reversion**:
across 11 assets, lag-1 autocorrelation ρ₁ correlated with in-sample
signal-to-noise at −0.856, and with out-of-sample Δ at −0.513 (p ≈ 0.11).

Tested by generating the data instead of observing it: AR(1) log returns with φ
set directly, innovation variance rescaled by √(1−φ²) so unconditional
volatility is constant across arms — otherwise two things vary at once. 20 seeds
per arm, real pipeline downstream.

| φ | realised ρ₁ | Δ (bps) | PBO | PBO std err |
|---|---|---|---|---|
| −0.4 | −0.398 | +0.518 | 0.359 | ±0.039 |
| −0.2 | −0.198 | −0.180 | 0.552 | ±0.055 |
| 0.0 | +0.003 | −0.365 | 0.564 | ±0.064 |
| +0.2 | +0.202 | +1.590 | 0.489 | ±0.054 |
| +0.4 | +0.402 | +1.980 | 0.356 | ±0.039 |

**Δ does not fall with φ.** It drifts slightly upward, non-monotonically, with
per-arm seed spreads of ±10 bps against arm means under 2 bps. The
mean-reversion reading is **withdrawn**; benches 034 and 035 carry retraction
banners.

**What does respond is PBO, and to |φ| rather than signed φ.** The response is a
hump: 0.359 at φ = −0.4, 0.564 at φ = 0, 0.356 at φ = +0.4. The two extremes
are indistinguishable from each other and sit 2.7 standard errors below the
centre. Selection generalises when serial structure is present in *either*
direction and degenerates toward a coin flip when it is absent — the martingale
signal-to-noise mechanism. It had been recorded as contradicted because the
earlier test used the signed quantity.

### 2.5 Bench 037 — the null had 4.8% power

The real corpus returns `corr(|ρ₁|, PBO) = −0.034`. Flat. Two readings: the
mechanism does not transfer, or 11 assets spanning |ρ₁| ≤ 0.30 could never have
resolved it.

Swept φ across the band the corpus actually occupies, 50 seeds per arm:

| φ | PBO | std err |
|---|---|---|
| 0.00 | 0.509 | ±0.030 |
| 0.15 | 0.491 | ±0.029 |
| 0.30 | 0.438 | ±0.027 |

Arm means correlate at **−0.918** — the mechanism is present even in the
realistic band, but shallow, against a per-arm PBO standard deviation of 0.21.

Power estimated by drawing 20,000 pseudo-corpora of 11 assets at the *measured*
|ρ₁| values, each asset's PBO sampled from the arm nearest its autocorrelation.
Observed correlation has median −0.112 and a 95% range of −0.656 … +0.467.
**4.8% reach two-tailed significance at n = 11 (|r| ≥ 0.602)** — the
false-positive rate.

**A corpus generated by the mechanism itself would usually have looked flat
too.** The −0.034 result is not evidence against anything. Settling this is a
corpus-acquisition question, not an analysis one.

### 2.6 Bench 038 — 34.6% variance reduction from depth beside quote

CUPED compresses outcome variance by `1 − ρ²(Y, X)` for a pre-experiment
control variate X:

```
Y_CUPED = Y − θ(X − E[X]),   θ = Cov(Y, X) / Var(X)
```

An earlier bench measured 1.9% headroom and concluded that was CUPED's ceiling
because the reference corpus is `{t, price}`. **That was wrong about the
repository** — the Plan 001 depth recorder had stopped, but its output was kept:
1,207 paired price/depth observations for BONK.

For an execution outcome driven by pool depth at fill time, the reachable
reduction is bounded by how well a depth reading taken *k* observations earlier
predicts depth at the fill:

| lag k | ρ(depth_t, depth_{t+k}) | CUPED reduction |
|---|---|---|
| 1 | +0.588 | **34.6%** |
| 2 | +0.511 | 26.1% |
| 5 | +0.439 | 19.3% |
| 10 | +0.299 | 8.9% |
| 30 | +0.114 | 1.3% |

Inside the 30–50% band external research prescribes.

**Price cannot substitute for depth.** Prior realised volatility explains 1.6%
of depth variation on the same series. That is why the earlier bench got 1.9% —
it was a fact about substituting price for depth, not about CUPED. It also means
*simulating* depth from prices can never reach the band: a deterministic
function of price carries no information price does not.

**The binding constraint is freshness, not volume.** The reduction halves by
lag 5 and is gone by lag 30. A depth history sampled once a minute is worth
almost nothing; the same reading captured beside each quote is worth a third of
the variance.

What 34.6% buys: σ_d falls 2.6 → 2.10 bps, so required paired cycles at 80%
power become:

| target effect | cycles |
|---|---|
| +0.35 bps (top of CLMM band) | **283** |
| +0.25 bps | 555 |
| +0.10 bps (bottom of band) | 3,470 |

A ~300-cycle paired execution experiment on a liquid CLMM major would resolve
whether that margin sits at the top of its range.

Two limits: one asset over one period, and BONK is a reserve-driven CPMM whose
depth process differs from tick-concentrated CLMM majors. And it bounds the
control variate, not the experiment — realised cost also carries routing and
priority-tip variance no pre-trade depth reading predicts.

---

## 3. Data pipeline architecture

### 3.1 The problem the design solves

Two defects made lag-0 capture impossible before the refactor:

1. **`PricePoller::poll()` returned `f64`.** Every quote arrived carrying
   `priceImpactPct`, `contextSlot` and a route plan, and all of it was dropped
   at the source. Depth could not be attached downstream because it no longer
   existed.
2. **A median-of-3 spike filter is a hidden one-tick lag.** The engine consumes
   the median of ticks `t−2..=t`, so on any sustained move the price the
   decision used is older than the `context_slot` on the row. Pairing a lag-0
   depth reading against that silently produces a lag-1 experiment.

### 3.2 Lag-0 by construction

`priceImpactPct` is already present in every `/quote` response. Taking it from
the **same response** as the price means the two share `context_slot` by
construction — the lag is structurally zero, not merely small, and costs zero
extra requests.

This is strictly better than the two-quote depth probe that produced the
original recording. That probe derived its spread from a small-clip and a
large-clip quote: two HTTP requests, no guarantee they were computed against
the same chain state, and no field recording whether they were.

The probe is retained as opt-in (`poll_snapshot_probed`) and now records both
slots so the gap is reported rather than assumed. A failed probe degrades to
the lag-0 row rather than losing the tick.

### 3.3 Schema

```rust
pub struct QuoteSnapshot {
    pub seq: u64,                     // monotonic; gaps = dropped polls
    pub t_ms: u64,                    // human alignment only — never for lag
    pub context_slot: Option<u64>,    // the freshness key
    pub price: f64,                   // as quoted
    pub price_used: Option<f64>,      // post-filter, what the engine consumed
    pub impact_bps: Option<f64>,      // lag-0 control variate, same response
    pub impact_raw: Option<String>,   // the API's exact string
    pub probe: Option<DepthProbe>,    // { depth_bps, probe_amount, context_slot }
    pub venue: Option<String>,
    pub hops: u8,
    pub latency_us: u64,
}
```

Design decisions and their rationales:

- **`context_slot`, not `t_ms`, is the lag key.** Poll interval and Solana slot
  time drift apart. Bench 038's decay is per observation; slots make that
  countable, wall clock does not.
- **`price` and `price_used` are both recorded.** Their difference is the spike
  filter's lag. Recording both lets the analysis pair the control variate
  against the price the decision saw rather than infer it.
- **`impact_raw` keeps the API's exact string.** `impact_bps` assumes
  `priceImpactPct` is a fraction (0.0012 = 12 bps). If that convention is ever
  wrong, every recorded row can be reinterpreted without re-recording. This is
  the extraction-loss lesson applied to our own capture: a unit error found
  after a month of recording is otherwise unrecoverable.
- **`seq` gaps are load-bearing.** CUPED lag is counted in observation steps, so
  a silently dropped poll would understate staleness.
- **`hops` is a covariate, not decoration.** A route change between quotes moves
  depth for reasons unrelated to liquidity.
- **`latency_us`** bounds how stale the reading already was on arrival.

### 3.4 Freshness as a type

```rust
pub enum Freshness {
    SameQuote,            // one response — lag 0 by construction
    SameSlot,             // two responses, same context_slot
    Stale { gap: u64 },   // two responses, gap slots apart
    Unknown,              // a slot is missing; staleness cannot be established
}
```

`is_usable()` admits `SameQuote | SameSlot | Stale{gap:1}`, drawn directly from
the decay table — by lag 5 the reduction has already fallen from 34.6% to
19.3%. A row with neither slot nor impact returns `Unknown`, never something
usable. Nine tests pin the unit convention, each freshness class, the
`price`/`price_used` split, and payload round-tripping.

### 3.5 Operational consequence

`impact_bps` costs zero extra requests, so capture need not be gated or run on
selected assets only. Every polled asset becomes a CUPED-ready corpus as a side
effect — which converts asset coverage from a hypothesis-testing activity
(which bench 037 showed is underpowered at n = 11) into corpus-building that
accrues for free during normal operation.

---

## 4. Method notes

**Retraction is first-class.** Claims are superseded by later benches with
forward-pointing banners rather than edited away: 034 → 035 → 036 is a
hypothesis built over two benches and killed by a third; 024 carries corrections
from 030 and 031. Commit subjects asserting since-refuted claims are left
standing, with the correction recorded in `docs/PROVENANCE.md` — history is not
rewritten to match later measurement.

**Claims are bound to evidence by tests.** `tests/claims.rs` fails the build if
documented prose cites a bench that does not exist; `tests/claim_ttl.rs` fails
if bench directories accumulate uncited. The second fired during this work and
forced a bench to be written up rather than left orphaned.

**Citation tests pin external numbers.** `tests/power.rs` reproduces all thirty
cells of a published sample-size table. That mechanism caught a convention
error: the reference's unpaired *required-N* column is a total across both arms
while its *power* column is quoted at 534 per group (N = 1,068 total). The
annotation stating this was an embedded image that did not survive text export,
so the source read as self-contradictory until the image was recovered.

**Recurring pattern across all of it:** every negative result is reported beside
its MDE, because a null without a detection floor is not a finding. Several
conclusions in this document are "we could not have seen it" rather than "it is
not there", and the two are kept distinct.
