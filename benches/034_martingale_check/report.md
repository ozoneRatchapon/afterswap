# Is the dissent a martingale signal-to-noise deficit?

`VR(q)` is the Lo-Mackinlay variance ratio at aggregation q, exactly 1 under a martingale. `rho_1` is lag-1 return autocorrelation, zero under a martingale. `theta_d` is the standardised paired signal-to-noise ratio of the selected machine's per-window edge over the population median across 1054 enumerated machines, zero under a true global null. PBO is bench 024's; the interval verdict is bench 031's.

| asset | PBO | 031 verdict | VR(2) | VR(5) | rho_1 | **theta_d** |
|---|---|---|---|---|---|---|
| BONK | — | clean | 0.769 | 0.559 | -0.2274 | +0.2453 |
| FLOKI | — | borderline | 1.010 | 0.854 | +0.0672 | +0.0815 |
| JTO | — | separable | 1.004 | 0.829 | +0.1265 | +0.1391 |
| JUP | — | clean | 1.043 | 0.979 | +0.0626 | +0.1010 |
| ORCA | — | clean | 1.031 | 0.925 | +0.0484 | +0.1948 |
| PEPE | — | clean | 0.688 | 0.457 | -0.2972 | +0.4145 |
| PYTH | — | not separable | 0.989 | 0.937 | +0.0183 | +0.1081 |
| RAY | — | clean | 1.016 | 0.972 | +0.0252 | +0.1325 |
| SHIB | — | clean | 0.848 | 0.611 | -0.1468 | +0.3966 |
| SOL_USDC | — | clean | 0.987 | 0.961 | +0.0299 | +0.1258 |
| WIF | — | clean | 1.021 | 0.984 | +0.0397 | +0.1483 |

## The martingale mechanism does not fit

| group | mean VR(2) | mean VR(5) | mean rho_1 | mean theta_d |
| --- | --- | --- | --- | --- |
| dissenting (FLOKI, JTO, PYTH) | 1.001 | 0.873 | +0.0707 | +0.1096 |
| clean (JUP, ORCA, RAY, SHIB) | 0.984 | 0.872 | -0.0027 | +0.2062 |

The prediction was that the dissenters sit closer to the martingale values on all three. They do not.
Their signal-to-noise is lower (+0.1096 against +0.2062), which fits — but their lag-1 autocorrelation is
*further* from zero, not nearer it. **JTO, the only asset bench 031 separates, has the largest positive
rho_1 in the corpus at +0.1265.** A martingale deficit cannot explain an asset that is the least
martingale-like of the eleven.

The grouping above is bench 024's, which bench 031 showed to be an overcount, so treat the group means
as descriptive only.

## What the data shows instead: mean reversion is what the machines eat

Across all eleven assets, rho_1 and theta_d correlate at **-0.856**. The three most mean-reverting series
— PEPE (-0.2972), BONK (-0.2274), SHIB (-0.1468) — carry the three highest signal-to-noise ratios in the
corpus, and all three are clean generalisers. Series with rho_1 at or above zero cluster at theta_d
around 0.1.

That reframes the whole question. The exit machines are not extracting a general edge that some series
happen to lack; they are extracting **mean reversion**, and they degrade toward coin-flip selection
wherever it is absent. JTO is not signal-free — it is positively autocorrelated, which is the one regime
a peak-drop exit rule is actively wrong about.

This is a hypothesis with a cheap test attached: an exit rule inverted for trending regimes should move
JTO's PBO. It is not evidence yet, and it does not explain FLOKI or PYTH, whose rho_1 sits near zero and
whose separation bench 031 rates borderline and absent respectively.

Round three's three named mechanisms are now all tested against our data: policy degeneracy is uniform
(bench 032), regime non-stationarity has partial support (bench 031's permutation diagnostic), and the
martingale deficit is contradicted here. The explanation that fits is not on the list.
