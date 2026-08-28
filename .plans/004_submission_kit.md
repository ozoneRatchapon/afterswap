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

> An exit that runs without you, follows a policy committed on-chain **before**
> it sells, costs nothing per decision, and is honest about not beating a
> trailing stop — because it was measured well enough to know.

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

**1:32–1:55 — The honest result. Do not skip this.**
Show the bench table or the README section.
> "Does it beat a trailing stop? No. We tuned on the first 60% of eleven real
> assets and scored on the last 40%: plus ten basis points, inside the noise.
> A full multiplicity correction across all 1,054 machines returns zero
> survivors. We can reliably pick the best machine; the best machine isn't
> profitable — and we published that instead of burying it."

**1:55–2:00 — The close.**
> "What's left is worth having: an exit that runs without you, commits before it
> sells, costs nothing, and reports its result honestly. That's the bar most
> retail exits don't even attempt."

### Recording notes
- Use the **BONK** pair for visual activity (it moves), but do **not** narrate
  the BONK edge — see the framing decision above.
- `?replay` gives a recorded deterministic segment: use it if live quotes are
  flat or the network is unreliable during recording. It guarantees the same
  visuals every take.
- **Do not demo `POST /decide` on camera.** It is still serving the pre-fix
  build and currently fails; the in-browser WASM path is the one that works and
  is the better story anyway.
- Record 1440p or better if the roster table is on screen — fingerprints need
  to be legible for the "not designed by us" point to land.

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

**Known caveat to state if asked about the API:**
> The hosted `POST /decide` endpoint is a preview on the Workers free plan and
> is currently unreliable; the in-browser WASM path has no such limit.

## Checklist

- [x] Video script written to time, with a shot list and a stated framing
      decision (lead with rigor, not with a number).
- [x] Recording notes covering the `?replay` fallback and the `/decide` trap.
- [x] Form fact sheet assembled from defensible figures only, including an
      explicit limitations answer.
- [ ] **Record the 2-minute video** — user-only.
- [ ] **Submit the Google Form before 23:59 ICT Sun 31 Aug 2026** — user-only.
