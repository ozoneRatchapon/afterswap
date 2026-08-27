# How many distinct strategies is the enumeration actually running?

Cross-sectional degeneracy of all 1054 enumerated machines on 120-tick windows. `λ₁/Σλ` is the share of correlation-matrix variance carried by the leading component; `N_eff` is the participation ratio (Σλ)²/Σλ², the effective number of independent strategies. `flat` counts machines whose performance never varies across windows. PBO figures are bench 024's.

| asset | windows | PBO | flat machines | **λ₁/Σλ** | **N_eff** | N_eff / N |
|---|---|---|---|---|---|---|
| BONK | 250 | — | 32 | 0.9059 | 1.2 | 0.0012 |
| FLOKI | 166 | — | 42 | 0.9124 | 1.2 | 0.0012 |
| JTO | 166 | — | 32 | 0.9045 | 1.2 | 0.0012 |
| JUP | 166 | — | 32 | 0.8969 | 1.2 | 0.0012 |
| ORCA | 166 | — | 32 | 0.8875 | 1.3 | 0.0012 |
| PEPE | 250 | — | 32 | 0.9104 | 1.2 | 0.0012 |
| PYTH | 166 | — | 32 | 0.8921 | 1.3 | 0.0012 |
| RAY | 166 | — | 32 | 0.8937 | 1.2 | 0.0012 |
| SHIB | 166 | — | 32 | 0.9046 | 1.2 | 0.0012 |
| SOL_USDC | 375 | — | 32 | 0.8747 | 1.3 | 0.0013 |
| WIF | 250 | — | 32 | 0.9065 | 1.2 | 0.0012 |

## It does not separate them — because it is everywhere

| group | mean λ₁/Σλ | mean N_eff | mean flat machines |
| --- | --- | --- | --- |
| dissenting (FLOKI, JTO, PYTH) | 0.9030 | 1.2 | 35 |
| clean (JUP, ORCA, RAY, SHIB) | 0.8957 | 1.2 | 32 |

The two groups are indistinguishable. Cross-sectional policy degeneracy is **not** what makes FLOKI, JTO
and PYTH behave differently — every asset in the corpus sits at the same concentration. Round three's
other two candidates, martingale signal-to-noise deficit and regime non-stationarity, are where to look
next.

## The finding this bench did not go looking for

Degeneracy is not a property of three assets. It is a property of the enumeration.

**λ₁ carries 87–91% of the correlation variance on every asset, and N_eff ≈ 1.2 out of 1,054.** The
search enumerates a thousand machines and runs, in effect, slightly more than one. `N_eff / N ≈ 0.0012`
across the entire corpus, with no meaningful spread between assets.

That reframes several earlier results rather than contradicting them:

- **It is the direct measurement behind K1.** Round three ruled out DSR and the Deflated Paired Metric
  on the grounds that dense cross-correlation collapses the effective trial count and invalidates the
  extreme-value threshold those statistics rest on. `N_eff = 1.2` is that condition, measured. Choosing
  Romano-Wolf stepdown — which resamples the joint dependence structure instead of assuming independent
  trials — was correct, and now for a stated reason rather than a cautious one.
- **It does not weaken the multiplicity result.** Romano-Wolf never assumed 1,054 independent tests, so
  "zero machines survive correction" is unaffected.
- **It does weaken how the search is described.** "All 1,054 enumerated machines" appears throughout
  these benches and reads as breadth. The breadth is not there. Whatever the 3-state alphabet expresses,
  it expresses it about one way, and the population is a thousand near-copies of a single policy.

Nothing here says the machines are identical — `flat` counts only 32–42 that never vary at all. It says
that what varies, varies together.
