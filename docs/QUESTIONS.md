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

## If only three get researched

1. **A1** — because everything else in this repo is downstream of whether our
   selection protocol is sound.
2. **C1** — because it decides how the human should spend their time on the
   next project, and the evidence here says the answer is counter-intuitive.
3. **D2** — because it is the only remaining hypothesis for a real edge that
   this project has not already falsified.
