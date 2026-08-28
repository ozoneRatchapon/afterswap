# The Verifiable Execution Rail — technical specification

> **Status 2026-08-28: R0–R3 DEPLOYED and ANCHORED on devnet.** R0
> `afterswap-rail` (23 tests, native+wasm). R1 multi-venue capture — live dry
> run: 21 records, slot gaps ≤ 1, Jupiter chose 11/21. R2 Sequencer DO — now
> live as the pure-Rust Worker `afterswap-rail`. R3 anchor poller + browser
> verifier (headless-Chrome check: 26/26 attestations, 9/9 proofs, 0
> failures, computed in-tab). The browser verifier also caught a real schema
> bug: a full-range u64 fingerprint crossing JSON as a number is mangled by
> JavaScript above 2^53 — it now crosses as hex.
>
> Live as of 2026-08-28:
> - dashboard <https://afterswap.solana-thailand.workers.dev>, rail worker
>   <https://afterswap-rail.solana-thailand.workers.dev> (DO migration `v1:
>   RailSequencer` confirmed via `wrangler deployments list`, not assumed).
> - **§7.2 done — the attestation key was rotated off the public dev seed.**
>   Production pubkey `887e4537…3451`; the seed lives with the executor only
>   (`--attest-seed-hex`), never in git. Falsified rather than asserted: a
>   dev-attested record now gets `400 attestation does not verify`. The DO
>   instance moved `rail-v1` → `rail-prod-v1` so the production chain starts
>   at seq 0 under the production key — the verifier checks every record
>   against the one current pubkey, so a dev-attested prefix would have read
>   as failures.
> - **§7.6 measured live** against the production edge: 70/70 ingested, 0
>   rejected, ingest→publicly-readable median 227 ms, p90 294 ms, max 393 ms
>   — inside the 30 s budget by ~76×. Caveat: the observer was the executor
>   machine, not a third host as §7.6 asks; the headroom is large enough that
>   the verdict does not turn on it.
> - **§7.7 done — first real anchors posted.** Production-keyed segment root
>   `9dbec084…79b0` (seq 0..63) anchored in devnet tx
>   `2Um3Jsvdk5uc…DpMzvt`, Finalized, fee ◎0.000005, memo byte-matching the
>   root. A live `/rail/proof/10` verifies under the native crate against
>   that anchored root.
>
> - **§7.5 done — the real executor now feeds production.** `--rail-ingest`
>   ships each attested record off the execution loop's critical path; 70 live
>   cycles gave **70 accepted, 0 rejected, 0 gaps**. The chain now carries real
>   multi-venue evidence (RFC 9421 provider-signed DFlow beside observed
>   Jupiter, real routes and context slots), and a second segment root
>   `ff63ca41…e99e` (seq 64..127) is anchored in devnet tx
>   `4FHorYfF178j…EJ7SWR` — memo byte-matching, and `/rail/proof/100` for a
>   real record VERIFIES under the native crate against it.
>
> The synthetic `TEST/SYNTH` prefix at seq 0..69 was **kept, not discarded**.
> It is labelled as what it is, the shipper's reconciler extended the deployed
> chain rather than forking a clean one, and deleting an anchored bootstrap to
> make the trail read better is the kind of tidying an audit trail exists to
> prevent.
>
> Still open: §7.3 R2 bucket, deliberately unbound — §8 keeps closed segments
> in DO SQLite so the rail stays free-tier complete.

Phase spec for roadmap #7b as re-scoped: sell **verifiability, not alpha**.
The statistical program closed every path to an alpha claim this project can
detect (benches 025/035: selection differential under the detection floor;
benches 033/037/038 + the clip and historical probes: CUPED unavailable on the
pairs whose margin is positive). What survived every bench is the thing no
statistic can take away: the record of what happened, provable.

## 0. Claims discipline

This rail produces **compliance artifacts aligned with MiCA Article 78**; it
does not make the legal claim "compliant", which is a determination for a
regulator and counsel, not a codebase. The distinction is the same one the
benches enforce between a measurement and a verdict. Concretely, Article 78's
operational requirements and the artifact that answers each:

| Article 78 requirement | artifact |
| --- | --- |
| best possible result across price/cost/speed/size | per-execution record of *every* venue quoted, with the pre-committed decision rule that chose |
| no single-broker reliance; multi-venue price discovery | the shadow quote: a second venue captured beside the primary on every execution |
| pre/post-trade data published within 30 s | public read endpoint serving the signed record immediately on ingest |
| immutable audit trail, 5-year retention | hash-chained records in append-only storage, segment roots anchored on-chain |

## 1. What already exists (build on, do not rebuild)

- **RFC 9421 signed DFlow quotes**, ed25519-verified client-side against
  DFlow's published key — shipped v4.2, verifies every quote at 0.055 ms.
- **Policy PDA program** (Pinocchio, immutable commits, blake3-64
  fingerprints) with the memo binding `afterswap:quote sha-256=<digest>` —
  the pattern section 3 reuses for anchoring.
- **`QuoteSnapshot`** with `context_slot` as the freshness key and
  `Freshness` as a type; **`parse_confirmed`** extracting realised fills from
  balance deltas, validated against a pinned mainnet transaction.
- **Worker**: static WASM deploy, hand-rolled WebCrypto Ed25519 (no web3.js),
  one SQLite Durable Object, free plan.

## 2. Shadow price discovery

### 2.1 The asymmetry that shapes everything

Verified 2026-08-28 against both live APIs: **DFlow signs its quotes
(RFC 9421); Jupiter does not** — a Jupiter quote response carries no
signature of any kind. The two venues therefore produce evidence of different
strength, and the record must say so rather than flatten it:

- **Provider-signed** (DFlow): proves the venue *offered* this price. Anyone
  can re-verify against DFlow's published key forever.
- **Self-attested** (Jupiter): proves only that *we observed* this response.
  We record the full body, its digest, and sign the observation. An auditor
  trusts it as far as they trust our attestation key — which is exactly as
  far as any single-observer record can carry. (Strengthening this needs
  independent co-observers; out of scope for v1 and stated as such.)

Jupiter does return `contextSlot`, so cross-venue slot alignment works: the
record carries both venues' slots and the gap between them, reusing the
`Freshness` discipline. A shadow quote more than 2 slots from the primary is
recorded and flagged, never silently compared.

### 2.2 Capture

One new type in `afterswap-dflow` (venue-generic, despite the crate name):

```rust
pub struct VenueQuote {
    pub venue: VenueId,              // Dflow | Jupiter | ...
    pub snapshot: QuoteSnapshot,     // price, context_slot, latency_us, route
    pub evidence: QuoteEvidence,
}

