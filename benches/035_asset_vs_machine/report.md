# Does the tournament pay, or does the asset choice pay?

> **Retracted in part by [bench 036](../036_reversion_causal/report.md).** The asset-first hypothesis below rests on a correlation between signed rho_1 and the selection differential. A controlled experiment setting rho_1 directly did not reproduce it. The attribution result — Δ undetectable on ten of eleven assets — is unaffected and is the part of this bench that stands.

Machine picked on the first 60% of windows, scored on the rest. **Δ** is the selection differential: the picked machine's mean per-window edge over the population median, in bps — the part of the outcome the tournament is responsible for, with common drift removed. **MDE** is the smallest Δ the test partition could have detected at 80% power. `rho_1(train)` is lag-1 return autocorrelation on the training half only, so it is available before any test data is touched.

| asset | train / test windows | rho_1(train) | **Δ (bps)** | MDE (bps) | detectable? |
|---|---|---|---|---|---|
| BONK | 150 / 100 | -0.2840 | +16.041 | 25.163 | no |
| FLOKI | 99 / 67 | +0.0018 | -13.770 | 68.818 | no |
| JTO | 99 / 67 | +0.0251 | -15.931 | 44.113 | no |
| JUP | 99 / 67 | -0.0359 | -1.462 | 53.832 | no |
| ORCA | 99 / 67 | -0.0230 | +4.721 | 25.427 | no |
| PEPE | 150 / 100 | -0.4268 | +11.748 | 9.127 | **yes** |
| PYTH | 99 / 67 | -0.0286 | -9.978 | 77.851 | no |
| RAY | 99 / 67 | -0.0363 | +11.211 | 63.695 | no |
| SHIB | 99 / 67 | -0.3044 | +8.053 | 16.069 | no |
| SOL_USDC | 225 / 150 | -0.0111 | +2.400 | 7.835 | no |
| WIF | 150 / 100 | -0.0303 | +21.035 | 28.632 | no |

## Result: the tournament's contribution is undetectable on 10 of 11 assets

Mean Δ is **+3.097 bps** and **1 of 11** assets have a Δ exceeding what their own test
partition could detect. The exception is PEPE, at +11.7 bps against an MDE of 9.1 — and PEPE is also the
most mean-reverting series in the corpus, at rho_1 = −0.427 on the training half.

Everywhere else the selection differential sits under the floor, often far under: RAY's +11.2 bps looks
substantial until its MDE of 63.7 is read beside it. **The tournament's out-of-sample contribution has
not been shown to be non-zero on ten of eleven assets.** Bench 025 reached the same place through
multiplicity correction — zero machines surviving — and this states it in bps rather than in
significance, which is the more useful unit for deciding whether to run the thing.

Note what is *not* claimed. Δ is positive on 8 of 11 assets and averages +3.1 bps. That is consistent
with a small real edge the sample cannot resolve, and equally consistent with nothing. The MDE column
is what separates "we measured a small effect" from "we could not have measured this effect if it were
there", and here it is the second.

## The asset-first hypothesis: suggestive, not established

`rho_1(train)` and out-of-sample Δ correlate at **-0.513** across 11 assets. The sign is right and it
matches bench 034's −0.856 between rho_1 and in-sample signal-to-noise, but with n = 11 this is
t ≈ −1.8, p ≈ 0.11. **It does not reach significance and should not be quoted as if it did.**

What survives is a testable deployment rule rather than a finding: measure rho_1 on history, and run
the tournament only where mean reversion is present. Both benches point that way, PEPE is the single
asset where the machinery demonstrably pays and also the most mean-reverting, and the mechanism is
plausible — a peak-drop exit is a mean-reversion bet by construction. None of that is evidence at
n = 11 spot assets.

The clean way to settle it is not more assets. It is a controlled series where rho_1 is set rather than
observed.
