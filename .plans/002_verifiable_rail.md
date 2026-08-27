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

Next action: R3 — segment-root anchoring via executor-signed memo tx + the
browser verifier page.
