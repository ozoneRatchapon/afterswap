# Using the engine from your agent — the free path

The engine is a 208 KB WASM binary served publicly. You don't need our
API (it's CPU-gated on the free Workers plan anyway) — run the engine
yourself, locally, for free, forever. Node 18+:

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
Hosted `POST /decide` (same responses, zero setup) activates with the
Workers Paid plan; pay-per-decision via pay.sh 402 is the roadmap (7b).
