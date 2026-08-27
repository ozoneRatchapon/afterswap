// Rail Sequencer — R2 of docs/RAIL.md: ingest, chain enforcement, hot reads,
// segment closing into R2, inclusion proofs.
//
// One deliberate deviation from the phrase "seq assignment": the DO does not
// assign sequence numbers, because the executor's attestation *covers* `seq`
// and `prev_hash` — a DO-assigned seq could never be signed by the key that
// saw the quotes. The executor authors the chain; this DO **enforces** it:
// single-writer acceptance, monotonic seq, prev_hash linkage against the
// stored tip, gap accounting. Same single-writer guarantee, trust boundary
// intact.
//
// Trust boundary: this object holds no keys. It verifies incoming records
// against the *registered public* attestation key using the same compiled
// `afterswap-rail` bytes the executor and auditors run (no TypeScript
// reimplementation of canonical encoding exists, so ingest verification
// cannot drift from what an auditor concludes). A compromised Worker can
// refuse records — visible as seq gaps — but cannot forge or alter them.

import { DurableObject } from "cloudflare:workers";
import init, {
  rail_verify_record,
  rail_record_hash,
  rail_merkle_root,
  rail_merkle_proof,
} from "../web-wasm/public/pkg/afterswap_wasm.js";
import wasmModule from "../web-wasm/public/pkg/afterswap_wasm_bg.wasm";

const ZERO64 = "0".repeat(64);
/// Records per closed segment. 64 keeps proof paths at 6 hops and segment
/// objects a few hundred KB.
const SEGMENT_SIZE = 64;
/// Hot rows kept in SQLite after archival, for low-latency recent reads.
const RING_KEEP = 512;
/// Ingest payload cap — a record is a few KB; a megabyte is not a record.
const MAX_BODY = 1 << 20;

interface RailEnv {
  RAIL_PUBKEY?: string;
  RAIL_ARCHIVE?: R2Bucket;
}

let ready: Promise<unknown> | null = null;
function wasmReady(): Promise<unknown> {
  ready ??= init({ module_or_path: wasmModule });
  return ready;
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", "access-control-allow-origin": "*" },
  });
}

export class RailSequencer extends DurableObject {
  sql: SqlStorage;
  railEnv: RailEnv;

  constructor(ctx: DurableObjectState, env: RailEnv) {
    super(ctx as never, env as never);
    this.railEnv = env;
    this.sql = ctx.storage.sql;
    this.sql.exec(
      `CREATE TABLE IF NOT EXISTS records (
         seq INTEGER PRIMARY KEY,
         hash TEXT NOT NULL,
         body TEXT NOT NULL,
         archived INTEGER NOT NULL DEFAULT 0
       )`,
    );
    this.sql.exec(
      `CREATE TABLE IF NOT EXISTS segments (
         root TEXT PRIMARY KEY,
         from_seq INTEGER NOT NULL,
         to_seq INTEGER NOT NULL,
         count INTEGER NOT NULL,
         r2_key TEXT NOT NULL,
         closed_ms INTEGER NOT NULL
       )`,
    );
    this.sql.exec(
      `CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v TEXT NOT NULL)`,
    );
  }

  private meta(k: string): string | null {
    const rows = [...this.sql.exec("SELECT v FROM meta WHERE k = ?", k)] as Array<{ v: string }>;
    return rows[0]?.v ?? null;
  }

  private setMeta(k: string, v: string) {
    this.sql.exec(
      "INSERT INTO meta (k, v) VALUES (?, ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
      k, v,
    );
  }

  async fetch(request: Request): Promise<Response> {
    await wasmReady();
    const url = new URL(request.url);
    const path = url.pathname;
    try {
      if (path === "/rail/ingest" && request.method === "POST") return await this.ingest(request);
      if (path === "/rail/records") return this.records(url);
      if (path.startsWith("/rail/proof/")) return await this.proof(Number(path.split("/").pop()));
      if (path === "/rail/stats") return this.stats();
      return json({ error: "unknown rail route" }, 404);
    } catch (e) {
      return json({ error: String(e) }, 500);
    }
  }

  private async ingest(request: Request): Promise<Response> {
    const pubkey = this.railEnv.RAIL_PUBKEY;
    if (!pubkey) return json({ error: "no attestation pubkey registered" }, 503);
    const body = await request.text();
    if (body.length > MAX_BODY) return json({ error: "record too large" }, 413);

    // Step 1: the same verification an auditor runs — attestation, amounts,
    // evidence digests, decision reproduction — via the shared wasm.
    const verdict = rail_verify_record(body, pubkey);
    if (verdict !== "ok") return json({ error: verdict }, 400);

    const record = JSON.parse(body) as { seq: number; prev_hash: string };
    const tipSeq = this.meta("tip_seq");
    const tipHash = this.meta("tip_hash");

    // Step 2: chain enforcement against the stored tip. On mismatch the
    // executor gets the expected tip back and must resync — the DO never
    // silently forks or reorders.
    if (tipSeq === null) {
      if (record.prev_hash !== ZERO64) {
        return json({ error: "genesis must cite the zero hash", expected_prev: ZERO64 }, 409);
      }
    } else {
      if (record.seq <= Number(tipSeq)) {
        return json({ error: "seq not monotonic", tip_seq: Number(tipSeq) }, 409);
      }
      if (record.prev_hash !== tipHash) {
        return json({ error: "prev_hash does not extend the tip", expected_prev: tipHash, tip_seq: Number(tipSeq) }, 409);
      }
    }

    const hash = rail_record_hash(body);
    if (hash.startsWith("err:")) return json({ error: hash }, 400);

    this.sql.exec("INSERT INTO records (seq, hash, body) VALUES (?, ?, ?)", record.seq, hash, body);
    if (tipSeq !== null && record.seq > Number(tipSeq) + 1) {
      this.setMeta("gaps", String(Number(this.meta("gaps") ?? "0") + 1));
    }
    this.setMeta("tip_seq", String(record.seq));
    this.setMeta("tip_hash", hash);
    this.setMeta("total", String(Number(this.meta("total") ?? "0") + 1));

    const archived = await this.maybeCloseSegment();
    return json({ accepted: record.seq, hash, segment_closed: archived });
  }

