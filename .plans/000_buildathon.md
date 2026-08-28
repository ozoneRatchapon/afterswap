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
      **Pre-recording flight check done 2026-08-28** (still `[ ]`: the
      recording itself is the deliverable and needs a human). Every shot
      dependency re-verified live — page/WASM/engine assets 200, served
      WASM 487,094 B = 476 KB, the 1,054 figure test-pinned, `?replay`
      200, and both devnet fallback PDAs present at 60 bytes. Table in
      `.plans/004_submission_kit.md`. One claim was corrected: the
      "11 demo commits spent" figure was removed as stale and
      unverifiable — `/slot` increments on read and the GET surface hides
      the counter, so the budget cannot be observed without spending it.
      Everything except the recording is prepared in
      `.plans/004_submission_kit.md`: a shot-by-shot script timed to 0:00–2:00,
      recording notes (use `?replay` if live quotes are flat; `/decide` is now
      deployed and measures 80/80 across two independent 40-call runs, so it is
      safe but optional to demo), a **"do not say" guardrail** listing the
      keeper / gasless / delegated-execution claims the program does not
      support, and a stated
      framing decision — lead with the rigor, not with the BONK number, because
      the README's own finding is that there is no durable edge.
      **Memo-shot risk retired, 2026-08-28 (verified live, not reasoned).** An
      earlier note warned the 1:10–1:32 commitment shot needs
      `signAndSendTransaction` and could trip Phantom's new-domain heuristics
      on workers.dev. That was wrong: it described the *live-mode* path
      (`commitPolicy`, `index.html:579`, `CHAIN = "solana:mainnet"`, real SOL,
      reachable only with the `livemode` checkbox on). The shot uses
      `commitDemoPolicy` (`index.html:963`), which fires automatically when
      live mode is **off**, is signed server-side by `/api/commit-policy`, and
      is broadcast to devnet by the page — no wallet, no popup, no heuristic.
      The script's "no wallet needed" line was already correct.
      Exercised end-to-end against production on 2026-08-28: `/api/commit-policy`
      → 200 with `signed_tx` + `policy_pda`, broadcast to devnet, PDA
      `ExiLSj7CGwFF1bknhJ6h48s1L5RZf8bKec1Nc2hZYcnt` created owned by
      `GEz2tF…8bD8`, 60 bytes decoding to position_id 10, fingerprint
      `0x165ef4aabbcc`, 3 states, tranche 1000 bps, committed_at
      2026-08-28T16:05:21Z. Cost: 1 of the 380-commit demo budget
      (`Scoreboard.MAX_DEMO_COMMITS`) and ~5000 devnet lamports.
      Fallback still available if the live take misbehaves: the original PDA
      `5LRDFS9WckZUA1BNoBmt6N3A6r2Pzie3TcULADSKEXiA` re-verified the same day —
      same fingerprint/states/tranche, commit tx succeeded (`err: None`,
      slot 488168150, fee 5000).
      **Flight check re-run 2026-08-29** so the take rests on same-day probes,
      not two-day-old ones: `/` and `/?replay` 200, served WASM still
      487,094 B, `/pkg/afterswap_wasm.js` 200, repo 200, `POST /decide` 200
      returning the documented `fills 7 / edge 700`, and all three devnet
      accounts still live (program executable, both fallback PDAs 60 bytes).
      Full table under "Re-verified again 2026-08-29" in
      `.plans/004_submission_kit.md`. **Nothing is left to prepare — the script,
      shot list, timings, fallbacks and guardrails are all written; only the
      recording itself remains, and it needs a human.**
      **Re-checked after the 2026-08-29 deploy.** The page the camera will see
      is no longer the one the script was written against: `/` is now 54,215 B
      and carries three `@media` breakpoints, so it lays out correctly below
      desktop width instead of overflowing. This *helps* the take — the window
      no longer has to be full-width to look right — and changes no shot, since
      every element the script names is unmoved and no JS was touched. Both
      WASM assets hash identically to the pre-deploy build, so the 1,054-FSM
      engine shot and the `?replay` fallback behave exactly as rehearsed.
