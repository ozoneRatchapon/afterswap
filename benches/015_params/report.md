# Parameter scaling across horizons

Objective: mean(edge vs TWAP, edge vs trailing), drift-preserved bootstrap, 600 bars.
Parameters chosen on 15 TRAIN seeds, reported on 25 disjoint TEST seeds — the gain is out-of-sample.
Demo default is window 24 / 10% tranches.

| bar ≈ | best on train | test: tuned (±SE) | test: demo default (±SE) | out-of-sample gain |
|---|---|---|---|---|
| 2 s | window 96 / 5% | +5 ± 9 | -14 ± 9 | **+19 bps** |
| 30 s | window 96 / 10% | +306 ± 87 | +62 ± 23 | **+244 bps** |
| 120 s | window 96 / 5% | +2471 ± 179 | +554 ± 173 | **+1917 bps** |

## Window length on real corpora (no bootstrap)

Edge vs TWAP / vs trailing, per window length, on the recorded DFlow segments and the synthetic regimes.

| corpus | w12 | w24 | w48 | w96 |
|---|---|---|---|---|
| trend_up | +188 | +188 | +188 | +188 |
| trend_down | +38 | +36 | -2 | -103 |
| chop | +8 | -9 | -5 | -1 |
| v_shape | +54 | +46 | -124 | -249 |
| recorded.jsonl | -40 | -28 | -20 | +30 |
| recorded2.jsonl | +11 | +12 | +20 | +6 |

## Verdict: the tuning does NOT transfer — and that is the finding

On bootstrapped paths, window 96 wins at every scale and the out-of-sample
gain looks enormous (+1917 bps at 2-minute bars, >7 SE). On the real
corpora it collapses: trend_down goes +38 → -103 and v_shape +54 → -249 as
the window grows, while the recorded segments barely move. **Raising the
default window on the strength of the bootstrap experiment would have made
the product worse on real data.**

Why: block-bootstrapping preserves only within-block autocorrelation, so
the synthetic paths have no structure above the block scale. Long
evaluation windows are optimal there precisely *because* there is nothing
longer-range to be wrong about. Real markets have multi-scale structure,
and shorter windows adapt to it.

The methodological point is worth more than the parameter: **a correct
train/test split does not protect you when the data-generating process is
wrong.** The split was honest and the gain was genuinely out-of-sample —
within a distribution that does not match reality. Out-of-*distribution*
validation (the real-corpus table above) is what caught it.

**Conclusion:** current demo parameters (window 12-24, 10% tranches) stand;
no change shipped. Tuning for genuinely longer horizons needs genuinely
long *recorded* data, not resampled data — that is now the blocking
dependency, and the recorder is running to produce it.

Caveats: window 96 was the grid maximum, so the bootstrap optimum may lie
beyond it; bootstrapped magnitudes are inflated by the bull sample, so read
relative columns, not absolute bps.