pub enum QuoteEvidence {
    /// RFC 9421 headers + the exact signature base, re-verifiable offline.
    ProviderSigned { sig_headers: String, body_sha256: [u8; 32] },
    /// Full response body retained; our attestation is the only warrant.
    Observed { body: Box<[u8]>, body_sha256: [u8; 32] },
}
```

The executor polls both venues concurrently (`tokio::join!`) inside the
existing `exec_ab` cycle — arrival step captures N quotes instead of one.
Same-notional, same-pair, same-tick; the clip probe showed request bursts
429 on the dev endpoints, so per-venue spacing stays configurable.

### 2.3 The decision rule is itself committed

"Best execution" is only auditable if the rule that chose is fixed *before*
the quotes arrive — otherwise every routing decision is defensible post-hoc.
The routing policy (v1: `argmax(out_amount)` net of recorded fees, tie to
primary) is serialised canonically, blake3-fingerprinted, and committed
through the existing policy PDA — the same mechanism that already commits
exit machines. Every audit record cites the fingerprint of the rule that
governed it. Changing the rule is a new commitment, never an edit.

## 3. The cryptographic logging pipeline

### 3.1 The record

```rust
pub struct AuditRecord {
    pub seq: u64,                 // per-instrument, monotonic; gaps load-bearing
    pub prev_hash: [u8; 32],      // hash chain within the instrument stream
    pub t_ms: u64,
    pub instrument: String,       // "SOL/USDC"
    pub quotes: Vec<VenueQuote>,  // primary + shadows, all venues quoted
    pub policy_fingerprint: u64,  // blake3-64 of the routing rule (§2.3)
    pub decision: RouteDecision,  // chosen venue + the rule's evaluated inputs
    pub fill: Option<FillRef>,    // signature, slot, realised legs (raw ints)
    pub attestation: [u8; 64],    // ed25519 over blake3(domain ‖ canonical record)
}
```

Ground rules, each one bought by a bug this project already hit:

- **Amounts are raw integer strings + decimals**, never floats. Canonical
  JSON with floats is unhashable-stable; and `uiAmount` already burned us
  (dropped low digits, null on zero). Same lesson, enforced at the schema.
- **`seq` gaps are load-bearing** — a dropped cycle must be visible in the
  stream, or the log silently conditions on capture success (the fill-rate
  lesson from the harness).
- **The fill is `parse_confirmed` output**, realised balance deltas — never
  the quote restated. An audit trail that echoes quotes as fills would be the
  precise failure Article 78 exists to prevent, committed cryptographically.

### 3.2 Hashing and signing

- **blake3** for leaves, the hash chain, and Merkle nodes (project standard;
  the policy program already fingerprints with blake3-64).
- **SHA-256 stays where external formats fix it**: RFC 9421 body digests and
  the existing on-chain memo format — those are the counterparty's formats,
  not ours to modernise.
- **A dedicated attestation keypair, held by the executor, never the
  Worker.** Signing arbitrary blobs with the *trading* key is a signature-
  confusion footgun; the preimage is domain-separated
  (`"afterswap-rail:v1" ‖ blake3(canonical_record)`) so no record can
  collide with a transaction. The attestation pubkey is published beside
  DFlow's.

### 3.3 Publication vs. immutability — two mechanisms, two clocks

**Publication (≤ 30 s):** the executor POSTs the signed record to the Worker
on completion of each cycle; the Worker serves it publicly on ingest.
Publication latency = one HTTP hop, comfortably inside the requirement.
Anchoring is *not* on this path.

**Immutability (5 years):** per instrument, records hash-chain via
`prev_hash`. Every segment (per-minute, configurable) the Worker computes a
Merkle root over the segment's records and anchors it on-chain in a memo —
`afterswap:rail blake3=<root> seq=<from>..<to>` — the identical pattern to
the shipped quote-digest memo. Tampering with any published record after its
segment anchors requires rewriting a Solana transaction. Anchor cost at
1/min ≈ 0.007 SOL/day; at 1/10min it is noise.

**Retention — as shipped:** no R2 bucket is bound (§7 step 3, skipped per
the §8 free-tier invariant), so `close_segment` takes the `Err(_)` branch:
closed records are marked `archived = 1` and **never deleted** — the trim
`DELETE` is gated behind a successful R2 put. Durability is therefore the
Durable Object's SQLite, not a bucket policy. At ~2–4 KB/record, >1M records
fit the free allowance.

**Retention — if the bucket is bound:** closed segments become
content-addressed objects in R2 (append-only by policy, no delete permission
on the writer token) and SQLite keeps only a `RING_KEEP` live window. Even
10k executions/day is ~40 MB/day → < 75 GB over five years → ~$1/month.
This path is implemented and unexercised; treat it as designed, not proven.

### 3.4 What an auditor verifies, independently

1. record's attestation signature against our published key
2. DFlow leg's RFC 9421 signature against DFlow's published key
3. fill signature exists on-chain; realised deltas match the record
4. `prev_hash` chain over the sequence; no `seq` gaps unexplained
5. Merkle path from record to an anchored root; anchor tx on-chain
6. decision reproduces from the quotes under the committed rule fingerprint

Steps 1–6 need our public endpoint, public Solana RPC, and nothing else from
us. That is the difference between this and the "asserted best execution"
the aggregator field publishes.

## 4. Deployment on the Worker/WASM setup

**Trust boundary first:** the Worker holds *no* keys. Execution and
attestation keys stay with the native executor. The Worker is untrusted
transport + storage whose honesty is enforced by the hash chain and anchors
— a Worker compromise can drop records (visible as seq gaps) but cannot
forge or alter them.

- **`afterswap-rail` crate** (new, small): canonical serialisation, blake3
  chain/Merkle, verify. Compiled natively for the executor **and to WASM for
  the Worker and the browser verifier** — one implementation, no TS
  reimplementation to drift (the G6 byte-parity discipline; WebCrypto has no
  blake3, which settles the build question anyway).
- **Sequencer DO** (one per instrument): assigns `seq`, maintains
  `prev_hash`, closes segments, computes roots. Single-threaded per object —
  ordering by construction. ⚠ Deploy caveat: our fallback deploy path (PUT
  API) rejects DO-binding changes while the versions API 10013 bug stands;
  adding the DO class + migration must go through a working `wrangler deploy`,
  verified with `wrangler deployments list`, not assumed from a green push.
- **Anchor scheduler**: Workers cron (1-min floor — fine, that *is* the
  segment cadence). The anchor memo tx is built and signed by the **native
  executor** on a poll of pending roots, not by the Worker, keeping the
  no-keys-in-Worker invariant. (The devnet demo's throwaway-key path in
  `commit.ts` stays demo-only.)
- **Endpoints**: `POST /rail/ingest` (attested records only — the DO
  verifies the attestation via the WASM module before accepting),
  `GET /rail/record/{instrument}/{seq}`, `GET /rail/segment/{id}`,
  `GET /rail/verify/{sig}` (convenience re-verification; auditors need not
  trust it, per §3.4).
- **CPU budget**: blake3 over a 4 KB record is microseconds; ingest fits the
  free plan's 10 ms. The Workers-Paid blocker from #7b applied to full
  enumeration, not to this. ~~R2 needs the paid plan~~ — **corrected
  2026-08-28**: R2 has its own free tier (verified against Cloudflare's
  pricing page: 10 GB-month storage, 1M class-A + 10M class-B operations per
  month, Standard storage only); enablement may require a payment method on
  file even at $0 usage — verify at setup.

## 5. Phasing

| phase | deliverable | proves |
| --- | --- | --- |
| R0 | `afterswap-rail` crate: record schema, canonical form, chain, verify — with the same test discipline as `cuped.rs` | the record format, before anything depends on it |
| R1 | shadow capture in `exec_ab`: N-venue arrival, `VenueQuote`, decision rule + fingerprint | multi-venue discovery on real cycles (paper mode — no capital) |
| R2 | Worker ingest + Sequencer DO + R2 segments + public reads | the 30 s publication path |
| R3 | segment anchoring + browser verifier page (reuses the shipped RFC 9421 verifier) | end-to-end §3.4 audit by a stranger |

Each phase lands with its falsifier: R1's dry run must show two venues'
slots within the gap bound on real ticks; R2's must show ingest-to-public
latency under 30 s from an external observer; R3's must have someone verify
a record with no access to our infrastructure.

## 6. Stated limits

- The shadow leg is single-observer attested (§2.1). An auditor gets proof
  of consistency, not proof Jupiter offered that price.
- Anchoring proves a record existed *by* anchor time and was not altered
  *after*; the ≤ 30 s window between execution and publication rests on our
  attestation alone.
- "5-year retention" is, **as deployed**, Durable Object SQLite plus anchors
  — no bucket is bound and nothing is trimmed. It is durable against us, not
  against Cloudflare and Solana both disappearing, and it rests on a single
  DO's storage rather than on an object-store policy. The R2 archive path
  (§3.3) is written but unexercised. A regulator may require a second
  custodian; the content-addressed segments make mirroring trivial, which is
  the design's answer — and executing §7 step 3 is what turns it on.

## 7. Deployment runbook (owner actions)

Everything below changes live infrastructure or spends from a key. Steps
1–2 and 4–7 have been executed (2026-08-28); step 3 is deliberately skipped
per the §8 free-tier invariant. Each step records what was *observed*, not
what was expected.

1. **Build the wasm bundle** (committed, but rebuild to be sure):
   `cargo build -p afterswap-wasm --target wasm32-unknown-unknown --release`
   then `wasm-bindgen <target>/wasm32-unknown-unknown/release/afterswap_wasm.wasm
   --target web --out-dir web-wasm/public/pkg`.
2. **Set the production attestation key.** Generate a 32-byte seed, keep it
   with the executor, put the *public* key in `wrangler.jsonc` `vars.RAIL_PUBKEY`
   (it is registered, not secret). The dev seed's pubkey currently in the
   config must not survive to production.
3. **Create the R2 bucket** `afterswap-rail-archive` (R2 free tier, per the
   §4 correction — not the paid plan),
   with no delete permission on the writer token — append-only by policy.
4. **Deploy** with `wrangler deploy`. ⚠ This carries a new DO class +
   migration (`v2: RailSequencer`); the PUT-API fallback path rejects
   DO-binding changes (error 10021) while the versions-API 10013 bug stands.
   Confirm with `wrangler deployments list` — never assume from a green push.
5. **Point the executor at production** — `--rail-ingest <origin>`:
   ```sh
   cargo run -p afterswap-server --release -- --exec-ab \
       --pair sol --cycles 70 --interval-ms 3000 --decision-delay-ms 400 \
       --out data/execution/rail_live_001.jsonl \
       --rail-out data/rail/prod_chain.jsonl \
       --attest-seed-hex "$(tr -d '\n ' < .devnet/rail_attest_seed.hex)" \
       --rail-ingest https://afterswap-rail.solana-thailand.workers.dev
   ```
   `crates/afterswap-server/src/rail_ship.rs` ships records off the execution
   loop's critical path — a channel into one task, one in-flight request.
   Three properties are non-negotiable and each is bought explicitly:

   * **The loop never blocks on the network.** A slow edge may delay the trail
     becoming public; it must never delay or reorder an execution.
   * **Order is preserved.** The Sequencer *enforces* `prev_hash == tip`, so a
     record posted out of order is rejected 409 and the chain stalls behind
     it. Concurrency on this path would be a correctness bug, not a speed-up.
   * **`--rail-out` is written and flushed before a record is enqueued**, so
     an ingest outage costs visibility, never the record. That is why
     `--rail-ingest` *requires* `--rail-out`: the file is both the replay
     source and what a restart resumes the chain from.

   Failures are classified rather than blanket-retried. A 400 means the record
   did not verify — retrying a bad signature is a busy-loop, not resilience.
   A 5xx or timeout is the edge, and gets 5 attempts over ~6 s of backoff. The
   subtle case is 409 `seq not monotonic`: that is exactly what a *successful*
   POST whose response was lost looks like on retry, so it is read against the
   `tip_seq` the Sequencer returns and counted as already-ingested when the tip
   has passed us — never by matching the message text.

   On start the shipper reconciles the local file against the live rail,
   because the two can disagree in opposite directions. File ahead of rail
   (retries exhausted): the backlog is replayed, otherwise the gap is
   permanent, since the chain only moves forward. Rail ahead of file (the file
   was lost): the live tip is fetched and adopted, because it is the tip
   `prev_hash` will be enforced against — continuing from a stale local tip
   forks and is rejected for the rest of the run. Failing to reach the rail
   here is fatal, not a warning: the silent degradation is starting a fork.

   A plaintext `--rail-ingest` to a remote host is refused (loopback exempt,
   for the falsifier's `wrangler dev`): over `http://` a network position can
   drop records indistinguishably from an outage.

   **Observed 2026-08-28.** 70 live cycles → **70 accepted, 0 rejected, 0
   failed, 0 seq gaps** against the production edge. The reconciler adopted
   the live tip at seq 69 rather than forking — the executor's chain file was
   empty, and the run still extended the deployed chain. Records carry real
   evidence: RFC 9421 `provider_signed` DFlow quotes beside `observed` Jupiter
   bodies, real routes (Meteora DLMM, PancakeSwap), real context slots.
