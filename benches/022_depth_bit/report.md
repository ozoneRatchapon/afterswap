# Does DFlow depth carry exit signal?

539 ticks from `data/incoming/bonk_depth.jsonl`, 60-tick windows (300 s each at the recorder's 5 s cadence): 4 train / 4 test. Best machine chosen on train under each protocol, scored on test. Edge is vs holding, in bps.

| protocol | best machine (train) | train edge | **test edge (±SE)** |
|---|---|---|---|
| 2-bit (shipping today) | #303 | +21.9 bps | **+42.2 ± 14.9 bps** |
| 3-bit (with depth) | #870 | +21.9 bps | **+28.7 ± 17.3 bps** |

> ⚠️ **Preliminary — too few windows to conclude.** With 4 test windows the two protocols' standard errors overlap heavily, so this table cannot distinguish them; treat it as a pipeline check, not a result. The recorder is still collecting; re-run with `cargo run -p afterswap-engine --example depth_bit --release` once the file is several times longer.

