# What else is possible — DFlow surface × katgpt-rs primitives

Written after the harness killed our own alpha claims (see README §2 and
`benches/018_train_test`). The point of this file is to answer, honestly:
given that direction prediction did not work, *what is actually left*, what
is measurable, and how would we know if it worked.

Everything marked ✅ was verified against the live API on 2026-08-27; ⛔ was
tried and did not work from our environment; 🔒 exists but needs access.

## 1. The DFlow surface, and how much of it we touch

| Surface | Status | What it would unlock for us |
|---|---|---|
| `GET /quote` | ✅ used | Price sensor. Our whole engine input today. |
| `GET /order` | ✅ used | Signed swap transaction — our actuator in live mode. |
| Multi-size `/quote` probing | ✅ used, then closed | **Executable depth**: BONK moves ~27 bps between a small and a large clip, SOL/USDC ~0.3 — a liquidity signal no CEX candle contains, and **not capturable**: 40.2 bps of pool fee, priority tip and latency drift on public routes leave −13.2 bps net. |
| `routePlan[].venue` | ✅ **newly exploited** | Free with every quote: which venues fill, how many hops. Route churn is a thin-liquidity tell; venue identity is a fill-quality tell. |
| `GET /venues` | ✅ available | 20+ venues enumerated (Whirlpools, Raydium ×4, Meteora ×3, HumidiFi, Byreal, Deriverse …). Lets us attribute execution quality per venue. |
| `GET /priority-fees` | ✅ available | Fee regime is a congestion signal *and* a real cost term our simulator currently ignores. |
| `GET /order-status` | ✅ available | Realized fill vs quoted — the ground truth for execution-quality work. |
| `GET /tokens` | ✅ available | Universe selection without hardcoding mints. |
| Declarative swaps (`/intent`, `/submit-intent`) | ⛔ `route_not_found` on the dev endpoint | Sandwich resistance and lower slippage on exactly our shape of flow (small, repeated, uninformed tranches). The single biggest *measurable* execution win available. |
| Book stream WS (10 levels of depth) | 🔒 access-gated | Real depth instead of inferred depth. Plan 001 becomes an order-book strategy rather than a probing hack. |
| Quote stream WS | 🔒 same gate | Sub-second sensor without polling; removes our 1 s quantization. |
| Sponsored swaps | ✅ documented | Gasless demo: a visitor could run a *real* mainnet tranche without holding SOL. |
| Request signing | ✅ **used** | Verified live: `x-sign-request: true` on the keyless dev endpoint returns RFC 9421 headers (`signature`, `signature-input`, `content-digest`), signed ed25519 under DFlow's published key, CORS-exposed. The demo now verifies both the body digest and the signature **in the visitor's own browser** before the machines act on a price. |
| Proof (identity) | ✅ documented | Verified-identity leaderboards; a machine marketplace needs sybil resistance. |
| MCP server / Agent CLI / Skills | ✅ documented | Distribution to agents — the market our `/decide` API and `docs/API.md` target. |

**Reading:** we are using perhaps a fifth of what DFlow exposes, and the parts
we are not using are precisely the parts where effects are *deterministic and
measurable* (execution cost, verifiability) rather than statistical (alpha).

## 2. katgpt-rs primitives worth pulling, re-selected for what survived

Our earlier ports (FSM enumeration, mutation, simulation gate, temporal
derivative, renoise) all aimed at *predicting better*. The evidence says stop.
These aim at *risking less* and *proving more* — closed-form, modelless, and
compatible with the null control:

| Primitive | Why it fits now |
|---|---|
| **Conformal predictive intervals** (Plan 340, default-on upstream) | Coverage-guaranteed bands with no training. Turns "sell 10%" into "sell 10% with a stated worst case", and it ships with the *Report the Floor* rule we already live by. |
| **Tropical (max,+) algebra** (Plan 337) | Aggregates by worst case rather than average — the right semiring for drawdown-first objectives, which is what an exit product should optimize. |
| **Viable manifold walk** (Plan 312) | Safe-navigation: stay inside a set of states from which recovery is possible. Reframes exits as "never enter an unrecoverable position", which is a claim we could actually validate. |
| **Step attribution / Δ≥0 commit gate** (Plan 381) | Only act when the improvement is provable; otherwise hold. Directly implements "do nothing unless you can show it helps". |
| **Self-advantage gate** (dead-compute detector) | Skip evaluation entirely when the state cannot change the decision — pure cost saving, measurable in µs. |
| **QMC belief sampling** (Plan 367) | Lower-variance estimates from the same number of windows: strictly better statistics, and statistics is our bottleneck. |

