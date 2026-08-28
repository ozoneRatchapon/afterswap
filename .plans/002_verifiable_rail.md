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

## Deployment ✅ DONE 2026-08-28 (RAIL.md §7)

- [x] **Production deploy** — dashboard `afterswap` and the pure-Rust rail
      Worker `afterswap-rail` both live on workers.dev. The DO-migration
      caveat did not bite: `v1: RailSequencer` went through a normal
      `wrangler deploy`, no 10013/10021 fallback, confirmed with `wrangler
      deployments list` rather than assumed from a green push.
- [x] **Real attestation key** — rotated off the public dev seed. Production
      pubkey `887e4537…3451` in `wrangler.jsonc`; the seed is executor-only
      (`--attest-seed-hex`, `gen_chain` now takes the same flag) and is not
      in git. Proven by falsification, not assertion: a dev-attested record
      is rejected `400 attestation does not verify`. DO instance `rail-v1` →
      `rail-prod-v1` so the production chain starts at seq 0 under the new
      key — `rail.html` verifies every record against the single current
      pubkey, so a dev-attested prefix would have shown as failures.
- [x] **External ≤30 s measurement** — 70/70 ingested against the production
      edge, 0 rejected, ingest→publicly-readable median 227 ms / p90 294 ms /
      max 393 ms. Inside budget by ~76×. Honest caveat: observed from the
      executor machine, not the third host §7.6 specifies.
- [x] **First funded anchor** — devnet, from a dedicated keypair
      (`58CQybdb…xiks`, 4.86 SOL). Root `9dbec084…79b0` (seq 0..63) anchored
      in tx `2Um3Jsvdk5uc…DpMzvt`, Finalized, fee ◎0.000005, memo
      byte-matching the root; `/rail/proof/10` from the live Worker VERIFIES
      under the native crate against that anchored root.
- [x] **`scripts/rail_falsifier.sh` un-stalled** — it still booted `wrangler
      dev` on the repo-root (dashboard) config, which has no RAIL binding.
      Two further defects surfaced while proving the fix rather than
      asserting it: `--config` is not enough, because wrangler runs
      `worker-build` in the *invoking* cwd and dies on the workspace
      Cargo.toml (`missing field \`package\``) — it has to `cd`; and the run
      was not repeatable, since a persisted miniflare DO makes the replayed
      log fail wholesale as `seq not monotonic`, so each run now gets a
      throwaway `--persist-to` dir. Two consecutive clean runs: 81/81
      ingested, 0 rejected, median 2.5–3.3 ms, fork 400, replay 409, segment
      closed, identical root `68462ce7…`, proof native-VERIFIED.
- [—] **R2 bucket** — **CLOSED, not executed.** Deliberately not created: the
      §8 free-tier invariant keeps closed segments in DO SQLite, and creating
      the bucket would breach it for no capability the rail lacks today. This
      is a resolved decision, not outstanding work — re-open only if the
      project leaves the free tier.
- [x] **Point the real executor at production ingest** (§7.5) —
      `--rail-ingest <origin>` and `crates/afterswap-server/src/rail_ship.rs`.
      70 live cycles: **70 accepted, 0 rejected, 0 failed, 0 seq gaps** against
      the production edge. The rail now carries real evidence (RFC 9421
      provider-signed DFlow beside observed Jupiter, Meteora DLMM /
      PancakeSwap routes), and segment root `ff63ca41…e99e` (seq 64..127) is
      anchored in devnet tx `4FHorYfF178j…EJ7SWR`, memo byte-matching, with
      `/rail/proof/100` for a real record VERIFIED under the native crate.

      Design decisions, each with the reason it is not the obvious one:
      shipping is a channel into a *single* task, because the Sequencer
      enforces `prev_hash == tip` — parallelism here is a correctness bug, not
      a speed-up. `--rail-out` is flushed *before* enqueue, so an outage costs
      visibility and never the record; that is why `--rail-ingest` requires
      it. Failures are classified rather than blanket-retried: 400 is the
      record (retrying a bad signature is a busy-loop), 5xx/timeout is the
      edge (5 attempts, ~6 s of backoff), and 409 is read against the returned
      `tip_seq` because a lost response looks identical to a replay. Plaintext
      to a remote host is refused — over `http://` a network position can drop
      records indistinguishably from an outage.

      The reconciler is what makes the retry message ("can be replayed") true
      rather than aspirational. Both directions were falsified locally against
      `wrangler dev`: file-ahead-of-rail replayed its backlog (3 queued,
      6 accepted, 0 rejected); rail-ahead-of-file adopted the live tip at
      seq 5 instead of forking. In production it adopted seq 69 from an empty
      local file — the synthetic prefix was **kept, not deleted**: discarding
      an anchored bootstrap to make the trail read better is what an audit
      trail exists to prevent.

- [x] **Latent `.gitignore` bug, found while landing the above** —
      `data/rail/.gitignore` held `data/rail/*.jsonl`. Patterns in a nested
      ignore file are relative to *that* directory, so it expanded to
      `data/rail/data/rail/*.jsonl` and matched nothing; the run's chain file
      showed up untracked. Same defect in `data/execution/.gitignore`.

- [x] **`afterswap-server` grew a `lib.rs`** — the https-only guard on
      `--rail-ingest` is a security control, so it needs a test that survives
      refactors. The crate was binary-only, which forces assertions into
      inline `#[cfg(test)]` blocks against the house rule that tests live in
      `tests/`. `src/lib.rs` is now an index of the six modules, `main.rs` is
      dispatch-only, and the guard is asserted from
      `crates/afterswap-server/tests/rail_ship_guards.rs`.

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
