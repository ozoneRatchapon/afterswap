# Plan 001 — Liquidity-aware exits (the DFlow-native edge)

## Why this, and why now

Bench 018 (chronological train/test, real bars) set the direction: the engine
beats **trailing stops on volatile tokens** out-of-sample (BONK +34 ± 10 bps,
PEPE +26 ± 11) but beats **TWAP nowhere**. Everything measured so far uses
price direction only — the same information every competitor has from candles.

DFlow exposes something they do not have: **executable depth across every
venue**. Probing quotes at several sizes shows a real, size-dependent price on
volatile tokens — BONK moves ~27 bps between a small and a large clip, while
SOL/USDC barely moves 0.3 bps. Selling into good depth and waiting out thin
depth is an exit signal that cannot be reconstructed from CEX candles, and it
is exactly the kind of edge a swap aggregator should enable.

**Hypothesis:** adding a depth bit to the machine alphabet improves exits on
volatile tokens, measured against trailing stops out-of-sample.

**Null result is a fine outcome** and gets recorded like the four before it
(PL ratings, magnitude bit, bootstrap tuning, per-regime stats).

## Checklist

- [x] Confirm the depth signal exists on DFlow quotes (BONK ~27 bps across
      sizes; SOL/USDC ~0.3 bps — memecoins only)
- [x] **Record dual-size DFlow quotes** for BONK — recorder running into
      `data/incoming/bonk_depth.jsonl` (5 s cadence, ~7-8 bps spread observed
      and varying). Needs ~2 h for the first usable A/B (≥ 6 windows of 120)
- [x] **Cheap test first, refactor only if it pays** — `sim::replay_exit_depth`
      (3rd unrolled bit: spread ≤ expanding median) + `load_depth_corpus`,
      no engine changes, no risk to the shipping path
- [x] **Harness**: `examples/depth_bit.rs` picks the best machine on TRAIN
      windows under each protocol and scores it on disjoint TEST windows
- [x] **First run (preliminary, bench 022_depth_bit)** — 534 ticks, 4 train /
      4 test windows: 2-bit +42.2 ± 14.9 vs 3-bit-with-depth +28.7 ± 17.3.
      The depth bit does **not** help so far, but the sample cannot separate
      them; the harness now says so in its own report. Recorder still running.
- [x] **Generalised the harness** (`examples/signal_bits.rs`): any candidate
      third bit runs on the identical protocol and code path via
      `sim::replay_exit_with_bit`, so depth, route-churn and hop-count are
      compared like-for-like. Caught a protocol defect doing it — single-argmax
      selection broke ties by index, so three different signals produced
      identical numbers because the chosen machine ignored the third bit
      entirely. Now averages the top-5 and reports the tie count.
- [x] **Candidate signals implemented**: depth-below-median, route-changed-venue,
      single-hop-route.
- [ ] **Re-run** when the recording is several times longer, then decide
- [ ] **Still gated on that re-run — only if the 3-bit protocol wins:** thread depth through
      `WindowStore` → `on_tick` → `evaluate_matrix` behind `depth_bit: bool`
      (default off), then re-run GOAT + wasm parity
- [ ] **A/B on a frozen corpus** (move the recording out of `data/incoming/`
      only when the recorder is stopped — see the corpus-freeze rule in
      ROADMAP 7e)
- [ ] **Ship or revert with a recorded reason**; update README claim table
- [ ] Re-run GOAT gates + wasm parity (G1–G6) before any deploy

## Running in parallel while the recorder fills

- [x] **Demo pair switcher** — SOL/USDC (familiar, no edge) or BONK/USDC
      (where bench 018 measured +34 ± 10 vs trailing). Switching resets the
      per-browser scoreboard so two markets never mix in one statistic.
- [x] **Live soak moved to BONK** with paired evaluation
      (`--pair bonk --paired`), so the live evidence is gathered in the market
      the claim is about rather than the one it is not.
- [ ] Report the BONK paired soak once it has enough cycles for a t-value.

## Adjacent results while the recorder fills

- [x] **Execution-cost model** (`fill_cost_bps`, cost-aware floors) — shipped,
      then measured: not a lever (bench 019_cost, <0.5 bp across 0→5 bps).
- [x] **Venue capture** added to the recorder: every quote already carries
      `routePlan[].venue` and hop count, so route churn is a free
      thin-liquidity signal alongside the size-spread one.
- [ ] Depth A/B once the recording has ≥ 6 windows (see checklist above).

## Rules this plan inherits

1. No claim without a floor and a standard error.
2. Corpus set frozen during an A/B; in-progress recordings live in
   `data/incoming/`.
3. Real data beats synthetic; report them split.
4. If a feature cannot be measured by an existing instrument, either build the
   instrument or default it off.
5. Never grow the model to chase an edge — that breaks the null control, which
   is the project's most valuable asset.
