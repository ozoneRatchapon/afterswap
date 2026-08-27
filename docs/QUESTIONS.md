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

## G0. What we learned before the answers arrived: it was never the statistics

Three false results appeared in a single day, and **none of them was a
statistical error**. Each was a data-hygiene error — the thing being measured
was not the thing we thought was being measured:

1. **Corpus mutated mid-experiment.** A background recorder appended a new file
   into the directory the benchmark scans, so an A/B compared different data on
   each side. Three benches were invalidated. *Caught because turning the
   feature off did not restore the previous numbers* — a control that only
   exists if you run it.
2. **Five simultaneous comparisons, no correction.** The live monitor starred
   any floor with |t| ≥ 1.96 while testing five of them, which fires on ~23% of
   clean runs by chance. *Caught because we had just built Romano-Wolf and
   recognised our own mistake in our own instrument.*
3. **Two engine configurations mixed in one measurement stream.** A long-running
   soak kept writing to the same file across a config change, and the pooled
   result crossed the significance threshold while the clean segment did not
   (t = −2.59 pooled versus −1.45 clean). *Caught by noticing a
   `route:"surprise"` line in a log that should no longer produce one* — a
   configuration we had retracted hours earlier was still running.

The generalisable lesson, and our answer to "what makes an autonomous loop
survivable": **not tighter statistics — provenance.** Every row of measured
data must carry which code version, which configuration and which corpus
produced it, and any pooling across those boundaries must be impossible rather
than merely discouraged. All three failures were invisible to analysis and
obvious from provenance.

Two practices adopted the same day: measurement runs from a **pinned binary
copy**, never from the path the build writes to, so developing cannot silently
mutate what is being measured; and a config change **starts a new measurement
stream** rather than appending to the existing one.

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
and most numeric table cells are embedded as images** (70 in round one, 194
in round two) that the text export drops silently — no placeholder, just
whitespace.

Round one survived: its power table exported as text, and the citation tests
verify our implementation against those published numbers — which is exactly
the failure mode citation tests exist to catch.

Round two lost four tables, and the first recovery pass found only one of
them — the cost table, because that was the one that answered the question in
hand. All four are now recovered from the `.docx` embedded media and recorded
in `docs/research/EXTRACTION_LOSS.md`. What the other three changed:

- **The full 5x6 sample-size table** carries an annotation round one lacks —
  the unpaired power column is quoted at 534 *per group*, not total. The
  number in that annotation was an image, so it reached us as `( /group)`,
  and `power.rs` recorded the reference as self-contradictory. It is not.
  With the label recovered our implementation reproduces all thirty cells,
  which is the independent cross-check a single copy of the table could never
  have given us.
- **The CSCV sizing passage** — unreadable in the export, every number an
  image — says `S = 10` is the minimum stable partition for `T < 400`, and
  that `S = 16` would push PBO toward 0.50 on slice-size grounds alone. We
  use `S = 10` at 166 windows. Right answer, previously unjustified.
- **The cost table's second column** shows CLMM liquid majors at **+0.1 to
  +0.3 bps** net, not negative. That is below this sample's detection floor
  (~1 bps paired) so it changes no decision and does not reopen Plan 001 —
  but it does mean "execution is unprofitable" is a claim about long-tail
  CPMM routes specifically, and the README should not generalise it.

The cost table's images truncate every range after the dash — the source
renders "20.0 -" and stops — but the prose above the table states each upper
bound separately, and the arithmetic closes on both ends:
`27 - (25+10+5+0.2) = -13.2` and `27 - (30+15+8+0.5) = -26.5`, exactly the
published range. Only the CLMM column's upper bounds have no prose
counterpart and stay unknown.

Rule adopted: **when adopting an external document, check whether its numbers
survived the export before acting on its prose — and when recovering, sweep
every image, not just the one you came for.**

# Round 3 — questions the recovered material created (2026-08-27)

Round 2's answers only became fully legible after every embedded image in both
documents was transcribed and merged back into the prose (264 images; see
[`docs/research/EXTRACTION_LOSS.md`](research/EXTRACTION_LOSS.md)). Reading the
continuous text rather than the surviving fragments changed three things: it
resolved a convention our code had recorded as contradictory, it surfaced a
sample-size floor that applies to 7 of our 11 assets, and it revealed that the
statistic round two spends its first section deriving is one we never built.
Same convention: **[self]** = more compute could answer it, **[external]** =
needs literature or other people's practice.