## 3. Ranked opportunities

Ordered by (defensibility × measurability) ÷ effort. "Measurable" means we can
attach a floor and a standard error, which is the only kind of claim this repo
is allowed to make.

1. **Execution-cost research, not alpha research.** Slippage, priority fees and
   venue choice are large (tens of bps), deterministic, and attributable.
   *This is where a real, defensible number is still available to us.*
   **First result in (`benches/019_cost`): per-fill cost is not the hidden
   variable.** Charging 0→5 bps on every fill, to the engine and to every floor
   alike, moves each comparison by less than half a bp — the tranche-count
   asymmetry we expected does not materialise because the engine often does not
   finish its ten tranches and the trailing stop often never triggers. The cost
   model stays in the engine (`fill_cost_bps`) for live trading; it is not a
   research lever. What remains untested here is the *declarative vs
   imperative* comparison, which needs production API access.
2. **End-to-end verifiable exits** — signed quote + committed policy PDA +
   on-chain fill. Nobody in this space can currently prove "the fill followed
   the policy, at a quote the venue actually offered". Uniquely ours because
   the policy program already exists on devnet.
   **✅ All three links shipped.** (1) Every quote is verified in-browser
   (RFC 9421 ed25519 against DFlow's published key) and an unverified quote is
   discarded, not traded on. (2) The policy PDA commits the machine
   fingerprint before the first sale. (3) The commitment transaction now
   carries the verified quote digest in a memo beside it, binding the policy
   to a specific signed price — verified on devnet
   (`afterswap:quote sha-256=…`, no program change needed).
   Remaining to make it end-to-end for *mainnet fills*: the same memo on the
   live sell transaction, which needs the production API.
3. ~~**Depth/venue-aware exits** (Plan 001)~~ — **closed**. The spread is
   real (27 bps on BONK) but smaller than its unavoidable cost on public
   routes (40.2 bps: 25 pool fee + 10 tip + 5 drift + 0.2 L1) — **−13.2 bps
   net**. Economics, not measurement.
4. **Execution-quality public dataset** — quotes, depth, venues, realized fills
   over time. A "DFlow execution weather report" is a product on its own and
   costs us only disk.
5. **Agent distribution** — the modelless decision layer at $0 marginal cost
   over the MCP/CLI/Skills surface DFlow already publishes.
6. **Prediction-market and perps exits** — same machinery where holding to
   resolution or to liquidation is the default failure.

## 4. How to research a market, compare, and improve

The method this repo has converged on, stated so it can be reused:

1. **Name the incumbent.** Not "the market" — the thing a user does today
   (TWAP, trailing stop, TP ladder, bracket, doing nothing). If you cannot
   name it, you cannot claim to beat it.
2. **Pair the comparison.** Run the alternative on the *same* path from the
   *same* entry. Unpaired absolute returns are ~7× noisier here, which is why
   534 live cycles still said nothing.
3. **Attach a standard error to every number.** Three claims in this project
   survived until an SE column was added and then died.
4. **Split by data provenance.** Report synthetic and real separately, always.
   Our headline was produced entirely by synthetic regimes and we did not
   notice until the split was automated.
5. **Validate out of *distribution*, not just out of sample.** A clean
   train/test split inside a wrong data-generating process still misleads
   (bench 015). Real data, chronological split, then a *different asset*.
6. **Test the selection, not just the result.** Two of four assets clearing
   significance is what luck looks like; eleven assets said so.
7. **Keep a null control.** A random-walk corpus where the correct answer is
   "nothing". If a change makes money there, the change is a bug.
8. **Publish the negative results.** They are the only reason to believe the
   positive ones.
