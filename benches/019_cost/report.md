# Execution cost changes the answer

Per-fill cost charged identically to the engine and to every floor, across 11 real 1-minute series, non-overlapping 200-bar windows. The engine and TWAP exit in ten tranches and pay ten times; the trailing stop exits once and pays once.

Reference points: a Solana base fee is 5,000 lamports and DFlow's medium priority fee estimate is 50,000 µlamports/CU (~10,000 lamports at 200k CU). On a $5 clip that pair is roughly **2 bps**; on a $50 clip roughly **0.2 bps**. Clip slippage adds more on thin pairs — our BONK depth recorder sees 5–27 bps between a small and a large clip.

| cost / fill | vs TWAP (across assets) | vs trailing stop (across assets) |
|---|---|---|
| 0 bps | -1.3 ± 1.9 | +3.4 ± 5.8 |
| 1 bps | -1.2 ± 1.9 | +3.4 ± 5.8 |
| 2 bps | -1.1 ± 1.9 | +3.3 ± 5.8 |
| 5 bps | -0.9 ± 1.9 | +3.1 ± 5.8 |

## Result: costs do not explain anything here

The hypothesis behind this bench was that a cost-free simulator flatters a
tranching exit, because it pays the fee ten times while a trailing stop pays it
once — and that correcting it would move the comparisons materially. **It does
not.** Between 0 and 5 bps per fill, every column moves by less than half a bp:
against TWAP the cost cancels almost exactly (both sides scale out), and
against the trailing stop the drift is ~0.3 bps, far inside the ±5.8 standard
error.

Two reasons, visible in the mechanics rather than the table: the engine
frequently does not complete all ten tranches inside a window, and the trailing
stop frequently never triggers at all — in which case it holds and pays
nothing, so there is no asymmetry to correct.

**What this closes:** our earlier benchmarks were not flattered by ignoring
execution cost, which is one fewer explanation for the gap between synthetic
and real results. The cost model stays in the engine (`fill_cost_bps`, applied
to fills and to the tournament's own replays) because live trading will need
it, but it is not a lever on the research question. Recorded here so nobody —
including us — spends another day assuming it is.