## I. The dissent that survived every explanation

**I1. Three assets return PBO ≈ 0.5 at every partition count. What class of
series does that?** [external]
*Incident:* FLOKI, JTO and PYTH sit at 0.52–0.62, 0.52–0.60 and 0.27–0.65
across S ∈ {6, 8, 10, 12, 16} (`benches/030_slice_sensitivity`), while JUP,
ORCA, RAY and SHIB — same 166 windows, same pipeline — never leave 0.05–0.28.
Slice size is ruled out. Round 2's answer to E2 supplied the floor hypothesis
and our own bench refuted it.
*Useful answer:* known series properties that make CSCV rank-selection
degenerate — autocorrelation structure, regime count, volatility clustering,
tick-size or listing artefacts — and which of them is cheapest to measure.

**I2. Is a PBO near 0.5 on a minority of assets evidence about the assets, or
about the model space?** [external]
*Why it matters:* we report "selection is sound" from 8 of 11 assets. If the
three dissenters mean the alphabet genuinely has no relative structure to find
on those series, that is a bounded and honest statement. If it means our
enumeration is unstable wherever a particular regime dominates, the 8 are
suspect too, and the headline claim of bench 024 is overstated.

**I3. What sample size would make the dissent decidable?** [self]
*Why it blocks us:* the earlier framing — "with 166 windows each we cannot yet
find out" — was never quantified. We now have the power machinery to answer it
for a difference in means; we have nothing equivalent for a difference in PBO.
*Useful answer:* the MDE analogue for a PBO estimate, so the question either
gets a data collection target or gets closed as undecidable.

## J. Reporting under a floor we do not meet

**J1. How should PBO be reported when the sample sits under the source's own
stated minimum?** [external]
*Incident:* round one states CSCV requires `T >= 500` outright and repeats it
for our exact candidate count. Our largest asset has 375 windows; seven have
166. Round two does not restate the floor — it prescribes `S = 10` for
`T < 400` instead — so the two documents disagree about whether our regime is
admissible at all. Neither endorses `T = 166`.
*Useful answer:* whether a sub-floor PBO is a weakened estimate that should
carry an interval, or an inadmissible one that should not be published as a
number at all.

**J2. When two external sources conflict, what decides?** [external]
*Incident:* round one gives `S = 16` as its worked CSCV example with no
sample-size caveat; round two rules `S = 16` out for our window counts and
prescribes `S = 10`. We followed round two on the grounds that it is later and
written about our data — a defensible heuristic we invented on the spot.
*Useful answer:* an actual arbitration rule, because this will recur every
time a research track spans more than one document.

**J3. Should a citation test fail when the source it cites is superseded?**
[self]
*Why it matters:* `tests/power.rs` now pins thirty cells of a published table.
That is exactly the mechanism that caught our convention error — and exactly
the mechanism that will quietly enforce a number a later document revises. The
evidence ladder handles claim decay; citations have no equivalent.

## K. The statistic we did not build

**K1. Do we need the Deflated Paired Metric at all, having gone straight to
Romano-Wolf?** [external]
*Incident:* round two derives DPM across its entire first section — Mertens
asymptotic variance, EVT maximum under the global null, the deflation formula
— and its prescribed pre-registration manifest gates on `DPM >= 0.95`. We have
no implementation. We built Romano-Wolf stepdown instead, which the same
document names as the rigorous alternative when candidates are cross-correlated
(they are).
*Useful answer:* whether DPM earns its place as a fast screening filter *during*
enumeration when a stepdown pass already runs after it, or whether skipping it
was correct and the manifest gate should be dropped rather than implemented.

**K2. What is the cross-sectional variance of our trial statistics, and is it
non-zero enough for EVT to apply?** [self]
*Why it blocks us:* the DPM table lists "requires non-zero cross-sectional
variance" as a sample requirement. Our 1,054 machines are enumerated from a
3-state alphabet and are heavily related; if their trial statistics cluster,
the EVT correction understates false discovery — which the document flags as
DPM's primary failure mode. This is measurable today from data we already have.

