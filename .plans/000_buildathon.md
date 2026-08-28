# AfterSwap — DFlow × Superteam Thailand Buildathon

> Theme: "build what happens after the swap". Deadline **23:59 ICT Aug 31, 2026**.
> Demo Day Sep 3, Bangkok. Prizes 500/300/200 USDC. Solo entry.

## Pitch

**AfterSwap** — the moment your DFlow swap fills, ~956 exhaustively enumerated
finite-state machines start competing for the right to manage your exit.
Wolfram-style ruliology (via `katgpt-ruliology` from katgpt-rs): enumerate ALL
simple exit programs, tournament them on rolling windows of live DFlow quotes,
Pareto-prune (payoff × complexity), and let a UCB1 bandit pick which machine
drives real tranche exits. A SimulationGate decides when market dynamics are
irreducible enough to justify re-running the tournament.

Demo line: "22 to 956 tiny machines fight over what happens after your swap."

## Architecture

- `crates/afterswap-engine` — DONE (2026-08-24). Pure, no IO.
  - `sim::replay_exit` — FSM over price window: input = tick direction,
    output 1 = sell tranche. Edge vs hold in bps. Hand-math verified.
  - `sim::evaluate_matrix` — strategy×window WinMatrix (scaling note in doc).
  - `bandit::ExitBandit` — UCB1 over `RuliologyArm`s (crate's from_strategies
    is game-payoff-only, so arms built here from Pareto survivors).
  - `engine::ExitEngine` — on_tick loop: bootstrap tournament → pruner →
    arms → live FSM drives position → realized reward per window →
    SimulationGate routes skip/light/full re-tournaments. Realized stats
    carried across rebuilds by FSM id (mean-seeded).
  - Tests 6/6 green in `tests/engine.rs` (incl. crash e2e: full exit locks
    value >0.6 while hold floor ≈0.45; monotone-rise invariant: nobody
    beats hold).
- `crates/afterswap-dflow` — STUB. Next: REST client.
- `crates/afterswap-server` — STUB. Then: axum + SSE + static dashboard.
- `web/` — dashboard (vanilla, no build step). Invoke dataviz skill first.

## DFlow integration (verified live 2026-08-24, no API key needed on dev)

- Base: `https://dev-quote-api.dflow.net`
- `GET /quote?inputMint=&outputMint=&amount=&slippageBps=` → Jupiter-style
  quote (outAmount/inAmount = price; routePlan venue; contextSlot).
  SOL→USDC probe worked: SOL ≈ $96.70, venue "Tessera V".
- `GET /order?...&userPublicKey=` → quote + base64 unsigned tx +
  lastValidBlockHeight (sign + send to mainnet RPC ourselves).
