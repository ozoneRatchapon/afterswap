# Submission kit — demo video script + form fact sheet

The two remaining boxes in `000_buildathon.md` are user-only: recording a video
and typing into a Google Form. Neither can be done from here. What *can* be
done here is everything except the recording and the typing, so that both are
short mechanical acts rather than blank-page work under a deadline.

**Deadline: 23:59 ICT, Sun 31 Aug 2026.** A working prototype that is never
submitted scores zero. This is the highest-value item left in the project.

## The framing decision, made once

The temptation in a buildathon video is to claim an edge. **Do not.** The
README's headline finding is that there is **no durable edge** — Romano–Wolf
stepdown returns zero survivors across 1,054 machines on 11 assets, and every
increase in statistical power shrank the apparent edge. The BONK **+34 ± 10 bps**
figure is real but the README itself reads it as *selection* (2 of 11 assets
clearing significance is what selection looks like), and the live BONK soak
testing exactly that is still running.

So the video sells what is actually defensible and rare:

> An exit that runs on its own in your browser, follows a policy committed
> on-chain **before** it sells, costs nothing per decision, and is honest about
> not beating a trailing stop — because it was measured well enough to know.

Judges see optimistic backtests all day. A team that built a harness good enough
to catch itself, and then published the negative result, is the differentiated
thing here. Lead with the rigor, not with a number.

## 2-minute video script (0:00–2:00)

Shot list assumes screen recording of `https://afterswap.solana-thailand.workers.dev`
with voiceover. Times are cumulative; total 120 s with ~5 s of slack.

**0:00–0:12 — The problem, on screen, no slides.**
Show a wallet or the swap moment, then the dashboard.
> "You swapped into SOL. Every wallet goes silent at exactly the moment that
> decides whether you made money — the exit. AfterSwap is what happens after
> the swap."

**0:12–0:30 — The enumeration. This is the hook.**
Show the live roster of machines with names and fingerprints.
> "Nobody designed these strategies. We enumerate every deterministic 3-state
> exit machine that can exist — 1,054 after behavioural dedup — and let them
> compete for the right to scale you out. No model, no training. A decision
> costs zero dollars and about 1.2 microseconds."

**0:30–0:48 — It's running in your tab. No backend.**
Open devtools or just say it while the quotes tick.
> "There is no backend. The whole engine is a 476 KB WASM binary running in
> your browser, polling DFlow directly — byte-identical to the native build.
> Nothing to rug-pull, nothing to pay for."

**0:48–1:10 — DFlow as sensor and actuator, with the signature check.**
Show a quote arriving; point at the verification.
> "DFlow is both eyes and hands. The implied price from `/quote` is the engine's
> only input, and every sell-tranche maps to an `/order`. And every quote is
> signature-checked in your own tab — RFC 9421, ed25519, against DFlow's
> published key. A quote that fails is discarded, not traded on. That costs
> 0.055 milliseconds."

**1:10–1:32 — The on-chain commitment. The strongest single claim.**
Show the devnet explorer link for a fresh commitment.
> "Before it sells, the machine commits its policy on-chain — an 18 KB Pinocchio
> program on devnet writes the blake3 fingerprint to an immutable PDA for 3,285
> compute units, bound to the exact signed quote by a memo. So 'this fill
> followed a policy committed in advance, at a price the venue really offered'
> is three cryptographic facts, not a claim. And it happens to you just by
> opening the demo — no wallet needed."