**K3. Does the prereg manifest need gates it cannot evaluate?** [self]
*Why it matters:* `prereg.rs` is generic over its gates, so nothing breaks. But
a manifest that pre-commits to `DPM >= 0.95` and then never computes it is the
pre-registration equivalent of a dead citation — the exact failure the
claim-evidence test was built to catch, one level up.

## L. Execution tracks the recovery reopened

**L1. Is a +0.1 to +0.3 bps CLMM margin reachable by any design we could run?**
[self]
*Incident:* the recovered cost table puts liquid CLMM majors at net positive,
against BONK's −13.2 to −26.5. Positive, but roughly an order of magnitude
under our paired MDE (~1 bps at σ_d = 2.6, N = 534). Detecting 0.25 bps paired
needs 849 cycles; 0.10 bps needs 5,306.
*Useful answer:* whether a variance-reduction design (tighter pairing, control
variates, common random numbers across arms) can cut σ_d enough to bring a
0.3 bps effect inside reach, or whether this is permanently below our floor and
should be recorded as such rather than left as an open opportunity.

**L2. Are the structural-urgency horizons worth a track?** [external]
*Incident:* round two prescribes pivoting strategy search toward
non-discretionary liquidity — lending liquidation boundaries (protocol penalty
500–1,000 bps), the 60-second window around perpetual funding settlement, and
derivative expiry auctions. We have evaluated none of them. They are the
document's actual recommendation, and the only one we have not either adopted
or refuted.
*Useful answer:* which of the three has a public, replayable data source, since
that decides whether this is a research track or a live-capital-only idea.

**L3. Does the verifiable-execution pitch survive its own economics?** [self]
*Why it matters:* MiCA Article 78 and the 30-second publication requirement
make the compliance framing concrete, and both documents converge on it. But
our own arithmetic says the execution edge we would be proving is negative on
long-tail routes and sub-MDE on liquid ones. The product may be "verifiable
best execution" rather than "better execution" — worth deciding deliberately
rather than by drift.

## M. Process, after the extraction failure

**M1. What is the general procedure for adopting an external document?** [self]
*Incident:* two rounds of research were acted on from a text export that
silently dropped 264 images. Round one survived by luck (its key table
exported as text); round two lost four tables, and the first recovery pass
found only the one that answered the question in hand.
*Useful answer:* a checklist worth committing — verify numeric survival before
acting, transcribe every image rather than the load-bearing ones, read the
merged text continuously, and treat "this symbol is obviously unimportant" as
a claim requiring the sentence around it.

**M2. Should the recovered documents be committed in merged form?** [self]
*Why it matters:* the `.txt` exports in `docs/research/` are still the lossy
originals, kept verbatim on purpose. The merged reconstructions exist only as
scratch files. Committing them makes the research readable but introduces a
transcription of our own making between the source and any future reader.

## Priority for round 3

If only three get researched: **I1** (what makes a series PBO-degenerate),
**J1** (how to report a sub-floor PBO), **K1** (whether DPM is owed a build).
The first two decide whether bench 024's headline holds; the third decides
whether our pre-registration manifest is honest.

# Round 3 answers received (2026-08-27)

Third document, read the right way from the start: exported as `.docx`, all 116
embedded images transcribed and substituted back into the prose before anything
was acted on. It answers the round-3 questions by name — I1, I2, J1, K1, L1 —
and it settles two of them against what we would have guessed.

## K1 — do not build DPM. Prune the gate instead.

Answered against building it. Our 1,054 machines are enumerated from one
alphabet and evaluated on identical series, so their returns are densely
cross-correlated (`ρ̄ → 1`), which "collapses the effective number of
independent trials (`N_eff ≪ N`) and invalidates parametric EVT thresholds".
DSR and DPM both rest on that EVT derivation. The directive is explicit:
*standardise on Romano-Wolf stepdown for confirmatory FWER control; prune
unused DPM gates from pre-registration manifests.*

We already run Romano-Wolf (`stepdown.rs`). So the action is a deletion, not a
build: **K3's worry resolves by removing the `DPM >= 0.95` gate from the
manifest spec**, not by implementing it.

## I1 — three named mechanisms for a degenerate PBO

1. **Martingale / signal-to-noise deficit.** Where returns approximate a
   martingale difference sequence, the true differential across all FSMs is
   zero; in-sample optimisation selects realisation noise and out-of-sample
   ranks centre on `ω_c = 0.50`.
