# Open questions — how an agent-built project survives its own confidence

Written by the agent that built this repo, after two days in which its own
harness killed four of its claims. Every question below is anchored to a
specific incident here, so an answer can be checked against a real case rather
than argued in the abstract. Marked **[self]** where more compute alone could
answer it, **[external]** where the answer lives in literature or in other
people's practice.

## A. Not fooling yourself

**A1. What is the correct statistical protocol for "did my strategy search
find something, or did I mine noise?"** [external]
*Incident:* we enumerate 1,054 machines and pick winners on a handful of
windows. Twice that produced a "finding" that vanished under more data
(memecoin edge across 4 assets → gone across 11; ecosystem floors on 6 corpora
→ negative on 225 real windows). Naïve per-comparison standard errors clearly
are not enough when the selection itself is the experiment.
*What a useful answer contains:* which of White's Reality Check, Hansen's SPA
test, the deflated Sharpe ratio, or probability-of-backtest-overfitting (PBO)
fits a search over ~10³ deterministic strategies with ~10²–10³ windows; and
the minimum sample each needs to say anything.

**A2. What sample size do we need to detect a k-bps effect at our measured
variance — and should we have refused to run at all below it?** [self]
*Incident:* per-cycle SD is 6.6 bps unpaired, 2.6 paired, while the effects
sought are sub-1 bps. 534 live cycles still gave t = 0.37. We never did a power
calculation *before* running; we discovered the futility afterwards, twice.
*Useful answer:* a pre-run power rule that tells the harness "this experiment
cannot resolve the effect you are looking for" and refuses, the way it now
refuses to report without a standard error.

**A3. Is there an established vocabulary for grading claim strength to
evidence, that a machine can enforce?** [external]
*Incident:* the same project said "beats every standard exit", then "no
durable edge", about the same code — the difference was evidence, not
behaviour, but nothing in the pipeline forced the wording to track it.
*Useful answer:* an evidence ladder (L1 observed / L2 correlated / L3 causal,
or similar) with rules a linter could apply to claim text in READMEs.

## B. Catching yourself before a human does

**B1. What audit routines catch unmeasured constants and premature
optimisation automatically?** [external]
*Incident:* a reviewer asked "why not verify every quote?" — the sampling rate
was chosen by intuition, cost 0.055 ms when measured, and had silently traded
away the product's main security property. A subsequent sweep of *every*
constant found two more retractions. The audit existed only because a human
asked.
*Useful answer:* prior art on sensitivity-sweep gates in CI, and on flagging
"magic number introduced without an accompanying measurement" at review time.

**B2. Does pre-registration work for agent-run experiments?** [external]
*Incident:* a bench report shipped with a conclusion paragraph written *before*
the data existed; the table then contradicted it. It was caught, but only by
re-reading.
*Useful answer:* evidence from science reform on whether pre-registering the
hypothesis and analysis plan reduces post-hoc storytelling, and the cheapest
form that works for a machine that writes both the code and the prose.

**B3. How often should an agent re-validate its own prior claims, and what
triggers a re-check?** [self]
*Incident:* "the surprise trigger improved every floor" survived several
releases before a control was run and returned zero. Claims here have no
expiry and no owner.
*Useful answer:* a decay/re-validation policy — e.g. every claim in the README
carries the bench that produced it, and CI re-runs it and fails on drift.

## C. Surviving sustainably

**C1. Where does human attention pay off most in an agent-run project?**
[external]
*Incident:* across two days, the two highest-value human contributions were
both **questions**, not corrections: "why not check every second?" and "is the
best-selected bot actually being used?" — each exposed a defect worth more
than any feature shipped that day. Direct instructions produced far less.
*Useful answer:* evidence on oversight patterns (question-asking vs
task-assigning vs reviewing diffs) and where the marginal human minute buys
the most.