- WS streams exist (quote/book/priority-fees) — docs at
  pond.dflow.net/resources/trading-api/websockets/*.md (append .md!).
- Price source v1: poll /quote every ~2s (0.1 SOL notional). v2: WS stream.
- Paper mode default (real quotes, simulated fills). `live` feature flag →
  solana-sdk signing for real tranche orders (tiny size for video).

## Deps

- `katgpt-ruliology` pinned git rev `6f392727…` (public upstream
  katopz/katgpt-rs; contains all needed APIs). Engine compiles against it.

## Day plan

- D1 Aug 24 ✅ scaffold, engine + 6 tests, DFlow probes, this plan
- D2 Aug 24 ✅ (early) dflow crate + paper loop; live e2e verified
- D3 Aug 24 ✅ (early) axum + SSE + dashboard v1 (validated dark palette,
  hero edge stat, price chart w/ fill markers, leaderboard, FSM diagram,
  gate panel, fills tape). Arm cap (24), bootstrap gate label,
  close-reward partial window, ClosedSummary locked result. Screenshots:
  /tmp/aswap_dash.png, /tmp/aswap_dash2.png. Run:
  `cargo run -p afterswap-server -- --serve 8787 --interval-ms 1000
   --window 12 --states 3 --tranche 0.1`
- D4 Aug 27 — dashboard v2 (FSM state diagrams, gate panel); `live` feature
  (solana-sdk sign+send); one real tiny mainnet swap
- D5 Aug 28 — deploy (fly.io), README polish, screenshots
- D6 Aug 29 — 2-min video, submission form, buffer
- D7 Aug 30–31 — slack for breakage; WASM stretch goal only if free
- Sep 3 — Demo Day BKK

## Submission checklist

- [x] Functional prototype (public URL) — `https://afterswap.solana-thailand.workers.dev`
      returns 200 (125 ms); the rail at
      `…-rail.solana-thailand.workers.dev/rail/stats` returns 200 (362 ms).
      Re-verified again immediately before submission, 2026-08-28: app 200
      (90 ms), rail `/rail/stats` 200 (416 ms), public repo 200 (933 ms).
      `POST /decide` was serving a failing pre-fix build; the FSM-table fix was
      deployed 2026-08-28 (version `4f69c750`) and re-measured at 80/80 across
      two independent 40-call runs, p50 76/69 ms, p95 134/125 ms — see
      `.plans/003_post_deploy_doc_edits.md`. All three surfaces are healthy.
- [ ] 2-minute demo video — **user-only**, cannot be produced from here
      Everything except the recording is prepared in
      `.plans/004_submission_kit.md`: a shot-by-shot script timed to 0:00–2:00,
      recording notes (use `?replay` if live quotes are flat; `/decide` is now
      deployed and measures 40/40, so it is safe but optional to demo), and a
      stated
      framing decision — lead with the rigor, not with the BONK number, because
      the README's own finding is that there is no durable edge.
- [x] Public repo (this one) — push to GitHub — `origin` is
      `https://github.com/ozoneRatchapon/afterswap.git`, `develop` and `main`
      both published. The /decide FSM-table fix (`cd75dcd`, `424c998`,
      `68bb7e0`, `27a3661`) is pushed. Note this is source-only — it is NOT
      deployed to prod.
      (**No current-HEAD hash is recorded here on purpose.** This entry went
      stale four separate times by naming one, because every later commit
      invalidated it — a fact that needs re-verifying on every read is worse
      than no fact. Check it live instead:
      `git rev-parse --short develop origin/develop` — equal means published.
      The checkbox is about the repo being public and
      current, which it is.)
- [x] "How DFlow integrates" writeup (README section) — README
      §"How DFlow integrates": DFlow as both **sensor** (implied /quote price
      is the engine's only input) and **actuator** (every sell-tranche maps to
      a /order), with the data-flow diagram
- [ ] Application via Google Form on stth-buildathon.vercel.app — **user-only**
      Answer text assembled in `.plans/004_submission_kit.md`, including a
      limitations answer. **Closes 23:59 ICT Sun 31 Aug 2026** — highest-value
      remaining item in the project.

## Status 2026-08-26 (v1.9)
- SHIPPED: engine (enum+tournament+UCB1+evolution+renoise+gate), dflow
  client (+live feature), axum server, WASM in-browser build DEPLOYED at
  https://afterswap.solana-thailand.workers.dev (zero servers), any-wallet
  live sells (Wallet Standard), localStorage learning persistence,
  auto-reopen learn-forever, GOAT G1–G6 all PASS (bench 004: +60.0 vs
  TWAP, +95.4 vs TP-ladder, +53.3 vs bracket, −24.5 vs trailing —
  disclosed), named machines + Thai-friendly UX.
- PARKED with evidence: docs/ROADMAP.md (input alphabet v2 #1 priority).
- REMAINING (user-only): 2-min video, Google Form (Aug 31 23:59 ICT),
  Demo Day luma signup, optional wallet-signed evidence tx.

## Status 2026-08-26 late (v2.1)
- Alphabet v2 SHIPPED (off-peak bit via 2-step unroll; bench 005/007:
  beats ALL standard Solana exits — TWAP +87.4, ladder +122.8, bracket
  +80.7, trailing +2.0; was −24.5 vs trailing in bench 004).
- PL ratings tried→reverted with record (bench 006; module kept).
- On-chain policy-commitment Memo before first live fill SHIPPED.
- G1–G6 re-validated incl. wasm byte-parity; 18 tests green; prod
  deployed. Docs synced (SUBMISSION/PITCH/README numbers = bench 007).

## Status 2026-08-26 night (v2.4)
- Policy program REWRITTEN in Pinocchio (74KB→18KB) and DEPLOYED devnet:
  GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8; first policy PDA committed
  + decoded/verified on-chain (user funded 5 devnet SOL).
- 2nd recorded corpus (792 ticks) in GOAT; bench 011 (6 corpora, final 884-tick corpus2): TWAP
  +71.9, trailing +6.5, ladder +109.9, bracket +76.1 — all floors beaten.
- Gate suite 25x faster (enumerate cache). Live soak monitor: 165+ cycles,
  mean +0.20 bps, learning trend positive. Docs synced to bench 010.

## Status 2026-08-28 (gates re-run)

GOAT G1–G6 re-run end to end on the shipped default configuration
(`benches/039_goat/report.md`): G1 determinism, G2a TWAP **+75.73**, G2b
random-arm **+6.90**, G3 worst cap cost **−0.6** (budget −10), G4 mean
`on_tick` **686 ns** / worst **111.4 µs**, G5 evolution ablation, G6 wasm
byte-parity — all PASS, 7/7 in `tests/goat.rs`.

The README's gate table had been carrying **+76.0 / +5.4**, which are the
*surprise-trigger-ON* numbers that ROADMAP retraction 1 turned off by
default. The table now reports the configuration that actually ships and
cites bench 039 instead of bench 001.

`scripts/g6_parity.sh` resolved the build directory from
`${CARGO_TARGET_DIR:-target}`, which cannot see a shared `target-dir` set in
`~/.cargo/config.toml`; the gate died on a missing `.wasm` rather than on a
parity failure. It now asks `cargo metadata`.


### Post-deploy re-measurement — prepared, 2026-08-28 10:42 UTC

The three documents that quote the pre-fix "20 ok, 20 failed" figure each
promise in writing to be updated with a measurement. Both halves of keeping
that promise are now done except the deploy itself:

- `scripts/decide_measure.sh [n]` runs the same 40-call procedure that produced
  the original figure and prints `n= ok= fail= rate= p50= p95= max= codes=`.
  Written down as a script so the re-measurement is the *same* measurement
  rather than a new one wearing its name. Validated end-to-end against prod.
- `.plans/003_post_deploy_doc_edits.md` holds the exact replacement text for
  README.md, docs/API.md and docs/ROADMAP.md, with only `<OK>`/`<FAIL>`/`<P50>`
  left to substitute. Written before the deploy so the wording cannot be tuned
  to a flattering result, and it carries a failure variant as well as a success
  one. Every anchor string was checked to match its file verbatim.

Spot checks at 10:39 UTC returned **0 ok / 7 failed** (all 503) against the
pre-fix build still in prod — consistent with the documented per-isolate
failure clustering, and not itself a measurement. Prod is unchanged; the
deploy remains harness-blocked here and needs the user.

## Status 2026-08-28 late (/decide fix, not yet in prod)

Cold `POST /decide` was blowing the Workers Free-plan 2,010 ms CPU ceiling
(~50% of calls returned Cloudflare 1101) because `FsmEnumerator::enumerate(3)`
ran per cold isolate. The enumeration is pure and deterministic, so it is now
computed at development time and shipped as a packed index table
(`crates/afterswap-engine/src/fsm_table_{1,2,3}.bin`, 2,108 B for the whole
n=3 space). Cold native enumeration 224.9 ms → 132.5 µs; cold `/decide` on
local `workerd` 752 ms → 7 ms (same-harness, both measured). WASM grew
473→476 KB (155 KB gz). `tests/fsm_table.rs` gates the table field-for-field
against live `FsmEnumerator::enumerate`, so it cannot silently drift.

Verified locally: clippy clean, workspace tests green, `scripts/g6_parity.sh`
G6 PASS, `wrangler deploy --dry-run` OK (515.98 KiB / 177.12 KiB gz).

**NOT DEPLOYED.** `npx wrangler deploy` was denied twice by the Claude Code
auto-mode permission classifier; production still serves the pre-fix build, so
the live `/decide` reliability is still the old 20/40. The prod re-measurement
and the three doc updates (README, docs/API.md, docs/ROADMAP.md, all of which
currently say "pending") are gated on that deploy and remain undone.

### Remaining unchecked, and why

Both open checklist boxes above are **user-only actions** and cannot be
produced from this environment:

- 2-minute demo video — requires recording/narration.
- Google Form submission on stth-buildathon.vercel.app — requires the user's
  own form entry. Deadline 23:59 ICT Aug 31, 2026.

Adjacent blocked items, tracked in their own plans: the BONK paired soak
report (`.plans/001_execution_edge.md`, soak still running, pre-registered
stopping rule forbids early reads) and the deliberately-skipped R2 bucket
(`.plans/002_verifiable_rail.md` §8 free-tier invariant).