2. **Cross-sectional policy degeneracy.** Under jump-dominated price action or
   coarse tick size, large subsets of machines execute identical trajectories.
   The correlation eigenspectrum collapses onto one component
   (`λ₁/Σλᵢ → 1`, `N_eff ≈ 1`) and rank assignment becomes unstable.
3. **Regime non-stationarity.** Non-overlapping feature distributions across
   blocks induce negative rank correlation between splits.

Mechanism 2 is measurable from data we already hold — that was K2, and it is
now the cheapest way to test which mechanism applies to FLOKI, JTO and PYTH.

## I2 and J1 — and where our own bench disagrees

The document says to **retain** the eight-asset finding: a degenerate PBO on a
minority "indicates that the model alphabet contains no generalizable relative
edge on those specific series, rather than invalidating findings on assets
where low PBO is robustly maintained". For reporting, J1's answer is to quote
sub-floor PBO "alongside stationary bootstrap confidence intervals".

We built that (`benches/031_pbo_interval`), and it partly supports the
partition the document tells us to retain — but not as bench 024 drew it. Of
the three dissenters, only **JTO** (0.417–0.647) clears the envelope of the
four clean generalisers. FLOKI overlaps by 0.036, borderline. PYTH overlaps
properly. **One dissents measurably, one is borderline, one does not.**

Getting there required rejecting our own first attempt. A stationary bootstrap
over windows reintroduces duplicate rows, which can land on both sides of a
CSCV split — the block-exchangeability violation the same document forbids for
overlapping windows, arriving through the resampler. It gave intervals 0.29–0.72
wide and separated nothing. Block permutation instead (contiguous blocks
shuffled, every window used exactly once) halved the widths and changed the
answer. The first result was not conservative, it was wrong — worth recording
because "wider interval" reads as "safer" and here it was not.

## L1 — CUPED brings the CLMM margin inside reach

Answered, and it changes the verdict. Using pre-trade pool volatility and order
arrival imbalance as a control variate:

```
Y_CUPED = Y − θ(X − E[X]),   θ = Cov(Y, X) / Var(X)
```

compresses variance by `1 − ρ²_{Y,X}` — typically **30–50% in high-frequency
crypto** — dropping the requirement for a +0.25 bps effect from 849 paired
cycles to **N ≈ 420–590**. The CLMM net margin of +0.10 to +0.35 bps is
therefore not permanently below our floor, which is what L1 asked.

## E3 / F1 — "edge versus hold" was ill-conditioned, and here is the algebra

New, and it supersedes our frictional-dominance reading. Decompose the selected
candidate's out-of-sample return against the population median:

```
R_{i*,OOS} = D_t + Δ_{i*}
             ↑     ↑ selection differential (+2.59 bps over median)
             └ common drift, swings −52 → +28 bps
```

Every machine shares `D_t`, so the thing we were trying to measure sits inside
a term two orders of magnitude noisier. Negative out-of-sample performance
persisted even in zero-friction simulation (−6.35 bps), which the document
reads as proof that this is **an ill-conditioned objective conflating market
beta with execution efficiency**, not frictional dominance. Our own bench had
already refuted the frictional-dominance diagnosis by test; this supplies the
mechanism.

## M2 — yes, commit the reconstructions

Instructed directly: adoption requires "automated pre-ingestion visual asset
audits, numeric continuity checks across table ranges, and committing
reconstructed, lossless research documents directly into the repository's
immutable research tree". Done — `docs/research/reconstructed/`.

## Two corroborations and one conflict

**The per-group convention, confirmed a third time.** This document states the
unpaired column *per group*: 68,380 / 91,541 / 113,210 at δ = 0.10, exactly
half of round one's 136,760 / 183,082 / 226,420 totals, and likewise at every
other effect size. The reading `power.rs` was corrected to now has three
independent sources agreeing.

**The gross-spread range, confirmed.** `+20.0 to +27.0 bps` — the upper bound
we reconstructed from prose after the table images truncated at the dash.

**Conflict on the CLMM gross spread.** Round one says 0.3 bps on SOL/USDC;
round two's table cell reads `0.2 –`; round three says `+0.8 to +1.2 bps`.
Three documents, three numbers, and the net margin (+0.10 to +0.35) is
identical in two of them — so at least one column does not add up. Unresolved,
and a live instance of J2, which is still unanswered.

