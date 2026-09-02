# Does a Schmitt-trigger drawdown bit beat one threshold?

3462 ticks from `data/incoming/recorded_long.jsonl`, 60-tick windows: 34 train / 23 test. Machines selected on train, scored on test, one code path for every arm. Edge is vs holding, in bps; `flips` is mean off-peak bit transitions per window.

`arm / disarm` in bps of drawdown from the running peak. **`disarm = arm` is the shipping memoryless bit** — `tests/hysteresis.rs` asserts the two replays agree bit-for-bit there, so those rows are the protocol itself.

| arm / disarm | flips | selection | train | **test (±SE)** | Δ vs 30/30 | RW p-adj |
|---|---|---|---|---|---|---|
| 10 / 10 (memoryless) | 3.5 | top-5 of 1 tied | +1.1 | **-0.4 ± 1.6** | -0.36 | 1.000 |
| 10 / 5 | 2.4 | top-5 of 1 tied | +1.0 | **-0.9 ± 2.0** | -0.90 | 1.000 |
| 10 / 2 | 2.1 | top-5 of 1 tied | +1.1 | **-0.7 ± 1.5** | -0.67 | 1.000 |
| 10 / 0 | 0.9 | top-5 of 2 tied | +1.0 | **-0.2 ± 0.8** | -0.21 | 1.000 |
| 20 / 20 (memoryless) | 2.2 | top-5 of 8 tied | +0.5 | **-0.1 ± 0.7** | -0.07 | 0.980 |
| 20 / 10 | 1.2 | top-5 of 8 tied | +0.5 | **-0.1 ± 0.7** | -0.07 | 1.000 |
| 20 / 5 | 1.0 | top-5 of 8 tied | +0.5 | **-0.1 ± 0.7** | -0.07 | 1.000 |
| 20 / 0 | 0.7 | top-5 of 8 tied | +0.5 | **-0.1 ± 0.7** | -0.07 | 1.000 |
| 30 / 30 ← shipping | 1.1 | top-5 of 1 tied | +2.2 | **-0.0 ± 0.9** | — | — |
| 30 / 15 | 0.5 | top-5 of 1 tied | +2.2 | **-0.2 ± 0.9** | -0.12 | 1.000 |
| 30 / 8 | 0.5 | top-5 of 1 tied | +2.6 | **-0.3 ± 1.3** | -0.27 | 1.000 |
| 30 / 0 | 0.4 | top-5 of 1 tied | +2.4 | **-0.4 ± 1.3** | -0.37 | 1.000 |
| 50 / 50 (memoryless) | 0.5 | top-5 of 1 tied | +0.6 | **-0.2 ± 0.5** | -0.15 | 1.000 |
| 50 / 25 | 0.2 | top-5 of 8 tied | +0.5 | **-0.1 ± 0.7** | -0.07 | 1.000 |
| 50 / 12 | 0.2 | top-5 of 8 tied | +0.5 | **-0.1 ± 0.7** | -0.07 | 1.000 |
| 50 / 0 | 0.1 | top-5 of 8 tied | +0.5 | **-0.1 ± 0.7** | -0.07 | 1.000 |

**Reading it.** The absolute test column carries 0.9 bps of SE — an MDE of 2.6 bps at 80% power, which is far larger than any plausible band effect, so that column cannot settle this. Pairing does not rescue it either, and that is worth stating plainly: the arms share price paths but *select different machines*, so the paths diverge and the median paired SE across arms is 0.95 bps — no better than the unpaired column, for a paired MDE of 2.66 bps. That is what this sample can rule out; a band effect smaller than it would be invisible here whatever the point estimates say. Romano–Wolf steps down over all 15 non-benchmark arms at α = 0.05, 2000 bootstraps, seed 20260829, so a ✓ has already paid for the sweep.

**Verdict.** No band survives the multiplicity correction. The `flips` column shows the trigger was genuinely engaged, so this is a measured null rather than an unengaged one: at these thresholds the chatter the band removes was not costing the machines anything this sample can see down to the paired MDE above. The shipping single threshold stays.

