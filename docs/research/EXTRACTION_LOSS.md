# Extraction loss and recovery — external research documents

Both source documents render formulas and most numeric table cells as
**images**. The plain-text exports committed beside this file drop that
content silently: no placeholder, just whitespace. This file records what was
lost and what the images actually said, recovered by exporting each document
as `.docx` and reading `word/media/` against the image references in
`word/document.xml`.

The `.txt` files are left **verbatim** — annotating them in place would
destroy their value as a faithful export. Read them alongside this file.

- Round one — `2026-08-27_epistemic_governance.txt`, 70 images
- Round two — `2026-08-27_nondirectional_execution.txt`, 194 images

## Round one: table survived, inline math did not

The power/sample-size table exported as text (`Paired: 5,306`,
`Unpaired: 136,760`, `60.33%` at N=534) and is what
`crates/afterswap-engine/src/power.rs` was verified against. Lost inline:
the CSCV symbols and the DSR variance/skew/kurtosis terms. Nothing
downstream depended on them.

## Round two: every numeric cell in four tables was an image

### 1. Sample size and achieved power — RECOVERED, and it closes an open question

sigma_d = 2.6 bps paired, sigma_u = 6.6 bps unpaired, N = 534.

| delta (bps) | regime | N @80% | N @90% | N @95% | power @534 |
| --- | --- | --- | --- | --- | --- |
| 0.10 | paired | 5,306 | 7,104 | 8,785 | 14.20% |
| 0.10 | unpaired | 136,760 | 183,082 | 226,420 | 4.34% |
| 0.25 | paired | 849 | 1,137 | 1,406 | 60.33% |
| 0.25 | unpaired | 21,882 | 29,294 | 36,228 | 9.00% |
| 0.50 | paired | 213 | 285 | 352 | 99.35% |
| 0.50 | unpaired | 5,472 | 7,324 | 9,058 | 23.51% |
| 1.00 | paired | 54 | 72 | 88 | 100.00% |
| 1.00 | unpaired | 1,368 | 1,832 | 2,266 | 69.70% |
| 2.00 | paired | 14 | 18 | 22 | 100.00% |
| 2.00 | unpaired | 342 | 458 | 568 | 99.86% |

Numerically identical to round one — but round two adds an annotation round
one lacks: the unpaired power cells read **"(534/group)"**, i.e. N = 1,068
total, while the unpaired required-N column is a total across both arms.
The 534 was an image, so the annotation reached us as a bare `( /group)`.

That missing label is why `power.rs` carried a note saying the reference's
two unpaired columns implied contradictory conventions. They do not. With
the label recovered, **our implementation reproduces all thirty cells** —
`tests/power.rs::reproduces_the_full_reference_table`. The note is corrected.

Also recovered: `z_0.975 ~ 1.95996`, `z_0.80 ~ 0.84162`, `z_0.90 ~ 1.28155`,
scaling factors `7.84886` and `10.5074`, the MDE formula
`sqrt((z_{1-a/2} + z_{1-b})^2 * sigma^2 / N)`, and the worked sentence: an
unpaired soak of 534 cycles has 9.00% power at delta = 0.25 bps — a **91.0%**
chance of missing a real effect.

### 2. CSCV partition sizing — RECOVERED, and it validates a config we chose blind

For T in [166, 375] windows (our range — the document is written about our
data):

- `S = 10` gives C(10,5) = **252** splits, 16-37 observations per slice
- `S = 16` gives C(16,8) = **12,870** splits, but only 10-23 per slice
- slices under **25** observations are dominated by within-slice sampling
  variance, driving PBO toward **0.50** regardless of true structure
- for **T < 400**, `S = 10` is the minimum stable configuration

`examples/overfit_check.rs` uses `SLICES = 10`, which is the prescribed
value. But read as a continuous sentence rather than as isolated glyphs, the
passage says something further that we had not accounted for: **the 25-
observation floor is per slice, and 7 of our 11 assets fall under it.**

| windows | obs/slice at S=10 | assets | PBO read |
| --- | --- | --- | --- |
| 375 | 37.5 | SOL_USDC | 0.087 |
| 250 | 25.0 | BONK, PEPE, WIF | 0.202, 0.198, 0.048 |
| 166 | **16.6** | FLOKI, JTO, JUP, ORCA, PYTH, RAY, SHIB | 0.623, 0.516, 0.190, 0.115, 0.448, 0.155, 0.075 |

`benches/024_overfit/report.md` closes on an open question: "Three assets
dissent — FLOKI (0.62), JTO (0.52), PYTH (0.45) ... We do not know why, and
with 166 windows each we cannot yet find out." The document offers a candidate
answer — under-sized slices drive PBO toward 0.50 on sampling variance alone —
and every dissenting asset is in the under-sized group, while all four assets
that clear the floor read between 0.048 and 0.202.

That is **consistent with, not proof of**, the artifact explanation: four of
the seven under-sized assets still read low, and three dissenters landing in a
seven-asset group would happen by chance about 21% of the time. What is not in
doubt is the reporting obligation — 7 of 11 PBO figures are computed in a
regime the source calls unreliable, whichever way they read.

