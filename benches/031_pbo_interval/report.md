# How much of the PBO spread is estimation noise?

CSCV/PBO over all 1054 enumerated machines, 120-tick windows, 10 slices. Interval is the 2.5th-97.5th percentile of 200 stationary-bootstrap resamples (Politis-Romano, geometric blocks, expected length 5 windows), seeded deterministically. Point estimate is bench 024's figure, recomputed here.

| asset | windows | PBO | 95% interval | width | bootstrap median | point inside? |
|---|---|---|---|---|---|---|
| BONK | 250 | 0.202 | 0.000 – 0.214 | 0.214 | 0.036 | yes |
| FLOKI | 166 | 0.623 | 0.000 – 0.718 | 0.718 | 0.230 | yes |
| JTO | 166 | 0.516 | 0.020 – 0.631 | 0.611 | 0.262 | yes |
| JUP | 166 | 0.190 | 0.000 – 0.548 | 0.548 | 0.123 | yes |
| ORCA | 166 | 0.115 | 0.000 – 0.460 | 0.460 | 0.071 | yes |
| PEPE | 250 | 0.198 | 0.000 – 0.127 | 0.127 | 0.012 | **no** |
| PYTH | 166 | 0.448 | 0.004 – 0.659 | 0.655 | 0.246 | yes |
| RAY | 166 | 0.155 | 0.000 – 0.560 | 0.560 | 0.103 | yes |
| SHIB | 166 | 0.075 | 0.000 – 0.290 | 0.290 | 0.067 | yes |
| SOL_USDC | 375 | 0.087 | 0.000 – 0.425 | 0.425 | 0.016 | yes |
| WIF | 250 | 0.048 | 0.000 – 0.421 | 0.421 | 0.024 | yes |

## The dissent is not separable from the population

The low group (JUP, ORCA, RAY, SHIB — same 166 windows) spans **0.000 – 0.560** across its own
intervals. **0 of 11 assets** have an interval that clears that envelope. Not one — including
FLOKI at 0.623 and SHIB at 0.075, whose intervals overlap across most of the unit interval.

Bench 024 reads three assets as dissenting and eight as generalising cleanly. At 166 windows over 252
splits, this data does not support that partition. The point estimates differ; the estimates are not
precise enough for the difference to mean anything. Bench 030 asked whether the dissent survives
repartitioning and found that it does — but a stable estimate is not the same as a distinguishable one.

## Caveat: this bootstrap is not clean, and says so

1 asset(s) have a point estimate falling **outside** their own bootstrap interval — PEPE at 0.198
against 0.000–0.127. A percentile interval that excludes its own statistic is a bias signal, not a
rounding artefact, and the mechanism is visible: resampling windows with replacement puts duplicate
rows into the matrix, and a duplicated row can land in both the training and testing half of a CSCV
split. That is precisely the block-exchangeability violation CSCV forbids — the same defect the source
raises against overlapping rolling windows, reintroduced through the back door by the resampler.

So read the widths, not the endpoints. The conclusion that survives is the weaker and more robust one:
PBO estimates at this sample size carry uncertainty of the same order as the entire range of values
being compared. A design that avoids duplication — m-out-of-n block subsampling without replacement, or
resampling at the level of splits rather than windows — is the next thing to try before any interval
here is quoted as a number.
