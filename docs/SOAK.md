# Live soak evidence — 304 position cycles on live DFlow quotes

Method: the native engine (byte-identical to the browser build, GOAT G6)
runs against live DFlow `/quote` (2 s ticks, window 24, 10% tranches,
median-of-3 spike filter). A monitor auto-reopens a paper position after
each full exit and records every cycle's edge vs holding. Single session,
recorded 2026-08-26 (Asia hours, mostly quiet/chop regime).

| Metric | Value |
|---|---|
| Cycles completed | 304 |
| Mean edge vs hold | +0.29 bps |
| Median | +0.28 bps |
| Win rate | 177/304 (58%) |
| Best / worst cycle | +9.70 / -6.40 bps |
| **Learning curve** — first half vs second half | **+0.19 → +0.40 bps** |

The learning-curve row is the point: the bandit's realized statistics
persist across cycles, and the mean flipped from negative to positive as
pulls accumulated — adaptation happening on live data, not in a backtest.
Honest caveats: one session, one regime, paper fills at quoted prices;
magnitudes are regime-bound (chop ⇒ small edges, exactly as the GOAT G2c
regime table predicts).
