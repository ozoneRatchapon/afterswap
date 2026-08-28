# Plan 001 — Liquidity-aware exits (the DFlow-native edge)

> **CLOSED 2026-08-27 — economics, not measurement.** The depth spread this
> plan was built to capture is 27 bps on BONK against 40.2 bps of unavoidable
> cost on public routes (25 pool fee + 10 priority tip + 5 latency drift +
> 0.2 L1), i.e. **−13.2 bps net**. The preliminary A/B had already found no
> benefit from a depth input bit; the arithmetic explains why looking harder
> would not have helped. Reopen only alongside private transaction tunnels,
> zero-fee routing and just-in-time on-chain simulation — all three, since
> the pool fee alone nearly exhausts the spread. Recorder stopped; the
> collected quotes stay in `data/incoming/` as a depth/venue dataset.

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
- [x] **Second run at 1,207 ticks** (bench 029_signal_bits, 12 train / 8 test):
      2-bit **+8.7 ± 8.0**, depth **+9.2 ± 10.5**, route-churn **+8.4 ± 8.2**,
      single-hop **+6.8 ± 8.6**. Every candidate sits inside every other
      candidate's standard error. **MDE is 22–30 bps**, so what this actually
      establishes is: no DFlow-only signal helps *by more than ~22 bps*, which
      is a weak statement. The harness now prints that column so the weakness
      is visible rather than implied.
- [—] **Re-run again at ~5,000 ticks** — **CLOSED, not executed.** The corpus
      is 1,207 ticks and the recorder stopped 2026-08-27 19:06. Reaching 5,000
      means restarting it for ~13 more hours on a track already closed for
      arithmetic, not for measurement (−13.2 bps net). Collecting more data
      *after* seeing an unfavourable result, in the hope the next look reads
      differently, is exactly the optional-stopping pattern commit `5b3eedd`
      pre-registered this project against. The null stands as recorded.
- [—] **Thread depth through the engine** — **CLOSED, not taken.** It was
      gated on the 3-bit protocol winning and it did not: bench 029 put depth
      at +9.2 ± 10.5 against the 2-bit control's +8.7 ± 8.0. No engine change
      was made; `sim::replay_exit_with_bit` stays a harness-only path, so the
      shipping engine never carried the feature and needs no revert.
- [x] **A/B on a frozen corpus** — ran against a stationary file: the recorder
      was already stopped when benches 029 and 038 read
      `data/incoming/bonk_depth.jsonl`, and it has not changed since.
      **The file deliberately stays in `data/incoming/`.** GOAT's `corpora()`
      does a non-recursive `read_dir("../../data")`, so promoting it would add
      a seventh corpus and silently move every documented GOAT and bench
      number in the repo — the precise failure ROADMAP 7e's freeze rule was
      written after. Freezing means "not changing under an experiment", not
      "moved into the scanned set"; for a closed track the move buys nothing
      and costs the comparability of every prior bench.
- [x] **Ship or revert with a recorded reason**; update README claim table —
      **reverted**, reason in README §"What we stopped doing, and why" (the
      27 bps depth spread against 40.2 bps of unavoidable cost) and in this
      plan's header. Claim table refreshed 2026-08-28 to bench 039.
- [x] Re-run GOAT gates + wasm parity (G1–G6) before any deploy — **all PASS**
      on 2026-08-28, `benches/039_goat/report.md`: G1 determinism, G2a TWAP
      +75.73, G2b random-arm +6.90, G3 worst cap cost −0.6 (budget −10), G4
      686 ns mean / 111.4 µs worst, G5 evolution ablation, G6 wasm
      byte-parity; 7/7 in `tests/goat.rs`. Two things fell out: the README
      gate table had been quoting the *surprise-trigger-ON* numbers that
      ROADMAP retraction 1 turned off by default, and `scripts/g6_parity.sh`
      could not find the `.wasm` when the target dir comes from
      `~/.cargo/config.toml` rather than `$CARGO_TARGET_DIR`.

## Running in parallel while the recorder fills

- [x] **Demo pair switcher** — SOL/USDC (familiar, no edge) or BONK/USDC
      (where bench 018 measured +34 ± 10 vs trailing). Switching resets the
      per-browser scoreboard so two markets never mix in one statistic.
- [x] **Live soak moved to BONK** with paired evaluation
      (`--pair bonk --paired`), so the live evidence is gathered in the market
      the claim is about rather than the one it is not.
