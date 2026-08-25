# GOAT report — AfterSwap exit engine

Config: window 12/6, 3-state FSMs, tranche 10%, 24-arm cap, open@30.

## G1 determinism — PASS

Bit-identical event stream + final value on every corpus, two runs.

## G2 floors

| corpus | engine | hold | TWAP | random(8) | vs TWAP | vs random | vs hold |
|---|---|---|---|---|---|---|---|
| trend_up | 1.02941 | 1.04464 | 1.00677 | 1.02488 | +224.9 bps | +44.2 bps | -145.8 bps |
| trend_down | 0.99878 | 0.96365 | 0.99685 | 0.99942 | +19.4 bps | -6.4 bps | +364.5 bps |
| chop | 1.00205 | 0.99998 | 1.00139 | 1.00166 | +6.6 bps | +3.9 bps | +20.7 bps |
| v_shape | 0.99875 | 1.01269 | 0.99193 | 0.99875 | +68.7 bps | -0.0 bps | -137.7 bps |
| recorded_dflow | 0.99791 | 1.00536 | 0.99943 | 0.99821 | -15.2 bps | -3.0 bps | -74.1 bps |

**G2a vs TWAP: +60.87 bps mean — PASS** · **G2b vs random-arm: +7.74 bps mean — PASS** · vs hold is report-only (regime-dependent opportunity cost).

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +0.0 bps
- trend_down: +0.0 bps
- chop: +9.6 bps
- v_shape: +0.0 bps
- recorded_dflow: +0.0 bps

**Worst cap cost +0.0 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **1.156µs**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **197.417µs**. Budgets: 1 ms / 1 s — PASS.