That test has now been run — `benches/030_slice_sensitivity`, sweeping
S over {6, 8, 10, 12, 16} — and it **refutes the artifact explanation**. At
S = 6 (27.7 observations per slice, above the floor) no dissenter converges:
FLOKI is flat at -0.023, JTO +0.034 and PYTH +0.202 move away. Across the whole
sweep the three dissenters never enter the 0.05-0.28 band the other four
166-window assets never leave. The separation does not track slice size.

So the source's floor is real guidance we should respect in reporting, but it
is not what produced our dissent. Bench 024's open question survives, one
candidate answer lighter.

In the text export this entire passage was unreadable — every number in it was
an image, including the 25.

The two documents **disagree** here, and more sharply than the S value
alone suggests. Round one's method table states CSCV's sample requirement as
`T >= 500` outright, and repeats it: "For M = 1,054 strategies across
T ~ 10^2-10^3 windows, CSCV requires T >= 500". Our largest asset has 375
windows and most have 166. Round two does not repeat that floor — it instead
prescribes how to configure S below it — but neither document endorses PBO at
T = 166. Round one's method table gives
`S = 16` as its worked example (C(16,8) = 12,870 combinations) with no
sample-size caveat; round two rules that out for our T and prescribes
`S = 10`. We follow round two, which is both the later document and the one
written about our data.

Round two also prescribes the test we now have: "tests verify that power and
sample size functions reproduce published analytical tables (e.g. confirming
N = 54 achieves 80% power at delta = 1.0 bps for sigma_d = 2.6 bps)".

Also recovered: PBO in [0.05, 0.20] with in-sample +2.5 to +16.8 bps
collapsing to -6 to +2 bps out-of-sample (matches our published figures), and
directional alpha decaying within 50-500 ms.

### 3. Cost table — the BONK column was already recovered; the second column was not

```
Net Margin = 27.0 bps - (25.0 + 10.0 + 5.0 + 0.2) bps = -13.2 bps
```

Confirmed verbatim from the embedded image. But the table has **two**
columns, and only the first was recovered on the earlier pass:

| Parameter | CPMM long-tail (BONK) | CLMM liquid major (SOL/USDC) |
| --- | --- | --- |
| Gross clip-size spread | 20.0 - 27 bps | 0.2 - ? bps |
| Base pool swap fee | 25 - 30 bps | 1.0 - ? bps |
| Priority / Jito tip | 10 - 15 bps | 0.5 - ? bps |
| Quote-to-block drift | 50 - 400 ms (5 - 8 bps) | 50 - 400 ms (0.1 - ? bps) |
| L1 base fee | 0.2 - 0.5 bps | - |
| **Net realizable margin** | **-13.2 to -26.5 bps** | **+0.1 to +0.3 bps** |

The table images themselves truncate every range after the dash — the glyphs
stop at x=102 of 174 in the PNG, so the cell renders "20.0 -" and nothing
more. The upper bounds are **not** lost, though: the prose paragraphs above
the table state each one separately, and the arithmetic closes exactly.

```
best case:  27 - (25 + 10 + 5 + 0.2) = -13.2 bps
worst case: 27 - (30 + 15 + 8 + 0.5) = -26.5 bps
```

Both endpoints of the published net-margin range reproduce from the prose
figures, which is what confirms the reconstruction rather than merely making
it plausible. The CLMM column's upper bounds are not stated beside the table, but
round one supplies one of them in passing: "an observed 27 bps spread on
long-tail tokens like BONK versus **0.3 bps on SOL/USDC**", which closes the
gross-spread cell at 0.2-0.3 bps. The rest of that column stays unknown; its
net range survived as its own images.

What this changes: liquid CLMM majors are **not** negative the way BONK is —
but +0.1 to +0.3 bps sits an order of magnitude below what a 534-cycle
experiment can detect (MDE ~1 bps paired at sigma_d = 2.6). It does not
reopen Plan 001, whose hypothesis was CPMM depth signal; it does mean the
"execution is unprofitable" framing is specific to long-tail CPMM routes.

### 4. Pre-registration manifest gates — RECOVERED

Reference instance: `S = 10` with a **300 s embargo**, candidate space
**N = 1,054**, and gates `MDE = 0.50 bps`, `power >= 80%`, `DPM >= 0.95`,
`PBO <= 0.20`.

Our `prereg.rs` is generic over these rather than hardcoding them, so there
is nothing to reconcile. One open cross-check: `examples/diagnosis.rs` uses
`EMBARGO = 1` window of 120 ticks; whether that meets the 300 s
recommendation depends on tick rate and has not been verified.

Competitive-landscape latencies were also images (Carbium 31 ms P50, Titan
87% claimed win-rate; the rest are ranges truncated by the same source
defect). Nothing depends on them.

## Rule

When adopting an external document, check whether its numbers survived the
export **before** acting on its prose — and when recovering, sweep every
image, not just the one you came for. Round two was first recovered only as
far as the decision in hand required, which left three further gutted tables
undetected behind an extraction note that implied the job was done.

Reproduce with:

```sh
curl -sL -o doc.docx "https://docs.google.com/document/d/<ID>/export?format=docx"
unzip -o doc.docx -d doc/        # images land in doc/word/media/
# map images to position by reading a:blip r:embed against word/_rels/document.xml.rels
```

Note the images are black-on-transparent RGBA: composite onto white before
viewing, or they render as solid black rectangles.