6. **Measure the 30 s falsifier for real**: from a machine that is neither
   the executor nor the Worker, poll `GET /rail/records` and measure
   execution-to-visible latency across a live run.
7. **Anchor for real**: `cargo run -p afterswap-server --features live
   --release -- --anchor --rail-base https://<host> --keypair <path> --rpc
   <url> --interval-secs 60`. Fees are ~5000 lamports per segment root.
   Verify the first memo on-chain by signature before trusting the loop.

## 8. Free-tier topology (the zero-capital invariant)

Adopted 2026-08-28 as an operating constraint: the rail runs **complete and
free, out of the box** — no paid plan, no mainnet fees.

| component | free-tier footing |
| --- | --- |
| Worker + Sequencer DO | Workers Free with SQLite Durable Objects — the same plan this repo's Scoreboard DO already runs on |
| record retention | **R2 binding is optional.** Unbound, closed segments are retained in the DO's SQLite instead of trimmed: at ~4 KB/record, >1M records fit the free allowance — years at our volume |
| archive (when enabled) | R2 free tier: 10 GB-month, 1M class-A / 10M class-B ops per month |
| anchoring | Solana **devnet** (`--anchor … --rpc https://api.devnet.solana.com`), fees from airdrop. A mainnet anchor is an optional upgrade, ~0.007 SOL/day at 1 segment/min |
| verifier page | static asset + the existing wasm bundle |

The pure-Rust worker (`crates/afterswap-worker`, workers-rs) replaced the
TypeScript sequencer: the DO links `afterswap-rail` as a plain dependency and
calls `verify_record`/`record_hash`/`merkle_root` as typed functions — the
JSON-string-into-wasm boundary where the u64 fingerprint bug lived does not
exist in this design. `rail.html` targets it cross-origin via
`?rail=<worker-base>`; every rail endpoint answers with
`access-control-allow-origin: *`.

Devnet caveat, stated rather than hidden: devnet history is periodically
reset, so a devnet anchor proves existence to anyone who checked before a
reset, not indefinitely. For the compliance product that durability gap is
the one thing the mainnet upgrade buys.
