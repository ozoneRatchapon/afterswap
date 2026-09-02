# Post-deploy `/decide` re-measurement — prepared doc edits

Everything here is written **before** the deploy, so the wording cannot be
tuned to a flattering result. Three documents quote the same pre-fix figure
("20 ok, 20 failed", measured 2026-08-28) and all three promise in writing to
be updated with what is *measured* after the FSM-table fix ships. This file is
the exact text for that update, with only the numbers left as placeholders.

## Procedure — run this, do not improvise a new one

```bash
npx wrangler deploy                     # the gating step; not yet done
scripts/decide_measure.sh 40            # same 40-call procedure as the pre-fix figure
```

`scripts/decide_measure.sh` prints a summary line:

```
n=40 ok=<OK> fail=<FAIL> rate=<RATE>% p50=<P50>ms p95=<P95>ms max=<MAX>ms codes=<CODES>
```

Substitute those tokens below. **Do not** substitute an expectation. The
predicted result is "near 40/40, because a cold call fell 752 ms → 7 ms in
local `workerd`" — a prediction is not a measurement, and if the measured
number contradicts it, the measured number is what ships.

If `<OK>` is not ~40, the fix did not do in prod what it did locally: say so
plainly in all three files and keep the preview warning, rather than softening
the numbers to fit the story already written around them.

**Sanity check before believing a good result:** prod failures cluster (a
killed request traps the per-isolate wasm instance), so a short run can read
0/3 or 3/3 by luck. Two spot checks on 2026-08-28 10:39 UTC returned 0 ok / 7
failed against the *pre-fix* build — consistent with the documented clustering.
Only the full 40-call run is the measurement.

### Second pre-fix baseline — 2026-08-28 10:47 UTC

A full 40-call run against the **same pre-fix build** that produced the
"20 ok, 20 failed" figure:

```
n=40 ok=25 fail=15 rate=62.5% p50=63ms p95=1694ms max=1817ms codes=200:25,503:15
```

Two consequences, both of which change how the post-deploy numbers must be
written up:

1. **The pre-fix failure rate is not a constant.** Three measurements of one
   unchanged build: 20/40 (50%), 25/40 (62.5%), and 0/7 in a spot check. Do
   **not** rewrite the "20 ok, 20 failed" figure to this one — both are real
   measurements of the same build, and quoting either alone as *the* pre-fix
   rate overclaims a stability the endpoint does not have. Where the docs
   state the pre-fix figure, say it varied run to run and give the range.
2. **It corroborates the diagnosed cause.** Successful calls land at
   p50 63 ms but p95 1694 ms / max 1817 ms — pressed against the fixed
   2,010 ms CPU ceiling, with the 503s the calls that tipped over it. That is
   the runtime-enumeration cost showing up directly in the latency tail, which
   is what the FSM table is supposed to remove. So the post-deploy check is
   **not only `ok=40`**: the p95 must also collapse toward p50. A run that
   returned 40/40 while still showing a ~1.7 s p95 would mean the ceiling was
   merely no longer being crossed, not that the work was gone — report that
   distinction rather than calling it a clean success.

---

## 1. `README.md` — the "Status" paragraph (~line 299)

**Replace:**

> **Status: unreliable preview — roughly half of calls fail.** Measured
> 2026-08-28 over 40 consecutive requests: **20 returned a real roster, 20
> failed.** Use the local WASM path (`docs/API.md`) when you need an answer
> every time.

**With (success case):**

> **Status: measured <DATE> over 40 consecutive requests — <OK> returned a real
> roster, <FAIL> failed (p50 <P50> ms, p95 <P95> ms).** The free-plan CPU
> ceiling that used to kill cold starts — a 1,694 ms p95 against a 2,010 ms
> limit — is no longer reached: the 1,054-machine
> enumeration is precomputed and shipped as a 2,108-byte table, so a cold call
> costs ~7 ms instead of 752 ms. The local WASM path (`docs/API.md`) remains the
> route for anything that must answer without a network hop.

**With (failure case — if `<OK>` is materially below 40):**

