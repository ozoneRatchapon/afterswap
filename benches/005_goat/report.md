# GOAT report — AfterSwap exit engine

Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@30.

## G1 determinism — PASS

Bit-identical event stream + final value on every corpus, two runs.

## G2 floors

| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04259 | 1.04464 | 1.00677 | 1.04193 | +355.8 bps | +6.3 bps | -19.6 bps |
| trend_down | 0.99944 | 0.96365 | 0.99685 | 0.99927 | +26.0 bps | +1.7 bps | +371.4 bps |
| chop | 1.00128 | 0.99998 | 1.00139 | 1.00019 | -1.1 bps | +10.9 bps | +13.0 bps |
| v_shape | 0.99875 | 1.01269 | 0.99193 | 0.99875 | +68.7 bps | -0.0 bps | -137.7 bps |
| recorded_dflow | 0.99819 | 1.00536 | 0.99943 | 0.99819 | -12.5 bps | +0.0 bps | -71.4 bps |

**G2a vs TWAP: +87.39 bps mean — PASS** · **G2b vs random-arm: +3.78 bps mean — PASS** · vs hold is report-only (regime-dependent opportunity cost).

## Ecosystem floors (report-only)

Trailing stop 50 bps · TP ladder 10×10 bps · bracket ±50 bps.

| corpus | engine | trailing | ladder | bracket | vs trail | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04259 | 1.04464 | 1.00570 | 1.00506 | -19.6 | +366.8 | +373.4 |
| trend_down | 0.99944 | 0.99465 | 0.96365 | 0.99465 | +48.2 | +371.4 | +48.2 |
| chop | 1.00128 | 0.99998 | 1.00063 | 0.99998 | +13.0 | +6.5 | +13.0 |
| v_shape | 0.99875 | 0.99479 | 1.00574 | 0.99479 | +39.8 | -69.5 | +39.8 |
| recorded_dflow | 0.99819 | 1.00536 | 1.00432 | 1.00529 | -71.4 | -61.1 | -70.7 |

**Means: vs trailing +1.99 bps · vs TP-ladder +122.82 bps · vs bracket +80.73 bps.**

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +0.0 bps
- trend_down: +0.0 bps
- chop: +11.4 bps
- v_shape: +0.0 bps
- recorded_dflow: +0.0 bps

**Worst cap cost +0.0 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **1.004µs**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **152.917µs**. Budgets: 1 ms / 1 s — PASS.

