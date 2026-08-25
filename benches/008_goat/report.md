# GOAT report — AfterSwap exit engine

Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@30.

## G1 determinism — PASS

Bit-identical event stream + final value on every corpus, two runs.

## G2 floors

| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |
|---|---|---|---|---|---|---|---|
| trend_up | 1.03731 | 1.04464 | 1.00677 | 1.04315 | +303.4 bps | -55.9 bps | -70.1 bps |
| trend_down | 0.99888 | 0.96365 | 0.99685 | 0.99844 | +20.4 bps | +4.4 bps | +365.6 bps |
| chop | 1.00130 | 0.99998 | 1.00139 | 0.99990 | -0.9 bps | +14.0 bps | +13.2 bps |
| v_shape | 0.99862 | 1.01269 | 0.99193 | 0.99873 | +67.4 bps | -1.1 bps | -139.0 bps |
| recorded_dflow | 0.99845 | 1.00536 | 0.99943 | 0.99822 | -9.9 bps | +2.3 bps | -68.8 bps |

**G2a vs TWAP: +76.08 bps mean — PASS** · **G2b vs random-arm: -7.27 bps mean — FAIL** · vs hold is report-only (regime-dependent opportunity cost).

## Ecosystem floors (report-only)

Trailing stop 50 bps · TP ladder 10×10 bps · bracket ±50 bps.

| corpus | engine | trailing | ladder | bracket | vs trail | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| trend_up | 1.03731 | 1.04464 | 1.00570 | 1.00506 | -70.1 | +314.3 | +320.9 |
| trend_down | 0.99888 | 0.99465 | 0.96365 | 0.99465 | +42.6 | +365.6 | +42.6 |
| chop | 1.00130 | 0.99998 | 1.00063 | 0.99998 | +13.2 | +6.6 | +13.2 |
| v_shape | 0.99862 | 0.99479 | 1.00574 | 0.99479 | +38.5 | -70.8 | +38.5 |
| recorded_dflow | 0.99845 | 1.00536 | 1.00432 | 1.00529 | -68.8 | -58.4 | -68.1 |

**Means: vs trailing -8.93 bps · vs TP-ladder +111.47 bps · vs bracket +69.41 bps.**

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +0.0 bps
- trend_down: +0.0 bps
- chop: +14.8 bps
- v_shape: +0.0 bps
- recorded_dflow: +0.0 bps

**Worst cap cost +0.0 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **1.118µs**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **193.417µs**. Budgets: 1 ms / 1 s — PASS.