> **Status: still unreliable — measured <DATE> over 40 consecutive requests:
> <OK> ok, <FAIL> failed.** The FSM precompute removed the enumeration cost
> (cold `/decide` 752 ms → 7 ms in local `workerd`), so whatever is failing in
> prod is *not* the enumeration ceiling and has not been diagnosed yet. Use the
> local WASM path (`docs/API.md`).

**Also delete the now-spent promise at the end of that section (~line 333):**

> ... so it stays a cache and never becomes a second
> source of truth. The prod success rate will be re-measured after the next
> deploy and this section updated with what it actually shows.

(Delete only the final sentence — "The prod success rate ... what it actually
shows." It is line-wrapped mid-sentence at README.md:332, so match on
`re-measured after the next` rather than on the whole sentence. Keep the
`tests/fsm_table.rs` clause that precedes it.)

Keep the whole pre-fix causal account (2,010 ms ceiling, 1101 vs 1102, poisoned
mutex, leaked handle, 503 degradation) — it is true history and explains why the
table exists. Retitle it "**How it used to fail**" so no reader takes it as
current behaviour.

## 2. `docs/API.md` — heading + measured line (~lines 29, 51)

**Heading, replace:**

> `## Hosted endpoint (preview, no key — expect ~50% failures)`

**With (success case):** `## Hosted endpoint (no key)`
**With (failure case):** `## Hosted endpoint (preview, no key — expect failures)`

**Replace the measured sentence (~line 51):**

> Measured 2026-08-28 over 40 consecutive calls: **20 ok, 20 failed.**

**With:**

> Measured <DATE> over 40 consecutive calls: **<OK> ok, <FAIL> failed**, p50
> <P50> ms / p95 <P95> ms. (The pre-fix build was unstable run to run — two
> 40-call runs on 2026-08-28 gave 20 ok / 20 failed and 25 ok / 15 failed, with
> a p95 of 1,694 ms against a 2,010 ms ceiling. The difference is the
> precomputed FSM table, below.)

**Replace the closing promise (~lines 58-62):**

> That enumeration is no longer paid at runtime: ... The 40-call figure above is
> pre-fix; the hosted endpoint will be re-measured after the next deploy and
> this line updated with the result.

**With:**

> That enumeration is no longer paid at runtime: its result is precomputed and
> shipped as the surviving raw indices (2,108 bytes), so a cold `/decide` in
> local `workerd` fell from **752 ms to 7 ms**. The figure above is the
> post-deploy measurement.

Leave the failure contract (`503 {"error":"engine unavailable, retry shortly"}`,
retry guidance) in place regardless of outcome — it is still the behaviour.

## 3. `docs/ROADMAP.md` — entry header + closing paragraph (~lines 252, 296)

**Header, replace:**

> **Preview ✅ built (v2.2), deployed but unreliable:**

**With (success case):** `**Preview ✅ built (v2.2), deployed:**`
**With (failure case):** leave the header exactly as it is.

**Replace the closing paragraph (~lines 296-298):**

> The 20/40 success figure above is **pre-fix**. It will be re-measured on the
> hosted endpoint after the next deploy, and this entry updated with the result
> rather than an expectation.

**With:**

> Re-measured on the hosted endpoint after deploy, <DATE>, same 40-call
> procedure (`scripts/decide_measure.sh`): **<OK> ok, <FAIL> failed**, p50
> <P50> ms / p95 <P95> ms. The pre-fix build did not have one stable rate —
> two 40-call runs the same day gave **20 ok / 20 failed** and **25 ok / 15
> failed** — so the comparison is against that range, not a single number.

Do **not** touch the "Correction (2026-08-28)" block. It is a retraction of an
earlier overclaim; retractions stay put even when the underlying problem is
fixed, because the record of having overclaimed is the point.

---

## Checklist

- [x] Measurement procedure captured as a script, so the re-measurement is the
      same measurement (`scripts/decide_measure.sh`, validated end-to-end).
- [x] Replacement text written for all three files, before the result is known,
      with both a success and a failure variant.
