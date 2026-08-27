# Plan 002 — Verifiable Execution Rail

Spec: `docs/RAIL.md`. Phases R0–R3 (rail crate → shadow capture → Worker
ingest/DO/R2 → anchoring + public verifier).

Status: SPEC DRAFTED 2026-08-28. Supersedes the alpha-selling framing of
roadmap #7b per benches 025/035 and the CUPED closure (033/037/038 + clip and
historical probes). Verified before drafting: Jupiter quote API returns
contextSlot but no signature headers; DFlow RFC 9421 signing already shipped
in v4.2.

R0 ✅ SHIPPED 2026-08-28 (`crates/afterswap-rail`): canonical encoding with a
pinned golden digest, content/record hash split, domain-separated attestation,
gap-reporting hash chain, RFC 6962-style Merkle (promote-odd, leaf/node
domains), standalone `verify_record`. 22 tests; compiles native and
wasm32-unknown-unknown, clippy-clean on both.

R1 ✅ SHIPPED 2026-08-28: `venues.rs` capture layer (DFlow raw+RFC 9421
headers, Jupiter shadow with pinned live fixture), no-f64 boundary into
`VenueQuote`, rule-v1 decision with the 2-slot desync guard, chain append with
restart resume, `rail_verify` auditor tool. Dry run on live ticks: 21 records,
chain verified, 21 provider-signed + 21 observed, slot gaps median 1 / max 1 —
falsifier passed 21/21 within bound. Jupiter won the decision 11/21.

R2 ✅ SHIPPED 2026-08-28 (local verification): `worker/rail.ts` RailSequencer
DO — ingest verifies via the shared wasm build of `afterswap-rail` (no TS
reimplementation), enforces (not assigns) the executor-signed chain, closes
64-record segments into content-addressed R2 objects, serves
/rail/{ingest,records,proof/:seq,stats}. `parse_confirmed` now emits raw
integer legs; live `FillRef` wired. Falsifier (local miniflare): 81/81
ingested, ingest→public-read median 3.7 ms / max 39.8 ms, fork and replay
rejected, segment closed, and the wasm-served proof VERIFIED under the native
crate. NOT DEPLOYED — production ≤30 s check needs a real `wrangler deploy`
(DO-binding migration caveat applies) plus an external observer.

R3 ✅ SHIPPED 2026-08-28 (local verification): anchor poller
(`server --anchor`, memo format `afterswap:rail blake3=<root> seq=<a>..<b>`,
dry-run verified; NO real anchor posted — needs a funded keypair),
`/rail/segments` + `/rail/anchored` claim endpoints, browser verifier
`rail.html` (headless-Chrome check: 26/26 attestations + 9/9 proofs verified
in-tab, 0 failures). The verifier caught a real bug: full-range u64
fingerprints mangled by JS JSON above 2^53 — now hex on the wire, legacy
numeric still parses.

Remaining (owner actions, RAIL.md §7): production deploy (DO migration
caveat), real attestation key, R2 bucket, external ≤30 s measurement, first
funded anchor.

## Pure-Rust worker + free-tier invariant (2026-08-28)

Adopted as an operating constraint: zero-capital, free-tier only (RAIL.md §8).
`crates/afterswap-worker` (workers-rs 0.8) replaced `worker/rail.ts` — the DO
links `afterswap-rail` directly; the JSON-string wasm boundary is gone. R2
binding optional: unbound, segments retain in DO SQLite (free 5 GB ≈ >1M
records). Anchoring targets devnet.

Verified locally: falsifier 81/81 (median 5.2 ms ingest→readable, fork 400,
replay 409, retention mode confirmed, proof native-VERIFIED — segment root
byte-identical to the TS implementation's root for the same records);
cross-origin browser pass (dashboard origin → rail worker origin, 26 attests
+ 9 proofs in-tab, 0 failures); G6 parity PASS after the wasm-bindgen
0.2.118→0.2.127 workspace bump (worker-build requirement).

Debug trail worth keeping: the "Critical error" was `SqlCursor::one()` —
Cloudflare's JS cursor.one() *throws* on zero rows and workers-rs surfaces it
as an uncatchable critical, not an Err. First diagnosis (bound parameters)
was wrong and was reverted; bindings work.
