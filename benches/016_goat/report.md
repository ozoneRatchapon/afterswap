# GOAT report — AfterSwap exit engine

Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@30.

## G1 determinism — PASS

Bit-identical event stream + final value on every corpus, two runs.

## G2 floors

| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04464 | 1.04464 | 1.00677 | 1.04193 | +376.1 bps | +26.0 bps | +0.0 bps |
| trend_down | 0.99952 | 0.96365 | 0.99685 | 0.99927 | +26.8 bps | +2.5 bps | +372.2 bps |
| chop | 1.00147 | 0.99998 | 1.00139 | 1.00019 | +0.9 bps | +12.8 bps | +14.9 bps |
| v_shape | 0.99875 | 1.01269 | 0.99193 | 0.99875 | +68.7 bps | -0.0 bps | -137.7 bps |
| dflow_recorded | 0.99839 | 1.00536 | 0.99943 | 0.99928 | -10.5 bps | -8.9 bps | -69.4 bps |
| dflow_recorded2 | 1.00012 | 0.99416 | 1.00071 | 1.00012 | -6.0 bps | +0.0 bps | +60.0 bps |

**G2a vs TWAP: +76.00 bps mean — PASS** · **G2b vs random-arm: +5.40 bps mean — PASS** · vs hold is report-only (regime-dependent opportunity cost).

## Ecosystem floors (report-only)

Trailing stop 50 bps · TP ladder 10×10 bps · bracket ±50 bps.

| corpus | engine | trailing | ladder | bracket | vs trail | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04464 | 1.04464 | 1.00570 | 1.00506 | +0.0 | +387.1 | +393.8 |
| trend_down | 0.99952 | 0.99465 | 0.96365 | 0.99465 | +49.0 | +372.2 | +49.0 |
| chop | 1.00147 | 0.99998 | 1.00063 | 0.99998 | +14.9 | +8.4 | +14.9 |
| v_shape | 0.99875 | 0.99479 | 1.00574 | 0.99479 | +39.8 | -69.5 | +39.8 |
| dflow_recorded | 0.99839 | 1.00536 | 1.00432 | 1.00529 | -69.4 | -59.1 | -68.7 |
| dflow_recorded2 | 1.00012 | 0.99723 | 0.99565 | 0.99490 | +28.9 | +44.9 | +52.5 |

**Means: vs trailing +10.53 bps · vs TP-ladder +114.01 bps · vs bracket +80.20 bps.**

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +39.9 bps
- trend_down: +0.2 bps
- chop: -0.6 bps
- v_shape: +0.0 bps
- dflow_recorded: +1.4 bps
- dflow_recorded2: +0.0 bps

**Worst cap cost -0.6 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **1.068µs**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **153.75µs**. Budgets: 1 ms / 1 s — PASS.

