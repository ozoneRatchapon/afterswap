# AfterSwap

> **Exhaustively enumerated exit machines, fighting over your position — live on DFlow.**
>
> DFlow × Superteam Thailand Buildathon 2026 — *"build what happens after the swap."*

You swapped into SOL. Now what? Every wallet goes silent at exactly the moment
that decides whether you make money: **the exit**. AfterSwap picks up where the
swap ends — it watches live DFlow quotes and lets a population of tiny machines
compete for the right to scale you out.

![dashboard](docs/dashboard.png)

## The idea (Wolfram ruliology, applied to trading)

Instead of hand-designing one exit heuristic (trailing stop, TWAP, take-profit
ladder), AfterSwap **enumerates the entire space of simple exit strategies** and
lets data pick the winner:

1. **Enumerate** — every deterministic finite-state machine up to N states over
   a binary input (price up-tick / down-tick) with a binary output (sell a
   tranche / hold). 3 states → **1,054 behaviorally distinct machines**
   (blake3-fingerprint dedup). No training, no gradients — the strategy space
   is *complete* by construction.
2. **Tournament** — every machine replays recent live price windows; a Pareto
   filter (edge vs complexity) plus a top-K cap keeps ~24 survivors.
3. **Bandit** — survivors become **UCB1 arms**. When you open a position, the
   bandit picks a machine to drive; every window its realized reward
   (tranche-exit value vs what naive holding would have done) updates its arm.
   Underperformers lose the seat.
4. **Gate** — a spectral irreducibility test on the win-matrix decides whether
   the next tournament can be **skipped, light, or full** — Wolfram's
   computational-irreducibility argument, used as a scheduler.

The engine is extracted from [`katgpt-ruliology`](https://github.com/katopz/katgpt-rs)
(Plan 188: "simple program strategies as bandit arms"), rebuilt standalone for
this buildathon on top of real market data.

## How DFlow integrates

DFlow is not a data feed bolted on the side — it is both the **sensor** and the
**actuator**:

- **Sensor**: the engine's only input is the implied price of DFlow
  [`GET /quote`](https://pond.dflow.net) (SOL→USDC, 0.1 SOL probe) polled every
  tick. The FSMs literally *are* functions of DFlow's aggregated liquidity —
  Tessera-routed quotes, not an oracle.
- **Actuator**: every `sell tranche` output maps to a DFlow swap. In **paper
  mode** (default) fills are simulated at the quoted price; in **live mode**
  the same event requests [`GET /order`](https://pond.dflow.net), signs the
  returned transaction with a local keypair, and submits it — declarative,
  sandwich-resistant execution for exactly the flow (small, repeated,
  uninformed tranches) DFlow's conditional liquidity is designed to price well.

```
DFlow /quote ──► tick ──► FSM population ──► UCB1 bandit ──► sell-tranche
     ▲                                                            │
     └────────────── DFlow /order (live mode) ◄───────────────────┘
```

## Run it

```bash
cargo run -p afterswap-server -- --serve 8787 \
  --interval-ms 1000 --window 12 --states 3 --tranche 0.1
```

Open http://localhost:8787, press **Open position** (paper — no wallet needed).
You'll watch machines get selected, sell tranches at live DFlow prices, and
accumulate realized edge vs holding on the leaderboard.

Terminal-only e2e (no dashboard):

```bash
cargo run -p afterswap-server -- --ticks 75 --interval-ms 1000 \
  --open-after 15 --window 12 --states 2
```

## Architecture

| Crate | What |
|---|---|
| `afterswap-engine` | FSM enumeration, window tournament, Pareto+cap pruning, UCB1 bandit, spectral simulation gate, tranche executor. Pure — no I/O. |
| `afterswap-dflow` | DFlow Trading API client (`/quote`, `/order`), price poller. Types verified against live captures. |
| `afterswap-server` | Paper loop + axum server, SSE snapshot stream, vanilla-JS/SVG dashboard. |

Every window the position is open emits an honest score:
`reward = tranche-exit value ÷ counterfactual hold value` (in bps). The
dashboard's hero number is that same measure over the whole position — if the
machines can't beat doing nothing, the number is red and we say so.

## Status & limits

- Paper mode is the demo: **quotes are real, fills are simulated** at quote
  price (no slippage/fee model beyond DFlow's own quoted amounts).
- Live mode sells real tranches but is deliberately minimal (throwaway keypair,
  no retry logic) — it is a buildathon proof, not custody software.
- Built during the buildathon (Aug 21–31, 2026); not previously released.

MIT.
