# Commit provenance corrections

This project's doctrine is that retraction should be cheap and deletion
impossible. That applies to its own history: when a commit message
misdescribes what the commit contains, the fix is a record here, not a
rewrite of published history.

## `f9e03a6` — message does not describe the contents

**Recorded 2026-08-27.**

Committed as:

```
docs: record the data-hygiene lesson — three false results in one day,
none of them statistical
```

Merged to `main` as `3231b70` ("docs: provenance lesson") and pushed.

**What it actually contains** — eight files, 713 insertions, none of which is
the data-hygiene lesson the subject line describes:

| file | what landed |
| --- | --- |
| `docs/research/EXTRACTION_LOSS.md` | new — full inventory and recovery of all 264 images embedded in the two external research documents, with the method to reproduce it |
| `docs/QUESTIONS.md` | corrected extraction note; **Round 3** questions (sections I–M) |
| `crates/afterswap-engine/src/power.rs` | `Z_POWER_95`; the unpaired sample-size convention note rewritten from "the reference contradicts itself" to the resolved reading (its power column is quoted per group, N = 1,068 total) |
| `crates/afterswap-engine/tests/power.rs` | reproduces all thirty cells of the published sample-size table, plus the document's worked example |
| `crates/afterswap-engine/examples/slice_sensitivity.rs` | new — sweeps the CSCV partition count per asset |
| `benches/030_slice_sensitivity/report.md` | new — result: the PBO dissent survives every partition count, refuting the slice-size artefact explanation |
| `benches/024_overfit/report.md` | its open question narrowed to point at bench 030 |
| `README.md` | the CLMM net-margin contrast added beside the BONK figure |

A `docs:` prefix is wrong for a commit carrying a 154-line Rust example, a
library change and 67 lines of tests.

**Why it happened.** A second process was operating on this working tree
concurrently — it committed on `develop`, checked out `main`, merged, and
returned to `develop` while these files were still being written. The commit
therefore captured work in progress under a subject written for different
work.

**Decision.** History stands. The commit is already on `origin/main`, and
rewriting a published tree to improve a subject line costs more than this
record does. Nothing in the tree is wrong — only its label.