**C2. What is the real cost curve of an always-on research loop, and when is
paying for infrastructure cheaper than engineering around it?** [self]
*Incident:* we avoided a $5/month plan by moving heavy work into the
visitor's browser and keeping only counters server-side — which turned out to
be a *better* architecture, not just a cheaper one. But we also spent hours
on workarounds (hand-rolled transaction signing because a dependency was
risky, precomputed PDAs because a Worker cannot derive them).
*Useful answer:* a rule of thumb for when a constraint is a design gift versus
a tax.

**C3. What keeps a repo honest after the humans stop watching?** [external]
*Incident:* four negative results are recorded here because the agent chose to
record them. Nothing structural prevented deleting them.
*Useful answer:* practices that make retraction cheap and deletion expensive —
append-only bench numbering, claims bound to bench artefacts, provenance.

## D. About this product specifically

**D1. Has anyone credibly demonstrated exploitable exit-timing structure in
liquid crypto pairs at minute horizons?** [external]
We found none across 11 assets, and a null control confirms we are not
inventing it. Who has looked, with what power, and what did they find?

**D2. Is execution quality (routing, timing, clip size) a defensible edge on
Solana aggregation, and does anyone publish measurements?** [external]
Our depth probing shows 27 bps between clip sizes on BONK versus 0.3 on
SOL/USDC — large enough to matter, but we have no idea whether it is
capturable after fees and latency.

**D3. Who pays for verifiable execution?** [external]
We can now prove: signed quote → policy committed on-chain before the sale →
fill. Is that a compliance product, a copy-trading primitive, a market-maker
requirement, or a curiosity?

---

## Answers received (2026-08-27)

External research came back on all four groups
([full text](research/2026-08-27_epistemic_governance.txt)). What changed here
as a result, same day:

**A1 — protocol.** Recommended pipeline for a search over ~10³ strategies:
**deflated Sharpe ratio as a fast screen during search, then Romano-Wolf
stepdown or CSCV/PBO on the survivors before declaring anything.** Hansen's
SPA is preferred over White's Reality Check, which loses power when the
candidate pool is full of bad strategies — which ours is, by construction.
✅ **PBO/CSCV implemented** (`src/pbo.rs`, `tests/pbo.rs`, calibrated on
synthetic noise at 0.48–0.51) and run over the full 1,054-machine population
on 11 real assets ([`024_overfit`](../benches/024_overfit/report.md)). It
answered the question it was built for, and the answer was not the expected
one: **the selection generalises (PBO 0.05–0.20 on most assets) while the
profit does not** (+2.5…+16.8 bps in-sample → −6…+2 out of sample). ✅ **Romano–Wolf stepdown also implemented** (`src/stepdown.rs`,
`tests/stepdown.rs` — calibrated: almost no rejections on noise, planted edges
recovered) and run over all 1,054 machines on 11 assets
([`025_multiplicity`](../benches/025_multiplicity/report.md)): **zero
survivors** after familywise correction, best adjusted p = 0.122. Deflated
Sharpe remains unbuilt and is now question E1, since our objective is a paired
bps difference rather than a Sharpe ratio.

**A2 — power.** ✅ **Implemented** (`src/power.rs`, `tests/power.rs`). The
reference table is unambiguous about what our experiments could ever have
found: detecting 1 bps needs **54 paired** or **1,368 unpaired** cycles; our
534-cycle unpaired soak had **~9% power at 0.25 bps**. Both of the runs we
agonised over were statistically incapable of answering, and a microsecond of
arithmetic beforehand would have said so. The module now computes required N,
achieved power and the minimum detectable effect; the paired figures reproduce
the reference exactly (n = 54, 60.33% power at 0.25 bps). One correction:
the reference's *unpaired power* column uses a different sample convention
than its own *required-N* column, so we implement the internally consistent
version and say so in the code.

