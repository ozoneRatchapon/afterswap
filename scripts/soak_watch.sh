#!/usr/bin/env bash
# Block until a running paired soak exits, then run the pre-registered report.
#
# The stopping rule in .plans/001_execution_edge.md forbids looking at the
# paired file before the run completes — so the report must not be a thing a
# human remembers to type at the right moment. This waits on the process and
# runs it exactly once, at the only time it is allowed to run.
#
#   scripts/soak_watch.sh <pid> [paired.jsonl] [outfile]
#
# Writes the report to stdout and to <outfile> (default reports/bonk_soak.txt).
set -euo pipefail

PID="${1:?usage: soak_watch.sh <pid> [paired.jsonl] [outfile]}"
FILE="${2:-data/incoming/bonk_soak_paired.jsonl}"
OUT="${3:-reports/bonk_soak.txt}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# `kill -0` is the portable liveness probe: signal 0 performs the permission
# and existence check without delivering anything. `ps`/`procs` name-matching
# is not used here — a stale alias once made a live process look dead.
while kill -0 "$PID" 2>/dev/null; do
    sleep 20
done

echo "soak PID $PID has exited; reporting $FILE" >&2
if [ ! -s "$FILE" ]; then
    echo "ERROR: $FILE is missing or empty — the run produced no cycles." >&2
    exit 1
fi

mkdir -p "$(dirname "$OUT")"
{
    echo "# BONK/USDC paired soak — pre-registered report"
    echo "# generated $(date -u '+%Y-%m-%dT%H:%M:%SZ') from $FILE (soak PID $PID)"
    echo "# pre-registration: .plans/001_execution_edge.md"
    echo
    bash "$HERE/soak_report.sh" "$FILE"
} | tee "$OUT"

echo "report written to $OUT" >&2
