# The Verifiable Execution Rail — technical specification

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

**Retention:** closed segments are content-addressed objects in R2
(append-only by policy, no delete permission on the writer token). A record
is ~2–4 KB; even 10k executions/day is ~40 MB/day → < 75 GB over five
years → R2 cost ~$1/month. The Durable Object holds only the live window
and the sequence counters.

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
  enumeration, not to this. R2, however, needs the paid plan's bindings —
  same $5/month unlock already priced into #7b.

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
- "5-year retention" is an R2 bucket policy plus anchors — durable against
  us; not against Cloudflare and Solana both disappearing. A regulator may
  require a second custodian; the content-addressed segments make mirroring
  trivial, which is the design's answer.
