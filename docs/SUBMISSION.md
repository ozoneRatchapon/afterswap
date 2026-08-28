# Submission draft — paste into the Google Form

## Exact form fields (as of the live form)

- **Email:** ratchapon.poc@gmail.com
- **Project Name:** AfterSwap
- **One-line description:** Exit autopilot for anyone who just swapped:
  1,054 exhaustively enumerated exit machines compete — and evolve — for
  the right to sell your position tranche by tranche on DFlow, honestly
  scored against doing nothing.
  - Strict-short variant: "1,054 evolving exit machines fight over your
    position — live on DFlow, scored honestly vs holding."
- **Project category:** Trading / DeFi
- **Team lead name:** Ratchapon [full name]
- **Telegram / X:** [your handles]
- **Team members + roles:** Solo — engineering, design, research. (Uses
  the third-party MIT crate katgpt-ruliology by @katopz for FSM
  enumeration primitives — disclosed below.)
- **Team size:** 1
- **Don't forget:** Demo Day signup is separate → https://luma.com/9pxt4y29


**Project name:** AfterSwap

**One-liner:** Exhaustively enumerated exit machines, fighting over your
position — live on DFlow.

**Team:** solo — [your name / @handle here]. Third-party: katgpt-ruliology
(MIT, @katopz) for FSM enumeration primitives.

**Description (short):**
Every swap UI goes silent at the moment that decides your PnL: the exit.
AfterSwap begins where the swap ends. It enumerates *every* deterministic
3-state exit strategy that can exist (1,054 behaviorally distinct
finite-state machines — Wolfram-style ruliology, no training), replays them
against live DFlow quote windows in a tournament, and lets the Pareto
survivors compete as UCB1 bandit arms for the right to scale you out,
tranche by tranche. Every window each machine's exit value is scored against
the honest counterfactual of doing nothing; losers get benched in real time.
A spectral irreducibility gate decides when the market has changed enough to
re-run the tournament. The dashboard shows the whole fight: live price,
tranche fills, the leaderboard, and the driving machine's state diagram.

**How DFlow is integrated:**
DFlow is both the sensor and the actuator. Sensor: the engine's only input
is the implied price from DFlow `GET /quote` (SOL→USDC probe, dev endpoint)
polled every tick — the machines are literally functions of DFlow's
aggregated liquidity. Actuator: every machine "sell tranche" output maps to
a DFlow swap — simulated at quoted price in paper mode; in live mode the
same event requests `GET /order`, signs the returned transaction with a
local keypair, and submits it. Small, repeated, uninformed tranches are
exactly the flow DFlow's declarative/conditional-liquidity design prices
well.

**Repo:** https://github.com/ozoneRatchapon/afterswap

**Demo video:** [link after recording — script in docs/DEMO.md]

**Run it:** `cargo run -p afterswap-server -- --serve 8787 --interval-ms 1000
--window 12 --states 3 --tranche 0.1 --replay data/recorded.jsonl`
then open http://localhost:8787 and press "Open position".

**Ecosystem benchmark:** measured against the exits Solana traders
actually use. On the 6-corpus mean (4 synthetic regimes + 2 recorded DFlow
segments) the engine beats every standard exit: TWAP/DCA **+75.7** bps,
TP-ladders **+113.7**, TP/SL brackets **+79.9**, Jupiter-style trailing stops
**+10.3** (bench `039_goat`, the shipped default configuration).

That mean is an upper bound, not a result, and we report it as such: it is
produced by the synthetic regimes, which are hand-specified and far cleaner
than real price action. **On the two recorded DFlow corpora alone the engine
loses to these floors** — trailing **−21.1**, TP-ladder **−7.9**, bracket
**−8.9** bps — while still clearing its two gated floors (TWAP **+75.73**,
random-arm **+6.90**). See `benches/017_real_horizon` for the larger
real-data test.

The trailing-stop result is the story: bench `004_goat` measured a −24.5 bps
*loss* (machines couldn't see distance-from-peak); we added that one input bit
and bench `005_goat` closed the gap — measured weakness → targeted fix,
all reproducible.
Same engine generalizes to DFlow prediction-market outcome tokens —
exits after the *bet*.

**Proof discipline:** GOAT-gated (methodology from katgpt-rs) — bit-reproducible
replays, byte-identical browser/native parity, ~1 µs/tick, every constant swept
for sensitivity (`benches/021_sensitivity`), pre-run power gating, an
evidence-ladder linter that fails the build when a claim outruns its evidence,
and four negative results recorded rather than deleted. Latest floors report:
`benches/039_goat/report.md`.

**Status:** built entirely during the buildathon (Aug 21–31); never released
before. Paper mode: quotes real, fills simulated. Live mode: feature-gated,
sells real tranches via DFlow orders.

## Full-form answers (second page)

**Working demo URL:** **https://afterswap.solana-thailand.workers.dev**
(deployed, verified). The full engine runs in the visitor's browser as
WASM; quotes fetched directly from DFlow's dev API (CORS-allowed). Add
`?replay` for the deterministic recorded segment. Zero servers — Cloudflare
Workers static assets. (Container/Fly kits remain in `deploy/` and repo
root as alternatives.)

**Where is DFlow used:** both directions — sensor (`GET /quote` polled
every tick is the engine's only market input) and actuator (each sell
signal → DFlow swap; live mode signs and submits `GET /order`
transactions).

**Why essential:** the product's promise is honest measurement against
*executable* prices — DFlow quotes are the ruler and the rails. The flow
generated (small, scheduled, deterministic, provably uninformed tranches)
is exactly what DFlow conditional liquidity prices well.

**Which product/API:** DFlow Swap API (dev): `/quote` + `/order`
(feature-gated Solana signing).

**User flow:** open dashboard (engine already polling DFlow) → Open
position → machine reads DFlow tick direction → sell-state fills tranche
at DFlow quote (live: signed /order tx) → windows score machine vs hold →
losers benched, mutants challenge → hero number = your edge.

**Built Aug 21–31:** everything, empty dir → v2.4 — incl. an on-chain
policy-registry program (Pinocchio, deployed on devnet, first policy
committed and verified) → engine (enumeration,
tournament, Pareto+cap, UCB1, spectral gate, evolution, renoise
confidence), DFlow client, axum+SSE dashboard (named machines,
plain-language feed), record/replay, the GOAT harness (G1–G6), a power module,
an overfitting test (CSCV/PBO), multiplicity correction (Romano–Wolf), a
pre-registration manifest and an evidence-ladder linter — plus an on-chain
policy program deployed to devnet and in-browser verification of every
DFlow-signed quote.

**Existed before?** No. Disclosure: depends on `katgpt-ruliology`, a
pre-existing third-party MIT open-source crate by @katopz (FSM enumeration
primitives, pinned public git rev) — used as a library dependency, same as
any crates.io dep; all product code built during the sprint.

**Explorer evidence (have now, no mainnet needed):**
- Policy program live on devnet:
  https://explorer.solana.com/address/GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8?cluster=devnet
- First committed exit policy (PDA, decoded+verified):
  https://explorer.solana.com/tx/2WHpDfMD3K5DNMheEdHKGm8djxKZPGeRLiYHyMmrVkzQKoykjz9iAQUBFEPquk8F3fdSRwow4BeDVqKgXqp4RLA5?cluster=devnet
- Optional extra: one mainnet live fill via wallet (Solscan link).
