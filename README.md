# AfterSwap

> **DFlow × Superteam Thailand Buildathon 2026** — "build what happens after the swap."

The moment your [DFlow](https://dflow.net) swap fills, hundreds of exhaustively
enumerated finite-state machines start competing for the right to manage your
exit. Ruliology — Wolfram's exhaustive enumeration of simple programs — applied
to the question every trader faces *after* the swap: **when do I get out?**

Built on [`katgpt-ruliology`](https://github.com/katopz/katgpt-rs) (FSM
enumeration, Pareto pruning, UCB1 arms, simulation gating) + the DFlow
Trading API (quotes, orders, live price stream).

**Status: day 1 of 7 — engine done (6/6 tests), DFlow client + dashboard next.**

## How it works

1. **Enumerate** — all distinct 3-state FSMs (~956) over a binary alphabet:
   input = price ticked up/down, output = hold / sell-a-tranche.
2. **Tournament** — every machine replayed over rolling windows of live DFlow
   quotes; payoff = exit value edge vs pure hold (bps).
3. **Prune** — Pareto front on (payoff, complexity); survivors become UCB1
   bandit arms.
4. **Execute** — the selected machine drives real tranche exits through
   DFlow `GET /order`; realized edge feeds back as its reward.
5. **Gate** — a SimulationGate re-runs the tournament only when market
   dynamics look computationally irreducible.

## Workspace

- `crates/afterswap-engine` — pure exit engine (no IO), tests in `tests/`
- `crates/afterswap-dflow` — DFlow Trading API client (paper by default,
  `live` feature for real signing)
- `crates/afterswap-server` — axum + SSE dashboard server
- `web/` — dashboard
