# Horizon sweep — does edge scale with holding horizon?

Block-bootstrapped from 1224 recorded DFlow ticks; 300 bars per run, 12 seeds per scale, engine window 24, 10% tranches.

| bar ≈ | exit horizon ≈ | vs hold | vs TWAP | vs trailing | mean |move|/bar |
|---|---|---|---|---|---|
| 2 s | ~1 min | -11.4 bps | -0.7 bps | -12.6 bps | 1.3 bps |
| 10 s | ~6 min | -89.1 bps | +9.3 bps | -61.8 bps | 3.7 bps |
| 30 s | ~20 min | -117.3 bps | +89.2 bps | +40.6 bps | 7.4 bps |
| 60 s | ~40 min | -306.7 bps | +43.1 bps | +50.6 bps | 11.4 bps |
| 120 s | ~80 min | -390.2 bps | +568.5 bps | +555.8 bps | 16.6 bps |
