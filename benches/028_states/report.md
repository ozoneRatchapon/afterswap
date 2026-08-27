# Ruliology frontier — does a bigger complete space help?

Objective: mean(edge vs TWAP, edge vs trailing) on real corpora and synthetic regimes, window 12, 10% tranches. Enumeration is exhaustive at each state count (blake3 behavioral dedup).

| n_states | machines | tournament setup | mean objective | per-corpus |
|---|---|---|---|---|
| 2 | 26 | 487.167µs | **+40.4 bps** | +188, +38, -9, +54, -41, +11 |
| 3 | 1054 | 201.823417ms | **+43.3 bps** | +188, +38, +8, +54, -40, +11 |
| 4 | 57068 | 8253.361929958s | **+41.0 bps** | +188, +37, -3, +54, -42, +11 |

## Verdict: three states is the frontier, and it is not close

| step | machines | setup cost | objective |
|---|---|---|---|
| 2 → 3 states | 26 → 1,054 | 487 µs → 202 ms (×414) | **+40.4 → +43.3** |
| 3 → 4 states | 1,054 → 57,068 | 202 ms → **8,253 s** (×40,800) | **+43.3 → +41.0** |

Completing the 4-state space costs **2.3 hours per tournament** against 202 ms,
and the extra 56,014 machines make the result *worse*. The per-corpus columns
barely move at all (+188, +38, +54, +11 recur at every state count), so the
additional capacity is not buying different behaviour — it is buying more ways
to pick a winner that does not generalise, which is exactly what the CSCV
result predicts.

This settles an architectural question that had been taken on faith: enumerate
exhaustively **up to three states**, and reach anything beyond that by
evolution, which samples the larger space at a cost that stays constant. That
is what the engine already does — now for a measured reason rather than an
intuition.

Practical note: this bench takes hours to reproduce because of the 4-state row.
The 2- and 3-state rows re-run in under a second.
