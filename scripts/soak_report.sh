#!/usr/bin/env bash
# Pre-registered analysis for a paired live soak (`--paired <file>`).
#
# Written 2026-08-28 BEFORE the BONK soak collected its first cycle, so the
# endpoint and the arithmetic are fixed independently of what the data says.
# See .plans/001_execution_edge.md for the registration.
#
#   PRIMARY   : mean vs_trailing_bps per completed cycle, with SE and t.
#   SECONDARY : vs hold / TWAP / ladder / bracket — reported, not interpreted.
#
# Usage: scripts/soak_report.sh data/incoming/bonk_soak_paired.jsonl
set -euo pipefail
FILE="${1:?usage: soak_report.sh <paired.jsonl>}"
python3 - "$FILE" <<'PY'
import json, math, sys

rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
n = len(rows)
if n < 2:
    print(f"{n} cycle(s) — too few to report a t. Nothing to say yet.")
    sys.exit(0)

# The primary endpoint is listed first and labelled; the rest are secondary and
# carry no claim. Order matters here: it is the registered order.
FIELDS = [("vs_trailing_bps", "PRIMARY  vs trailing"),
          ("vs_hold_bps",     "         vs hold"),
          ("vs_twap_bps",     "         vs TWAP"),
          ("vs_ladder_bps",   "         vs ladder"),
          ("vs_bracket_bps",  "         vs bracket")]

ticks = [r["ticks"] for r in rows]
print(f"cycles {n} · ticks {sum(ticks)} · median cycle {sorted(ticks)[n//2]} ticks\n")
print(f"{'endpoint':24} {'mean':>9} {'SE':>8} {'t':>7} {'win rate':>10}")
for key, label in FIELDS:
    xs = [r[key] for r in rows]
    mean = sum(xs) / n
    var = sum((x - mean) ** 2 for x in xs) / (n - 1)
    se = math.sqrt(var / n)
    t = mean / se if se > 0 else float("nan")
    wins = sum(1 for x in xs if x > 0)
    print(f"{label:24} {mean:>+9.2f} {se:>8.2f} {t:>+7.2f} {wins:>4}/{n:<5}")

primary = [r["vs_trailing_bps"] for r in rows]
mean = sum(primary) / n
se = math.sqrt(sum((x - mean) ** 2 for x in primary) / (n - 1) / n)
t = mean / se if se > 0 else 0.0
# 1.96 is the two-sided 5% normal cutoff; n here is large enough that the
# t-distribution correction is smaller than the rounding already applied.
verdict = "SIGNIFICANT at 5%" if abs(t) >= 1.96 else "NOT significant at 5%"
mde = 1.96 * se * 2  # smallest effect this n could detect at ~80% power
print(f"\nPrimary endpoint: {mean:+.2f} bps (SE {se:.2f}, t {t:+.2f}) — {verdict}.")
print(f"MDE at 80% power: {mde:.1f} bps — an effect smaller than this could not "
      f"have been seen at n = {n}.")
PY
