# Live soaks — 535 SOL/USDC cycles, then 253 paired BONK/USDC cycles

Two runs on live DFlow quotes. The SOL/USDC run is first; the pre-registered
BONK/USDC paired run that closes it out is at the bottom of this file.

**Headline, stated honestly: this soak did NOT establish a statistically
significant live edge.** The point estimate is slightly positive; the
noise is far larger. Numbers below are what the run actually produced,
including the parts that don't flatter the product.

## Method

The native engine (byte-identical to the browser build, GOAT G6) runs
against live DFlow `/quote` — 2 s ticks, 24-tick windows, 10% tranches,
median-of-3 spike filter. A monitor auto-reopens a paper position after
each full exit and records every cycle's edge vs holding. Single
continuous session, 2026-08-26/27, SOL/USDC.

## Results

| Metric | Value |
|---|---|
| Cycles | 535 |
| Mean edge vs hold | **+0.10 bps** (SE 0.29, t = +0.36) |
| Median | +0.21 bps |
| Win rate | 297/535 (55.5%) |
| Best / worst cycle | +72.94 / -68.84 bps |

**t = +0.36 is not significant.** With per-cycle standard deviation
of 6.6 bps, separating a sub-1 bps effect from zero needs far more
cycles than one session provides. Reporting this as "the robots beat
holding" would be false.

## Two regimes in one session

The session split cleanly when SOL rallied ~686 bps:

| Phase | Cycles | Mean | Note |
|---|---|---|---|
| Chop | 510 | +0.17 bps (SE 0.09, t = +1.82) | thin positive tilt, still not significant |
| Rally | 25 | -1.28 bps | dispersion explodes: +72.9 / -68.8 |

The rally is the honest weak spot and it matches the GOAT G2c regime
table exactly: an exit product pays opportunity cost when price runs
away. What the engine did well there was *adapt* — the bandit moved the
seat from sell-heavy machines (Calm Wombat, outputs [1,1,1,0]) to a
4-state holder bred by evolution (Humble Stoat, outputs [1,0,0,1],
realized +9.2 bps/pull) within ~10 minutes of the regime change.
Adaptation is observable; profit from it, on this sample, is not proven.

## Retraction

An earlier version of this file (304 cycles) reported a
"negative-to-positive learning curve" from a first-half/second-half
split. **That did not survive the longer run** — over 510 chop
cycles the same split reads +0.25 → +0.09.
The original trend was noise interpreted as signal. It is retracted here
rather than quietly edited away.

## What changed after this run (v2.6)

Two fixes went in because of these numbers: **off-policy credit**
(every arm learns from every window, not just the seated one — bench 013
improved all floors) and **paired online evaluation**
(`--paired`): reference exits now run from the same entry on the same
ticks, so live comparisons cancel path noise the way the bench does. The
next soak reports per-floor means with t-values rather than a single
noisy absolute edge.

## BONK/USDC paired soak — 253 cycles, 2026-08-28

**Headline: a clean null, and this time a well-powered one.** The engine did
not beat trailing stops on live BONK/USDC. Unlike the SOL run above, that null
is informative rather than merely noisy — see the power note below.