- [ ] Report the BONK paired soak once it has enough cycles for a t-value.
      **Pre-registration, written 2026-08-28 before a single cycle was
      collected** (the live evidence in `docs/SOAK.md` is SOL/USDC only, so
      the market the +34 ± 10 claim is about has never been soaked):
      - Primary endpoint: **mean `vs_trailing_bps` per completed cycle**, with
        its standard error and t. Chosen because bench 018's BONK claim is
        against trailing stops.
      - Secondary, reported but not interpreted as findings: vs hold, TWAP,
        ladder, bracket.
      - Stopping rule: **stop at 300 completed cycles or at 4,500 ticks,
        whichever comes first** — fixed now, not revisited after looking. If
        the run is cut short, report the cycle count reached and treat the
        result as underpowered rather than re-launching for more.
      - Paper mode, live BONK/USDC quotes, zero capital.

      **Harness defect and restart, 2026-08-28 08:51 UTC — disclosed.** The
      first launch (PID 22851, 07:02 UTC) reached tick 3,146 having recorded
      exactly **one** cycle. Cause: the CLI paper loop latched `opened` on the
      first position and never cleared it, so the position closed at tick 44
      and the process idled for the remaining ticks. `--paired` had therefore
      never been capable of producing the pre-registered sample; the dashboard's
      learn-forever reopen was never wired into the CLI. Fixed in `0415e44`
      with a regression test (`tests/paired_soak_cycles.rs`, verified to fail
      without the fix: 1 cycle in 1,200 ticks, and 58 cycles in 1,500 with it).

      This restart is **not** the "cut short → re-launch for more" the stopping
      rule forbids: the first run did not collect an underpowered sample, it
      collected no sample, because the instrument did not implement the design.
      The stopping rule is unchanged (300 cycles or 4,500 ticks, whichever
      first). **Bias vector disclosed:** that single cycle was visible before
      the restart and was favourable (+0.61 bps vs trailing). It is therefore
      **discarded, not merged** — the new run starts from an empty file
      (the broken output is archived outside the repo at
      `/tmp/bonk_soak_paired.BROKEN_1cycle.jsonl`). Restarted as PID 25782.

      **Analysis-script amendment, 2026-08-28 10:28 UTC — disclosed, made
      while blind to the data.** Auditing `scripts/soak_report.sh` before the
      run finished (soak at tick ~2823/4500; the paired file was NOT read —
      not a row, not a summary) turned up two arithmetic faults. Both are
      corrections, not design changes: the primary endpoint, the secondary
      list, the reported order and the stopping rule are all untouched.

      1. **MDE was wrong by 40%.** The script computed `1.96 * se * 2`
         (3.92·SE) and labelled it the minimum detectable effect at 80% power.
         The correct quantity — and this repo's own audited definition, in
         `crates/afterswap-engine/src/power.rs::mde_from_se` — is
         `(z_α + z_power)·SE` = 2.8016·SE. The script had been contradicting
         the crate it is meant to report on. That module exists precisely
         because this project already shipped two ~9%-power experiments, so an
         inflated MDE here is a repeat of the exact failure it was written to
         prevent. On a worked example the reported MDE drops 4.5 → 3.2 bps.
      2. **Significance assumed n was large.** The verdict used a hardcoded
         normal cutoff of 1.96, justified in-comment by "n here is large
         enough" — an assertion about n written before n was known, and wrong
         if the 4,500-tick cap binds before 300 cycles. It now computes the
         exact two-sided Student-t p-value on n−1 df (regularized incomplete
         beta, Lentz continued fraction), verified against published critical
         values at df = 10, 20, 49, 86, 1000 and the normal limit — all
         return p = 0.05000. The report now prints p alongside t.

      Also hardened: the script fails with a named-field error instead of a
      `KeyError` traceback if the paired schema drifts from
      `afterswap-server::shadow::PairedCycle`, and it states explicitly that
      the secondary rows are uncorrected for multiplicity.

      The duplication that allowed fault 1 to persist — a shell script
      re-declaring constants the crate already owns — is now covered by
      `tests/power.rs::soak_report_script_agrees_with_power_module`, which
      asserts the script carries both z constants at full precision, applies
      them in the `(Z_ALPHA + Z_POWER80) * se` shape, and has not
      reintroduced the 3.92·SE form. Mutation-tested: reverting the live line
      to the old formula makes it fail, restoring it makes it pass.

## Adjacent results while the recorder fills

- [x] **Execution-cost model** (`fill_cost_bps`, cost-aware floors) — shipped,
      then measured: not a lever (bench 019_cost, <0.5 bp across 0→5 bps).
- [x] **Venue capture** added to the recorder: every quote already carries
      `routePlan[].venue` and hop count, so route churn is a free
      thin-liquidity signal alongside the size-spread one.
- [x] Depth A/B once the recording has ≥ 6 windows — done twice: bench 022
      (4/4 windows, preliminary) and bench 029 (12 train / 8 test, 1,207
      ticks). Bench 038 later reused the same recording for a different
      question and found the depth reading *is* worth 34.6% CUPED variance
      reduction at lag 1 — as a control variate, not as an exit signal.

## Rules this plan inherits

1. No claim without a floor and a standard error.
2. Corpus set frozen during an A/B; in-progress recordings live in
   `data/incoming/`.
3. Real data beats synthetic; report them split.
4. If a feature cannot be measured by an existing instrument, either build the
   instrument or default it off.
5. Never grow the model to chase an edge — that breaks the null control, which
   is the project's most valuable asset.
