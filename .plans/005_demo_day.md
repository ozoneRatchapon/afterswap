# 005 — Demo Day blueprint (Sep 3, Bangkok)

> Positioning: AfterSwap as a frictionless exit tool **and** the Universal
> Verifiable Execution Layer on Solana. Written 2026-08-29, after the codebase,
> submission and benchmarks were locked.

---

## 0. The spine: three tiers, labelled everywhere

This project's single differentiator is that it does not overclaim. The
buildathon README's headline finding is a **negative result** — Romano–Wolf
returns zero survivors across 1,054 machines. A Demo Day pitch that jumps from
that to "universal layer for AI, IoT, ESG and ZK" without visible seams reads
as bait-and-switch to any judge who read the repo, and it burns the one asset
nobody else has.

So the vision is not the problem. **Unlabelled** vision is. Every claim in this
blueprint carries one of three tags, on the slide and in the doc:

| Tag | Meaning | Test |
|---|---|---|
| 🟢 **LIVE** | Running in production now | A stranger can verify it from a URL, today |
| 🟡 **BUILT** | Code + tests exist, not deployed | `cargo test` passes; no mainnet/devnet presence |
| 🔵 **DESIGN** | Spec only, zero code | Honest as a roadmap, dishonest as a capability |

Using these tags *is the pitch*. A team that ships its own roadmap with the
weak parts marked is demonstrating the discipline it is selling. Say that out
loud at 2:45.

### Verified state as of 2026-08-29

🟢 **LIVE — the full chain, third-party verifiable right now:**

```
record seq 100  →  hash 1052f81c1d56d05d…d71660c1
                →  6-node Merkle proof
                →  segment root ff63ca41db2baf46…861ae99e   (seq 64..127)
                →  devnet tx 4FHorYfF178joehk…Nh4EJ7SWR      (slot 489246357)
                →  memo: "afterswap:rail blake3=ff63ca41…861ae99e seq=64..127"
```

- Rail: 140 records, 0 seq gaps, 2 segments closed, **2 anchored**,
  attest key `887e4537d43618b9…`
- Multi-venue in one slot: record **seq 138**, `So11/EPjF`, slot `442282366` —
  DFlow `provider_signed` 107,579,448 vs Jupiter `observed` 107,581,629,
  **chose Jupiter**
- Browser verifier: `https://afterswap.solana-thailand.workers.dev/rail`
  (`/rail.html` 307s there), WASM `rail_verify_record`, `rail_record_hash`,
  `rail_merkle_verify`
- Policy commit program `GEz2tF…8bD8` executable on devnet, 3,285 CU/commit

🟡 **BUILT, not deployed:** Phase B tags 1–6 (delegated execution,
`ValidateAndSell` policy-bound enforcement, `AnchorFill` per-fill memo), 34
policy tests green. Blocked on the vault-vs-delegate custody decision.

🔵 **DESIGN, zero code:** everything ZK — selective disclosure, viewing keys,
confidential amounts. `rg -i 'zero-knowledge|zk-|viewing key|confidential'`
over the repo returns **nothing**. Also design-only: the domain-agnostic v2
schema (§2.1). **The receipt modal (§1) is now 🟡 BUILT** — implemented in
`web-wasm/public/rail.html`, executed against live data and real WASM, not yet
deployed.

---

## 1. The Digital Execution Receipt

### 1.1 The one design rule

**The seal reports the weakest link, not the average.**

This is not a stylistic choice. Live record 138 chose **Jupiter**, whose quote
is `observed` — self-attested by our key, not signed by Jupiter — over DFlow's
`provider_signed` quote, on a **0.2 bps** difference. A single green shield
over that receipt would be a lie of exactly the kind this project exists to
refuse. Three checks, three independent statuses, and the summary badge takes
the **minimum**.

This also happens to be the best moment in the demo: you show a receipt that
grades *itself* amber, and explain why. No competing demo will do that.

### 1.2 Component states

| State | Colour | Badge | Meaning |
|---|---|---|---|
| `verified` | green | 🛡 Verified | Cryptographically checked in this tab |
| `attested` | amber | ⚠ Attested only | Our key vouches; the venue signed nothing |
| `pending` | grey | ◷ Anchoring | Segment not yet closed/anchored |
| `failed` | red | ✕ Failed | Hash, signature or proof mismatch — show the diff |

