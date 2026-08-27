# New objective: implementation shortfall, and its risk variants

Arrival-price shortfall in bps (**positive is worse**), 2 bps charged per fill to every strategy including the baseline. 120-tick windows, chronological 60/40 split, machine selected on train under each objective and measured on test, TWAP(10×6) as the baseline on the same windows.


# Regime: no price impact (the flawed simulator)


## Selection objective: min mean shortfall

| asset | test windows | engine mean | TWAP mean | paired Δmean (±SE) | engine SD | TWAP SD | **SD ratio vs TWAP** | t̄ eng/matched | **SD ratio vs speed-matched TWAP** | engine CVaR90 | TWAP CVaR90 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| BONK | 100 | -11.6 | +2.9 | -14.5 ± 7.7 | 118.1 | 107.6 | **1.10 [0.92, 1.47]** | 56/55 | **0.91 [0.74, 1.13]** | +263.3 | +189.3 |
| FLOKI | 67 | +0.6 | -2.5 | +3.1 ± 23.8 | 217.8 | 88.2 | **2.47 [1.65, 3.62]** | 117/116 | **1.32 [1.09, 1.50]** | +347.6 | +153.1 |
| JTO | 67 | +13.9 | +0.1 | +13.7 ± 12.9 | 139.1 | 78.7 | **1.77 [1.47, 2.12]** | 90/88 | **1.06 [0.94, 1.19]** | +275.8 | +127.6 |
| JUP | 67 | -7.0 | -7.0 | +0.0 ± 16.9 | 169.0 | 89.5 | **1.89 [1.51, 2.35]** | 107/110 | **1.16 [1.08, 1.24]** | +340.0 | +172.8 |
| ORCA | 67 | -9.5 | -3.3 | -6.2 ± 6.4 | 97.6 | 65.2 | **1.50 [1.26, 1.75]** | 56/55 | **1.17 [0.99, 1.39]** | +139.0 | +120.1 |
| PEPE | 100 | -1.8 | +4.2 | -6.0 ± 5.4 | 85.7 | 100.0 | **0.86 [0.59, 0.97]** | 14/16 | **0.90 [0.82, 1.08]** | +168.0 | +200.8 |
| PYTH | 67 | -4.1 | -9.1 | +5.0 ± 22.7 | 253.4 | 112.3 | **2.26 [1.93, 2.66]** | 117/116 | **1.25 [1.21, 1.29]** | +494.1 | +168.8 |
| RAY | 67 | -15.2 | -1.7 | -13.5 ± 19.9 | 196.0 | 77.0 | **2.55 [2.04, 3.12]** | 119/121 | **1.29 [1.21, 1.39]** | +366.0 | +140.9 |
| SHIB | 67 | -10.1 | -13.0 | +2.9 ± 17.6 | 170.1 | 85.1 | **2.00 [1.30, 3.23]** | 106/104 | **1.18 [1.07, 1.23]** | +328.4 | +122.0 |
| SOL_USDC | 150 | -6.9 | -0.5 | -6.4 ± 6.0 | 94.4 | 65.0 | **1.45 [1.10, 2.12]** | 72/72 | **1.14 [0.91, 1.49]** | +137.9 | +111.2 |
| WIF | 100 | -16.1 | -0.4 | -15.7 ± 7.5 | 134.6 | 107.4 | **1.25 [1.08, 1.62]** | 68/66 | **0.94 [0.83, 1.05]** | +264.1 | +192.5 |

**vs TWAP: mean SD ratio 1.74, significant on 1/11. vs speed-matched TWAP: 1.12, significant on 0/11.**


## Selection objective: min shortfall SD

