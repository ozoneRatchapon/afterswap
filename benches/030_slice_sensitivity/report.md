# Does the PBO dissent survive an adequately sized slice?

CSCV/PBO over all 1054 enumerated machines, 120-tick windows, partition count swept across [6, 8, 10, 12, 16]. An external source places a floor of 25 observations per slice, below which PBO is driven toward 0.50 by sampling variance alone; cells under that floor are marked †. Bench 024 used S = 10 throughout.

| asset | windows | S=6 (20 splits) | S=8 (70 splits) | S=10 (252 splits) | S=12 (924 splits) | S=16 (12870 splits) |
|---|---|---|---|---|---|---|
| BONK | 250 | 0.250 | 0.243 | 0.202 | 0.184† | 0.211† |
| FLOKI | 166 | 0.600 | 0.543† | 0.623† | 0.602† | 0.517† |
| JTO | 166 | 0.550 | 0.543† | 0.516† | 0.601† | 0.526† |
| JUP | 166 | 0.050 | 0.200† | 0.190† | 0.214† | 0.233† |
| ORCA | 166 | 0.050 | 0.186† | 0.115† | 0.210† | 0.125† |
| PEPE | 250 | 0.150 | 0.229 | 0.198 | 0.085† | 0.093† |
| PYTH | 166 | 0.650 | 0.386† | 0.448† | 0.412† | 0.272† |
| RAY | 166 | 0.100 | 0.143† | 0.155† | 0.163† | 0.176† |
| SHIB | 166 | 0.150 | 0.114† | 0.075† | 0.281† | 0.110† |
| SOL_USDC | 375 | 0.150 | 0.157 | 0.087 | 0.074 | 0.080† |
| WIF | 250 | 0.050 | 0.086 | 0.048 | 0.036† | 0.038† |

## Movement of the three dissenting assets

| asset | PBO at S=10 (under floor) | PBO at S=6 (over floor) | change |
|---|---|---|---|
| FLOKI | 0.623 | 0.600 | -0.023 |
| JTO | 0.516 | 0.550 | +0.034 |
| PYTH | 0.448 | 0.650 | +0.202 |

## Verdict: slice size is not the explanation

The artifact hypothesis predicts the dissenters fall toward the population once
their slices clear the floor. They do not. At S = 6 — 27.7 observations per
slice, above the stated floor — FLOKI is unchanged (-0.023), and JTO (+0.034)
and PYTH (+0.202) move *away* from the population. Not one converges.

The sweep says it more strongly than the pairwise comparison does. Across all
five partition counts, FLOKI stays in 0.52-0.62, JTO in 0.52-0.60 and PYTH in
0.27-0.65, while the four other 166-window assets (JUP, ORCA, RAY, SHIB) stay
inside 0.05-0.28 throughout. The two groups never overlap, and the split
between them does not track observations per slice. Whatever separates them is
a property of those three series, not of how we partitioned them.

Read S = 6 with care: 20 splits quantises PBO to steps of 0.05, so every value
in that column is a multiple of 0.05 and single-asset moves there are coarse.
The sweep-wide separation, not the S = 6 delta, is what carries the result.

What this does **not** settle is why those three dissent. Bench 024's open
question stands, minus one candidate answer — which is the point of running it.