  /// Close a segment once enough unarchived records accumulate: compute the
  /// Merkle root over their record hashes (order = seq order), write the
  /// segment to R2 keyed by its root (content-addressed), keep the index row,
  /// trim the hot ring.
  private async maybeCloseSegment(): Promise<string | null> {
    const pending = [...this.sql.exec(
      "SELECT seq, hash, body FROM records WHERE archived = 0 ORDER BY seq ASC",
    )] as Array<{ seq: number; hash: string; body: string }>;
    if (pending.length < SEGMENT_SIZE) return null;

    const batch = pending.slice(0, SEGMENT_SIZE);
    const root = rail_merkle_root(JSON.stringify(batch.map((r) => r.hash)));
    if (root.startsWith("err:")) throw new Error(root);
    const key = `segments/${root}.jsonl`;

    // R2 first, index after: a crash between the two re-closes the segment
    // idempotently (same content → same root → same key).
    if (this.railEnv.RAIL_ARCHIVE) {
      await this.railEnv.RAIL_ARCHIVE.put(key, batch.map((r) => r.body).join("\n"));
    }
    const from = batch[0].seq;
    const to = batch[batch.length - 1].seq;
    this.sql.exec(
      "INSERT OR REPLACE INTO segments (root, from_seq, to_seq, count, r2_key, closed_ms) VALUES (?, ?, ?, ?, ?, ?)",
      root, from, to, batch.length, key, Date.now(),
    );
    this.sql.exec("UPDATE records SET archived = 1 WHERE seq >= ? AND seq <= ?", from, to);
    this.sql.exec(
      "DELETE FROM records WHERE archived = 1 AND seq NOT IN (SELECT seq FROM records ORDER BY seq DESC LIMIT ?)",
      RING_KEEP,
    );
    return root;
  }

  private records(url: URL): Response {
    const since = Number(url.searchParams.get("since") ?? "-1");
    const limit = Math.min(Number(url.searchParams.get("limit") ?? "50"), 200);
    const rows = [...this.sql.exec(
      "SELECT body FROM records WHERE seq > ? ORDER BY seq ASC LIMIT ?", since, limit,
    )] as Array<{ body: string }>;
    return new Response(`[${rows.map((r) => r.body).join(",")}]`, {
      headers: { "content-type": "application/json", "access-control-allow-origin": "*" },
    });
  }

  /// Inclusion proof relative to a *closed* segment. A record still in the
  /// open segment has no root yet, and pretending otherwise would hand out
  /// proofs that anchor to nothing — callers get told to wait instead.
  private async proof(seq: number): Promise<Response> {
    if (!Number.isFinite(seq)) return json({ error: "bad seq" }, 400);
    const seg = [...this.sql.exec(
      "SELECT root, from_seq, to_seq, r2_key FROM segments WHERE from_seq <= ? AND to_seq >= ?", seq, seq,
    )] as Array<{ root: string; from_seq: number; to_seq: number; r2_key: string }>;
    if (seg.length === 0) {
      return json({ error: "record not in a closed segment yet; proofs exist once the segment closes" }, 409);
    }
    const { root, r2_key } = seg[0];

    // Prefer hot rows; fall back to the R2 object (the archived path).
    let lines: Array<{ seq: number; hash: string }> = [...this.sql.exec(
      "SELECT seq, hash FROM records WHERE seq >= ? AND seq <= ? ORDER BY seq ASC",
      seg[0].from_seq, seg[0].to_seq,
    )] as Array<{ seq: number; hash: string }>;
    if (lines.length === 0 && this.railEnv.RAIL_ARCHIVE) {
      const obj = await this.railEnv.RAIL_ARCHIVE.get(r2_key);
      if (!obj) return json({ error: "segment object missing from archive" }, 500);
      const bodies = (await obj.text()).split("\n");
      lines = bodies.map((b) => ({
        seq: (JSON.parse(b) as { seq: number }).seq,
        hash: rail_record_hash(b),
      }));
    }
    const index = lines.findIndex((l) => l.seq === seq);
    if (index < 0) return json({ error: "seq absent from its segment" }, 500);

    const proof = rail_merkle_proof(JSON.stringify(lines.map((l) => l.hash)), index);
    if (proof.startsWith("err:")) return json({ error: proof }, 500);
    return json({
      seq,
      record_hash: lines[index].hash,
      segment_root: root,
      proof: JSON.parse(proof),
      note: "verify with afterswap_rail::merkle_verify; root anchoring lands in R3",
    });
  }

  private stats(): Response {
    const segs = [...this.sql.exec(
      "SELECT root, to_seq FROM segments ORDER BY to_seq DESC LIMIT 1",
    )] as Array<{ root: string; to_seq: number }>;
    const nseg = [...this.sql.exec("SELECT COUNT(*) AS n FROM segments")] as Array<{ n: number }>;
    return json({
      tip_seq: this.meta("tip_seq") === null ? null : Number(this.meta("tip_seq")),
      tip_hash: this.meta("tip_hash"),
      total_accepted: Number(this.meta("total") ?? "0"),
      seq_gaps: Number(this.meta("gaps") ?? "0"),
      segments_closed: nseg[0]?.n ?? 0,
      latest_root: segs[0]?.root ?? null,
      latest_root_to_seq: segs[0]?.to_seq ?? null,
    });
  }
}