`failed` must be reachable in the demo build. A verifier that cannot show you a
red is not a verifier. Keep a tampered fixture record on hand.

### 1.3 The three checks

**a) Market Comparison** — multi-venue snapshot, same slot.

```
┌ Market comparison ───────────────────── slot 442282366 ┐
│  DFlow    107,579,448 USDC   🛡 signed   RFC 9421      │
│  Jupiter  107,581,629 USDC   ⚠ observed  our attest    │  ← chosen, +0.2 bps
│                                                        │
│  Chose the better price. Note the asymmetry: the       │
│  venue we chose does not sign its quotes.              │
└────────────────────────────────────────────────────────┘
```

Status: `attested` (amber) whenever the **chosen** venue is `observed`. Do not
average the two venues' trust levels.

**b) Policy Compliance** — execution inside committed bounds.

```
┌ Policy compliance ─────────────────────────────────────┐
│  Fingerprint  1d38fb026e3d6d84   🛡 committed on-chain  │
│  Committed    slot 442280110  (before the fill)         │
│  Bounds       tranche ≤ 25%, min-out ≥ 99.5%     🔵     │
└────────────────────────────────────────────────────────┘
```

Honesty note for the slide: the **commitment** is 🟢 LIVE (the PDA exists, the
fingerprint is on devnet). **Bounds enforcement** is 🟡 BUILT — that is
`ValidateAndSell` in Phase B, tested, not deployed. Mark that row 🔵/🟡 in the
demo build rather than showing a green check the chain is not yet backing.

**c) Solana Anchor** — the Merkle root on chain.

```
┌ Solana anchor ─────────────────────────────────────────┐
│  Leaf     1052f81c…d71660c1                            │
│  Proof    6 nodes            🛡 verified in this tab    │
│  Root     ff63ca41…861ae99e                            │
│  Memo     "afterswap:rail blake3=ff63ca41… seq=64..127"│
│  Tx       4FHorYfF…EJ7SWR   devnet slot 489246357  ↗   │
└────────────────────────────────────────────────────────┘
```

`↗` opens Solana Explorer. **The judge should click it.** The memo string is
readable on the block explorer with no tooling — that is the whole trick, and
it lands in two seconds.

### 1.4 Timing claim — measure it, never hardcode it

Do **not** ship the string "verified in 3ms". Wrap the real calls and render
the measured value:

```js
// These return a plain "ok" string (or an error string) — not JSON. Network is
// timed out of the number on purpose, so it describes the verifier, not the wifi.
let ms = 0;
let t = performance.now();
const attest = rail_verify_record(recordJson, attestPubkeyHex);  // "ok" | "err: …"
const leaf   = rail_record_hash(recordJson);
ms += performance.now() - t;
const proof = await (await fetch(`${BASE}/rail/proof/${seq}`)).json();
t = performance.now();
const incl = rail_merkle_verify(leaf, JSON.stringify(proof.proof), proof.segment_root);
ms += performance.now() - t;
// → "verified in-browser via WASM in 0.4 ms (cryptography only, excludes network)"
```

**Measured 2026-08-29**, real WASM against live records: **0.2–0.7 ms warm**,
**~5.2 ms on the first call** (JIT). Render the live value — it is smaller than
the round number anyone would have invented, and it cannot become false.

A live number that moves between takes is more convincing than a round one,
and it cannot become false. The sub-claim that actually matters is **"no server
was asked"** — that is the zero-trust property, and it is 🟢 LIVE today.

### 1.5 DOM skeleton

```html
<dialog class="receipt" data-state="attested">
  <header>
    <span class="seal" data-state="attested">⚠ Attested</span>
    <h2>Execution receipt · seq 138</h2>
    <p class="sub">So11/EPjF · slot 442282366 · <span id="verify-ms"></span></p>
  </header>
  <section class="check" data-state="attested" data-check="market">…</section>
  <section class="check" data-state="verified" data-check="policy">…</section>
  <section class="check" data-state="verified" data-check="anchor">…</section>
  <footer>
    <a class="explorer" target="_blank" rel="noopener">View anchor on Solana ↗</a>
    <button class="raw">Show raw record + proof</button>
  </footer>
</dialog>
```