| asset | test windows | engine mean | TWAP mean | paired Δmean (±SE) | engine SD | TWAP SD | **SD ratio vs TWAP** | t̄ eng/matched | **SD ratio vs speed-matched TWAP** | engine CVaR90 | TWAP CVaR90 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| BONK | 100 | +14.8 | +2.9 | +11.9 ± 6.9 | 95.5 | 107.6 | **0.89 [0.43, 1.06]** | 8/6 | **1.02 [0.99, 1.23]** | +141.9 | +189.3 |
| FLOKI | 67 | -3.0 | -2.5 | -0.5 ± 7.3 | 57.2 | 88.2 | **0.65 [0.43, 0.77]** | 11/11 | **1.10 [0.69, 1.42]** | +123.5 | +153.1 |
| JTO | 67 | +10.3 | +0.1 | +10.1 ± 7.2 | 61.5 | 78.7 | **0.78 [0.52, 0.98]** | 16/16 | **1.03 [0.71, 1.30]** | +178.1 | +127.6 |
| JUP | 67 | -1.1 | -7.0 | +5.9 ± 7.8 | 65.8 | 89.5 | **0.73 [0.43, 0.95]** | 15/16 | **1.00 [0.59, 1.37]** | +160.5 | +172.8 |
| ORCA | 67 | -1.9 | -3.3 | +1.4 ± 5.6 | 36.9 | 65.2 | **0.57 [0.43, 0.68]** | 7/6 | **1.22 [1.01, 1.55]** | +87.9 | +120.1 |
| PEPE | 100 | +8.5 | +4.2 | +4.3 ± 6.2 | 83.6 | 100.0 | **0.84 [0.48, 0.99]** | 8/6 | **1.01 [0.94, 1.57]** | +154.8 | +200.8 |
| PYTH | 67 | -7.2 | -9.1 | +1.8 ± 11.0 | 44.5 | 112.3 | **0.40 [0.33, 0.49]** | 6/6 | **1.01 [1.00, 1.02]** | +58.3 | +168.8 |
| RAY | 67 | -1.5 | -1.7 | +0.2 ± 9.0 | 67.6 | 77.0 | **0.88 [0.49, 1.22]** | 18/16 | **1.14 [0.75, 1.45]** | +143.8 | +140.9 |
| SHIB | 67 | -3.7 | -13.0 | +9.3 ± 8.7 | 61.8 | 85.1 | **0.73 [0.35, 1.05]** | 11/11 | **1.44 [0.76, 1.79]** | +138.3 | +122.0 |
| SOL_USDC | 150 | +3.1 | -0.5 | +3.6 ± 3.3 | 31.5 | 65.0 | **0.48 [0.36, 0.56]** | 6/6 | **1.45 [0.97, 2.28]** | +56.5 | +111.2 |
| WIF | 100 | +6.2 | -0.4 | +6.6 ± 6.6 | 85.5 | 107.4 | **0.80 [0.44, 0.90]** | 14/16 | **0.84 [0.63, 1.16]** | +160.5 | +192.5 |

**vs TWAP: mean SD ratio 0.70, significant on 8/11. vs speed-matched TWAP: 1.12, significant on 0/11.**


## Selection objective: min CVaR(90)

| asset | test windows | engine mean | TWAP mean | paired Δmean (±SE) | engine SD | TWAP SD | **SD ratio vs TWAP** | t̄ eng/matched | **SD ratio vs speed-matched TWAP** | engine CVaR90 | TWAP CVaR90 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| BONK | 100 | -3.2 | +2.9 | -6.1 ± 7.1 | 85.0 | 107.6 | **0.79 [0.52, 0.98]** | 21/22 | **0.85 [0.64, 1.19]** | +174.5 | +189.3 |
| FLOKI | 67 | -2.7 | -2.5 | -0.2 ± 7.1 | 74.2 | 88.2 | **0.84 [0.52, 1.02]** | 16/16 | **1.13 [0.65, 1.45]** | +168.4 | +153.1 |
| JTO | 67 | +10.3 | +0.1 | +10.1 ± 7.2 | 61.5 | 78.7 | **0.78 [0.52, 0.98]** | 16/16 | **1.03 [0.71, 1.30]** | +178.1 | +127.6 |
| JUP | 67 | -5.5 | -7.0 | +1.5 ± 6.7 | 57.1 | 89.5 | **0.64 [0.51, 0.75]** | 13/11 | **1.07 [0.90, 1.26]** | +126.8 | +172.8 |
| ORCA | 67 | -3.8 | -3.3 | -0.5 ± 5.6 | 34.7 | 65.2 | **0.53 [0.40, 0.70]** | 9/11 | **0.87 [0.80, 0.92]** | +72.1 | +120.1 |
| PEPE | 100 | +0.9 | +4.2 | -3.2 ± 4.7 | 85.8 | 100.0 | **0.86 [0.65, 0.94]** | 15/16 | **0.91 [0.84, 1.13]** | +175.2 | +200.8 |
| PYTH | 67 | +8.7 | -9.1 | +17.8 ± 11.1 | 108.9 | 112.3 | **0.97 [0.67, 1.24]** | 25/22 | **1.19 [0.83, 1.53]** | +302.8 | +168.8 |
| RAY | 67 | -1.5 | -1.7 | +0.2 ± 9.0 | 67.6 | 77.0 | **0.88 [0.49, 1.22]** | 18/16 | **1.14 [0.75, 1.45]** | +143.8 | +140.9 |
| SHIB | 67 | -0.4 | -13.0 | +12.5 ± 8.7 | 65.4 | 85.1 | **0.77 [0.36, 1.14]** | 15/16 | **1.23 [0.64, 1.57]** | +151.1 | +122.0 |
| SOL_USDC | 150 | +3.1 | -0.5 | +3.6 ± 3.3 | 31.5 | 65.0 | **0.48 [0.36, 0.56]** | 6/6 | **1.45 [0.97, 2.28]** | +56.5 | +111.2 |
| WIF | 100 | -2.4 | -0.4 | -2.0 ± 5.4 | 95.9 | 107.4 | **0.89 [0.68, 1.00]** | 28/28 | **0.93 [0.73, 1.11]** | +202.0 | +192.5 |

