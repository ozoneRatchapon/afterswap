# 2-minute demo script

Target: 120 seconds. Record the dashboard at 1280×900, terminal visible in a
corner for the enumeration log line.

## Prep (before recording)

```bash
cargo run -p afterswap-server -- --serve 8787 \
  --interval-ms 1000 --window 12 --states 3 --tranche 0.1
```

Wait ~20s so the first tournament has run (leaderboard populated). Browser at
http://localhost:8787, no position open.

## Script

**0:00–0:15 — hook.**
"Every swap UI goes silent at the moment that decides your PnL: the exit.
This is AfterSwap — it begins where the swap ends."

**0:15–0:40 — the idea.** Point at the Machines tile ("1,054 enumerated").
"Instead of one hand-designed exit heuristic, we enumerate *every* 3-state
exit machine that can exist — 1,054 of them — and replay each against live
DFlow quote windows. A Pareto filter keeps 24; they become bandit arms."

**0:40–1:20 — the fight.** Click **Open position**.
"I just opened half a SOL, paper mode, priced by real DFlow quotes."
Point at: FSM diagram ("this machine — it has a name, watch the feed —
is driving; orange states sell"),
first orange fill markers on the chart, fills tape ticking.
Read one feed line out loud — "Eager Puffin saw a dip, moved to its sell
state, sold 10%" — that's the whole explainability story in one sentence.
"Every window, the machine's exit value is scored against doing nothing.
Win → it keeps the seat. Lose → the bandit benches it." Point at leaderboard
real-bps column updating.

**1:20–1:45 — the number.** Point at hero stat when position closes.
"That's the whole product in one number: edge versus never selling, measured
honestly on live DFlow prices."

**1:45–2:00 — DFlow + close.**
"DFlow is the sensor and the actuator — quotes in, and in live mode every
sell tranche is a signed DFlow order, with the machine's exit policy
committed on-chain before the first sale (program live on devnet). Post-swap execution, running
strategies nobody designed. AfterSwap."

## Deterministic mode (recommended for recording)

Live markets can go flat mid-take. Replay a recorded segment instead — same
engine, same dashboard, reproducible theater:

```bash
cargo run -p afterswap-server -- --serve 8787 --interval-ms 1000   --window 12 --states 3 --tranche 0.1 --replay data/recorded.jsonl
```

The recording loops, so the demo never runs out. To capture a fresh segment
from live quotes, add `--record data/my-session.jsonl` to any live run.
State honestly in the video if the segment is replayed ("recorded DFlow
quotes" on the narration) — quotes are still real DFlow data.

## Fallback

If the market is dead flat during recording, the machines mostly hold —
that's fine: narrate "flat market, machines correctly sit on their hands,"
then cut to a pre-recorded active segment (screenshots in docs/).
