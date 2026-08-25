// AfterSwap Worker: serves the static WASM dashboard (assets) and the
// agent-facing decision API (roadmap 7b preview). Same engine, same wasm
// binary the browser runs — here instantiated server-side per request.
//
//   POST /decide  { "prices": [f64...], "open_at": usize? }
//     → tournament over the prices + (optionally) a full simulated exit
//       from open_at, with the machine roster and final edge.
//   GET  /decide  → usage.

import init, { WasmEngine, parity_run } from "../web-wasm/public/pkg/afterswap_wasm.js";
// Wrangler bundles .wasm imports as WebAssembly.Module.
import wasmModule from "../web-wasm/public/pkg/afterswap_wasm_bg.wasm";

let ready: Promise<unknown> | null = null;

const CORS = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, POST, OPTIONS",
  "access-control-allow-headers": "content-type",
};

const USAGE = {
  service: "AfterSwap decision API (preview)",
  engine: "afterswap-engine v2.1 — 1,054 enumerated exit FSMs + evolution, GOAT-gated (G1–G6)",
  usage: "POST /decide with JSON {prices: number[] (>= 30 ticks), open_at?: number}",
  returns: "machine roster ranked by simulated edge; with open_at: a full simulated exit (fills, final value, edge vs hold in bps)",
  determinism: "same input → byte-identical output (G1/G6 gated)",
  disclaimer: "paper simulation on your supplied prices; not financial advice; not an order router",
  source: "https://github.com/ozoneRatchapon/afterswap",
};

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", ...CORS },
  });
}

export default {
  async fetch(request: Request, env: unknown, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname !== "/decide") {
      // Everything else is handled by static assets (dashboard).
      return new Response("not found", { status: 404, headers: CORS });
    }
    if (request.method === "OPTIONS") return new Response(null, { headers: CORS });
    if (request.method === "GET") return json(USAGE);
    if (request.method !== "POST") return json({ error: "POST JSON to /decide" }, 405);

    let body: { prices?: unknown; open_at?: unknown };
    try {
      body = await request.json();
    } catch {
      return json({ error: "invalid JSON" }, 400);
    }
    const prices = Array.isArray(body.prices)
      ? body.prices.filter((p): p is number => typeof p === "number" && isFinite(p) && p > 0)
      : [];
    if (prices.length < 30 || prices.length > 10_000) {
      return json({ error: "prices must be 30..10000 positive numbers" }, 400);
    }

    ready ??= init(wasmModule);
    await ready;

    // Roster: feed prices through a fresh engine, read the tournament out.
    const engine = new WasmEngine(12, 3, 0.1, 24);
    for (const p of prices) engine.on_tick(p);
    const snap = JSON.parse(engine.snapshot(0));
    const machines = (snap.arms ?? []).map((a: {
      name: string; id: string; n_states: number; generation: number;
      sim_edge_bps: number; complexity: number;
    }) => ({
      name: a.name,
      fingerprint_blake3_64: BigInt(a.id).toString(16),
      states: a.n_states,
      generation: a.generation,
      sim_edge_bps: a.sim_edge_bps,
      complexity: a.complexity,
    }));

    // Optional: full simulated exit from open_at (same code path as the
    // GOAT gates — G1/G6 discipline applies to this output too).
    let simulation: unknown = null;
    const openAt = typeof body.open_at === "number" ? Math.floor(body.open_at) : null;
    if (openAt != null) {
      if (openAt < 1 || openAt >= prices.length - 1) {
        return json({ error: "open_at out of range" }, 400);
      }
      const sim = JSON.parse(parity_run(JSON.stringify(prices), openAt));
      simulation = {
        final_value_norm: sim.final_value_norm,
        hold_value_norm: sim.hold_value_norm,
        edge_vs_hold_bps: sim.edge_vs_hold_bps,
        fills: sim.fills,
        fully_exited: sim.closed,
        events: sim.events_json.trim().split("\n").filter(Boolean).map((l: string) => JSON.parse(l)),
      };
    }

    return json({
      engine: USAGE.engine,
      ticks: prices.length,
      strategies_enumerated: snap.strategies_enumerated,
      gate: snap.gate,
      machines,
      simulation,
      disclaimer: USAGE.disclaimer,
    });
  },
};