`seal` state = `min(market, policy, anchor)` over the ordering
`failed < pending < attested < verified`. Implement that as a helper, not by
hand, so it cannot drift.

### 1.6 What the live data actually does — measured, not assumed

The modal was executed against the live rail (real WASM, real records, a DOM
harness) before this section was written. Three findings change the demo:

**a) A green receipt is rare, and that is the honest result.** Of **70**
multi-venue records, only **10** earn green. A receipt goes green only when the
quotes share a `context_slot` *and* the chosen venue signed. Sixty are amber —
either the quotes are a slot apart (not a like-for-like comparison) or the
chosen venue was Jupiter, which signs nothing.

> Say this on stage: *"Sixty of our seventy comparisons are amber. The receipt
> is not decoration — most of the time it is telling you the comparison was
> imperfect, and we ship it anyway."*

**b) When DFlow wins, it is an exact tie — and the tie-break is a feature.**
Every DFlow win in the live set is equal to the lamport (seq 96:
107,482,926 vs 107,482,926). The tie breaks toward the venue that **signs**.
That is the only defensible way to break it, and the modal now says so in
words. When Jupiter wins it is by a real margin (seq 97: 0.16 bps).

**c) Demo picks, verified rendering 2026-08-29:**

| seq | Renders | Why |
|---|---|---|
| **120**, **123** | 🟢 all four green, explorer link live | same slot + DFlow signed + anchored |
| **121**, **122**, **124** | 🟡 amber overall, anchor green | chosen venue unsigned, or quotes a slot apart |
| 137 | green market, **pending** anchor | segment 128+ not closed yet |

Both 120 and 123 sit inside the default 25-row view (tip 139 → rows 115–139),
so the green/amber side-by-side needs **no URL parameters and no scrolling**.
Open 120, then 121. That is the whole beat.

### 1.7 Verification performed

- Real WASM (`afterswap_wasm_bg.wasm`) loaded in Node against live records:
  `rail_verify_record` **ok**, `rail_merkle_verify` **ok**, 6-node proofs,
  anchor `4FHorYfF…`.
- **Tamper test passes**: `out_amount + 1` → `err: attestation does not verify`.
  The red state is reachable, which is the only thing that makes the green
  state mean anything.
- Full page executed under a linkedom DOM with live data: **25 rows render**,
  stats line correct, seal states as tabled in 1.6, explorer link populated.
- `node --check` clean on the extracted module.

**Not verified:** real-browser layout, `<dialog>` visuals, `showModal()`
behaviour and CSS. The DOM harness stubs `showModal`. Open it in a browser
before relying on it on stage.

---

## 2. The Universal & Confidential Rail Schema

### 2.1 What exists vs. what "universal" costs

🟢 The **live** record shape already is your four-part schema:

| Your layer | Live field | Status |
|---|---|---|
| `PreStateSnapshot` | `quotes[]` + `evidence{kind, body_sha256, body_b64}` | 🟢 LIVE |
| `PolicyConstraint` | `policy_fingerprint` + devnet PDA | 🟢 commit LIVE / 🟡 enforcement BUILT |
| `ExecutionProof` | leaf → Merkle proof → segment root → memo anchor | 🟢 LIVE |
| `ClientVerifier` | `/rail` + WASM verify fns | 🟢 LIVE |

**But it is not domain-agnostic today, and saying so would be false.** The
live fields are finance-typed: `instrument`, `quotes`, `in_mint`, `out_mint`,
`in_amount`, `out_amount`, `route`, `chosen_venue`, `net_out`. A sensor feed
does not have a mint.

The honest claim — and it is still a strong one — is: **the shape generalises;
the field names do not.** What generalises is the four-part structure of
*signed context → committed rule → inclusion proof → client-side verifier*,
which is genuinely domain-independent. Getting to a literal shared schema is a
v2 with a migration, not a relabel:

```
v2 (🔵 DESIGN)              v1 (🟢 LIVE)
subject: string             instrument
observations: Obs[]         quotes[]
  ├ source: string            venue
  ├ at: {slot?, t_ms}         context_slot, t_ms
  ├ value: bytes              out_amount
  └ evidence: Evidence        evidence          ← unchanged, already generic
policy_fingerprint          policy_fingerprint  ← unchanged
action: {chosen, evaluated} decision            ← unchanged in shape
outcome: Outcome | null     fill
attestation: ed25519        attestation         ← unchanged
```

Three of seven fields already generalise untouched. That is the slide: *"we
did not design this to be universal — we noticed it already was, in the parts
that carry the cryptography."*

### 2.2 The three verticals

Each keeps `evidence`, `policy_fingerprint`, the Merkle anchor and the WASM
verifier **byte-identical**. Only `subject`, `observations` and `outcome`
change. All three are 🔵 DESIGN.

**a) Autonomous AI Agent Spend Guard**

| Layer | Binding |
|---|---|
| `PreStateSnapshot` | Provider's signed price quote (OpenAI/Anthropic/pay.sh), captured before the call |
| `PolicyConstraint` | PDA: max spend/hour, allowed providers, max unit price |
| `ExecutionProof` | Each API call = a leaf; segment anchored hourly |
| `ClientVerifier` | The *principal* — not the agent — re-verifies its agent stayed in budget |

Why it is the strongest vertical: the buyer already has the problem *today*,
it is measured in money, and the agent cannot be trusted to self-report — which
is exactly the trust asymmetry the rail is built for. It also composes with
pay.sh (`ROADMAP.md` §7b) rather than competing with it.

**b) Verifiable ESG & Edge Telemetry**

| Layer | Binding |
|---|---|
| `PreStateSnapshot` | Sensor reading + device attestation key (secure element) |
| `PolicyConstraint` | PDA: calibration hash, sample interval SLA, acceptable range |
| `ExecutionProof` | 64 readings/segment → one anchor → **sub-penny per batch** |
| `ClientVerifier` | Auditor verifies offline, no vendor API |

State the honest limit on this slide: the rail proves **a reading was recorded
and never altered afterwards**. It cannot prove the sensor was not lying. That
is a hardware-attestation problem, not a ledger problem. Saying so is what
separates you from every ESG-on-blockchain pitch a judge has already sat
through.

**c) Private Transactions & ZK-Compliance** — see §3.

### 2.3 The cost argument, with a real number

Anchoring is **one memo tx per 64 records**. Live measured: the anchor memo
consumed **36,122 CU** of 200,000. At devnet-equivalent mainnet pricing that is
a fraction of a cent per 64 records — so **per-record anchoring cost is
sub-penny by two orders of magnitude**, which is what makes the IoT/telemetry
vertical arithmetically possible at all.

Do not present a mainnet fee estimate you have not measured. Say "one memo per
64 records, 36,122 CU measured" and let the judge do their own arithmetic.

---

## 3. Privacy & Selective Disclosure (🔵 DESIGN — all of it)

### 3.1 The strongest privacy claim is already true, and it needs no ZK

Look at what is actually on chain today:

```
afterswap:rail blake3=ff63ca41db2baf…861ae99e seq=64..127
```

**That is all of it.** No wallet, no amount, no instrument, no venue. The chain
carries a 32-byte hash and a range. The record — amounts, venues, identity —
lives off-chain and is disclosed by whoever holds it, to whoever they choose.

So: **commitment and disclosure are already separated by construction.** The
architecture is privacy-preserving today, not because of a circuit, but
because it anchors a hash instead of a transaction. Lead with this. It is 🟢
LIVE, it is verifiable on the explorer in the same click as §1.3c, and it is
the honest version of "private by design".

### 3.2 The ladder, cheapest first

| Rung | Mechanism | Hides | Effort | Tag |
|---|---|---|---|---|
| 0 | Hash-only anchor | everything not disclosed | **done** | 🟢 LIVE |
| 1 | **Viewing keys** — encrypt record to auditor's X25519 key, publish ciphertext hash as the leaf | everything from everyone but the auditor | ~days | 🔵 |
| 2 | Token-2022 **confidential transfers** for the fill leg | on-chain amounts | ~weeks | 🔵 |
| 3 | **ZK proof of a property** — "executed within N bps of best observed quote" without revealing the record | the record, while still proving compliance | research | 🔵 |