*Recording note (verified live 2026-08-28).* This shot needs **no wallet and
no live mode**. `commitDemoPolicy` (`index.html:963`) fires on its own once a
live arm exists while the `livemode` checkbox is **off**; the Worker signs at
`/api/commit-policy` and the page broadcasts to devnet, so Phantom never
appears and its new-domain heuristic never applies. Do **not** tick `livemode`
for this shot — that is the other path (`commitPolicy`, `index.html:579`),
which is mainnet, spends real SOL, and does pop the wallet.
Two gotchas: the commit is rate-limited to **one per browser per hour** via
`localStorage["afterswap-demo-commit-at"]`, so clear that key (or use a fresh
profile) before the take; and it is capped at **380 total** across all
visitors (`Scoreboard.MAX_DEMO_COMMITS`). **The remaining budget cannot be
read without spending it:** `/slot` (`worker/scoreboard.ts:38`) increments
on every read, and the GET surface deliberately excludes the counter
(`WHERE floor != 'slot'`, line 83). An earlier "11 spent" figure was
removed here — it was stale (the 2026-08-28 end-to-end test spent one more)
and is not re-checkable. Recognise exhaustion on camera by its signature:
`/api/commit-policy` returns **429 `demo commit budget spent`**. If that
appears, stop and use the fallback PDA below rather than retrying.
Note the counter increments *before* the devnet broadcast is known to have
succeeded, so a failed broadcast still burns one.
Fallback if nothing commits on camera: open the already-verified PDA
`5LRDFS9WckZUA1BNoBmt6N3A6r2Pzie3TcULADSKEXiA` on the devnet explorer
(fingerprint `0x165ef4aabbcc`, 3 states, 10% tranches) and narrate the same
line — the claim is the commitment existing, not it being made on camera.

### Pre-recording flight check (all re-verified live 2026-08-28)

Every shot dependency was exercised against production immediately before this
was written, so the take should not surface a dead surface. Re-run these if you
record on a later day — the first three are perishable.

| Shot | Dependency | Verified state |
|---|---|---|
| 0:12–0:30 | "1,054 machines" | `fsm_table::decode(3).len() == 1054`, pinned by `crates/afterswap-engine/tests/fsm_table.rs:52` |
| 0:30–0:48 | "476 KB WASM" | served `/pkg/afterswap_wasm_bg.wasm` = **487,094 bytes = 476 KB**, HTTP 200 |
| 0:30–0:48 | page + engine assets | `/`, `/pkg/afterswap_wasm.js`, `/pkg/afterswap_wasm_bg.wasm` all **200** (0.06–0.29 s) |
| 0:48–1:10 | live quote feed | `/?replay` **200**; replay fallback wired at `index.html:704`, and auto-engages after 5 unreachable ticks (`index.html:1017`) |
| 1:10–1:32 | fallback PDA | `5LRDFS9WckZUA1BNoBmt6N3A6r2Pzie3TcULADSKEXiA` — live on devnet, **60 bytes**, owner `GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8` |
| 1:10–1:32 | 2026-08-28 test PDA | `ExiLSj7CGwFF1bknhJ6h48s1L5RZf8bKec1Nc2hZYcnt` — same, also live (second usable fallback) |

**Two things this check could not cover.** The remaining demo-commit budget is
unobservable without consuming one (see the note above), so whether the live
1:10 commit fires on camera is genuinely unknown until you record — that is
what the two verified fallback PDAs are for. And a warm-up is still required:
the first calls against an idle Worker can take ~2 s, so load the page once
before rolling.

**1:32–1:55 — The honest result. Do not skip this.**
Show the bench table or the README section.
> "Does it beat a trailing stop? No. We tuned on the first 60% of eleven real
> assets and scored on the last 40%: plus ten basis points, inside the noise.
> A full multiplicity correction across all 1,054 machines returns zero
> survivors. We can reliably pick the best machine; the best machine isn't
> profitable — and we published that instead of burying it."

**1:55–2:00 — The close.**
> "What's left is worth having: an exit that runs on its own in your tab,
> commits before it sells, costs nothing, and reports its result honestly.
> That's the bar most retail exits don't even attempt."

### Recording notes
- Use the **BONK** pair for visual activity (it moves), but do **not** narrate
  the BONK edge — see the framing decision above.
- `?replay` gives a recorded deterministic segment: use it if live quotes are
  flat or the network is unreliable during recording. It guarantees the same
  visuals every take.
