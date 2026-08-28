# Using the engine from your agent — the free path

The engine is a 476 KB WASM binary (155 KB gzipped over the wire) served
publicly. You don't need any API key — run the engine yourself, locally,
for free, forever. Node 18+:

```js
// agent.mjs — decisions in your process, sub-millisecond, deterministic
const PKG = "https://afterswap.solana-thailand.workers.dev/pkg";
const init = (await import(`${PKG}/afterswap_wasm.js`)).default;
const { WasmEngine, parity_run } = await import(`${PKG}/afterswap_wasm.js`);
await init(await (await fetch(`${PKG}/afterswap_wasm_bg.wasm`)).arrayBuffer());

const prices = [/* your ticks, ≥30 */];

// Roster: which machines would a tournament seat on these prices?
const engine = new WasmEngine(12, 3, 0.1, 24);
for (const p of prices) engine.on_tick(p);
const { arms } = JSON.parse(engine.snapshot(0));
console.log(arms.slice(0, 3).map(a => `${a.name} ${a.sim_edge_bps.toFixed(1)}bps`));

// Full simulated exit from tick 30 (same code path as the GOAT gates):
const sim = JSON.parse(parity_run(JSON.stringify(prices), 30));
console.log(`edge vs hold: ${sim.edge_vs_hold_bps.toFixed(1)} bps, fills: ${sim.fills}`);
```

Determinism contract: same prices → byte-identical output (GOAT G1/G6).

## Hosted endpoint (preview, no key — expect ~50% failures)

`POST /decide` runs the same engine server-side — zero setup, same
determinism contract. It takes 30..10,000 positive prices and returns the
roster a tournament would seat:

```bash
curl -X POST https://afterswap.solana-thailand.workers.dev/decide \
  -H 'content-type: application/json' \
  -d '{"prices":[/* >=30 numbers */]}'
```

```json
{ "mode": "roster", "ticks": 40, "strategies_enumerated": 1054,
  "machines": [ { "name": "Humble Viper",
                  "fingerprint_blake3_64": "ecc9d22c5dbc6a0a",
                  "states": 3, "generation": 0,
                  "sim_edge_bps": 2.99 } ] }
```

It is CPU-bound on the free Workers plan, so treat it as a preview rather
than a throughput path — the local WASM route above has no such ceiling.
Measured 2026-08-28 over 40 consecutive calls: **20 ok, 20 failed.** The
free-plan CPU ceiling is 2,010 ms and the cold 1,054-machine enumeration
cost 1.0–2.0 s under wasm, so about half of cold starts were killed;
enumeration is process-cached, so a warm call cost 1 ms. Failures return
`503 {"error":"engine unavailable, retry shortly"}` — retry, and prefer the
local WASM route for anything that must always answer.

That enumeration is no longer paid at runtime: its result is precomputed
and shipped as the surviving raw indices (2,108 bytes), so a cold
`/decide` in local `workerd` fell from **752 ms to 7 ms**. The 40-call
figure above is pre-fix; the hosted endpoint will be re-measured after the
next deploy and this line updated with the result.
Pay-per-decision via pay.sh 402 is the roadmap (7b).
