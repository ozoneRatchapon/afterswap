# Which DFlow-only signal belongs in the alphabet?

1207 ticks from `data/incoming/bonk_depth.jsonl`, 60-tick windows: 12 train / 8 test. Best machine picked on train, scored on test, identical protocol for every candidate. Edge is vs holding, in bps.

| third bit | selection | train | **test (±SE)** | MDE |
|---|---|---|---|---|
| 2-bit (shipping today) | top-5 of 2 tied | +21.5 | **+8.7 ± 8.0** | 22.4 bps |
| + depth below median | top-5 of 1 tied | +16.8 | **+9.2 ± 10.5** | 29.5 bps |
| + route changed venue | top-5 of 1 tied | +20.4 | **+8.4 ± 8.2** | 23.0 bps |
| + single-hop route | top-5 of 1 tied | +23.1 | **+6.8 ± 8.6** | 24.2 bps |

Read the MDE column first: a candidate signal is only ruled out down to that effect size. Differences smaller than it are invisible to this sample regardless of what the point estimates say.


> ⚠️ **Preliminary — 8 test windows is too few to separate these.** Treat as a pipeline check; re-run as the recorder fills.