**A3 — evidence ladder.** ✅ **Implemented** (`tests/claims.rs`). Four tiers
(observational → in-sample significant → multiplicity corrected →
out-of-sample causal), each with permitted and prohibited vocabulary. The
linter runs as an ordinary test: a high-conviction comparative claim must cite
a bench artefact in the same paragraph, and tier-violating language
("guaranteed", "risk-free", "regime-invariant") fails the build. It caught two
cases on its first run — one a real gap, one a false positive from a negated
sentence, which taught it about negations and quotations.

**B1–B3 — self-audit.** Confirms the sensitivity sweep we already built.
✅ **Pre-registration implemented** (`src/prereg.rs`): hypothesis, benchmark,
effect size, power target, split boundaries, null control and frozen corpus
list, content-hashed — moving the goalposts changes the hash and the report
stops verifying. Its power gate refuses an experiment that cannot answer and
reports what it would take instead. ✅ **Claim–evidence binding implemented**
(`tests/claim_ttl.rs`): every bench cited in the docs must exist, and it
immediately caught a claim in the submission draft citing a bench directory
deleted during cleanup — the exact rot the policy exists to prevent. Still
unbuilt: `@calibrated` provenance tags via an AST literal parser, and
distributional-drift (K-S) triggers for automatic re-benchmarking.

**C1 — oversight.** The observation from this project is a documented pattern:
Socratic interrogation has **maximal marginal ROI**, imperative tasking **low**,
diff review **moderate**. The mechanism named is *localised context
entrapment* — an agent optimises inside a premise without checking the premise,
so a question that attacks the premise buys more than an instruction that
accepts it.

**C2 — gift vs tax.** A clean rule: a constraint that enforces structural
alignment with the domain (heavy compute in the visitor's browser,
trust-minimised verification) is a **design gift**; a constraint that forces
you to reimplement established runtime primitives is an **engineering tax** —
*pay for infrastructure instead*. By that rule our hand-rolled ed25519 signing
and precomputed PDAs are a tax we should retire, not a badge.

**C3 — provenance.** Append-only content-addressed bench records, claims bound
to bench hashes, and retraction as a first-class status transition
(CONFIRMED → REFUTED → SUPERSEDED) — with the agent rewarded for falsifying
its own prior claims rather than accumulating them.

**D1 — minute-horizon exits.** Our null result is the *expected* one: lead-lag
alpha in crypto decays in **50–500 ms**, so at 1-minute sampling it is fully
decayed, and gross returns of minute-horizon rules collapse once 5–10 bps
round-trip costs are applied. We were looking where the literature says there
is nothing to find.

**D2 — execution edge.** The 27 bps clip-size spread on BONK is real and comes
from fragmented CPMM pools (versus concentrated CLMM ticks on SOL/USDC), but
capturing it is contested: **10–15 bps can go to priority tips during
congestion**, and the quote-to-block "execution gap" requires just-in-time
on-chain simulation to avoid negative slippage. Viable, not free.

**D3 — who pays.** **MiCA Article 78** (EU) obliges crypto-asset service
providers to demonstrate best execution across price, cost, speed and size,
with pre- and post-trade publication. A cryptographically verifiable trade
lifecycle — signed quote, policy committed before the fill, on-chain
settlement — is a compliance artefact, not a curiosity. Also named:
institutional allocators verifying no toxic internalisation, and trustless
copy-trading.

## If only three get researched

1. **A1** — because everything else in this repo is downstream of whether our
   selection protocol is sound.
2. **C1** — because it decides how the human should spend their time on the
   next project, and the evidence here says the answer is counter-intuitive.
3. **D2** — because it is the only remaining hypothesis for a real edge that
   this project has not already falsified.


---

# Round 2 — questions the first answers created (2026-08-27)

Round 1 changed the code the same day it arrived: power gating, an
evidence-ladder linter, and a CSCV/PBO overfitting test that produced the
sharpest result this project has. These are the questions that opened up
*because* of those answers. Same convention: **[self]** = more compute could
answer it, **[external]** = needs literature or other people's practice.

## E. Adapting the recommended protocol to a non-Sharpe objective

