# DFlow partner ask — verifiable execution rail + dataset

Audience: DFlow team.
Context: AfterSwap, DFlow × Superteam Thailand Buildathon 2026.
Repo: https://github.com/ozoneRatchapon/afterswap
Status: live devnet integration; mainnet-ready design in `docs/PHASE_B_DELEGATED_EXECUTION.md`.

---

## 1. The one-paragraph version

AfterSwap is a post-swap exit engine that runs in the user's browser,
consumes DFlow `GET /quote` as its only sensor, and acts through
`GET /order` as its actuator. Every quote is RFC 9421-verified in the
user's own tab before the engine acts on it. The engine's exit policy is
committed to a Solana PDA *before* any sell follows it. The fill is
anchored to the exact signed quote it followed.

This is the "Verifiable Execution Rail" — a chain of three cryptographic
facts (signed quote → committed policy → verified fill) that, as of
today, **no project in this space can produce end-to-end for mainnet
fills**. We have the program, the verification, and the anchoring. What
we do not have is the production API surface that makes the last link
(mainnet fills) as clean as the first two (signed quotes, policy
commitment).

This document is the ask, the value we bring back, and the specific
interfaces we need.

---

## 2. What we bring to DFlow

### 2.1 Provably uninformed flow — the flow your design is built for

DFlow's thesis: protect market makers from toxic flow, which yields
better pricing for benign flow. Our machines are deterministic public
policies with a blake3 fingerprint, committed on-chain before any fill
follows them. They are the **least informed flow that can exist**: the
decision rule is fixed, public, and verifiable before the order is
placed. A wallet running AfterSwap should, in principle, earn better
quotes than a wallet running a black-box ML exit.

This is not a pitch line. It is a research conversation:

- **Hypothesis:** DFlow's conditional-liquidity pricing is more favorable
  to deterministic, pre-committed exit flow than to reactive,
  post-price-observation flow.
- **Test:** run the same exit strategy (same tranches, same timing)
  through (a) the imperative `GET /order` path and (b) the declarative
  `/intent` path, on the same assets, same hours, same size. Measure
  realized slippage, priority fees, and venue composition. We have the
  paired-evaluation harness (bench `039_goat` methodology) and the
  execution-cost model (bench `019_cost`) to run this in days, not
  weeks.
- **Output:** a published, reproducible comparison — DFlow's first
  public dataset on the execution-quality differential between
  declarative and imperative order flow, measured on real post-swap
  volume, not synthetic.

### 2.2 An execution-quality public dataset

We already record, per cycle:

- primary quote (DFlow, RFC 9421-signed, with `routePlan[].venue`)
- the engine's decision and the machine fingerprint that made it
- the realized fill (signature, slot, balance deltas via
  `parse_confirmed`)
- the committed policy PDA address and memo binding

What we do not yet record, because we do not yet have access:

- `GET /order-status` realized fill vs. quoted — the ground truth for
  execution quality
- `GET /priority-fees` — the congestion signal and real cost term
- multi-venue depth (the book stream WS, currently access-gated)

If we get production access, we commit to publishing a
**"DFlow execution weather report"**: quotes, venues, realized fills,
priority fees, over time, per instrument. This is a product on its own
and a distribution channel for DFlow's API. It costs us disk. It is the
kind of public artifact that makes a venue's execution quality legible
to the users who choose it.

### 2.3 A verifiable-compliance artifact

The signed quote → committed policy → verified fill chain is, in
substance, the kind of execution record that MiCA Article 78 contemplates
for algorithmic trading: a pre-committed rule, a price the venue
actually offered, a fill that followed. We do not claim compliance —
that is a determination for a regulator and counsel. We do claim that
the artifact is **provable by a third party without calling any of us**:
the auditor needs our public endpoint, public Solana RPC, and nothing
else. (`RAIL.md` §3.4 lists the six verification steps.)

For DFlow, this means: a customer who uses AfterSwap to manage their
exit has an execution record that is verifiable against DFlow's own
signed quote. That is a trust artifact DFlow can offer its users, and
it is one that only DFlow can co-produce (the signature is theirs).

### 2.4 A research partner, not just an API consumer

We are a solo buildathon team, but the harness is not a one-off: the
paired-evaluation methodology (bench `039_goat`), the null control
(random-walk corpus), the overfitting test (CSCV/PBO), and the
execution-cost model (bench `019_cost`) are a reusable research
infrastructure. We are willing to collaborate on:

- the declarative-vs-imperative execution-quality comparison (§2.1)
- the execution-quality dataset (§2.2)
- the verifiable-compliance artifact spec (§2.3), if DFlow wants to
  offer it to its users
- any of the above, published under both names

---

## 3. What we need from DFlow

Ordered by impact on the product. "✅" = already verified working from
our environment; "🔒" = exists but access-gated; "⛔" = attempted and
failed.

### 3.1 Production API key + rate limits (highest priority)

**Current state:** we run on the keyless dev endpoint. The dev endpoint
has no stable rate limit (measured: 40-call runs gave 20/40 and 25/40
success on different days), no production SLA, and the
`route_not_found` on `/intent` (see §3.2) may be a dev-endpoint
artifact.

**Ask:** a production API key with documented rate limits and a stable
endpoint. The key does not need to be free — we can pay — but it needs
to be *stable* and *documented*, so that the verifiable chain's first
link (signed quote) is produced by a production-grade service, not a
dev endpoint that may change or disappear.

