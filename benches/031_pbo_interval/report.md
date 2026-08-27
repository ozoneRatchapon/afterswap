# How much of the PBO spread is estimation noise?

CSCV/PBO over all 1054 enumerated machines, 120-tick windows, 10 slices. Interval is the 2.5th-97.5th percentile of 200 block permutations (contiguous blocks of 5 windows, order shuffled, every window used exactly once), seeded deterministically. Point estimate is bench 024's figure, recomputed here.

| asset | windows | PBO | 95% interval | width | bootstrap median | point inside? |
|---|---|---|---|---|---|---|
| BONK | 250 | 0.202 | 0.032 – 0.254 | 0.222 | 0.131 | yes |
| FLOKI | 166 | 0.623 | 0.333 – 0.667 | 0.333 | 0.508 | yes |
| JTO | 166 | 0.516 | 0.417 – 0.647 | 0.230 | 0.540 | yes |
| JUP | 166 | 0.190 | 0.107 – 0.369 | 0.262 | 0.234 | yes |
| ORCA | 166 | 0.115 | 0.036 – 0.286 | 0.250 | 0.147 | yes |
| PEPE | 250 | 0.198 | 0.000 – 0.099 | 0.099 | 0.028 | **no** |
| PYTH | 166 | 0.448 | 0.278 – 0.567 | 0.290 | 0.437 | yes |
| RAY | 166 | 0.155 | 0.063 – 0.361 | 0.298 | 0.190 | yes |
| SHIB | 166 | 0.075 | 0.048 – 0.242 | 0.194 | 0.147 | yes |
| SOL_USDC | 375 | 0.087 | 0.000 – 0.091 | 0.091 | 0.020 | yes |
| WIF | 250 | 0.048 | 0.000 – 0.095 | 0.095 | 0.028 | yes |

## Partly separable — and the first design said otherwise

The low group (JUP, ORCA, RAY, SHIB — same 166 windows) spans **0.036 – 0.369** across its
own intervals. **1 of 11 assets** clear that envelope: JTO, at 0.417–0.647, does not overlap
it. FLOKI overlaps by 0.036 (0.333 against the envelope's 0.369) — technically inside, close enough to
the edge that it should not be leaned on. PYTH, at 0.278–0.567, overlaps properly and is not
distinguishable from clean generalisation.

That is a weaker claim than bench 024's and a stronger one than this bench's first version made. Read
literally: **one asset dissents measurably, one is borderline, one does not dissent at all.** Bench 024
reports three. Bench 030 established the dissent is stable under repartitioning; stability is necessary
for it to be real, and for two of the three it is still not sufficient.

Worth stating plainly because it cuts against the previous entry in this bench's own history: with a
stationary bootstrap the intervals came out 0.29–0.72 wide and nothing separated. Removing the
duplicate rows halved the widths to 0.09–0.33 and changed the answer. The first result was not
conservative — it was wrong.

## Diagnostic: shuffling block order lowers PBO, and that is a finding

1 asset(s) have a point estimate falling outside their own interval — PEPE, at 0.198 against
0.000–0.099. It is not alone in direction: every asset with 250 or more windows has a permutation
median well below its point estimate (BONK 0.131 vs 0.202, PEPE 0.028 vs 0.198, SOL_USDC 0.020 vs
0.087, WIF 0.028 vs 0.048), while the 166-window assets sit close to theirs.

If block order were uninformative the permutation median would centre on the point estimate. It does
not, and the gap grows with series length. Some of the PBO we measure is produced by the temporal
ordering of blocks rather than by overfitting — which is the signature of round three's third
mechanism, regime non-stationarity: a strategy tuned on early blocks failing on later ones. That
mechanism was listed as a candidate explanation for the dissent; this is the first evidence in our own
data that it operates at all.

Consequence for these intervals: on long series they are shifted low relative to the statistic, so the
overlap test above is conservative there. It does not affect the three 166-window assets the test is
actually about.