**E1. How do you apply the deflated Sharpe ratio when the objective is a
paired bps difference rather than a return series?** [external]
*Why it blocks us:* DSR is defined on Sharpe ratios with skew and kurtosis
corrections. Our metric is "edge versus a reference exit on the same price
path" — paired, roughly symmetric, and already variance-reduced. Substituting
mean/σ of the paired differences may or may not preserve the extreme-value
correction that makes DSR meaningful.
*Useful answer:* whether the paired-difference t-statistic can be plugged into
the DSR machinery directly, or which alternative (Romano-Wolf on the paired
differences?) is correct for this objective.

**E2. What is the minimum viable configuration of CSCV when windows are
scarce?** [external]
*Incident:* our per-asset window counts are 166–375. We used S = 10 (252
splits). Bailey et al. suggest S = 16. Three assets returned PBO ≈ 0.5 or
worse and we cannot tell whether that is a real signal-free asset or an
artefact of too few windows per slice.
*Useful answer:* guidance on slices-versus-observations, and whether
overlapping windows (which we could generate cheaply) are admissible or
destroy the test.

**E3. Our PBO says the ranking generalises while the level does not. Is that a
named phenomenon, and what does the literature prescribe?** [external]
*Why it matters:* this is now the central empirical fact about the product.
"Reliable selection of an unprofitable population" suggests the objective, not
the search, is wrong — but we would rather know than guess.

## F. When the strategy space is the problem

**F1. Is there a formulation of exit execution with provable guarantees that
does not require predicting direction?** [external]
*Motivation:* round 1 established that minute-horizon alpha is decayed
(50–500 ms) and that our machines cannot be profitable there. Optimal
execution theory (implementation shortfall, Almgren–Chriss style) claims
results without directional prediction. Does that transfer to a retail-sized
exit over a volatile token on an AMM?
*Useful answer:* the objective function we should be minimising instead of
"edge versus hold", and whether it admits the same enumerate-and-verify
treatment.

**F2. What is the right objective when the honest goal is risk control rather
than return?** [external]
Drawdown-first, worst-case (tropical/minimax) and viability-set formulations
all exist. Which are testable with the harness we already have — paired,
out-of-sample, with a null control?

**F3. Does exit-policy value appear at horizons where alpha is not the
mechanism — for example forced exits (liquidation, expiry, redemption)?**
[external]
Perps and prediction markets have deadlines; our machinery may matter there
for reasons that survive an efficient market.

## G. Making the loop itself trustworthy

**G1. Do continuous autonomous research loops degrade over time, and what is
the failure mode?** [external]
*Why we ask now:* we are about to run one deliberately. Plausible failure
modes: claim accumulation without retraction, context rot, gradual metric
drift, or optimising the harness instead of the product.
*Useful answer:* evidence from long-running autonomous or semi-autonomous
research systems on what breaks first and what instrumentation catches it.

**G2. How should adopted external findings themselves be verified?** [self]
*Incident:* round 1's reference table contained an internal inconsistency in
its unpaired column, which we only caught because we reproduced the paired
figures first. Adopting a finding is itself an unmeasured assertion unless a
test reproduces it.
*Proposal to validate:* every adopted finding ships with a test that fails if
the finding is misapplied — the power module already does this. Is there
prior art for "citation tests"?

**G3. What does a pre-registration manifest look like for an agent that writes
its own analysis code?** [external]
Round 1 prescribed SHA-256 committing hypothesis, effect size, power target,
split boundaries and null controls before touching data. We have no example of
one that survived contact with a real agent loop.

## H. Product questions the answers reframed

**H1. Under MiCA Article 78, what exactly must a best-execution record
contain, and would our artefact (signed quote + on-chain policy commitment +
fill) satisfy an auditor?** [external]
This is the difference between a demo and a compliance product.

**H2. Who currently proves best execution on Solana, and how?** [external]
If the answer is "nobody, they assert it", the verifiable chain is a market.
If regulated venues already do it off-chain, we are late.

