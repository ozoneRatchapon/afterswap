#!/usr/bin/env bash
# Measure the hosted POST /decide success rate over N consecutive calls.
#
# This is the exact procedure behind the "20 ok, 20 failed" figure quoted in
# README.md, docs/API.md and docs/ROADMAP.md. It exists so the post-deploy
# re-measurement is the same measurement, not a new one with the same name.
#
#   scripts/decide_measure.sh [n] [url]
#
# Prints a per-call log to stderr and a one-line summary to stdout:
#   n=40 ok=40 fail=0 rate=100.0% p50=8ms p95=41ms max=63ms codes=200:40
set -euo pipefail

N="${1:-40}"
URL="${2:-https://afterswap.solana-thailand.workers.dev/decide}"

# 30 ticks is the documented minimum. A fixed synthetic series keeps the
# request identical across runs, so a rate change is the server changing and
# not the input.
BODY=$(python3 -c '
import json
print(json.dumps({"prices": [100.0 + (i % 7) * 0.5 for i in range(40)], "open_at": 30}))')

ok=0; fail=0; times=(); codes=()
for i in $(seq 1 "$N"); do
    # %{http_code} and %{time_total} in one shot; body discarded except for
    # the mode field, which is what distinguishes a real roster from a 503.
    resp=$(curl -sS -o /tmp/decide_body.$$ -w '%{http_code} %{time_total}' \
                -X POST "$URL" -H 'content-type: application/json' \
                --max-time 30 -d "$BODY" 2>/dev/null || echo "000 0")
    code="${resp%% *}"; secs="${resp##* }"
    ms=$(python3 -c "print(round(float('$secs') * 1000))")
    mode=$(python3 -c "
import json,sys
try: print(json.load(open('/tmp/decide_body.$$')).get('mode','-'))
except Exception: print('-')" 2>/dev/null || echo '-')
    codes+=("$code")
    case "$code" in
        200) ok=$((ok + 1)); times+=("$ms")
             printf 'call %2d  %s  %5sms  mode=%s\n' "$i" "$code" "$ms" "$mode" >&2 ;;
        *)   fail=$((fail + 1))
             printf 'call %2d  %s  %5sms  FAIL\n' "$i" "$code" "$ms" >&2 ;;
    esac
done
rm -f /tmp/decide_body.$$

TIMES="${times[*]:-}" CODES="${codes[*]}" N="$N" OK="$ok" FAIL="$fail" python3 -c '
import os
ts = sorted(int(x) for x in os.environ["TIMES"].split()) or [0]
def pct(p):
    return ts[min(len(ts) - 1, int(round((p / 100) * (len(ts) - 1))))]
tally = {}
for c in os.environ["CODES"].split():
    tally[c] = tally.get(c, 0) + 1
n, ok, fail = int(os.environ["N"]), int(os.environ["OK"]), os.environ["FAIL"]
codes = ",".join("{}:{}".format(k, v) for k, v in sorted(tally.items()))
rate = 100.0 * ok / n
print("n={} ok={} fail={} rate={:.1f}% p50={}ms p95={}ms max={}ms codes={}".format(
    n, ok, fail, rate, pct(50), pct(95), ts[-1], codes))
print("(latency percentiles cover successful calls only)")'