This is the run the previous section promised ("the next soak reports per-floor
means with t-values"). It was **pre-registered in full before a single cycle was
collected** (`.plans/001_execution_edge.md`): primary endpoint, secondary list,
stopping rule and analysis script were all fixed in advance, because the market
the BONK claim is about had never been soaked.

### Method

Same engine and tick cadence as above, but **paired**: every reference exit runs
from the same entry on the same ticks, so path noise cancels the way it does in
the bench. Live BONK/USDC quotes, paper mode, zero capital.

**Stopping rule, fixed in advance:** stop at 300 completed cycles or 4,500
ticks, whichever comes first. **The tick cap bound first** — the run ended at
**4,461 ticks with 253 completed cycles** (median 10 ticks per cycle). So the
sample is 253, not the 300 the upper case would have given. That is the rule
working as written, not a run cut short, and no extension was launched.

### Results

| endpoint | mean | SE | t | p | win rate |
|---|---|---|---|---|---|
| **PRIMARY — vs trailing** | **-0.02 bps** | 0.45 | -0.05 | **0.9566** | 145/253 |
| vs hold | -0.11 | 0.37 | -0.29 | 0.7709 | 144/253 |
| vs TWAP | -0.10 | 0.41 | -0.25 | 0.8017 | 144/253 |
| vs ladder | -0.49 | 0.37 | -1.32 | 0.1884 | 142/253 |
| vs bracket | -0.54 | 0.44 | -1.21 | 0.2293 | 143/253 |

The primary endpoint is **-0.02 bps (t = -0.05, p = 0.9566 on 252 df)** — as
close to exactly zero as this measurement can resolve. **Secondary rows carry no
claim**: they are reported for completeness and are *not* corrected for
multiplicity. Every point estimate is negative, which is worth stating plainly,
but all five sit within ~1.3 SE of zero and none should be read as a negative
finding either.

### Why this null is worth more than the SOL null

**MDE at 80% power is 1.3 bps.** Bench 018 measured BONK at **+34 ± 10 bps vs
trailing**. An effect of that size is roughly **26x the smallest effect this run
could detect** — it would have been unmissable. So this is not the usual
underpowered shrug where the honest summary is "we cannot tell"; it is a genuine
**failure to reproduce** the specific claim, in the specific market the claim was
about, on the specific comparison the claim was against.

**What it does not establish.** Bench 018 was chronological train/test on
historical 1-minute bars; this is live paper trading with paired references over
a single ~2.5-hour session. The conditions differ, so this does not show the
bench arithmetic was wrong — it shows the claim does not survive contact with
the live market here. One session also cannot speak to other regimes.

**How this lands against what the repo already says:** README and ROADMAP had
already retracted BONK +34 on statistical grounds alone — two significant assets
out of four is what selection looks like when you stop at four. This run is an
*independent, live, pre-registered* test of that retracted claim, and it agrees
with the retraction. The retraction was made before this evidence existed and is
not revised by it.

### Disclosure

The pre-registration records that the paired file was not read during the run —
not a row, not a summary. That held for its contents, with one narrow exception
to log honestly: near the end of the run a `wc -l` on the paired file revealed
the **cycle count** (245 at the time). It exposed no outcome data, and the
stopping rule is enforced by the soak process rather than by the observer, so it
could not have induced optional stopping. It is recorded here because a
disclosure record that quietly omits its own exceptions is worthless.

Two analysis-script faults were also found and fixed **while blind to the data**,
before the run finished — an MDE overstated by 40% and a significance test that
assumed large n instead of computing the exact Student-t p. Both were arithmetic
corrections; the endpoints, the reported order and the stopping rule were
untouched. Details in `.plans/001_execution_edge.md`.

Raw report: `reports/bonk_soak.txt`. Raw cycles:
`data/incoming/bonk_soak_paired.jsonl`.

## What this does NOT invalidate

The GOAT bench claims (vs TWAP, trailing stop, TP-ladder, TP/SL bracket)
are *paired* comparisons: every strategy is replayed on the identical
price path, so path noise cancels. That is a much lower-variance
measurement than the **SOL** soak's unpaired absolute edge vs holding, and
those results reproduce deterministically with one command
(latest report for the shipped default: `benches/039_goat/report.md`; the
SOL soak was run against the then-current `012_goat`). That run measures a
different, noisier quantity — and says so.

**This paragraph does not extend to the BONK run above.** That one *was*
paired, against the same four references, so it does not differ from the
bench in variance discipline — it differs in being live rather than
recorded. Its null therefore cannot be waved off as "noisier measurement":
the honest reading is the one given in its own section — a genuine failure
to reproduce bench 018's BONK claim on live quotes, with the recorded-corpus
GOAT results left standing.
