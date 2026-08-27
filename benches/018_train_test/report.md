# Train/test across time, on real bars

Parameters chosen on the first 60% of each 1-minute series, scored on the last 40%. Non-overlapping 200-bar windows; means ±SE. Public CEX reference history, not DFlow quotes.

| asset | test windows | best on train | test vs TWAP | test vs trailing | test vs hold | baseline (24/10%) vs TWAP |
|---|---|---|---|---|---|---|
| BONK | 60 | w24 / 10% | +7 ± 8 | +34 ± 10 | -2 ± 30 | +7 ± 8 |
| FLOKI | 40 | w48 / 10% | -20 ± 18 | -13 ± 22 | -12 ± 29 | -8 ± 8 |
| JTO | 40 | w24 / 10% | -2 ± 9 | -11 ± 10 | +28 ± 34 | -2 ± 9 |
| JUP | 40 | w48 / 5% | +0 ± 24 | +5 ± 25 | +4 ± 22 | -20 ± 15 |
| ORCA | 40 | w48 / 5% | +17 ± 13 | -6 ± 11 | +34 ± 22 | -7 ± 10 |
| PEPE | 60 | w24 / 10% | -8 ± 9 | +26 ± 11 | -41 ± 28 | -8 ± 9 |
| PYTH | 40 | w48 / 5% | +49 ± 33 | +68 ± 39 | +39 ± 24 | -20 ± 16 |
| RAY | 40 | w48 / 25% | -12 ± 22 | -17 ± 24 | -17 ± 24 | -7 ± 10 |
| SHIB | 40 | w48 / 25% | +21 ± 23 | +29 ± 26 | +35 ± 28 | -14 ± 11 |
| SOL_USDC | 90 | w12 / 5% | +0 ± 3 | -4 ± 6 | -19 ± 10 | -6 ± 3 |
| WIF | 60 | w12 / 25% | +4 ± 16 | +4 ± 21 | -40 ± 30 | +9 ± 15 |

## Across 11 assets (SE over assets, not windows)

- vs TWAP: **+5.3 ± 5.7 bps**
- vs trailing stop: **+10.5 ± 7.9 bps**

Each asset contributes one number, so this asks whether the result survives
asset selection rather than whether one asset's windows were lucky.

