# Does setting the autocorrelation move the edge?

Synthetic AR(1) log returns, 20 seeds per arm, 200 windows of 120 ticks. Unconditional volatility is held at 8 bps per tick across every arm by rescaling the innovation variance, so `phi` is the only thing that varies. Machine picked on the first 60% of windows and scored on the rest, over all 1054 enumerated machines — the same pipeline as bench 035. **Δ** is the selection differential over the population median; **PBO** is CSCV at 10 slices on the full series.

| phi | realised rho_1 | Δ (bps) | Δ spread | **PBO** | PBO spread | PBO std err |
|---|---|---|---|---|---|---|
| -0.4 | -0.3982 | +0.518 | -5.8 … +4.4 | 0.359 | 0.131 … 0.655 | ±0.039 |
| -0.2 | -0.1976 | -0.180 | -9.3 … +9.5 | 0.552 | 0.095 … 0.905 | ±0.055 |
| +0.0 | +0.0025 | -0.365 | -8.7 … +12.9 | 0.564 | 0.063 … 0.976 | ±0.064 |
| +0.2 | +0.2022 | +1.590 | -6.0 … +16.7 | 0.489 | 0.052 … 0.933 | ±0.054 |
| +0.4 | +0.4016 | +1.980 | -8.2 … +13.0 | 0.356 | 0.016 … 0.690 | ±0.039 |

## Result: the mean-reversion hypothesis is refuted

From phi = -0.4 to phi = +0.4, Δ moves **+0.518 → +1.980 bps**. Δ monotone decreasing across all five
arms: **no**. The prediction was a monotone fall. Δ does not fall — it drifts slightly upward, and every
arm's seed spread is ±10 bps against arm means under 2 bps, so nothing here is separated in Δ at all.

**Benches 034 and 035 do not survive this.** Their case was a correlation between signed rho_1 and the
selection differential — −0.856 in-sample, −0.513 out-of-sample, p ≈ 0.11 — and setting rho_1 directly
does not reproduce it. The 3-state exit alphabet is not a mean-reversion detector. That reading is
withdrawn.

## What did respond: PBO, to the magnitude of phi rather than its sign

PBO moves **0.359 → 0.356** end to end, which understates it, because the response is not monotone. It is
a hump: **0.359 ± 0.039 at phi = −0.4, 0.564 ± 0.064 at phi = 0, 0.356 ± 0.039 at phi = +0.4.** The two
extremes are indistinguishable from each other and both sit about 0.2 below the centre — a gap of 2.7
standard errors.

Selection generalises when returns carry serial structure of **either** sign, and degenerates toward a
coin flip when they do not. Dispersion moves the same way: the seed-to-seed spread at phi = 0 is
roughly 1.6x that at |phi| = 0.4, so near-martingale series produce not just a worse PBO but a less
repeatable one.

That is round three's first mechanism — the martingale signal-to-noise deficit — which bench 034
recorded as contradicted. Bench 034 tested signed rho_1. Against |rho_1| under controlled conditions,
the mechanism holds.

## But the real corpus does not confirm it, and that gap is the finding

Across our eleven assets, **corr(|rho_1|, PBO) = −0.034**. Flat. The grouping is suggestive — the three
assets with |rho_1| >= 0.13 (BONK 0.202, PEPE 0.198, SHIB 0.075) all sit low and tight, while the eight
below 0.13 scatter from 0.048 to 0.623 — and that pattern is what the synthetic arms predict. With
three assets in the first group it is also worth nothing.

So the position is: **under control, |phi| moves PBO; in our data, it does not measurably.** Either the
effect is swamped at the |rho_1| <= 0.30 our assets actually exhibit, or an AR(1) process is too clean a
model of a real return series for the result to transfer. Both are testable and neither is tested.

## Where this leaves I1

Three mechanisms were named. Policy degeneracy is uniform across the corpus and explains nothing
(bench 032). Mean reversion — our own addition, not on the list — is refuted here. Regime
non-stationarity has one piece of support (bench 031's permutation diagnostic) and no direct test. The
martingale deficit works in simulation and not in our data.

FLOKI, JTO and PYTH remain unexplained. What has changed is that the question is smaller than it looked:
bench 031 showed only JTO separates measurably, and the synthetic arms show that near-martingale series
produce PBO estimates that scatter across most of the unit interval on their own. Three assets landing
high in a group of eight whose PBO is barely repeatable may not require a mechanism.
