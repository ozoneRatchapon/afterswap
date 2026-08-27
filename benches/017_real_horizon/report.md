# Horizon sweep on real market structure

45000 genuine 1-minute SOL/USDC bars (~31 days), aggregated to longer bars and split into non-overlapping 200-bar windows. Engine window 24 bars, 10% tranches. Means ±SE across windows.

Data: public CEX reference prices, **not** DFlow quotes — used only to study structure across timescales.

| bar | position spans | windows | vs hold | vs TWAP | vs trailing | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| 1 min | ~2.8 h | 225 | -9 ± 5 | -4 ± 1 | -5 ± 3 | -7 ± 3 | -2 ± 3 |
| 5 min | ~14.2 h | 45 | -45 ± 29 | -2 ± 7 | +2 ± 9 | -5 ± 13 | +13 ± 9 |
| 15 min | ~42.5 h | 15 | -173 ± 95 | -19 ± 17 | -5 ± 28 | -28 ± 28 | +12 ± 26 |
| 30 min | ~85.0 h | 7 | -300 ± 228 | +33 ± 15 | +49 ± 50 | +22 ± 28 | +54 ± 46 |
| 60 min | ~170.0 h | 3 | -581 ± 485 | +13 ± 3 | +33 ± 64 | -125 ± 37 | +44 ± 58 |