**Recommendation, and it is a real engineering call:** rung 1 gets you ~80% of
"selective disclosure" for days of work and no cryptographic risk. Present
rung 1 as the next step and rung 3 as the horizon. Do not imply a circuit
exists.

### 3.3 The honest caveat that will earn you the room

If a judge asks about rung 3, the correct answer is the specific one:

> "The blocker is the hash. Our leaves are blake3, chosen because it is fast
> in WASM. blake3 in-circuit is expensive — a practical ZK version means
> moving the leaf hash to something like Poseidon, which changes the record
> format and every verifier that consumes it. That is a v2 of the schema, not
> a feature flag. We know what it costs; we have not paid it."

Naming the specific obstacle is the difference between a roadmap and a wish.
Most teams asked this question say "we'll add ZK". You can say *why it is hard
and what it would break* — which signals you have actually thought about it.

### 3.4 The paradox slide, resolved in one line

> **The paradox:** compliance demands you prove you behaved; privacy demands
> you reveal nothing.
> **The resolution:** commit to everything, publicly and immutably. Reveal
> selectively, later, to whoever has the right to ask.
> **Status:** the committing half is live. The selective-reveal half is
> designed, and the next rung is viewing keys, not a circuit.

---

## 4. The 3-minute narrative

> **Timing warning.** Five sections plus a live demo in 180 seconds is tight —
> the live segment (§4.2) is 50 s and carries all the risk. Rehearse with a
> stopwatch and be ready to cut §4.3 to 30 s. Pre-warm every surface: the
> Worker is ~2 s cold, ~0.1 s warm.

### 4.1 — 0:00–0:40 · The everyday hook

**On screen:** photos — a taxi meter seal, a certified weighing scale sticker,
a fuel pump calibration sticker. Then a black box with a `?`.

> "You have never once checked whether the taxi meter was honest. You did not
> need to — there is a seal on it. Same for the scale at the market, and the
> fuel pump. We built an entire civilisation of small trusted measurements,
> and we did it with a sticker and an inspector.
>
> Now: your swap executed. Your AI agent spent your money. A sensor said the
> factory was within emissions limits. Where is the seal? There isn't one. You
> get a screenshot and a promise."

**The line to land:** *"Web3 replaced the institution but forgot the sticker."*

### 4.2 — 0:40–1:30 · Live product demo (the risky 50 seconds)

Sequence, rehearsed:

1. **(10s)** Open the demo — already warm. A machine takes a position. "One
   click. The policy is committed on-chain *before* it can sell."
2. **(10s)** Show the exit running. "Cruise control. It sells on a rule,
   whether you are watching or not."
3. **(20s)** Click through to `/rail` — **the money moment.** Point at the
   green check appearing. "That check just ran *in this tab*, in WASM, in
   [read the live number] milliseconds. No server was asked whether the data
   is good. My laptop checked."
4. **(10s)** Click the explorer link. The memo appears:
   `afterswap:rail blake3=ff63ca41… seq=64..127`. "And there is the root, on
   Solana, anchored before I walked on stage."

**Fallback:** if live quotes are flat or the network is hostile, `?replay`
gives the recorded deterministic segment. Record a screen capture of this
exact sequence beforehand and have it one keystroke away. A stalled live demo
costs more than a video.

**Do not skip:** show the **amber** badge on record 138 and say the sentence in
§1.1. Ten seconds, and it is the most credible thing in the pitch.

### 4.3 — 1:30–2:15 · The universal horizon

**On screen:** one diagram, the four layers, with three columns beside it —
*Trading / AI Agents / Telemetry* — and the middle rows highlighted as
**identical**.

> "Here is what we noticed after we built it. The cryptography does not know
> it is looking at a trade. Signed context, a rule committed in advance, an
> inclusion proof, a verifier that trusts nobody — none of those four are
> financial. Swap the observations for an AI agent's provider price quotes and
> it is a spend guard the agent cannot forge. Swap them for sensor readings
> and it is tamper-proof ESG telemetry at one anchor per 64 readings.
>
> Three of the seven fields are already domain-agnostic — the three that carry
> the cryptography. The rest is renaming, and I am marking it design, not
> done."

