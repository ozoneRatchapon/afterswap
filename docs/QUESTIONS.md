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
profit does not** (+2.5…+16.8 bps in-sample → −6…+2 out of sample). Deflated
Sharpe and Romano-Wolf remain unbuilt; DSR in particular needs adapting,
since our objective is a paired bps difference rather than a Sharpe ratio.

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

**B1–B3 — self-audit.** Confirms the sensitivity sweep we already built, and
adds three we have not: `@calibrated` provenance tags enforced by an AST
literal parser, cryptographically committed **pre-registration manifests**
(SHA-256 of hypothesis + analysis plan, recorded before data access), and
**claim TTL decay** with distributional-drift (K-S) triggers.

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
