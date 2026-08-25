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

- [ ] Functional prototype (public URL)
- [ ] 2-minute demo video
- [ ] Public repo (this one) — push to GitHub
- [ ] "How DFlow integrates" writeup (README section)
- [ ] Application via Google Form on stth-buildathon.vercel.app

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
