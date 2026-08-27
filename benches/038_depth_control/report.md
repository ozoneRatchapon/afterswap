# Does a real depth reading earn the prescribed CUPED reduction?

1207 paired price/depth observations for BONK from the Plan 001 recorder (`data/incoming/bonk_depth.jsonl`). CUPED compresses variance by `1 - rho^2`. For an execution outcome driven by pool depth at fill time, the reachable reduction is bounded by how well a depth reading taken *k* ticks earlier predicts it.

| lag k | rho(depth_t, depth_t+k) | CUPED reduction | in the prescribed 30-50% band? |
|---|---|---|---|
| 1 | +0.588 | **34.6%** | **yes** |
| 2 | +0.511 | **26.1%** | no |
| 5 | +0.439 | **19.3%** | no |
| 10 | +0.299 | **8.9%** | no |
| 30 | +0.114 | **1.3%** | no |

## Price cannot stand in for depth

| relationship | rho | variance explained |
| --- | --- | --- |
| prior realised volatility -> depth | +0.126 | 1.6% |
| depth -> next-tick abs return | +0.080 | 0.6% |
| prior realised volatility -> next-tick abs return | +0.143 | 2.1% |

A price-derived variate explains **1.6%** of depth variation. That is the gap bench 033 ran into: its
1.9% ceiling was not a fact about CUPED, it was a fact about substituting price for depth. Depth is a
different observable, and on this series price does not carry it.

## Verdict

**A depth reading one tick old delivers 34.6% variance reduction** — inside round three's prescribed
30-50% band, and roughly 18x what price-derived proxies achieved in bench 033.

The binding constraint is **freshness, not volume**. The reduction halves by lag 5 and is gone by lag
30, so the control variate has to be a pre-trade quote taken within a tick or two of the fill — which
is exactly what the signed quote in the verifiable exit chain already is. A depth history sampled once
a minute would be worth almost nothing; the same history sampled beside each quote is worth a third of
the variance.

Two limits on this number, both real:

- **One asset, one period.** BONK is a long-tail CPMM. The margin worth chasing is on liquid CLMM
  majors (+0.10 to +0.35 bps), whose depth process is a different shape — tick-concentrated rather
  than reserve-driven — and this result should not be assumed to transfer to SOL/USDC.
- **It bounds the control variate, not the experiment.** Realised execution cost also carries routing
  and priority-tip variance that no pre-trade depth reading predicts, so 34.6% is a ceiling on the
  depth component rather than on the outcome as a whole.

What it settles is the question that was open: the 30-50% figure is reachable on real depth data we
already hold, and simulating depth from prices could never have reached it.
