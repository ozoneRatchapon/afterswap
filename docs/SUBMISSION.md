# Submission draft — paste into the Google Form

**Project name:** AfterSwap

**One-liner:** Exhaustively enumerated exit machines, fighting over your
position — live on DFlow.

**Team:** solo — [your name / @handle here]

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

**Proof discipline:** the engine is GOAT-gated (methodology from katgpt-rs):
bit-reproducible replays, +59.0 bps mean vs a same-cadence TWAP floor and
+5.8 bps vs random arm selection across 5 corpora, 1 µs/tick — full report
in `benches/001_goat/report.md`.

**Status:** built entirely during the buildathon (Aug 21–31); never released
before. Paper mode: quotes real, fills simulated. Live mode: feature-gated,
sells real tranches via DFlow orders.
