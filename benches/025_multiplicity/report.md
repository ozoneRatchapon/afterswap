# Does any single machine survive multiplicity correction?

Romano–Wolf stepdown over all 1054 enumerated machines against holding, 400 bootstrap resamples, familywise α = 0.05, 120-tick windows. `MDE` is the smallest effect the sample could have detected at 80% power — a null result is only meaningful beside it.

| asset | windows | best machine mean | best t | **adjusted p** | survivors | MDE |
|---|---|---|---|---|---|---|
| BONK | 250 | +9.8 bps | 1.28 | **1.000** | 0 | 21.4 bps |
| FLOKI | 166 | +0.6 bps | 0.72 | **0.818** | 0 | 2.4 bps |
| JTO | 166 | +1.2 bps | 2.22 | **0.399** | 0 | 1.6 bps |
| JUP | 166 | +1.1 bps | 0.62 | **0.895** | 0 | 5.0 bps |
| ORCA | 166 | +1.1 bps | 1.02 | **1.000** | 0 | 3.1 bps |
| PEPE | 250 | +0.6 bps | 1.14 | **1.000** | 0 | 1.6 bps |
| PYTH | 166 | +2.3 bps | 1.01 | **1.000** | 0 | 6.3 bps |
| RAY | 166 | +0.4 bps | 0.80 | **1.000** | 0 | 1.5 bps |
| SHIB | 166 | +4.0 bps | 2.14 | **0.122** | 0 | 5.2 bps |
| SOL_USDC | 375 | +0.2 bps | 1.12 | **1.000** | 0 | 0.4 bps |
| WIF | 250 | +0.5 bps | 1.96 | **1.000** | 0 | 0.8 bps |

**Total machines surviving familywise correction across every asset: 0.**

The enumerate-and-select pipeline is now tested end to end: the search is
reproducible (G1), the browser and native engines agree byte for byte (G6), the
selection generalises rather than mining noise (bench 024, PBO 0.05–0.20), and
this test asks whether any individual member of the space has an edge that
survives having looked at a thousand candidates. Read the MDE column beside any
zero — it states what the data could have found had it been there.

