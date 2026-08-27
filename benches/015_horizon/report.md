# Horizon sweep — does edge scale with holding horizon?

Block-bootstrapped from 1224 recorded DFlow ticks; 300 bars per run, 12 seeds per scale, engine window 24, 10% tranches.

The recorded window was strongly bullish (~+5% over the sample), and
bootstrapping compounds that drift, so the drift-preserved run says more
about market direction than about exit skill. The **de-meaned** run is the
real test: same return distribution, zero drift.


## Drift preserved (bullish sample)

| bar ≈ | exit horizon ≈ | vs hold | vs TWAP | vs trailing | mean |move|/bar |
|---|---|---|---|---|---|
| 2 s | ~1 min | -11.4 bps | -0.7 bps | -12.6 bps | 1.3 bps |
| 10 s | ~6 min | -89.1 bps | +9.3 bps | -61.8 bps | 3.7 bps |
| 30 s | ~20 min | -117.3 bps | +89.2 bps | +40.6 bps | 7.4 bps |
| 60 s | ~40 min | -306.7 bps | +43.1 bps | +50.6 bps | 11.4 bps |
| 120 s | ~80 min | -390.2 bps | +568.5 bps | +555.8 bps | 16.6 bps |

## De-meaned (drift removed) — the exit-skill test

| bar ≈ | exit horizon ≈ | vs hold | vs TWAP | vs trailing | mean |move|/bar |
|---|---|---|---|---|---|
| 2 s | ~1 min | +6.4 bps | -0.6 bps | +1.6 bps | 1.3 bps |
| 10 s | ~6 min | -19.8 bps | -9.8 bps | -29.6 bps | 3.8 bps |
| 30 s | ~20 min | +45.3 bps | -14.1 bps | -5.3 bps | 7.4 bps |
| 60 s | ~40 min | +144.8 bps | -33.0 bps | -16.3 bps | 11.5 bps |
| 120 s | ~80 min | +94.7 bps | -26.2 bps | +41.1 bps | 17.4 bps |
