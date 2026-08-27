# Does CUPED have anything to work with here?

> **Superseded by [bench 038](../038_depth_control/report.md).** This bench concluded that 1.9% was CUPED's ceiling for us because the corpus is `{t, price}`. That is true of `data/reference/` but not of the repository — `data/incoming/bonk_depth.jsonl` holds 1,207 paired price/depth rows kept when the Plan 001 recorder stopped. On real depth, a one-tick-old reading delivers **34.6%**, inside the prescribed band. The measurements below stand as a bound on price-derived proxies; the conclusion drawn about CUPED does not.

Correlation between the per-window paired edge differential and two pre-window control variates built from the 120 ticks preceding each window. CUPED compresses variance by `1 − ρ²`, so the reduction column is that identity, not a measurement of CUPED itself. Outcome is the best-in-sample machine's per-window edge against the population median, over 1054 enumerated machines.

| asset | windows | ρ(Y, prior vol) | reduction | ρ(Y, prior drift) | reduction | best |
|---|---|---|---|---|---|---|
| BONK | 249 | -0.015 | 0.0% | +0.072 | 0.5% | 0.5% |
| FLOKI | 165 | +0.018 | 0.0% | -0.179 | 3.2% | 3.2% |
| JTO | 165 | -0.066 | 0.4% | -0.055 | 0.3% | 0.4% |
| JUP | 165 | +0.015 | 0.0% | -0.061 | 0.4% | 0.4% |
| ORCA | 165 | -0.077 | 0.6% | -0.042 | 0.2% | 0.6% |
| PEPE | 249 | +0.271 | 7.3% | +0.130 | 1.7% | 7.3% |
| PYTH | 165 | -0.029 | 0.1% | +0.091 | 0.8% | 0.8% |
| RAY | 165 | +0.039 | 0.1% | -0.075 | 0.6% | 0.6% |
| SHIB | 165 | +0.189 | 3.6% | -0.013 | 0.0% | 3.6% |
| SOL_USDC | 374 | +0.067 | 0.4% | +0.064 | 0.4% | 0.4% |
| WIF | 249 | +0.079 | 0.6% | +0.184 | 3.4% | 3.4% |

## Verdict: 1.9% mean reduction, against a prescribed 30–50%

Price-derived control variates carry almost nothing about the paired edge differential. The best of the
two, per asset, averages a **1.9% variance reduction** — against the 30–50% round three cites and the
drop from 849 to 420–590 cycles that figure implies. On these variates the required sample barely moves.

That is a bound on the proxies, not a refutation of the method. The variates round three names —
pre-trade pool volatility and order arrival imbalance — are depth-book quantities, and our corpus is
`{t, price}`. A price series cannot express order arrival imbalance at all. What this bench rules out
is the cheap version: CUPED on data we already hold does not bring the +0.10 to +0.35 bps CLMM margin
inside reach.

Making L1 answerable needs the depth-aware feed back, which is the recorder Plan 001 closed. That is a
real decision with a cost attached, not an implementation detail — and it is the same feed twice over,
since the outcome CUPED would be applied to is a paired execution A/B rather than the edge-vs-hold
objective used here as a stand-in.