**Why this matters to DFlow:** a production key is the difference
between "a buildathon demo uses DFlow" and "a verifiable execution
rail, auditable by third parties, runs on DFlow rails in production."
The latter is the kind of reference architecture that other teams will
build on top of.

### 3.2 Declarative swaps: `/intent` + `/submit-intent` (second priority)

**Current state:** ⛔ `route_not_found` on the dev endpoint. This is
the single biggest *measurable* execution win available to us
(`OPPORTUNITIES.md` §3.1): small, repeated, uninformed tranches are
exactly the flow shape declarative orders are designed to handle.

**Ask:** either (a) confirm the correct dev-endpoint path for
`/intent` and `/submit-intent`, or (b) provide a production endpoint
where they are available. We have the paired-evaluation harness ready
to measure the differential the moment we can send both order types
side by side.

**Why this matters to DFlow:** the declarative-vs-imperative
comparison (§2.1) is a published artifact that demonstrates the value
of DFlow's conditional-liquidity design on real post-swap volume. It
is the kind of result that answers the question "why use DFlow instead
of a CEX" with a number, not an adjective.

### 3.3 `GET /order-status` — realized fill ground truth (third priority)

**Current state:** ✅ documented, not yet used. We infer fills from
balance deltas (`parse_confirmed`), which works but is a derived
measurement. `GET /order-status` would give us the venue's own
realized-fill record, which is the ground truth for the
execution-quality dataset (§2.2).

**Ask:** confirm `GET /order-status` is available on the production
endpoint and document its schema (fields, latency, retention). We will
incorporate it as the primary fill reference in the dataset, with
`parse_confirmed` as the cross-check.

### 3.4 Book stream WS + quote stream WS (fourth priority)

**Current state:** 🔒 access-gated. The book stream (10 levels of
depth) would convert our inferred-depth signals into real depth, and
the quote stream would remove our 1-second polling quantization.

**Ask:** access to the WS endpoints for the production key. Not
blocking for v1 (the REST surface is sufficient for the verifiable
rail), but the execution-quality dataset is significantly stronger
with real depth than inferred depth.

### 3.5 Sponsored swaps (fifth priority)

**Current state:** ✅ documented. A visitor could run a *real* mainnet
tranche without holding SOL, which removes the last friction from the
demo.

**Ask:** confirm sponsored swaps are available on the production
endpoint and document the integration (which field in `/order`
triggers sponsorship, who pays the fee). We will add a "sponsored
tranche" mode to the demo as a buildathon showcase and a production
on-ramp.

### 3.6 Research collaboration (ongoing)

**Ask:** a DFlow engineer or researcher as a co-author on:
- the declarative-vs-imperative execution-quality paper (§2.1)
- the execution-quality dataset schema and first release (§2.2)
- the verifiable-compliance artifact spec, if DFlow wants to offer it
  to its users (§2.3)

We bring the harness, the methodology, and the negative results. DFlow
brings the API, the data, and the domain expertise. Both names on the
paper.

---

## 4. What we will not ask for

- **We will not ask for a rebate.** The value exchange here is
  research and verifiability, not fee reduction. If the
  declarative-vs-imperative comparison shows that uninformed flow earns
  better quotes, that is a research finding, not a negotiation.
- **We will not ask for exclusive access.** The dataset (§2.2) is
  public. The verifiable rail (§2.3) is a spec, not a walled garden.
  The partnership is about producing a public artifact that
  demonstrates DFlow's execution quality, not about locking it down.
- **We will not claim compliance.** MiCA Article 78 alignment is a
  property of the artifact, not a certification. The determination is
  for a regulator. We will state this clearly in every publication.

---

## 5. Timeline

| Week | What we do | What we need from DFlow |
|---|---|---|
| 0 (now) | Buildathon demo live; devnet policy PDA deployed; verifiable chain shipped (quote verify → policy commit → memo anchor) | — |
| 1 | Phase B design complete (`docs/PHASE_B_DELEGATED_EXECUTION.md`); LiteSVM test plan written | Production API key (§3.1) |
| 2 | Phase B implementation + tests; devnet deploy of the enforced gate | `/intent` endpoint (§3.2) |
| 3 | Paired-evaluation harness pointed at production; first declarative-vs-imperative comparison run | `GET /order-status` (§3.3) |
| 4 | Execution-quality dataset v0 published (quotes, venues, fills, priority fees) | WS access (§3.4), co-author (§3.6) |
| 5–8 | Phase B audit; mainnet deploy; sponsored-swap mode; full execution-quality dataset v1 | — |

---

## 6. The honest limit

We do not have a durable price edge. Our harness says so, clearly,
across 11 real assets, with the overfitting test and the null control
in the repo. What we have instead is the only thing that survived the
harness: a **verifiable execution chain** that is provable by a third
party without calling any of us, and a **research infrastructure** that
can measure execution quality in days rather than weeks.

The ask in this document is for the API surface that makes the
verifiable chain end-to-end for mainnet fills, and for a research
collaboration that produces a public artifact demonstrating DFlow's
execution quality on real post-swap volume. Both are things DFlow can
say yes to without giving up anything, and both produce something
public that is better for DFlow's users than either of us could
produce alone.

We are happy to walk through the harness, the negative results, and the
verifiable chain in whatever order is useful. The code is in the repo;
the benchmarks are reproducible with one command; the program is live
on devnet with the explorer link in `docs/PITCH.md`.