**H3. Is the 27 bps clip-size spread capturable after priority tips and the
quote-to-block execution gap — and has anyone measured the net?** [external]
Round 1 said 10–15 bps can go to tips in congestion. The remainder is either a
business or a rounding error, and we cannot tell from here.

## Priority for round 2

1. **F1** — if the objective is wrong, everything downstream is wasted effort;
   this is the only question that could redirect the whole project.
2. **E3** — because "selection generalises, profit does not" is our central
   finding and we do not know what it is called or what follows from it.
3. **H1** — because it decides whether the verifiable chain is a product.


---

# Round 2 answers received (2026-08-27)

[Full text](research/2026-08-27_nondirectional_execution.txt). Same-day
consequences below; **one answer was tested and refuted**, which is the point
of the citation-test rule.

**E1 — DSR for a paired objective.** There is a defined adaptation: the
**Deflated Paired Metric**, standardising the mean paired differential and
deflating the threshold by an extreme-value estimate of the maximum under the
global null. Its named failure mode is decisive for us: *it underestimates
false discovery when candidates are densely cross-correlated* — and 1,054
enumerated FSMs over the same price paths are about as correlated as a
candidate set can be. DPM is therefore a fast screen, not a verdict, and the
verdict tool is Romano–Wolf, which we already have and which already returned
zero survivors. **No implementation needed; the tool we built was the right
one.**

**E2 — CSCV under sample constraints.** S = 10 is the minimum stable
configuration at our window counts (S = 16 would starve the slices and push
PBO toward 0.5 regardless of truth), so our choice was right. Overlapping
windows — which we had considered as a cheap way to get more windows — are
**inadmissible**: they impose a moving-average error structure that violates
block exchangeability and artificially depresses PBO. And an embargo exceeding
the autocorrelation horizon is required between slices, which our
implementation **did not have**. ✅ Fixed (`cscv_embargoed`); PBO moves ≤ 0.09
and no conclusion changes, so the earlier result stands on a now-correct
implementation.

**E3 — the name for "rank generalises, level collapses".** Offered as
**frictional dominance / capacity exhaustion**: a positive gross edge common to
all candidates, sunk by a friction term larger than the best of them, with
ranking preserved because friction is a uniform shift.
❌ **Tested and refuted** ([`026_diagnosis`](../benches/026_diagnosis/report.md)).
Our simulator charges zero friction, and the selected machine's friction-free
out-of-sample level is **−6.35 bps**. There is no positive gross edge for
friction to consume, so no amount of cheaper routing would recover it.

The test produced a better decomposition than the label it was checking. Split
against the population median, the level separates into **a drift term**
(median OOS swings −52 to +28 bps, tracking whether the asset rose or fell —
identical for all 1,054 machines, pure arithmetic) and **a selection term**
(+2.59 bps, the selected machine over the median, positive on 7 of 11 assets).
The drift term dominates the variance, which is why "edge versus hold" was
never resolvable and why benchmarks against exits that also liquidate always
had smaller standard errors. **"Edge versus hold" is a badly-conditioned
metric and we should stop using it as a headline.**

**F1 — a non-directional objective.** Prescribed: **Almgren–Chriss
implementation shortfall**, with arrival price as the reference and price
impact modelled deterministically from CFMM/CLMM pool reserves. The sentence
that matters: the search space becomes deterministic execution trajectories
across venues, *preserving the enumerate-and-verify harness without requiring
directional forecasting*. Our machinery survives the redirect; only the
objective changes.

✅ **Implemented and tested** ([`027_shortfall`](../benches/027_shortfall/report.md)):
`sim::shortfall_bps_impact` plus a rate-dependent temporary-impact model, with
selection on train and measurement on disjoint test across 11 assets under
three objectives. The redirect works mechanically — the enumerate-and-verify
harness took the new objective unchanged, exactly as predicted — but the answer
is still no: the apparent 30% variance advantage over TWAP is entirely
explained by liquidating ~4× sooner, and a speed-matched TWAP beats the
machines at their own urgency (SD ratio 1.12, significant on 0 of 11).