- [x] `npx wrangler deploy` — done by the user 2026-08-28. Version
      `4f69c750-f06e-44f6-981e-81cd4c48233a`. One asset changed:
      `/pkg/afterswap_wasm_bg.wasm` — the artifact verified pre-deploy to carry
      all three FSM tables byte-for-byte.
- [x] Run `scripts/decide_measure.sh 40` and substitute the tokens. Done; the
      **success** variant shipped.

## Result — 2026-08-28, post-deploy

```
n=40 ok=40 fail=0 rate=100.0% p50=76ms p95=134ms max=189ms codes=200:40
```

Against the two pre-fix runs of the same procedure (20/40 and 25/40):

| | pre-fix | post-fix |
|---|---|---|
| ok / 40 | 20, then 25 | **40** |
| p50 | 63 ms | 76 ms |
| p95 | 1,694 ms | **134 ms** |
| max | 1,817 ms | **189 ms** |

**Both pre-registered conditions are met, not just the headline.** The
criterion written before the deploy was that `ok=40` alone would not count as
success — the latency tail had to collapse too, since 40/40 with a ~1.7 s p95
would mean the 2,010 ms ceiling was merely no longer being crossed rather than
the enumeration work being gone. The p95 fell 1,694 ms → 134 ms and the max
189 ms now sits ~10x under the ceiling, so the work is genuinely gone. p50 rose
slightly (63 → 76 ms), which is not a regression of interest: the pre-fix p50
was computed over successful calls only, i.e. over a survivor-biased sample of
the fastest warm hits.

Caveat kept deliberately at the time: that was one 40-call run, and the pre-fix
build had proved the rate can vary run to run, so a single clean run was
evidence the ceiling no longer binds rather than proof of a permanent 100%.

### Replication — 2026-08-28, second post-deploy run

That caveat was the weakest claim in the shipped docs, so the same procedure was
run a second time against the same deployed version, unchanged:

```
n=40 ok=40 fail=0 rate=100.0% p50=69ms p95=125ms max=151ms codes=200:40
```

**80/80 across two independent runs**, and the tail was *tighter* the second
time (p95 134 → 125 ms, max 189 → 151 ms). This is the check the pre-fix
instability demanded: a build whose rate swings 20/40 → 25/40 → 0/7 does not
produce two consecutive clean 40s, so replication distinguishes a fix from a
lucky draw in a way one run cannot. README / API / ROADMAP were updated from
"40 consecutive requests" to the two-run figure.

The honest residual: two runs bound the variance far better than one, but both
were taken within minutes of each other from one client location. They do not
speak to behaviour under concurrency or from other Cloudflare colos.

### Other prod endpoints — checked for the same failure mode

`/decide` was fixed; the question is whether any sibling route sits near the
same 2,010 ms free-plan ceiling. `worker/index.ts` exposes only two others:

- `/api/score` — Durable Object read, no wasm engine. Probed 10x: **10/10 200**,
  80–217 ms, no ceiling exposure.
- `/api/commit-policy` — ed25519 signing only; bounded, constant work, no
  enumeration. **Deliberately not probed.** Each call consumes one of a limited
  set of demo position slots and signs a real devnet transaction, so spending
  slots on a latency measurement days before the demo costs more than the
  measurement is worth. The code path is short enough to rule out by reading.

Neither runs the 1,054-machine enumeration that broke `/decide`, so the fix
closed the only instance of that failure mode in the deployed surface.

### Follow-on edits made beyond the prepared text

Substituting the tokens left three sentences that had become false, all fixed:

- `README.md` — the causal account was in present tense ("about half of cold
  starts **are** killed") and one sentence still claimed "the numbers above are
  the *pre-fix* measurement." Retitled **How it used to fail**, put in past
  tense, and the claim corrected. The history itself was kept.
- `docs/ROADMAP.md` — same present-tense problem, plus the header
  "**Ceiling removed at the source (2026-08-28, not yet re-measured in prod)**"
  was stale. The "Correction (2026-08-28)" retraction block was **not** touched.
- `.plans/004_submission_kit.md` / `.plans/000_buildathon.md` — the recording
  note said "do **not** demo `/decide` on camera, it is still the failing
  pre-fix build." No longer true; corrected to safe-but-optional.