## K1 and K2 — closed the same day, one with nothing to do

**K1 needed no code change.** `prereg.rs` never had a DPM field; our manifest
gates on effect size, power and alpha only. There was no `DPM >= 0.95` gate to
prune. The question resolves as "already correct, now for a stated reason".

**K2 was measured** — `benches/032_policy_degeneracy` — and it answers more
than it was asked.

It does *not* explain the dissent. λ₁/Σλ is 0.87–0.91 and `N_eff ≈ 1.2` on
**every asset**; the dissenting group averages 0.9030 against the clean group's
0.8957. Cross-sectional policy degeneracy is uniform across the corpus, so it
cannot be what distinguishes FLOKI, JTO and PYTH. That leaves round three's
other two mechanisms, and bench 031's permutation diagnostic has since produced
the first evidence for one of them (regime non-stationarity: shuffling block
order lowers PBO, more so on longer series).

What it does establish is about the search itself: **1,054 enumerated machines
have an effective count of 1.2.** `N_eff / N ≈ 0.0012`, everywhere. This is the
condition round three invoked when ruling out DSR and DPM — dense
cross-correlation collapsing the effective trial count — now measured rather
than assumed. Romano-Wolf remains valid (it resamples the joint dependence
structure and never assumed independent trials), so the multiplicity result is
untouched. What does not survive is the description: "all 1,054 enumerated
machines" reads as breadth, and the breadth is not there.

## L1 — answered: the feed exists, and it delivers 34.6%

**Superseded the section below.** `benches/038_depth_control` found that the
depth data was never gone — the Plan 001 recorder stopped, but
`data/incoming/bonk_depth.jsonl` kept 1,207 paired price/depth observations for
BONK.

