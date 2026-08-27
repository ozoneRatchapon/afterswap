# Plan 002 — Verifiable Execution Rail

Spec: `docs/RAIL.md`. Phases R0–R3 (rail crate → shadow capture → Worker
ingest/DO/R2 → anchoring + public verifier).

Status: SPEC DRAFTED 2026-08-28. Supersedes the alpha-selling framing of
roadmap #7b per benches 025/035 and the CUPED closure (033/037/038 + clip and
historical probes). Verified before drafting: Jupiter quote API returns
contextSlot but no signature headers; DFlow RFC 9421 signing already shipped
in v4.2.

Next action: R0 — `afterswap-rail` crate with the record schema and its tests.