**vs TWAP: mean SD ratio 0.77, significant on 6/11. vs speed-matched TWAP: 1.07, significant on 1/11.**


# Regime: **with rate-dependent temporary impact**


## Selection objective: min mean shortfall

| asset | test windows | engine mean | TWAP mean | paired Δmean (±SE) | engine SD | TWAP SD | **SD ratio vs TWAP** | t̄ eng/matched | **SD ratio vs speed-matched TWAP** | engine CVaR90 | TWAP CVaR90 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| BONK | 100 | -8.5 | +4.9 | -13.4 ± 7.6 | 117.0 | 107.5 | **1.09 [0.92, 1.46]** | 56/55 | **0.90 [0.73, 1.12]** | +264.2 | +191.3 |
| FLOKI | 67 | +0.7 | -0.5 | +1.2 ± 23.8 | 217.9 | 88.2 | **2.47 [1.65, 3.62]** | 117/116 | **1.32 [1.09, 1.50]** | +347.8 | +155.1 |
| JTO | 67 | +22.7 | +2.1 | +20.5 ± 18.2 | 185.0 | 78.7 | **2.35 [1.88, 2.95]** | 118/116 | **1.27 [1.22, 1.32]** | +385.9 | +129.6 |
| JUP | 67 | -6.2 | -5.0 | -1.2 ± 18.8 | 184.3 | 89.5 | **2.06 [1.67, 2.58]** | 117/116 | **1.25 [1.18, 1.33]** | +354.4 | +174.8 |
| ORCA | 67 | -6.4 | -1.3 | -5.1 ± 14.9 | 147.6 | 65.2 | **2.26 [1.74, 3.09]** | 119/121 | **1.26 [1.19, 1.32]** | +263.2 | +122.1 |
| PEPE | 100 | -6.5 | +6.2 | -12.6 ± 3.1 | 100.2 | 99.9 | **1.00 [0.92, 1.07]** | 39/38 | **0.97 [0.82, 1.06]** | +195.1 | +202.8 |
| PYTH | 67 | -3.9 | -7.1 | +3.2 ± 22.7 | 253.4 | 112.2 | **2.26 [1.93, 2.66]** | 117/116 | **1.25 [1.22, 1.29]** | +494.1 | +170.7 |
| RAY | 67 | -15.2 | +0.3 | -15.5 ± 19.9 | 196.0 | 77.0 | **2.55 [2.04, 3.12]** | 119/121 | **1.29 [1.21, 1.39]** | +366.0 | +142.9 |
| SHIB | 67 | -8.8 | -11.0 | +2.2 ± 17.6 | 170.2 | 85.1 | **2.00 [1.30, 3.22]** | 106/104 | **1.18 [1.07, 1.23]** | +329.4 | +124.0 |
| SOL_USDC | 150 | -16.2 | +1.5 | -17.7 ± 5.7 | 105.0 | 65.0 | **1.62 [1.34, 2.23]** | 98/99 | **1.16 [1.08, 1.28]** | +139.4 | +113.2 |
| WIF | 100 | -26.6 | +1.6 | -28.2 ± 13.8 | 188.6 | 107.4 | **1.76 [1.36, 2.50]** | 84/82 | **1.21 [1.09, 1.35]** | +322.2 | +194.5 |

