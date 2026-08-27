# Is the search finding structure, or mining noise?

CSCV/PBO over all 1054 enumerated machines, 120-tick windows, 10 slices (252 splits per asset). PBO is the fraction of splits where the in-sample winner lands below the out-of-sample median: 0 = selection generalises, 0.5 = coin flip, >0.5 = anti-predictive. Calibrated on synthetic noise at 0.48–0.51 (see `tests/pbo.rs`).

| asset | windows | **PBO** | mean OOS rank of IS winner | IS perf | OOS perf |
|---|---|---|---|---|---|
| BONK | 250 | **0.202** | 0.694 | +13.6 bps | +0.1 bps |
| FLOKI | 166 | **0.623** | 0.353 | +6.7 bps | -11.1 bps |
| JTO | 166 | **0.516** | 0.455 | +16.8 bps | +2.1 bps |
| JUP | 166 | **0.190** | 0.696 | +5.6 bps | -6.4 bps |
| ORCA | 166 | **0.115** | 0.784 | +7.5 bps | -1.1 bps |
| PEPE | 250 | **0.198** | 0.711 | +6.8 bps | -5.7 bps |
| PYTH | 166 | **0.448** | 0.513 | +10.4 bps | -5.4 bps |
| RAY | 166 | **0.155** | 0.738 | +7.3 bps | -6.1 bps |
| SHIB | 166 | **0.075** | 0.857 | +11.2 bps | +2.6 bps |
| SOL_USDC | 375 | **0.087** | 0.815 | +2.5 bps | -2.4 bps |
| WIF | 250 | **0.048** | 0.879 | +3.0 bps | -5.1 bps |

## What this separates that nothing else did

Two different failures were being conflated by every earlier bench, and CSCV
splits them apart:

**Selection is sound.** PBO is **low on 7 of 11 assets** (0.05–0.20), and the
in-sample winner's mean out-of-sample rank sits at **0.70–0.88** — well above
the 0.5 a coin flip would give, against a procedure calibrated at 0.48–0.51 on
synthetic noise. The tournament really does identify machines that are better
than their peers, and that ranking persists on data it never saw. Our
enumerate-and-select machinery is not mining noise.

**Profitability is absent.** The same table shows in-sample performance of
+2.5 to +16.8 bps collapsing to roughly **−6 to +2 bps out of sample**. The
level does not survive even though the ordering does.

Put together: *we can reliably pick the best machine; the best machine is not
profitable.* That is a much sharper statement than "no edge", and it points
somewhere specific — the problem is not our search, our statistics or our
selection, it is that the strategy space itself contains no profitable member
at these horizons. Enriching the alphabet or enlarging the population cannot
fix that; only a different objective (execution cost, risk control) or a
different market can.

Three assets dissent — FLOKI (0.62), JTO (0.52), PYTH (0.45) — where selection
is a coin flip or worse. We still do not know why, but we know one thing it is
not: `benches/030_slice_sensitivity` sweeps the partition count from 6 to 16
and the dissent holds at every setting, including partitions large enough to
clear the 25-observations-per-slice floor an external source warns about. It is
a property of those three series, not of how the data was sliced.

A further caveat supersedes the reading above: `benches/031_pbo_interval` puts
block-permutation intervals on every figure in this table. They are 0.09-0.33
wide, and only **JTO** (0.417-0.647) clears the envelope of the four clean
generalisers. FLOKI overlaps it by 0.036 — borderline. PYTH overlaps properly
and is not distinguishable from clean generalisation at all.

**Three dissenters is an overcount.** One dissents measurably, one is
borderline, one does not. The point estimates in this table are correct; the
partition drawn from them is not.

