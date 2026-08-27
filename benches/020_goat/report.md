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
| dflow_recorded | 0.99819 | 1.00536 | 0.99943 | 0.99819 | -12.5 bps | +0.0 bps | -71.4 bps |
| dflow_recorded2 | 1.00015 | 0.99416 | 1.00071 | 1.00014 | -5.6 bps | +0.1 bps | +60.3 bps |

**G2a vs TWAP: +75.73 bps mean — PASS** · **G2b vs random-arm: +6.90 bps mean — PASS** · vs hold is report-only (regime-dependent opportunity cost).

## Ecosystem floors (report-only)

Trailing stop 50 bps · TP ladder 10×10 bps · bracket ±50 bps.

| corpus | engine | trailing | ladder | bracket | vs trail | vs ladder | vs bracket |
|---|---|---|---|---|---|---|---|
| trend_up | 1.04464 | 1.04464 | 1.00570 | 1.00506 | +0.0 | +387.1 | +393.8 |
| trend_down | 0.99952 | 0.99465 | 0.96365 | 0.99465 | +49.0 | +372.2 | +49.0 |
| chop | 1.00147 | 0.99998 | 1.00063 | 0.99998 | +14.9 | +8.4 | +14.9 |
| v_shape | 0.99875 | 0.99479 | 1.00574 | 0.99479 | +39.8 | -69.5 | +39.8 |
| dflow_recorded | 0.99819 | 1.00536 | 1.00432 | 1.00529 | -71.4 | -61.1 | -70.7 |
| dflow_recorded2 | 1.00015 | 0.99723 | 0.99565 | 0.99490 | +29.3 | +45.2 | +52.8 |

**All corpora: vs trailing +10.26 · vs TP-ladder +113.74 · vs bracket +79.93 bps.**

**Recorded DFlow corpora only (2): vs trailing -21.05 · vs TP-ladder -7.92 · vs bracket -8.94 bps.**

**Synthetic regimes only (4): vs trailing +25.91 · vs TP-ladder +174.57 · vs bracket +124.36 bps.**

The headline mean is produced by the synthetic regimes, which are
hand-specified and far cleaner than real price action. On the recorded
DFlow data the engine loses to these floors. Treat the all-corpora row as
an upper bound, not a result — see `benches/017_real_horizon` for the
larger real-data test.

## G3 arm-cap ablation (24 vs uncapped)

- trend_up: +39.9 bps
- trend_down: +0.2 bps
- chop: -0.6 bps
- v_shape: +0.0 bps
- dflow_recorded: +0.0 bps
- dflow_recorded2: +0.0 bps

**Worst cap cost -0.6 bps — PASS** (budget −10 bps).

## G4 latency (release)

Mean on_tick **778ns**, worst tick (bootstrap tournament incl. 1,054-FSM enumeration) **124.167µs**. Budgets: 1 ms / 1 s — PASS.

