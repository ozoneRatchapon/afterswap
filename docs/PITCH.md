# Pitch kit — one product, five stories

The rule: same facts, different *first sentence*. Every audience gets the
truth; each gets the truth they care about first.

---

## 1. Traders / end users — "the bag-sitter"

**15 sec:** "You're great at buying and terrible at selling — everyone is.
AfterSwap is 1,054 tiny robots that fight for the right to sell your bag,
tranche by tranche. The ones that lose money get benched. Live, on DFlow."

- Language: no FSMs, no bandits. Say **robots**, **benched**, **scoreboard**.
- Demo choreography: open a position, point at the **activity feed** ("watch
  them take the seat, sell, get scored"), end on the **hero number** ("green
  means the robots beat you doing nothing").
- The hook is honesty: "most tools show you PnL; we show you PnL *versus
  having done nothing*. If we can't beat doing nothing, the number is red
  and we say so."

## 2. Buildathon judges — map to their four words

They said: *usefulness, clarity, execution, originality.*

- **Usefulness:** every swap creates a position; no wallet helps you exit
  it. We begin where the swap ends — the literal theme.
- **Clarity:** the entire product is one number — edge vs never-selling,
  measured on live DFlow quotes. 30-second demo, one button.
- **Execution:** working prototype, live DFlow integration both directions
  (/quote sensor, /order actuator), 15 tests, deterministic replay,
  microsecond engine. Built solo in days on a proven engine (katgpt-rs).
- **Originality:** nobody hand-designed these strategies. We enumerated
  *every possible* 3-state exit machine, then let evolution breed 4-state
  ones no enumeration could reach. Judges will not see a second entry that
  ships a leaderboard of *evolved finite-state machines* with a proof
  report.

**One-liner for the form:** "Exhaustively enumerated exit machines,
fighting over your position — live on DFlow."

## 3. DFlow team — speak order-flow

**15 sec:** "AfterSwap turns one swap into a stream of small, scheduled,
*provably uninformed* sells — exactly the flow your conditional liquidity
is designed to price well. We're a post-swap volume factory."

- Their thesis: protect MMs from toxic flow → better pricing for benign
  flow. Our machines are deterministic public policies with a blake3
  fingerprint — the *least* informed flow that can exist. A wallet running
  AfterSwap should, in principle, earn better quotes. That's a research
  conversation, not just an integration.
- Roadmap tease: map sell-states to **declarative orders** (the FSM *is* a
  conditional-liquidity program), and commit the machine's hash on-chain →
  verifiable exit policies on DFlow rails.
- Ask: dev API rate limits / an API key, and whether prediction-market
  tokens (Kalshi) are in scope for exit machines.

## 4. Technical experts — the proof-first story

**15 sec:** "No model, no training, no prompt. We enumerate the complete
space of 3-state exit FSMs — 1,054 after behavioral dedup — tournament
them on live quote windows, run the survivors as UCB1 arms, and evolve
past the enumerable frontier. Bit-reproducible, gate-tested, 1.2 µs/tick,
pure Rust."

- Lead with the **GOAT report** (`benches/002_goat/report.md`): named
  floors (TWAP, random-arm), 5 corpora, ablations, bit-identical replays.
  "+71.9 bps vs TWAP mean across 6 corpora" is a claim with a reproduce command, not a chart
  crop.
- The spicy details they'll bite on: Pareto front degenerating to the whole
  space on flat markets (why the cap exists), rewarding partial windows so
  fast exits still teach the bandit, renoise rank-stability as per-decision
  confidence, spectral irreducibility as a re-tournament scheduler.
- Provenance: FSM enumeration/mutation primitives from @katopz's open-source katgpt-ruliology crate (MIT, pinned git rev); all trading logic, integration, and proofs built during the sprint.
- **The modelless thesis (credit @katopz):** many decisions don't need an LLM at all — rules + shallow reasoning + ruliology gets you $0 marginal cost, ~µs latency, determinism, and self-evolution. AfterSwap is that thesis applied to exits: the LLM was the *compiler* (built the system once, held accountable by GOAT gates that reverted two of its confident ideas); the *engine* never makes an API call. In an agent economy, the modelless decision layer undercuts every LLM-per-decision competitor on unit economics — that's the business, not just the aesthetics.

## 5. Community / social (Superteam TH) — the meme

- "Your exit is currently being decided by machine **#165ef4, generation
  2**. It has never read a chart. It is beating you."
- "1,054 robots walked into a tournament. 24 got jobs. One is selling my
  SOL right now."
- Thread hook: screenshot of the hero number + activity feed; CTA = run it
  yourself in one cargo command (no wallet needed, paper mode).
- Demo-day booth trick: let the audience open the position — "crowd opens,
  robots close." The dashboard is the spectacle; keep it on the big screen.

---

## Demo-day extras

- **On-chain program (when asked "where's the program?"):** "Live on devnet — an 18 KB Pinocchio policy registry; the machine's fingerprint is committed to an immutable PDA before the first fill follows it. Here's the explorer link, and the first committed policy decoded on-chain. Phase B is SPL-delegate execution validated against that commitment — approval once, not per tranche, still non-custodial." (explorer: GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8, devnet)
- **The redemption-arc benchmark line:** "We benchmarked against what Solana actually uses and found we *lost* to Jupiter's trailing stop by 24 bps — because our machines couldn't see distance-from-peak. We added that one input bit. Now we beat every standard exit on Solana: DCA +87, TP-ladders +123, brackets +81, and trailing stops themselves +2 — and trailing-stop behavior simply *emerged* from the enumeration as a special case. Every number reproduces with one command." (A measured weakness fixed beats a spotless claim.)
- **Engineering-discipline line (if asked about process):** "We also tried Plackett–Luce ratings for arm ranking — measured worse on every floor, reverted, and kept the negative result in the repo. We ship what measures better, not what sounds smarter."

## Q&A armor (any audience)

- **"Is +5.8 bps even a lot?"** — On one 2-minute demo, no. The point is
  the *measurement discipline*: honest counterfactual, gate-tested across
  regimes (+364 bps in the downtrend regime, where exits matter). Scale
  comes from horizon and size, honesty doesn't change.
- **"Why not an LLM/ML model?"** — Exits need determinism, auditability,
  and microsecond latency. A 16-byte machine you can verify beats a black
  box you can't. (And: it's provably uninformed flow — an ML exit isn't.)
- **"What if the machines are wrong?"** — They're scored every window and
  benched in real time; the confidence badge says how stable the current
  decision is; and the worst case is bounded by tranching.
- **"Is this released?"** — No. Built entirely during the buildathon.
