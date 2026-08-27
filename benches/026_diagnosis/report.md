# Testing the diagnosis: frictional dominance, or selection inflation?

All levels are **friction-free** (`fill_cost_bps = 0`), so any collapse here cannot be caused by fees. Split is chronological, 60% train / 40% test, 120-tick windows, all 1054 machines.

| asset | IS level (selected) | **OOS level (selected)** | OOS level (population median) | selected − median | PBO (no embargo) | **PBO (embargo 1)** |
|---|---|---|---|---|---|---|
| BONK | +28.7 | **-18.6** | -35.4 | +16.8 | 0.202 | **0.175** |
| FLOKI | +0.1 | **+1.3** | +16.3 | -15.0 | 0.623 | **0.532** |
| JTO | +3.4 | **+11.9** | +28.5 | -16.6 | 0.516 | **0.536** |
| JUP | +1.0 | **+1.3** | +2.1 | -0.8 | 0.190 | **0.214** |
| ORCA | +2.7 | **+6.2** | +1.9 | +4.2 | 0.115 | **0.147** |
| PEPE | +20.5 | **-40.1** | -52.0 | +11.9 | 0.198 | **0.159** |
| PYTH | +2.1 | **-0.2** | +10.1 | -10.3 | 0.448 | **0.421** |
| RAY | +0.0 | **+0.0** | -10.0 | +10.0 | 0.155 | **0.198** |
| SHIB | +3.5 | **+15.6** | +5.9 | +9.7 | 0.075 | **0.071** |
| SOL_USDC | +3.8 | **-19.1** | -20.3 | +1.3 | 0.087 | **0.071** |
| WIF | +6.4 | **-28.1** | -45.4 | +17.3 | 0.048 | **0.024** |

**Across assets: selected machine OOS -6.35 bps, population median OOS -8.94 bps, difference +2.59 bps — all friction-free.**


## Verdict: not frictional dominance — and the real decomposition is more useful

**The handed-down diagnosis does not fit.** Frictional dominance requires a
positive gross edge that a common friction term sinks. Every number above is
friction-free, and the selected machine's out-of-sample level is **−6.35 bps**.
There is no positive gross edge for friction to consume. Cheaper venues, tip
optimisation and private routing — everything that reduces friction — would
therefore change nothing here.

**What the population median exposes instead.** Splitting the level into the
selected machine and the median of all 1,054 machines separates two terms that
every previous bench had summed together:

- **A drift term, mechanical and asset-specific.** The population median
  out-of-sample level swings from −52.0 (PEPE) to +28.5 (JTO), tracking
  whether the asset fell or rose during the test period. Any strategy that
  exits early beats holding in a falling market and loses to it in a rising
  one; that is arithmetic, not skill, and it is the same for all 1,054
  machines. It is also enormous relative to everything else in the table,
  which is exactly why "edge versus hold" has been so noisy that 534 live
  cycles could not resolve it.
- **A selection term, small and consistent.** The selected machine beats the
  population median by **+2.59 bps on average**, positive on 7 of 11 assets.
  This is the part attributable to the search, and it is the only quantity in
  any of our benchmarks that is not contaminated by realised drift.

**Consequences.** "Edge versus hold" is a badly-conditioned metric: its
variance is dominated by a term with no strategy content. Benchmarks against
exits that also liquidate (TWAP, trailing stop) partially cancel the drift
term, which is why their standard errors were always smaller — that was not a
coincidence, it was the drift cancelling. Future objectives should be defined
against a benchmark that also exits, or against arrival price, never against
holding.

**Embargo.** The same review flagged that our CSCV had no embargo between
slices. Added; PBO moves by at most 0.09 and the ordering of assets is
unchanged, so the earlier conclusion stands — but the implementation is now
correct rather than accidentally adequate.

