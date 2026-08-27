// Global paired scoreboard: every visitor's completed paper cycles,
// aggregated. SQLite-backed (the only Durable Object storage the Workers
// free plan allows) and deliberately trivial — counters only, so it stays
// far inside the free CPU budget.
//
// Honesty note surfaced in the UI: these are self-reported browser
// results, not audited. The reproducible claims live in `benches/`.

import { DurableObject } from "cloudflare:workers";

const FLOORS = ["hold", "twap", "trailing", "ladder", "bracket"] as const;
type Floor = (typeof FLOORS)[number];

/// Reject implausible submissions: one paper cycle cannot move 100%.
const MAX_ABS_BPS = 10_000;

export class Scoreboard extends DurableObject {
  sql: SqlStorage;

  constructor(ctx: DurableObjectState, env: unknown) {
    super(ctx as never, env as never);
    this.sql = ctx.storage.sql;
    this.sql.exec(
      `CREATE TABLE IF NOT EXISTS totals (
         floor TEXT PRIMARY KEY,
         sum_bps REAL NOT NULL DEFAULT 0,
         wins INTEGER NOT NULL DEFAULT 0,
         cycles INTEGER NOT NULL DEFAULT 0
       )`,
    );
  }

  async fetch(request: Request): Promise<Response> {
    const cors = {
      "access-control-allow-origin": "*",
      "content-type": "application/json",
    };
    if (request.method === "POST") {
      let body: { vs?: Record<string, unknown> };
      try {
        body = await request.json();
      } catch {
        return new Response(JSON.stringify({ error: "invalid JSON" }), { status: 400, headers: cors });
      }
      const vs = body.vs ?? {};
      for (const floor of FLOORS) {
        const raw = vs[floor];
        if (typeof raw !== "number" || !isFinite(raw) || Math.abs(raw) > MAX_ABS_BPS) continue;
        this.sql.exec(
          `INSERT INTO totals (floor, sum_bps, wins, cycles) VALUES (?, ?, ?, 1)
           ON CONFLICT(floor) DO UPDATE SET
             sum_bps = sum_bps + excluded.sum_bps,
             wins = wins + excluded.wins,
             cycles = cycles + 1`,
          floor,
          raw,
          raw > 0 ? 1 : 0,
        );
      }
      return new Response(JSON.stringify({ ok: true }), { headers: cors });
    }

    const rows = [...this.sql.exec("SELECT floor, sum_bps, wins, cycles FROM totals")];
    const out: Record<string, { mean_bps: number; wins: number; cycles: number }> = {};
    for (const r of rows as Array<{ floor: Floor; sum_bps: number; wins: number; cycles: number }>) {
      out[r.floor] = {
        mean_bps: r.cycles > 0 ? r.sum_bps / r.cycles : 0,
        wins: r.wins,
        cycles: r.cycles,
      };
    }
    return new Response(
      JSON.stringify({
        floors: out,
        note: "self-reported browser paper cycles, unaudited; reproducible claims are in benches/",
      }),
      { headers: cors },
    );
  }
}
