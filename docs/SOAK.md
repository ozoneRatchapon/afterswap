# Live soak — 535 position cycles on live DFlow quotes

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

## What this does NOT invalidate

The GOAT bench claims (vs TWAP, trailing stop, TP-ladder, TP/SL bracket)
are *paired* comparisons: every strategy is replayed on the identical
price path, so path noise cancels. That is a much lower-variance
measurement than this soak's unpaired absolute edge vs holding, and
those results reproduce deterministically with one command
(latest report for the shipped default: `benches/039_goat/report.md`;
this soak was run against the then-current `012_goat`). This file
measures a different, noisier quantity — and says so.