**vs TWAP: mean SD ratio 1.95, significant on 0/11. vs speed-matched TWAP: 1.19, significant on 0/11.**


## Selection objective: min shortfall SD

| asset | test windows | engine mean | TWAP mean | paired Δmean (±SE) | engine SD | TWAP SD | **SD ratio vs TWAP** | t̄ eng/matched | **SD ratio vs speed-matched TWAP** | engine CVaR90 | TWAP CVaR90 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| BONK | 100 | +10.6 | +4.9 | +5.7 ± 6.0 | 89.8 | 107.5 | **0.83 [0.43, 0.99]** | 10/11 | **0.99 [0.75, 1.10]** | +162.5 | +191.3 |
| FLOKI | 67 | +8.2 | -0.5 | +8.7 ± 7.3 | 56.1 | 88.2 | **0.64 [0.42, 0.76]** | 11/11 | **1.08 [0.68, 1.39]** | +130.6 | +155.1 |
| JTO | 67 | +20.8 | +2.1 | +18.7 ± 7.1 | 59.4 | 78.7 | **0.75 [0.51, 0.95]** | 16/16 | **1.00 [0.68, 1.26]** | +182.7 | +129.6 |
| JUP | 67 | +9.4 | -5.0 | +14.4 ± 7.8 | 63.9 | 89.5 | **0.71 [0.41, 0.93]** | 15/16 | **0.98 [0.57, 1.33]** | +164.8 | +174.8 |
| ORCA | 67 | +9.5 | -1.3 | +10.8 ± 5.7 | 33.6 | 65.2 | **0.52 [0.41, 0.64]** | 7/6 | **1.11 [0.96, 1.36]** | +89.2 | +122.1 |
| PEPE | 100 | +19.0 | +6.2 | +12.8 ± 6.2 | 82.7 | 99.9 | **0.83 [0.47, 0.99]** | 8/6 | **1.00 [0.94, 1.54]** | +161.9 | +202.8 |
| PYTH | 67 | +18.5 | -7.1 | +25.6 ± 11.0 | 106.7 | 112.2 | **0.95 [0.66, 1.22]** | 25/22 | **1.17 [0.82, 1.51]** | +305.6 | +170.7 |
| RAY | 67 | +11.2 | +0.3 | +10.9 ± 8.0 | 54.3 | 77.0 | **0.71 [0.34, 1.05]** | 13/11 | **1.04 [0.43, 1.62]** | +114.3 | +142.9 |
| SHIB | 67 | +10.0 | -11.0 | +21.0 ± 8.7 | 64.5 | 85.1 | **0.76 [0.35, 1.11]** | 14/16 | **1.21 [0.61, 1.55]** | +157.7 | +124.0 |
| SOL_USDC | 150 | +14.4 | +1.5 | +13.0 ± 3.3 | 30.6 | 65.0 | **0.47 [0.35, 0.54]** | 6/6 | **1.41 [0.92, 2.24]** | +64.4 | +113.2 |
| WIF | 100 | +17.0 | +1.6 | +15.4 ± 6.6 | 83.6 | 107.4 | **0.78 [0.43, 0.89]** | 14/16 | **0.82 [0.61, 1.12]** | +166.2 | +194.5 |

**vs TWAP: mean SD ratio 0.72, significant on 8/11. vs speed-matched TWAP: 1.07, significant on 0/11.**


## Selection objective: min CVaR(90)

