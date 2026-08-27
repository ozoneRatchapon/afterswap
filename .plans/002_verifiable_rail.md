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

Next action: R2 — Worker ingest + Sequencer DO + R2 segments + public reads
(the 30 s publication path).
