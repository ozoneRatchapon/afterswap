# 041 — ms-cadence feed substrate (Binance SOL/USDT bookTicker)

Raw millisecond-grade top-of-book recording used as the evidence substrate for
the ms-cadence spike (`.plans/006_ms_cadence.md`). This is the fast unsigned
feed plane candidate: it steers the FSM at ms cadence while a DFlow-signed
quote prices each actual fill.

## Provenance

- Source stream: `wss://stream.binance.com:9443/ws/solusdt@bookTicker`
- Recorded: 2026-08-30 ~00:34 local (ICT), 300 s duration
- Recorder: single Node WebSocket client, one JSON line per bookTicker event

## Format

`solusdt_bookticker.jsonl.gz` — gzipped JSONL, one event per line:

```json
{"t":1788024852632,"bid":"104.74000000","ask":"104.75000000"}
```

- `t` — local receive time, ms since epoch
- `bid` / `ask` — best bid / best ask price (USDT), kept as Binance's
  decimal strings — parse at load time, don't trust float round-trips

## This recording, measured from the file itself

- 11,717 events over 299.8 s = **39.1 events/s**
- inter-event gap ms: p50=0, p90=26, p99=616, max=2,357
- timestamps strictly monotonic, no gaps in capture (recorder exit 0)

## Context measurements (from the live spike, same session)

- SOL/USDT bookTicker: ~50 events/s (inter-event gap p50=0 / p90=24 / p99=374 ms)
- SOL/USDT trades: ~6.6/s; BONK/USDT trades: ~2/s; SOL/USDC: too thin (~0.5/s)
- DFlow dev quote API: 1.05–1.32 s RTT — not an ms source; used per-fill only
- Engine tick cost: ~1.2 µs — ms cadence is feed-bound, not compute-bound

## Next steps

See `.plans/006_ms_cadence.md` §next steps: ≥24 h recording, re-bench per the
018 protocol, then a native two-plane prototype in `afterswap-server`.