- **`POST /decide` is now safe to demo** (deployed 2026-08-28, version
  `4f69c750`; measured 80/80 over two independent 40-call runs, p50 76/69 ms,
  p95 134/125 ms). The earlier "do not demo
  it" note applied to the pre-fix build and no longer holds. It is still
  optional: the in-browser WASM path remains the better story, and a live curl
  costs seconds of runtime you may not have in a 2-minute cut.
- Record 1440p or better if the roster table is on screen — fingerprints need
  to be legible for the "not designed by us" point to land.
- **Warm the endpoint before demoing it live.** Re-measured 2026-08-28: the
  80/80 success rate reproduces, but the *first* run after the worker has been
  idle puts about three of forty calls near 2.1 s (cold start). A second run
  immediately after is p50 59 ms / p95 125 ms / max 155 ms, zero calls over
  1 s. Fire a throwaway `/decide` call before the take.

### Do not say — claims the build does not support

Verified against the source 2026-08-28. Each of these is a real feature of some
*other* product, and saying it invites a judge to open the program and find it
missing.

- **"Keepers execute the exits."** The string `keeper` appears **nowhere in the
  source or the docs** — the only hits in the tree are inside this guardrail.
  There is no keeper, no crank, no off-chain executor.
- **"Gasless tranche exits."** Nothing is gasless. The user signs the policy
  commitment and pays that fee plus PDA rent. Outside this guardrail, `gasless`
  occurs exactly once in the tree — `docs/OPPORTUNITIES.md`, listing sponsored
  swaps as an *unbuilt* opportunity ("a visitor **could** run…").
- **"On-chain policy delegation" / "sign once and it trades for you."** The
  Pinocchio program (`crates/afterswap-policy/src/lib.rs`) has exactly **one**
  instruction, `CommitPolicy`, which writes an immutable PDA. It has no exit
  instruction and no delegate authority. Its own doc comment says "Phase B
  (delegated execution) builds on this" — i.e. delegated execution is not
  built. `docs/ROADMAP.md` §4 marks it post-buildathon work.

**What to say instead** — the commitment story, which is fully shipped and is
the stronger claim anyway:

> "You sign once to commit the policy on-chain — an immutable PDA holding the
> blake3 fingerprint of the exact exit machine that governs the position,
> written *before* any fill follows it. That commitment is what makes every
> later fill auditable against a rule fixed in advance. Delegated execution
> against that commitment is Phase B, and it is not in this build."

The engine itself does run unattended — in the browser tab, with no wallet
popup per decision, because it is simulating and deciding rather than signing.
Say "runs on its own in your tab", not "runs without you", so the difference
between *deciding* and *custodially executing* stays visible.

## Google Form fact sheet — copy-paste answers

The form's exact fields are on `stth-buildathon.vercel.app` and are not known
from here, so this is the raw material rather than a field-by-field fill.
**Every figure below is one the repo can defend.**

| Field likely asked | Answer |
|---|---|
| Project name | AfterSwap |
| One-liner | Exhaustively enumerated exit machines, fighting over your position — live on DFlow. |
| Live URL | https://afterswap.solana-thailand.workers.dev |
| Repo | https://github.com/ozoneRatchapon/afterswap |
| Track / theme | "Build what happens after the swap" |
| Chain / network | Solana; exit-policy registry live on **devnet** |
| Program ID | `GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8` (devnet) |
| Try the API | paste-ready `curl` below — **give this, not a bare URL** (see the 403 caveat) |

**Paste-ready API check (verified working 2026-08-28):**

```sh
curl -s https://afterswap.solana-thailand.workers.dev/decide \
  -H "content-type: application/json" \
  -d '{"prices":[1.0,1.02,1.04,1.06,1.08,1.1,1.12,1.14,1.16,1.18,1.2,1.22,1.24,1.26,1.28,1.3,1.32,1.34,1.36,1.38,1.35,1.32,1.29,1.26,1.23,1.2,1.17,1.14,1.11,1.08,1.05,1.02,0.99,0.96,0.93,0.9],"open_at":1}'
```

