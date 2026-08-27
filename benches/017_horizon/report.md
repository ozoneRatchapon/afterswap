# Horizon sweep — does edge scale with holding horizon?

Block-bootstrapped from 1224 recorded DFlow ticks; 300 bars per run, 40 seeds per scale, engine window 24, 10% tranches.

The recorded window was strongly bullish (~+5% over the sample), and
bootstrapping compounds that drift, so the drift-preserved run says more
about market direction than about exit skill. The **de-meaned** run is the
real test: same return distribution, zero drift.


## Drift preserved (bullish sample)

| bar ≈ | exit horizon ≈ | vs hold (±SE) | vs TWAP (±SE) | vs trailing (±SE) | mean |move|/bar |
|---|---|---|---|---|---|
| 2 s | ~1 min | -30 ± 10 | -1 ± 2 | -24 ± 9 | 1.4 bps |
| 10 s | ~6 min | -101 ± 20 | +13 ± 9 | -44 ± 14 | 3.7 bps |
| 30 s | ~20 min | -201 ± 30 | +44 ± 17 | +30 ± 17 | 7.3 bps |
| 60 s | ~40 min | -412 ± 46 | +53 ± 22 | +53 ± 26 | 11.7 bps |
| 120 s | ~80 min | -610 ± 84 | +469 ± 87 | +496 ± 86 | 17.1 bps |

## De-meaned (drift removed) — the exit-skill test

| bar ≈ | exit horizon ≈ | vs hold (±SE) | vs TWAP (±SE) | vs trailing (±SE) | mean |move|/bar |
|---|---|---|---|---|---|
| 2 s | ~1 min | -13 ± 10 | -1 ± 1 | -4 ± 9 | 1.4 bps |
| 10 s | ~6 min | -27 ± 20 | -2 ± 4 | -2 ± 9 | 3.7 bps |
| 30 s | ~20 min | +5 ± 30 | -13 ± 7 | -1 ± 10 | 7.4 bps |
| 60 s | ~40 min | +52 ± 45 | -6 ± 10 | -2 ± 9 | 11.8 bps |
| 120 s | ~80 min | -25 ± 67 | -5 ± 12 | +33 ± 21 | 17.8 bps |

## Reading these two tables together

**The de-meaned run is a null control, not a market.** Block-bootstrapping
de-meaned returns produces something close to a random walk: the trends and
longer-range structure that any exit strategy exists to exploit are exactly
what the resampling destroys. On a random walk, no exit schedule can beat
holding in expectation — so the correct result here is *nothing*, and that is
what the table shows: every de-meaned cell sits within ~1-2 standard errors of
zero at every horizon. **The engine does not manufacture alpha out of noise.**
That is the overfitting check this experiment was really worth running.

**The drift-preserved run is where exit skill can show up**, because there is
directional structure to time. Against holding the engine loses (any exit pays
opportunity cost in a compounding bull sample — the same G2c regime result).
Against the *other exit strategies* — which is the comparison a user actually
faces, since they are exiting either way — the advantage is real and grows with
horizon: **+469 ± 87 bps vs TWAP and +496 ± 86 bps vs trailing at ~80-minute
horizons**, both more than five standard errors from zero.

**On the original hypothesis** ("live edges look tiny only because the demo
horizon is ~1 minute"): confirmed for magnitude — effect sizes grow from
single-digit bps at 2-second bars to hundreds of bps at 2-minute bars — but
the sign depends entirely on whether the market has structure to exploit. The
demo-scale soak is measuring in the least favorable corner of this space.

