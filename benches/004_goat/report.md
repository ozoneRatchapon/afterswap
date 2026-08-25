# GOAT report — AfterSwap exit engine

Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@30.

## G1 determinism — PASS

Bit-identical event stream + final value on every corpus, two runs.

## G2 floors

| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |
|---|---|---|---|---|---|---|---|
| trend_up | 1.02941 | 1.04464 | 1.00677 | 1.02601 | +224.9 bps | +33.1 bps | -145.8 bps |
| trend_down | 0.99878 | 0.96365 | 0.99685 | 0.99941 | +19.4 bps | -6.3 bps | +364.5 bps |
| chop | 1.00161 | 0.99998 | 1.00139 | 1.00152 | +2.3 bps | +0.9 bps | +16.3 bps |
| v_shape | 0.99875 | 1.01269 | 0.99193 | 0.99875 | +68.7 bps | -0.0 bps | -137.7 bps |
| recorded_dflow | 0.99791 | 1.00536 | 0.99943 | 0.99821 | -15.2 bps | -3.0 bps | -74.1 bps |

**G2a vs TWAP: +59.99 bps mean — PASS** · **G2b vs random-arm: +4.96 bps mean — PASS** · vs hold is report-only (regime-dependent opportunity cost).

## Ecosystem floors (report-only)

Trailing stop 50 bps · TP ladder 10×10 bps · bracket ±50 bps.

| corpus | engine | trailing | ladder | bracket | vs trail | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| trend_up | 1.02941 | 1.04464 | 1.00570 | 1.00506 | -145.8 | +235.7 | +242.3 |
| trend_down | 0.99878 | 0.99465 | 0.96365 | 0.99465 | +41.5 | +364.5 | +41.5 |
| chop | 1.00161 | 0.99998 | 1.00063 | 0.99998 | +16.3 | +9.8 | +16.3 |
| v_shape | 0.99875 | 0.99479 | 1.00574 | 0.99479 | +39.8 | -69.5 | +39.8 |
| recorded_dflow | 0.99791 | 1.00536 | 1.00432 | 1.00529 | -74.1 | -63.8 | -73.4 |

**Means: vs trailing -24.45 bps · vs TP-ladder +95.36 bps · vs bracket +53.29 bps.**

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +0.0 bps
- trend_down: +0.0 bps
- chop: +5.2 bps
- v_shape: +0.0 bps
- recorded_dflow: +0.0 bps

**Worst cap cost +0.0 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **1µs**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **172.708µs**. Budgets: 1 ms / 1 s — PASS.

