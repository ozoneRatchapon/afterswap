# 006 — Millisecond cadence: what it takes, measured

Owner asked 2026-08-30 whether the swap loop can run in ms instead of
seconds, then green-lit an investigation. **Hard rule observed throughout:
nothing here touches the deployed demo before the form deadline
(23:59 ICT Sun 31 Aug).** Prod stays `91e6771`, ticking at 1 s.

## What was measured (all probed live 2026-08-30, not assumed)

### The current feed is the bottleneck, and it has no fast lane
- `dev-quote-api.dflow.net/quote` round trip: **1.05–1.32 s** over five
  samples; ~0.8–0.9 s of that is the server itself (connect subtracted).
  Polling it faster than ~1 s returns the same quote later.
- No streaming surface exists on that host: `/docs`, `/stream`,
  `/quote/stream` all 404 (`/` is 200).

### Pyth Hermes is no longer a free drop-in
- `hermes.pyth.network/v2/updates/price/latest` returns **`unauthorized`**
  — the public endpoint now wants an API key. Pyth remains attractive
  (updates carry verifiable attestations, which fits our receipt story)
  but it is a signup + key, not a URL swap. Pyth Lazer (the ms-latency
  tier) is permissioned as well.

### Millisecond information exists, free, on the CEX websocket
Measured over ~15–20 s windows each:

| stream | events/s | inter-arrival p50 / p90 / p99 (ms) |
|---|---|---|
| Binance SOL/USDT bookTicker | ~50 | 0 / 24 / 374 |
| Binance SOL/USDT trades | ~6.6 | 0 / 3 / 2861 (bursty) |
| Binance BONK/USDT trades | ~2 | 0 / 0 / 1055 |
| Binance SOL/USDC trades | ~0.5 | 1016 / 8041 / — (too thin) |

The quote stream (bookTicker) is the real cadence ceiling: ~20 ms typical
gaps, i.e. **~50 genuine price updates per second** on the liquid pair.
This is also the same family of source bench 018 already used ("public
CEX reference history, 1-minute bars"), so it extends the existing
evidence chain rather than forking it.

### The engine does not care about wall time
`afterswap-engine` has **zero** wall-clock dependencies — no `Duration`,
`Instant`, `SystemTime`, or sleeps; every horizon is denominated in tick
counts (`tick: u64`). At the audited ~1.2 µs/tick, 50 ticks/s is a
rounding error. Feeding it ms ticks *runs* today; what changes is the
**meaning** of a tick, not the mechanics.

## The two floors that do not move

1. **Settlement: ~400 ms.** A real fill lands in a Solana slot. Best
   possible anywhere is decide-in-µs, land-next-slot — which the current
   architecture already permits.
2. **Evidence cadence.** Bench 018's headline (+34 ± 10 bps vs trailing
   on BONK) was measured on **1-minute bars** — note, not even 1 s. The
   FSM tables and window semantics were selected at that cadence. At
   20 ms ticks the same machines read a different process; the edge claim
   must be re-earned, not re-labelled.

## Decided architecture (post-judging): two planes

- **Decision plane (fast, unsigned):** CEX websocket (or Pyth with a
  key) drives the FSM at ms cadence. This data steers; it is never
  presented as the audited price.
- **Execution plane (slow, signed):** at the moment the machine says
  sell, fetch one DFlow signed quote (RFC 9421, as today) and price the
  fill off *that*. The receipt story — "every fill's price is
  provider-signed and verifiable in your browser" — survives intact,
  because the signed quote is still the thing on the receipt.

This mirrors how real execution desks work (decide on fast data, execute
on the venue's quote) and is honest about which numbers carry signatures.

## Substrate recorded for the re-bench

`benches/041_ms_feed/` — 5 minutes of SOL/USDT bookTicker at native
cadence (`{t, bid, ask}` per line, ms timestamps), recorded 2026-08-30.
Enough to smoke-test the enumeration at ms ticks; a real re-bench wants
hours, recorded the same way.

## Ordered next steps (all post-Sep-3, none before the deadline)

- [ ] Record ≥24 h of bookTicker for SOL/USDT + BONK/USDT (same recorder).
- [ ] Re-run the enumeration + tournament + train/test split (bench 018
      protocol) on ms bars; publish as the next numbered bench. The edge
      number that survives is the only one we may claim.
- [ ] Prototype the two-plane loop natively (`afterswap-server`), not in
      the browser first: WS feed → engine → on sell, one signed DFlow
      quote → paper fill + rail receipt.
- [ ] Only then decide whether the *browser* demo changes cadence at all
      — the 1 s wall-clock lockstep is a demo feature, not a limitation.