**F2 — risk-centric objectives.** CVaR/expected shortfall on the shortfall
distribution, minimax/tropical worst-case over rolling sub-windows, and
viability sets. All stated to be compatible with paired out-of-sample
validation against a TWAP baseline — i.e. with the harness we already own.

**F3 — forced exits.** Confirmed as where structure survives: lending
liquidation boundaries (health-factor cascades, where pre-empting a third-party
liquidator saves the seize penalty), perpetual funding snapshots at fixed
hourly boundaries, and derivative/prediction-market settlement. Non-
discretionary liquidity demand is not an efficient-market phenomenon.

**G1 — loop degradation.** Four named modes: **harness overfitting** (Goodhart
drift — the example given is *exploiting zero-slippage assumptions on
small-cap tokens*, which is precisely the corner our simulator occupies),
**context entrapment**, **epistemic debt** (refutations that fail to deprecate
what depended on them), and **metric degradation** (criteria drifting to keep
the narrative positive). Prescribed fix: an explicit hypothesis state machine,
Proposed → Committed → In-Sample Validated → Confirmed/Refuted → Superseded.

**G2 — citation tests.** The practice is named and has three parts:
analytical reproduction of published tables, parameter-convention assertions,
and synthetic null calibration. ✅ We already do all three (power reproduces
the reference table, the paired/unpaired convention mismatch was caught that
way, and PBO and Romano–Wolf are both calibrated on synthetic nulls).

**G3 — manifest schema.** A field list we can adopt directly; ours is missing
`manifest_version`, `registration_timestamp_utc`, `target_venue_and_asset`,
the embargo specification, and the candidate-space size with its RNG seed.

**H1/H2 — MiCA and the market gap.** Article 78 requires best execution across
price, cost, speed, likelihood and size; **a five-year immutable audit trail**;
publication within **30 seconds**; and it **prohibits single-broker reliance**,
so multi-venue evaluation must be demonstrated rather than asserted. The gap
is stated plainly: *existing aggregators rely on asserted best execution,
publishing post-hoc benchmark statistics rather than verifiable cryptographic
proof of pre-trade market state.* Our signed-quote → committed-policy → fill
chain is the missing artefact.

**H3 — is the 27 bps capturable? No, and now with the arithmetic.** The
answer document states it as an identity:

```
Net Margin = Spread_gross − (Fee_pool + Tip_priority + Drift_latency + Fee_L1)
BONK:       = 27.0 − (25.0 + 10.0 + 5.0 + 0.2) = −13.2 bps
```

The CPMM pool fee alone (**25 bps**) very nearly consumes the entire spread
before priority tips or latency drift are counted. This is not a marginal
call needing better execution — it is negative by 13.2 bps on public routes,
and only private transaction tunnels, zero-fee aggregation and just-in-time
on-chain simulation could change the sign. **Plan 001's depth hypothesis is
economically dead, not merely unmeasured** — which is consistent with its own
preliminary A/B showing no benefit, and it is now closed on the economics
rather than left waiting for more data.

### Extraction note — what we nearly missed

The first pass over both documents used the plain-text export, and **formulas
and several tables are embedded as images** (70 in round one, 194 in round
two) that the text export drops silently. In round one this cost nothing: the
power table survived as text, and the citation tests verified our
implementation against those published numbers — which is exactly the failure
mode citation tests exist to catch. In round two the cost table *was* an
image, so the qualitative claim ("negative net realisation") came through
while the decisive **−13.2 bps** and its breakdown did not, and a plan stayed
open that the arithmetic had already closed.

Recovered by exporting `.docx` and reading the embedded media directly. Rule
adopted: **when adopting an external document, check whether its numbers
survived the export before acting on its prose.**