On real depth, `ρ(depth_t, depth_{t+1}) = +0.588`, a **34.6% variance
reduction** — inside round three's prescribed 30–50% band and roughly 18× what
price-derived proxies managed. The binding constraint is freshness, not volume:
the reduction halves by lag 5 and is gone by lag 30, so the control variate has
to be a pre-trade quote taken within a tick or two of the fill. That is exactly
what the signed quote in the verifiable exit chain (#7h) already is.

Price cannot substitute: prior realised volatility explains **1.6%** of depth
variation. Bench 033's 1.9% ceiling was a fact about that substitution, not
about CUPED — and it follows that *simulating* depth from prices could never
have reached the band, since a deterministic function of price carries no
information price does not.

Two limits stand. It is one asset over one period, and BONK is a long-tail
CPMM — the margin worth chasing is on liquid CLMM majors, whose depth process
is tick-concentrated rather than reserve-driven. And it bounds the control
variate rather than the experiment: realised cost also carries routing and
priority-tip variance no pre-trade depth reading predicts.

## L1 — the original reading, superseded above

Measured before building: `benches/033_cuped_headroom`. CUPED's reduction is
exactly `1 − ρ²(Y, X)`, so the prescription is checkable without implementing
it. Against price-derived control variates — pre-window realised volatility and
pre-window drift, the only ones our `{t, price}` corpus supports — the mean
achievable reduction is **1.9%**, against the 30–50% round three cites. PEPE's
7.3% is the best on the board.

The variates the document names are depth-book quantities: pre-trade pool
volatility and order arrival imbalance. A price series cannot express order
arrival imbalance at all, so this bounds the proxies rather than refuting the
method — but it rules out the cheap version. CUPED on data we already hold does
not bring the +0.10 to +0.35 bps CLMM margin inside reach.

Answering L1 properly needs the depth-aware recorder back, twice over: once for
the control variate, and once because the outcome is a paired execution A/B
rather than the edge-vs-hold objective used here as a stand-in. That is a
decision with a cost, and it belongs to whoever closed Plan 001 — not a task
that can be coded around.

## I1 — all three prescribed mechanisms tested, none of them fits

`benches/034_martingale_check` closes the list.

- **Policy degeneracy** (bench 032): uniform across the corpus. Cannot
  distinguish anything.
- **Regime non-stationarity** (bench 031's diagnostic): partial support —
  shuffling block order lowers PBO, more on longer series.
- **Martingale signal-to-noise deficit** (bench 034): **contradicted.** The
  prediction is that dissenting series sit nearer the martingale values. Their
  θ_d is lower, which fits, but their lag-1 autocorrelation is *further* from
  zero, not nearer. JTO — the only asset bench 031 separates — has the largest
  positive ρ₁ in the corpus at +0.1265. It is the least martingale-like series
  we hold.

What the data shows instead is sharper than any of the three. Across eleven
assets **ρ₁ and θ_d correlate at −0.856**: the three most mean-reverting series
(PEPE −0.297, BONK −0.227, SHIB −0.147) carry the three highest signal-to-noise
ratios and all generalise cleanly, while everything at or above ρ₁ = 0 clusters
near θ_d ≈ 0.1.

That looked like a finding — the exit machines extracting mean reversion — and
it carried a cheap test. **The test refuted it.**

`benches/036_reversion_causal` sets ρ₁ directly on synthetic AR(1) series with
unconditional volatility held constant, then runs the real pipeline. If the
machines ate mean reversion, Δ would fall as φ rises. It does not: Δ moves
+0.518 → +1.980 bps across φ = −0.4 → +0.4, non-monotone, with per-arm seed
spreads of ±10 bps against arm means under 2 bps. Nothing is separated. **The
mean-reversion reading is withdrawn**, and benches 034 and 035 carry retraction
banners saying so.

What does respond is PBO, and to **|φ| rather than signed φ**: 0.359 ± 0.039 at
φ = −0.4, 0.564 ± 0.064 at φ = 0, 0.356 ± 0.039 at φ = +0.4. The extremes are
indistinguishable from each other and sit 2.7 standard errors below the centre.
Selection generalises when serial structure is present in either direction and
degenerates toward a coin flip when it is not — which is round three's
martingale mechanism after all. Bench 034 recorded it as contradicted because
it tested the signed quantity.

Our corpus returns `corr(|ρ₁|, PBO) = −0.034`, and for a while that read as a
failure to transfer. **It was not — `benches/037_reversion_power` shows the
test had 4.8% power.**

Sweeping φ across the band our assets actually occupy (|ρ₁| ≤ 0.30), the
mechanism is present but shallow: PBO falls 0.509 → 0.438, arm means
correlating at −0.918, against a per-arm standard deviation of 0.21. Drawing
20,000 pseudo-corpora at our measured |ρ₁| values, 4.8% reach significance at
n = 11 — the false-positive rate. A corpus generated *by the mechanism itself*
would usually have looked flat too.

So the flat correlation is not evidence against anything, and settling this is
a corpus question rather than an analysis one: it needs assets with stronger
autocorrelation, or many more of them.

FLOKI, JTO and PYTH remain unexplained — but the question is smaller than it
looked. Bench 031 showed only JTO separates measurably, and the synthetic arms
show near-martingale series produce PBO estimates that scatter across most of
the unit interval unaided. Three assets landing high in a group of eight whose
PBO is barely repeatable may not need a mechanism at all.

## K2 follow-through — the description is fixed

`README.md` now reads 1,054 as coverage rather than diversity, with the
effective count of 1.2 stated inline. Historical bench reports are left as
written: they are dated evidence, and editing them to match a later measurement
is the failure mode this project keeps a provenance file for.

## The attribution result, which no retraction touches

`benches/035_asset_vs_machine` measures what the tournament contributes, in the
drift-free form round three prescribes: Δ, the picked machine's edge over the
population median, selected on 60% of windows and scored on the rest.

**Δ exceeds its own detection floor on 1 of 11 assets.** Mean Δ is +3.1 bps and
positive on 8 of 11, which is consistent with a small real edge and equally
consistent with nothing — RAY's +11.2 bps sits against an MDE of 63.7. The one
asset that clears is PEPE, +11.7 against 9.1.

Bench 025 reached the same place through multiplicity correction. This states
it in bps: **the tournament's out-of-sample contribution has not been shown to
be non-zero on ten of eleven assets.**

## What is still actionable without further research

- test whether the |φ| mechanism transfers by widening the synthetic sweep to
  the |ρ₁| ≤ 0.3 range our assets actually occupy, where it should be weakest
- FLOKI and PYTH remain unexplained by any tested mechanism, and may not need
  one
