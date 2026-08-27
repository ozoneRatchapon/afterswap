# Which DFlow-only signal belongs in the alphabet?

583 ticks from `data/incoming/bonk_depth.jsonl`, 60-tick windows: 5 train / 4 test. Best machine picked on train, scored on test, identical protocol for every candidate. Edge is vs holding, in bps.

| third bit | selection | train | **test (±SE)** |
|---|---|---|---|
| 2-bit (shipping today) | top-5 of 12 tied | +26.6 | **+19.5 ± 23.7** |
| + depth below median | top-5 of 2 tied | +26.6 | **+25.3 ± 26.4** |
| + route changed venue | top-5 of 2 tied | +26.6 | **+27.3 ± 26.8** |
| + single-hop route | top-5 of 2 tied | +26.6 | **+27.5 ± 27.4** |

> ⚠️ **Preliminary — 4 test windows is too few to separate these.** Treat as a pipeline check; re-run as the recorder fills.

