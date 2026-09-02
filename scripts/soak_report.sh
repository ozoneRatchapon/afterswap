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
# ---------------------------------------------------------------------------
# AMENDMENT 2026-08-28 10:28 UTC — made while STILL BLIND to the data.
#
# Disclosed in full because amending a pre-registered script is exactly the
# move that pre-registration exists to constrain. At the time of this edit the
# soak was mid-run (tick ~2823/4500) and NO ONE HAD READ THE PAIRED FILE — not
# a row, not a summary. The two changes below are arithmetic corrections that
# could not have been steered by the result, and neither touches the endpoint,
# the field order, or the stopping rule.
#
#   1. MDE was `1.96 * se * 2` (= 3.92·SE). That is not the minimum detectable
#      effect at 80% power; it overstated it by 40%. The project's own audited
#      definition is crates/afterswap-engine/src/power.rs::mde_from_se —
#      `(Z_ALPHA + z_power) * se` = (1.96 + 0.8416)·SE = 2.8016·SE. The script
#      now matches that, so the shell report and the Rust power gate cannot
#      disagree. (power.rs exists because this project already shipped two
#      ~9%-power experiments; a wrong MDE here is the same failure again.)
#
#   2. Significance used a hardcoded normal cutoff of 1.96 with a comment
#      asserting "n here is large enough" — an assumption about n made before
#      n was known, and wrong if the tick cap binds before 300 cycles. It now
#      computes the exact two-sided Student-t p-value on n-1 df via the
#      regularized incomplete beta. Verified against published critical values
#      at df = 10, 20, 49, 86, 1000 and the normal limit (all p = 0.05000).
#
#   3. (10:34 UTC, still blind) The schema-drift error named a struct
#      `PairedCycle` that has never existed; the emitted type is
#      shadow::PairedResult. Message text only — no arithmetic, no
#      endpoint, no field order changed. Pinned by
#      afterswap-engine/tests/power.rs::soak_report_script_matches_paired_result_schema.
# ---------------------------------------------------------------------------
#
# Usage: scripts/soak_report.sh data/incoming/bonk_soak_paired.jsonl
set -euo pipefail
FILE="${1:?usage: soak_report.sh <paired.jsonl>}"
python3 - "$FILE" <<'PY'
import json, math, sys

# --- Student-t two-sided tail, no scipy on this machine -------------------
# Regularized incomplete beta by continued fraction (Lentz). P(|T|>t) with v
# df is I_x(v/2, 1/2) at x = v/(v+t^2).
def _betacf(a, b, x):
    MAXIT, EPS, FPMIN = 200, 3e-16, 1e-300
    qab, qap, qam = a + b, a + 1.0, a - 1.0
    c, d = 1.0, 1.0 - qab * x / qap
    if abs(d) < FPMIN: d = FPMIN
    d = 1.0 / d
    h = d
    for m in range(1, MAXIT + 1):
        m2 = 2 * m
        aa = m * (b - m) * x / ((qam + m2) * (a + m2))
        d = 1.0 + aa * d
        if abs(d) < FPMIN: d = FPMIN
        c = 1.0 + aa / c
        if abs(c) < FPMIN: c = FPMIN
        d = 1.0 / d
        h *= d * c
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2))
        d = 1.0 + aa * d
        if abs(d) < FPMIN: d = FPMIN
        c = 1.0 + aa / c
        if abs(c) < FPMIN: c = FPMIN
        d = 1.0 / d
        de = d * c
        h *= de
        if abs(de - 1.0) < EPS: break
    return h

def _betai(a, b, x):
    if x <= 0.0: return 0.0
    if x >= 1.0: return 1.0
    bt = math.exp(math.lgamma(a + b) - math.lgamma(a) - math.lgamma(b)
                  + a * math.log(x) + b * math.log1p(-x))
    if x < (a + 1.0) / (a + b + 2.0):
        return bt * _betacf(a, b, x) / a
    return 1.0 - bt * _betacf(b, a, 1.0 - x) / b

def t_p_two_sided(t, df):
    if not math.isfinite(t) or df <= 0: return float("nan")
    return _betai(df / 2.0, 0.5, df / (df + t * t))

Z_ALPHA, Z_POWER80 = 1.959963985, 0.841621234   # match power.rs exactly

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

missing = sorted({k for k, _ in FIELDS if any(k not in r for r in rows)}
                 | ({"ticks"} if any("ticks" not in r for r in rows) else set()))
if missing:
    sys.exit(f"ERROR: paired file is missing required field(s): {', '.join(missing)}. "
             f"Expected the schema written by afterswap-server::shadow::PairedResult.")

def stats(xs):
    m = sum(xs) / len(xs)
    var = sum((x - m) ** 2 for x in xs) / (len(xs) - 1)
    se = math.sqrt(var / len(xs))
    t = m / se if se > 0 else float("nan")
    return m, se, t

ticks = [r["ticks"] for r in rows]
print(f"cycles {n} · ticks {sum(ticks)} · median cycle {sorted(ticks)[n // 2]} ticks\n")
print(f"{'endpoint':24} {'mean':>9} {'SE':>8} {'t':>7} {'p':>9} {'win rate':>10}")
for key, label in FIELDS:
    xs = [r[key] for r in rows]
    m, se, t = stats(xs)
    p = t_p_two_sided(t, n - 1)
    wins = sum(1 for x in xs if x > 0)
    print(f"{label:24} {m:>+9.2f} {se:>8.2f} {t:>+7.2f} {p:>9.4f} {wins:>4}/{n:<5}")

mean, se, t = stats([r["vs_trailing_bps"] for r in rows])
p = t_p_two_sided(t, n - 1)
verdict = "SIGNIFICANT at 5%" if p < 0.05 else "NOT significant at 5%"
mde = (Z_ALPHA + Z_POWER80) * se     # == power.rs::mde_from_se(se, Z_POWER_80)
print(f"\nPrimary endpoint: {mean:+.2f} bps (SE {se:.2f}, t {t:+.2f}, "
      f"p {p:.4f} on {n - 1} df) — {verdict}.")
print(f"MDE at 80% power: {mde:.1f} bps — an effect smaller than this could not "
      f"have been seen at n = {n}.")
print("\nSecondary rows carry no claim: they are reported for completeness and "
      "are not corrected for multiplicity.")
PY
