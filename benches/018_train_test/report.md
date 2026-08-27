# Train/test across time, on real bars

Parameters chosen on the first 60% of each 1-minute series, scored on the last 40%. Non-overlapping 200-bar windows; means ±SE. Public CEX reference history, not DFlow quotes.

| asset | test windows | best on train | test vs TWAP | test vs trailing | test vs hold | baseline (24/10%) vs TWAP |
|---|---|---|---|---|---|---|
| SOL/USDC | 90 | w12 / 5% | +0 ± 3 | -4 ± 6 | -19 ± 10 | -6 ± 3 |
| BONK | 60 | w24 / 10% | +7 ± 8 | +34 ± 10 | -2 ± 30 | +7 ± 8 |
| WIF | 60 | w12 / 25% | +4 ± 16 | +4 ± 21 | -40 ± 30 | +9 ± 15 |
| PEPE | 60 | w24 / 10% | -8 ± 9 | +26 ± 11 | -41 ± 28 | -8 ± 9 |
