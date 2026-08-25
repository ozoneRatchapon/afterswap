# GOAT report — AfterSwap exit engine

Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@30.

## G1 determinism — PASS

Bit-identical event stream + final value on every corpus, two runs.

## G2 floors

| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04216 | 1.04464 | 1.00677 | 1.04022 | +351.5 bps | +18.6 bps | -23.8 bps |
| trend_down | 0.99891 | 0.96365 | 0.99685 | 0.99903 | +20.7 bps | -1.2 bps | +365.9 bps |
| chop | 1.00130 | 0.99998 | 1.00139 | 0.99990 | -0.9 bps | +14.0 bps | +13.2 bps |
| v_shape | 0.99875 | 1.01269 | 0.99193 | 0.99875 | +68.7 bps | -0.0 bps | -137.7 bps |
| recorded_dflow | 0.99791 | 1.00536 | 0.99943 | 0.99820 | -15.2 bps | -2.9 bps | -74.1 bps |

**G2a vs TWAP: +84.94 bps mean — PASS** · **G2b vs random-arm: +5.68 bps mean — PASS** · vs hold is report-only (regime-dependent opportunity cost).

## Ecosystem floors (report-only)

Trailing stop 50 bps · TP ladder 10×10 bps · bracket ±50 bps.

| corpus | engine | trailing | ladder | bracket | vs trail | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04216 | 1.04464 | 1.00570 | 1.00506 | -23.8 | +362.5 | +369.1 |
| trend_down | 0.99891 | 0.99465 | 0.96365 | 0.99465 | +42.9 | +365.9 | +42.9 |
| chop | 1.00130 | 0.99998 | 1.00063 | 0.99998 | +13.2 | +6.6 | +13.2 |
| v_shape | 0.99875 | 0.99479 | 1.00574 | 0.99479 | +39.8 | -69.5 | +39.8 |
| recorded_dflow | 0.99791 | 1.00536 | 1.00432 | 1.00529 | -74.1 | -63.8 | -73.4 |

**Means: vs trailing -0.42 bps · vs TP-ladder +120.35 bps · vs bracket +78.29 bps.**

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +0.0 bps
- trend_down: +0.0 bps
- chop: +14.8 bps
- v_shape: +0.0 bps
- recorded_dflow: +0.0 bps

**Worst cap cost +0.0 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **1.066µs**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **189.458µs**. Budgets: 1 ms / 1 s — PASS.

