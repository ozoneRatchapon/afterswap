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
    this.sql.exec(
      `CREATE TABLE IF NOT EXISTS ip_quota (
         ip_hash TEXT PRIMARY KEY,
         n INTEGER NOT NULL DEFAULT 0,
         window_start INTEGER NOT NULL DEFAULT 0
       )`,
    );
  }

  /// Demo commitments are real devnet transactions paid from a throwaway
  /// balance, so the slot table is also the budget.
  static readonly MAX_DEMO_COMMITS = 380;

  /// ...and the budget is global, so without a per-visitor cap one client
  /// can spend all 380 before anyone else arrives. `/api/commit-policy` is
  /// an unauthenticated signing oracle by design (the whole point is a real
  /// commitment with no wallet), so the cap is the only thing standing
  /// between the demo and an empty slot table.
  ///
  /// This bounds one address, not one attacker: a distributed caller still
  /// gets `MAX_PER_IP` per address it controls. Turnstile is the next rung
  /// if that becomes the real threat.
  static readonly MAX_PER_IP = 3;
  static readonly IP_WINDOW_MS = 3_600_000;

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/slot/release") {
      // Hand a slot back when signing failed after it was taken: nothing
      // reached the chain, so that position's policy PDA is still
      // uncommitted.
      //
      // Only the most recently issued slot can be released. `cycles` is both
      // the count and the next PDA index, so decrementing when a later slot
      // has already gone out would re-issue a PDA someone else is committing
      // — and `CommitPolicy` is immutable per PDA, so that second commit
      // fails on chain. A silently burnt slot is the better failure than a
      // collision, so anything but the top of the stack is declined.
      const want = Number(url.searchParams.get("slot"));
      const cur = [...this.sql.exec("SELECT cycles FROM totals WHERE floor = 'slot'")] as Array<{ cycles: number }>;
      const used = cur[0]?.cycles ?? 0;
      if (!Number.isInteger(want) || used !== want + 1) {
        return new Response(JSON.stringify({ released: false, reason: "not the last slot issued" }), {
          headers: { "content-type": "application/json" },
        });
      }
      this.sql.exec(`UPDATE totals SET cycles = ${want} WHERE floor = 'slot'`);
      // The slot is uncommitted again, so the visitor should not be charged
      // for it either — otherwise a flaky signer eats their three attempts.
      const ipHash = url.searchParams.get("ip") ?? "";
      if (/^[0-9a-f]{16}$/.test(ipHash)) this.refund_ip_quota(ipHash);
      return new Response(JSON.stringify({ released: true }), {
        headers: { "content-type": "application/json" },
      });
    }
    if (url.pathname === "/slot") {
      // The caller is required to name a visitor. Making this mandatory
      // rather than optional is the point: an omitted `ip` used to mean an
      // uncapped request, which is exactly the bypass the cap exists to
      // close.
      const ipHash = url.searchParams.get("ip") ?? "";
      if (!/^[0-9a-f]{16}$/.test(ipHash)) {
        return new Response(JSON.stringify({ error: "missing visitor key" }), {
          status: 400, headers: { "content-type": "application/json" },
        });
      }
      if (!this.take_ip_quota(ipHash)) {
        return new Response(JSON.stringify({ error: "per-visitor commit limit reached; try again later" }), {
          status: 429, headers: { "content-type": "application/json" },
        });
      }
      const rows = [...this.sql.exec("SELECT cycles FROM totals WHERE floor = 'slot'")] as Array<{ cycles: number }>;
      const used = rows[0]?.cycles ?? 0;
      if (used >= Scoreboard.MAX_DEMO_COMMITS) {
        // Give the quota back: the global budget refused, so this visitor
        // spent nothing.
        this.refund_ip_quota(ipHash);
        return new Response(JSON.stringify({ error: "demo commit budget spent" }), {
          status: 429, headers: { "content-type": "application/json" },
        });
      }
      this.sql.exec(
        `INSERT INTO totals (floor, sum_bps, wins, cycles) VALUES ('slot', 0, 0, 1)
         ON CONFLICT(floor) DO UPDATE SET cycles = cycles + 1`,
      );
      return new Response(JSON.stringify({ slot: used }), {
        headers: { "content-type": "application/json" },
      });
    }
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

    const rows = [...this.sql.exec("SELECT floor, sum_bps, wins, cycles FROM totals WHERE floor != 'slot'")];
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

  /// Charge one commit against `ipHash`, rolling the window over when it has
  /// expired. Returns false when this visitor is already at the cap.
  ///
  /// A fixed window, not a sliding one: a visitor can get 2 x MAX_PER_IP
  /// across a window boundary. That costs at most a handful of the 380 and
  /// keeps this to one row and one statement, which is what a counter-only
  /// DO on the free CPU budget should cost.
  private take_ip_quota(ipHash: string): boolean {
    const now = Date.now();
    // Drop expired windows first. Without this the table grows one row per
    // distinct address forever, which an attacker with a wide address pool
    // turns into unbounded DO storage even though the cap stops them
    // spending slots. Bounded by "addresses seen in the last hour", so the
    // scan stays trivial.
    this.sql.exec(
      "DELETE FROM ip_quota WHERE window_start < ?", now - Scoreboard.IP_WINDOW_MS,
    );
    const rows = [...this.sql.exec(
      "SELECT n, window_start FROM ip_quota WHERE ip_hash = ?", ipHash,
    )] as Array<{ n: number; window_start: number }>;
    const row = rows[0];
    const fresh = !row || now - row.window_start >= Scoreboard.IP_WINDOW_MS;
    if (!fresh && row.n >= Scoreboard.MAX_PER_IP) return false;
    const n = fresh ? 1 : row.n + 1;
    const start = fresh ? now : row.window_start;
    this.sql.exec(
      `INSERT INTO ip_quota (ip_hash, n, window_start) VALUES (?, ?, ?)
       ON CONFLICT(ip_hash) DO UPDATE SET n = excluded.n, window_start = excluded.window_start`,
      ipHash, n, start,
    );
    return true;
  }

  /// Undo one `take_ip_quota`, floored at zero so a duplicate refund cannot
  /// mint quota.
  private refund_ip_quota(ipHash: string): void {
    this.sql.exec(
      "UPDATE ip_quota SET n = MAX(n - 1, 0) WHERE ip_hash = ?", ipHash,
    );
  }
}