Returns `fills: 7`, `fully_exited: false`, `edge_vs_hold_bps: 700` on a
rise-then-fall series — the machine scales out on the way up while hold rides
the whole drawdown down. **Say what this is when you show it:** a hand-made
price path chosen to make the mechanism visible, *not* evidence of edge. The
honest-limitations answer above is the claim; this is a functioning-endpoint
demo. Quoting the +700 without that sentence walks straight into the
"do not say" guardrail.

Two reasons to hand judges this command rather than the bare URL: it warms the
worker (so they see the ~60 ms path, not a ~2 s cold start), and `curl` is not
subject to the 403 that a plain Python client can hit.

**How DFlow is integrated (2–3 sentences):**
> DFlow is both sensor and actuator. The implied price from `/quote` is the
> engine's only input — no oracle, no other feed — and every sell-tranche the
> winning machine emits maps to a `/order`. Every quote is verified in the
> browser against DFlow's RFC 9421 ed25519 signature before it is allowed to
> influence a decision.

**What's novel (2–3 sentences):**
> Rather than designing exit strategies, we enumerate all 1,054 distinct
> deterministic 3-state exit machines and run a tournament between them, with
> the winner's policy committed to Solana before it sells. The result is
> auditable end-to-end: a signed venue quote, a pre-committed blake3 policy
> fingerprint on-chain, and the fill that followed it.

**Honest limitations (include this — it is the differentiator):**
> We found no durable edge over standard exits and we say so: across 11 real
> assets, +10.5 ± 7.9 bps vs trailing stops, inside the noise, and a
> Romano–Wolf multiplicity correction across all 1,054 machines returns zero
> survivors. The product is a disciplined, verifiable, zero-cost automated exit
> — not alpha.

**If asked how you know the pipeline is not leaking (look-ahead / overfit):**
> We ran our own selection pipeline against a synthetic null. Bench 036 sweeps
> an AR(1) parameter with unconditional volatility pinned at 8 bps per tick, so
> `phi` is the only thing that varies, and its `phi = +0.0` arm is a series with
> no predictability at all (realised rho_1 = +0.0025, 20 seeds, 200 windows of
> 120 ticks, all 1,054 machines, picked on the first 60% and scored on the rest).
> A pipeline with look-ahead leakage would still report a positive selection
> differential there. Ours reports **Delta = -0.365 bps** — negative, and well
> inside its own -8.7 to +12.9 seed spread, i.e. indistinguishable from zero,
> which is the only honest answer on an unpredictable series. The companion
> number moves the right way too: **PBO peaks at 0.564 at phi = 0** and falls to
> ~0.36 at |phi| = 0.4, so the selection is a coin flip exactly where there is
> nothing to find and better where there is.

Why this belongs in the form answer: it is the one result that shows the *method*
was tested, not just the strategy. The honest-limitations answer says we found no
edge; this says the machinery that failed to find one is also incapable of
inventing one. Source: `benches/036_reversion_causal/report.md` (control sweep),
with `benches/037_reversion_power/report.md` narrowing the sweep to the band our
real assets occupy.

**Known caveat to state if asked about the API:**
> The hosted `POST /decide` endpoint runs on the Workers free plan, whose
> 2,010 ms CPU ceiling used to kill about half of cold starts. That is fixed —
> the 1,054-machine enumeration is precomputed into a 2,108-byte table, and the
> endpoint now measures 80 of 80 across two independent 40-call runs, p95 134 ms
> and 125 ms (2026-08-28). It was measured twice on purpose: the pre-fix build's
> rate swung run to run (20/40, then 25/40), so one clean run could not have
> distinguished a fix from a lucky draw. The in-browser WASM path has no such
> ceiling at all.

**Before submitting — two API-probe caveats found by re-verification 2026-08-28:**

