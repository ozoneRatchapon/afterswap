# Pre-registrations

A hypothesis noticed in data is not a finding; it is a reason to design a test.
These manifests fix the hypothesis, the benchmark, the effect size, the power
target, the sample size and the null control **before** the confirming data
exists, and hash the whole thing. A report that cites a manifest whose
parameters have drifted will not verify.

The mechanism exists because of a specific temptation this project ran into:
watching a live measurement stream grow and waiting for something to cross a
significance threshold. That is optional stopping, and it manufactures
significance out of patience. The fix is to freeze what was seen as
*exploratory*, state the test, and then collect fresh data.

| id | hypothesis | status |
|---|---|---|
| [001](001_ladder_underperformance.json) | The engine underperforms a TP ladder by ≥ 0.8 bps per cycle on BONK | **OPEN** — 539 exploratory cycles archived (`data/incoming/exploratory_539.jsonl`), 722 fresh cycles required, none of them counted yet |

Resolution rules, fixed in advance:
- The result is read **once**, when the pre-registered sample size is reached.
- If the four control floors move together, the effect is a measurement
  artefact and the hypothesis is rejected regardless of the ladder number.
- The outcome is recorded whichever way it falls, as CONFIRMED or REFUTED.
