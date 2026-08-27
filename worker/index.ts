// AfterSwap Worker: serves the static WASM dashboard (assets), the agent
// decision API, and the global scoreboard Durable Object.
//
//   POST /decide        { prices, open_at? } — engine decisions (CPU-gated
//                        on the free plan; see README)
//   POST /api/score     { vs: {hold, twap, trailing, ladder, bracket} }
//                        — one completed paper cycle from a visitor
//   GET  /api/score     — aggregate across every visitor
//
// The scoreboard DO is SQLite-backed (the only backend the free plan
// allows) and does nothing but accumulate counters, so it stays far inside
// the free CPU budget — unlike /decide, which needs a full enumeration.

import init, { WasmEngine, parity_run } from "../web-wasm/public/pkg/afterswap_wasm.js";
import wasmModule from "../web-wasm/public/pkg/afterswap_wasm_bg.wasm";

export { Scoreboard } from "./scoreboard";

import { b58decode, commitPolicy } from "./commit";
import pdaTable from "./pda_table.json";

let ready: Promise<unknown> | null = null;

const CORS = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, POST, OPTIONS",
  "access-control-allow-headers": "content-type",
};

const USAGE = {
  service: "AfterSwap decision API (preview)",
  engine: "afterswap-engine — 1,054 enumerated exit FSMs + evolution, GOAT-gated (G1–G6)",
  usage: "POST /decide with JSON {prices: number[] (>= 30 ticks), open_at?: number}",
  returns: "machine roster ranked by simulated edge; with open_at: a full simulated exit",
  determinism: "same input → byte-identical output (G1/G6 gated)",
  disclaimer: "paper simulation on your supplied prices; not financial advice; not an order router",
  source: "https://github.com/ozoneRatchapon/afterswap",
};

interface Env {
  SCOREBOARD: DurableObjectNamespace;
  /// Throwaway devnet keypair (base58 64-byte secret) used only to show
  /// visitors a real on-chain policy commitment without a wallet.
  DEMO_KEYPAIR?: string;
}

const POLICY_PROGRAM = "GEz2tFVTrrtHjvHKw2BTNrjndEQ54SSUMoMEUvHk8bD8";

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", ...CORS },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (request.method === "OPTIONS") return new Response(null, { headers: CORS });

    if (url.pathname === "/api/score") {
      // One global instance: the aggregate is the whole point.
      const id = env.SCOREBOARD.idFromName("global-v1");
      return env.SCOREBOARD.get(id).fetch(request);
    }

    if (url.pathname === "/api/commit-policy") {
      if (request.method !== "POST") return json({ error: "POST only" }, 405);
      if (!env.DEMO_KEYPAIR) return json({ error: "demo signer not configured" }, 503);
      let body: {
        fingerprint?: unknown;
        n_states?: unknown;
        tranche_bps?: unknown;
        blockhash?: unknown;
        quote_digest?: unknown;
      };
      try {
        body = await request.json();
      } catch {
        return json({ error: "invalid JSON" }, 400);
      }
      const fpHex = typeof body.fingerprint === "string" ? body.fingerprint : "";
      if (!/^[0-9a-f]{1,16}$/.test(fpHex)) return json({ error: "bad fingerprint" }, 400);
      const blockhash = typeof body.blockhash === "string" ? body.blockhash : "";
      if (!/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(blockhash)) {
        return json({ error: "bad blockhash" }, 400);
      }
      const nStates = Number(body.n_states);
      const trancheBps = Number(body.tranche_bps);
      if (!(nStates >= 1 && nStates <= 4) || !(trancheBps >= 1 && trancheBps <= 10_000)) {
        return json({ error: "bad params" }, 400);
      }

      // The DO hands out position slots and enforces the demo budget.
      const slotRes = await env.SCOREBOARD
        .get(env.SCOREBOARD.idFromName("global-v1"))
        .fetch("https://do/slot", { method: "POST" });
      const slot = (await slotRes.json()) as { slot?: number; error?: string };
      if (slot.slot == null) return json({ error: slot.error ?? "no slots left" }, 429);
      const policyPda = (pdaTable as { pdas: string[] }).pdas[slot.slot];
      if (!policyPda) return json({ error: "slot out of range" }, 429);

      try {
        const signedTx = await commitPolicy({
          secretKey: b58decode(env.DEMO_KEYPAIR),
          blockhash,
          programId: POLICY_PROGRAM,
          owner: (pdaTable as { owner: string }).owner,
          policyPda,
          positionId: slot.slot,
          fingerprint: BigInt("0x" + fpHex),
          nStates,
          trancheBps,
          quoteDigest:
            typeof body.quote_digest === "string" ? body.quote_digest : null,
        });
        return json({ signed_tx: signedTx, policy_pda: policyPda, cluster: "devnet" });
      } catch (e) {
        return json({ error: String(e).slice(0, 200) }, 502);
      }
    }

    if (url.pathname !== "/decide") {
      return new Response("not found", { status: 404, headers: CORS });
    }
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

    const openAt = typeof body.open_at === "number" ? Math.floor(body.open_at) : null;
    if (openAt != null) {
      if (openAt < 1 || openAt >= prices.length - 1) {
        return json({ error: "open_at out of range" }, 400);
      }
      const sim = JSON.parse(parity_run(JSON.stringify(prices), openAt));
      return json({
        engine: USAGE.engine,
        mode: "simulation",
        ticks: prices.length,
        simulation: {
          final_value_norm: sim.final_value_norm,
          hold_value_norm: sim.hold_value_norm,
          edge_vs_hold_bps: sim.edge_vs_hold_bps,
          fills: sim.fills,
          fully_exited: sim.closed,
        },
        disclaimer: USAGE.disclaimer,
      });
    }

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
    }));
    return json({
      engine: USAGE.engine,
      mode: "roster",
      ticks: prices.length,
      strategies_enumerated: snap.strategies_enumerated,
      machines,
      disclaimer: USAGE.disclaimer,
    });
  },
};
