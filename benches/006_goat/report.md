# GOAT report — AfterSwap exit engine

Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@30.

## G1 determinism — PASS

Bit-identical event stream + final value on every corpus, two runs.

## G2 floors

| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04216 | 1.04464 | 1.00677 | 1.04211 | +351.5 bps | +0.5 bps | -23.8 bps |
| trend_down | 0.99894 | 0.96365 | 0.99685 | 0.99927 | +21.0 bps | -3.4 bps | +366.2 bps |
| chop | 1.00059 | 0.99998 | 1.00139 | 1.00011 | -8.0 bps | +4.7 bps | +6.1 bps |
| v_shape | 0.99875 | 1.01269 | 0.99193 | 0.99875 | +68.7 bps | -0.0 bps | -137.7 bps |
| recorded_dflow | 0.99828 | 1.00536 | 0.99943 | 0.99822 | -11.6 bps | +0.6 bps | -70.4 bps |

**G2a vs TWAP: +84.32 bps mean — PASS** · **G2b vs random-arm: +0.48 bps mean — PASS** · vs hold is report-only (regime-dependent opportunity cost).

## Ecosystem floors (report-only)

Trailing stop 50 bps · TP ladder 10×10 bps · bracket ±50 bps.

| corpus | engine | trailing | ladder | bracket | vs trail | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04216 | 1.04464 | 1.00570 | 1.00506 | -23.8 | +362.5 | +369.1 |
| trend_down | 0.99894 | 0.99465 | 0.96365 | 0.99465 | +43.1 | +366.2 | +43.1 |
| chop | 1.00059 | 0.99998 | 1.00063 | 0.99998 | +6.1 | -0.4 | +6.1 |
| v_shape | 0.99875 | 0.99479 | 1.00574 | 0.99479 | +39.8 | -69.5 | +39.8 |
| recorded_dflow | 0.99828 | 1.00536 | 1.00432 | 1.00529 | -70.4 | -60.1 | -69.8 |

**Means: vs trailing -1.05 bps · vs TP-ladder +119.71 bps · vs bracket +77.65 bps.**

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +0.0 bps
- trend_down: +0.0 bps
- chop: +5.1 bps
- v_shape: +0.0 bps
- recorded_dflow: +0.0 bps

**Worst cap cost +0.0 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **1.527µs**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **281.667µs**. Budgets: 1 ms / 1 s — PASS.