1. **The p95 figures are warm-state.** Re-measured today: 80/80 again, and a
   warm run reproduces p50 59 ms / p95 125 ms / max 155 ms exactly. But the
   first run against an idle worker put ~3 of 40 calls near 2.1 s. Every call
   still returned 200, so "80 of 80" is safe to assert as written; the p95
   number is not, unless the endpoint is warm. If a judge cold-curls it once,
   they may see ~2 s. Say "p95 125 ms warm" rather than "p95 125 ms".

2. **A plain Python client can get `403 Forbidden` from the documented API.**
   Reproduced 12/12 with `urllib`'s default headers against
   `POST /decide`; `curl` and browser user-agents get 200. Nothing in
   `worker/` or `crates/afterswap-worker/` blocks anything — the scan is
   clean — so this is **Cloudflare edge bot management on the account**
   (Bot Fight Mode or a managed WAF rule), configured in the dashboard, not
   in the repo. It is also not a stable header rule: the same UA returned 200
   minutes earlier and 403 later, so it is stateful or probabilistic.
   **Risk:** the form answer points judges at `POST /decide`, and a judge who
   reaches for Python gets a 403 and reasonably concludes the API is broken.
   **Decide before submitting** — either turn Bot Fight Mode off for the
   submission window (an account setting, so a human call, not one I can or
   should make), or give judges a `curl` invocation in the form answer rather
   than a bare URL. The second is free and reversible; prefer it.

**Known caveat to state if asked about regulation / MiCA:**
> The verifiable rail produces best-execution artifacts **aligned with MiCA
> Article 78** — every venue quoted per execution, a pre-committed decision
> rule, a second venue captured beside the primary, immediate public reads, and
> hash-chained records whose segment roots are anchored on-chain (devnet). We
> deliberately do **not** say "compliant": that is a determination for a
> regulator and counsel, not for a codebase. `docs/RAIL.md` §0 states this
> ceiling in the repo itself.

**If asked how strong the audit trail is — do not flatten the two venues:**
> DFlow signs its quotes (RFC 9421, ed25519) so anyone can re-verify forever
> that DFlow *offered* that price. Jupiter does not sign anything, so its leg is
> self-attested: we record the full body and digest and sign the observation,
> which proves we saw it, not that Jupiter offered it. Recording that asymmetry
> honestly rather than flattening it is the point.

## Checklist

- [x] Video script written to time, with a shot list and a stated framing
      decision (lead with rigor, not with a number).
- [x] Recording notes covering the `?replay` fallback and `/decide` status.
- [x] **"Do not say" guardrail** — keeper / gasless / delegated-execution
      claims audited against the source and ruled out, with the shipped
      commitment story written as the replacement line.
- [x] **MiCA answer held at "aligned with", not "compliant"**, plus a
      DFlow-signed vs Jupiter-self-attested answer so the evidence asymmetry
      is not flattened under questioning.
- [x] Form fact sheet assembled from defensible figures only, including an
      explicit limitations answer.
- [x] **Re-verified every perishable prod claim in this kit against live
      production, 2026-08-28** (after the rail deploy). App worker still at
      version `4f69c750`; `/decide` reproduces **80/80** across two
      independent 40-call runs. Two corrections landed above rather than
      being left to a judge to find: the p95 figure is **warm-state only**
      (a cold worker puts ~3 of 40 calls near 2.1 s), and the documented API
      returns **403 to a plain Python client** (reproduced 12/12; not our
      code — Cloudflare edge bot management on the account). Mitigated with
      a paste-ready `curl` in the answer table.
- [x] **Synthetic-null / leakage answer added**, sourced from bench 036's
      `phi = +0.0` arm (Delta = -0.365 bps, PBO 0.564). Figures read from
      `benches/036_reversion_causal/report.md`, not from memory.
- [ ] **Record the 2-minute video** — user-only.
- [ ] **Submit the Google Form before 23:59 ICT Sun 31 Aug 2026** — user-only.