| asset | test windows | engine mean | TWAP mean | paired Δmean (±SE) | engine SD | TWAP SD | **SD ratio vs TWAP** | t̄ eng/matched | **SD ratio vs speed-matched TWAP** | engine CVaR90 | TWAP CVaR90 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| BONK | 100 | +6.5 | +4.9 | +1.6 ± 7.1 | 83.3 | 107.5 | **0.77 [0.51, 0.95]** | 21/22 | **0.83 [0.63, 1.16]** | +178.9 | +191.3 |
| FLOKI | 67 | +7.9 | -0.5 | +8.4 ± 6.8 | 85.2 | 88.2 | **0.97 [0.58, 1.17]** | 25/28 | **1.01 [0.59, 1.25]** | +211.1 | +155.1 |
| JTO | 67 | +20.8 | +2.1 | +18.7 ± 7.1 | 59.4 | 78.7 | **0.75 [0.51, 0.95]** | 16/16 | **1.00 [0.68, 1.26]** | +182.7 | +129.6 |
| JUP | 67 | -2.6 | -5.0 | +2.4 ± 7.3 | 101.6 | 89.5 | **1.14 [0.88, 1.35]** | 44/44 | **1.03 [0.80, 1.21]** | +238.9 | +174.8 |
| ORCA | 67 | +5.6 | -1.3 | +6.9 ± 5.6 | 34.5 | 65.2 | **0.53 [0.40, 0.69]** | 9/11 | **0.86 [0.79, 0.91]** | +80.0 | +122.1 |
| PEPE | 100 | +7.7 | +6.2 | +1.5 ± 4.7 | 85.4 | 99.9 | **0.85 [0.65, 0.94]** | 16/16 | **0.90 [0.83, 1.12]** | +178.8 | +202.8 |
| PYTH | 67 | +18.5 | -7.1 | +25.6 ± 11.0 | 106.7 | 112.2 | **0.95 [0.66, 1.22]** | 25/22 | **1.17 [0.82, 1.51]** | +305.6 | +170.7 |
| RAY | 67 | +9.0 | +0.3 | +8.7 ± 9.0 | 66.1 | 77.0 | **0.86 [0.47, 1.20]** | 18/16 | **1.11 [0.73, 1.41]** | +148.3 | +142.9 |
| SHIB | 67 | -4.4 | -11.0 | +6.6 ± 8.3 | 82.8 | 85.1 | **0.97 [0.54, 1.38]** | 29/28 | **1.06 [0.59, 1.46]** | +200.2 | +124.0 |
| SOL_USDC | 150 | +14.4 | +1.5 | +13.0 ± 3.3 | 30.6 | 65.0 | **0.47 [0.35, 0.54]** | 6/6 | **1.41 [0.92, 2.24]** | +64.4 | +113.2 |
| WIF | 100 | +8.8 | +1.6 | +7.2 ± 6.4 | 88.8 | 107.4 | **0.83 [0.54, 0.94]** | 21/22 | **0.87 [0.65, 1.11]** | +180.6 | +194.5 |

**vs TWAP: mean SD ratio 0.83, significant on 6/11. vs speed-matched TWAP: 1.02, significant on 1/11.**


## Verdict: the objective changed, the answer did not

Read against TWAP alone, this looked like the project's first durable result:
selecting machines for minimum shortfall variance gives an **SD ratio of 0.70,
significantly below 1 on 8 of 11 assets** — execution ~30% more predictable
than TWAP, out of sample, on real bars, with bootstrap confidence intervals.
It survived adding a rate-dependent impact model (0.72), which was the first
control we ran.

**The second control destroys it.** The selected machines liquidate roughly
four times sooner than TWAP (mean liquidation time ~6–15 ticks against TWAP's
33). Faster liquidation mechanically reduces timing variance — that is the
Almgren–Chriss frontier, not skill. Compressing a plain TWAP to the *same mean
liquidation time* and comparing against that reproduces the entire advantage:
**SD ratio 1.12, significant on 0 of 11 assets.** The machines are marginally
*worse* than the trivial schedule at their own urgency.

So the finding was never "these machines execute better". It was "these
machines execute sooner", restated in units of variance. We moved along the
efficient frontier and mistook it for beating it — until the benchmark was
matched on the dimension that was actually doing the work.

**Method note.** The first principle in our own research method is *name the
incumbent*: not "the market", but the thing a user would otherwise do. This
result is what happens when the named incumbent is right in kind but wrong in
parameter. A benchmark must be matched on every dimension the strategy is free
to vary, or the comparison measures the mismatch instead of the strategy.