- [x] Public repo (this one) — push to GitHub — `origin` is
      `https://github.com/ozoneRatchapon/afterswap.git`, `develop` and `main`
      both published. The /decide FSM-table fix (`cd75dcd`, `424c998`,
      `68bb7e0`, `27a3661`) is pushed *and* is now deployed (version
      `4f69c750`, 2026-08-28) — the old "source-only, NOT deployed" note here
      is superseded.
      **The ahead-by-N staleness found 2026-08-29 is RESOLVED.** The repo was
      public but behind: `origin/develop` sat at `6a4987f` while local carried
      six unpublished commits (`d549910`, `dcba122`, `8ea2850`, `da9dbb0`,
      `b49b5f1`, `c4cac83`). Pushed 2026-08-29 — `6a4987f..c4cac83`, all six
      published. A judge opening the repo now sees bench 040 (the
      Schmitt-trigger null), the recovered research doc, the synthetic-null
      leakage answer, the responsive/a11y fix to the demo page, and the
      Browser-Integrity-Check diagnosis. Nothing outstanding here.
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
      **Paste-ready as of 2026-08-29.** Every field is drafted: one-liner, live
      URL, repo, program ID, "how DFlow integrates", "what's novel", honest
      limitations, the synthetic-null / leakage answer, the MiCA "aligned with
      (not compliant)" answer, the DFlow-signed vs Jupiter-self-attested
      asymmetry, and the API caveat — now carrying **40/40 on two separate
      days**. The `curl` in the answer table was re-run today and returns the
      documented body. **The last open question is closed too:** the `403` was
      Cloudflare's zone-default **Browser Integrity Check** (`error code:
      1010`), not Bot Fight Mode, and the decision is to change nothing and
      ship the `curl` — see below.

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

> **SUPERSEDED 2026-08-28 (kept as record).** The user ran the deploy; version
> `4f69c750` shipped the FSM-table fix and the re-measurement was taken. See
> the checklist entry at the top of this file and `003_post_deploy_doc_edits.md`.
> `/decide` has since measured **40/40 four times across two days** (08-28 and
> 08-29). Nothing below this line about "pre-fix build in prod" is still true.

## Status 2026-08-28 late (/decide fix — SUPERSEDED, it is now in prod)

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

~~**NOT DEPLOYED.**~~ **Resolved.** This paragraph recorded a deploy blocked by
the Claude Code auto-mode permission classifier. The user ran it; production
serves version `4f69c750`, live `/decide` measures **40/40** (08-28 ×2, 08-29
×2), and the three doc updates landed. Retained only so the blocked-then-
unblocked sequence stays legible.

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

## Status 2026-08-29 — submission prep closed out

Both remaining checklist boxes are user-only and stay `[ ]`; everything that
could be prepared for them is now done, and one open question was closed.

### The `403` is Browser Integrity Check, not Bot Fight Mode

The kit had recorded the Python `403` as "Cloudflare edge bot management on the
account (Bot Fight Mode or a managed WAF rule)" and as "stateful or
probabilistic". Reading the response body instead of the status line settles it:

```
HTTP 403 · Server: cloudflare · error code: 1010
```

**Error 1010 is the Browser Integrity Check**, which Cloudflare documents as
denying visitors "lacking standard user agents", and which is **on by default**.
Four probes today:

| Client | `POST /decide` |
|---|---|
| `urllib` default (`Python-urllib/3.x`) | **403 · 1010**, 12/12 |
| `curl/8.7.1` | **200** |
| Chrome UA | **200** |
| `python-requests/2.32.3` | **200** |

Three corrections follow, and all three shrink the risk:

1. **Not probabilistic** — 12/12, flipping purely on `User-Agent`, same body and
   route in the same minute. The earlier "200 earlier, 403 later" was a
   different UA, not drift.
2. **Not ours and not `/decide`-specific** — plain `GET /` 403s from the same
   client, so it is the whole hostname.
3. **The realistic Python judge is unaffected** — `python-requests`, what anyone
   actually reaches for, returns 200. Only `urllib`'s bare default UA is banned.

**Decision: change nothing.** BIC is a *zone-level* setting and `workers.dev` is
Cloudflare's zone, not one in this account, so the toggle is most likely not
even present in the dashboard. Making it present would mean attaching a custom
domain two days before the deadline, for a client no judge will use. The form
answer already hands judges a paste-ready `curl`, which is free, reversible and
warms the worker. If a judge does report a 403, the one-line reply is in
`004_submission_kit.md`.

### Everything perishable re-verified

`/`, `/?replay`, both WASM assets, the repo, `POST /decide` (documented body),
the policy program and both fallback PDAs — all live today. `/decide` measured
**40/40 twice more** (p50 74/64 ms, p95 132/112 ms), so the "80 of 80" claim now
holds on two separate days. Table in `004_submission_kit.md`.

### Both non-user commands are now DONE — only the video and the form remain

- ~~**`git push origin develop`**~~ — **DONE 2026-08-29**: `6a4987f..c4cac83`,
  all six commits published. The repo a judge opens now matches local `develop`;
  bench 040, the recovered research doc, the leakage answer and the demo-page
  a11y fix are all visible. This item needed the user only because the auto-mode
  classifier had blocked it on an earlier pass — it was not blocked on retry.
- ~~**`npx wrangler deploy`**~~ — **DONE 2026-08-29** (run by the user; the
  classifier blocked it here on every attempt). Commit `b49b5f1` — responsive
  breakpoints, `:focus-visible`, `<noscript>`, `modulepreload`, CSS/attributes
  only, no JS touched — is now live.

**Post-deploy verification, run live 2026-08-29 after the deploy landed:**

| Check | Result |
|---|---|
| `/` | **200 · 54,215 B · 0.10 s** — byte-identical to local `index.html` |
| `/?replay` | 200 · 54,215 B |
| `/pkg/afterswap_wasm_bg.wasm` | 200 · 487,094 B · sha256 **unchanged** |
| `/pkg/afterswap_wasm.js` | 200 · 14,688 B · sha256 **unchanged** |
| Public repo | 200 |
| Rail `/rail/stats` | 200 · 0.33 s |
| `POST /decide` (documented `curl`) | 200 — `fills 7`, `fully_exited false`, `edge_vs_hold_bps 700` |
| `scripts/decide_measure.sh 40` | **40/40**, p50 67 ms, p95 125 ms, max 132 ms |
| Program `GEz2tF…8bD8` | live devnet, executable, owner `BPFLoaderU…` |
| PDA `5LRDFS9W…EXiA` | live, 60 B, owner `GEz2tF…8bD8` |
| PDA `ExiLSj7C…Ycnt` | live, 60 B, owner `GEz2tF…8bD8` |

All seven shipped markers confirmed present in the served HTML: the three
`@media` breakpoints (980/760/560px), `:focus-visible`, `<noscript>`,
`role="status"`, `modulepreload`. The engine did not regress — both WASM assets
hash the same before and after — and `/decide` now measures **40/40 on three
separate runs across two days**, so the "80 of 80" line in the form answer is
if anything understated.

### What is genuinely left

Two items, both requiring a human, neither preparable any further:

1. **Record the 2-minute video.** Script, shot list, timings, framing decision,
   `?replay` fallback, the "do not say" guardrail and two fallback PDAs are all
   written in `.plans/004_submission_kit.md`. Warm the page first — cold is
   ~2 s, warm ~0.1 s.
2. **Submit the Google Form** — **closes 23:59 ICT Sun 31 Aug 2026**.
   Every field is paste-ready in `.plans/004_submission_kit.md`. Unsubmitted
   is zero regardless of everything above.