**Slide must carry the 🔵 tag.** Say "design" out loud.

### 4.4 — 2:15–2:45 · Privacy & institutional compliance

**On screen:** the memo string, huge, alone. Then the full record beside it.

> "Compliance says prove you behaved. Privacy says reveal nothing. Look at
> what we actually put on chain — a hash and a range. No wallet. No amount. No
> instrument. The commitment is public and permanent; the record is yours, and
> you disclose it to whoever earns the right to ask.
>
> That half is live today. The next rung is viewing keys — encrypt the record
> to an auditor's key. Full zero-knowledge proofs of best execution are the
> horizon, and I can tell you exactly what blocks them: our leaf hash is
> blake3, and blake3 in-circuit is expensive. That is a schema change, not a
> feature flag."

### 4.5 — 2:45–3:00 · Closing

> "We looked hard for trading edge with a harness good enough to catch
> ourselves, and we published the negative result. What survived is better:
> zero-cost, mathematically checkable credibility — one memo per 64 records,
> verified in a browser tab that trusts no one.
>
> Every claim in this deck was tagged live, built, or design. Three of them
> were design and I said so. That is the product."

**Last line on screen:** *AfterSwap — the seal on the meter, for anything
Solana can anchor.*

### 4.6 Slide inventory (10 slides)

| # | Slide | Tag | Asset needed |
|---|---|---|---|
| 1 | Taxi meter / scale / pump seals + black box | — | 4 photos |
| 2 | "Web3 replaced the institution, forgot the sticker" | — | type only |
| 3 | Live demo (no slide — screen capture) | 🟢 | warm browser + recorded fallback |
| 4 | The receipt modal, amber state, annotated | 🟢/🟡 | **build §1** |
| 5 | Explorer screenshot, memo highlighted | 🟢 | screenshot `4FHorYfF…` |
| 6 | Four-layer diagram × three verticals | 🔵 | **build diagram** |
| 7 | Spend guard / ESG detail | 🔵 | table from §2.2 |
| 8 | Privacy paradox, memo vs. record | 🟢/🔵 | side-by-side |
| 9 | The disclosure ladder, rungs 0–3 | 🟢/🔵 | table from §3.2 |
| 10 | Closing + the tag legend | — | legend must appear |

---

## 5. Build order before Sep 3 (five days)

Ranked by demo impact per hour. Everything here is additive — **no changes to
locked code, benchmarks or the submission.**

1. ~~**The receipt modal (§1)**~~ — ✅ **BUILT 2026-08-29** in
   `web-wasm/public/rail.html`, verified against live data and real WASM (§1.7).
   Remaining for the owner: **open it in a real browser** (the DOM harness
   stubs `showModal`), then **deploy** — `wrangler` is classifier-blocked here.
2. **Screenshot + record the fallback capture** — insurance for §4.2.
3. **The four-layer × three-vertical diagram** — slide 6, the whole of §4.3.
4. **Tag legend on every slide** — mechanical, 30 minutes, and it is what makes
   the vision safe to present.
5. *Optional, only if 1–4 are done:* a tampered fixture record so the modal can
   show a **red** state on demand. Powerful, not essential.

**Explicitly not before Sep 3:** viewing keys, any ZK work, the v2 schema
rename, deploying Phase B. Each is weeks, and none of them changes what a judge
sees in 180 seconds.

---

## 6. Open decisions for the owner

1. **Does the receipt modal go on the demo page, or stay on `/rail`?** Demo
   page is a better narrative (one surface, no context switch); `/rail` is
   lower risk (already deployed and verified). Recommendation: build it on
   `/rail`, link it prominently from the demo page.
2. **Phase B custody (vault vs. delegate) still gates deployment** — unchanged
   from the pre-submission recommendation, and now it also gates whether slide
   4's policy row can ever go green. Not a Sep 3 decision.
3. **Is "Universal Verifiable Execution Layer" the headline, or the second
   act?** This blueprint puts the exit tool first and the universal claim at
   1:30, because the live demo is the credibility that buys the vision. Going
   universal-first inverts that and asks judges to believe the big claim before
   seeing the small proof.
