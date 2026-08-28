#!/usr/bin/env bash
# R2 falsifier — ingest → public-read latency, chain enforcement, segment
# closing + proofs, against a LOCAL wrangler dev (miniflare). Local numbers
# verify the pipeline, not production latency: the production ≤30 s check
# needs a real deploy and an external observer, which is a deliberate,
# user-authorised action — not this script's.
set -euo pipefail
cd "$(dirname "$0")/.."
PORT="${PORT:-8791}"
LOG=/tmp/rail_falsifier.jsonl
# The dev seed's pubkey (seed [0xA5; 32], hardcoded in gen_chain). Local only.
DEV_PUBKEY=29e5833a915a6429a4e3a7948475c338ef436eb82be89c92f059704403db9d55

cp data/rail/sol_usdc.jsonl "$LOG"
cargo run -p afterswap-rail --example gen_chain --release --quiet -- "$LOG" 60

# The rail lives in its own pure-Rust Worker; the repo-root config is the
# dashboard and has no RAIL binding. Must `cd` rather than pass `--config`:
# wrangler runs the `worker-build` step in the *invoking* cwd, so from the
# repo root it parses the workspace Cargo.toml and dies on `missing field
# `package``. `exec` makes this subshell become wrangler, so the trap below
# kills the real process and not just a shell wrapping it.
# `--var` pins the local instance to the dev seed's pubkey, keeping this
# hermetic: the deployed config carries the production key, whose seed is
# not in the repo and must not be needed to run the falsifier.
# `--persist-to` a throwaway dir: the rail is append-only, so a DO carried
# over from a previous run rejects the whole replayed log as `seq not
# monotonic`. A fresh chain per run is what makes this repeatable.
STATE_DIR=$(mktemp -d)
( cd crates/afterswap-worker && exec npx wrangler dev \
    --var "RAIL_PUBKEY:$DEV_PUBKEY" --port "$PORT" --local \
    --persist-to "$STATE_DIR" \
) >/tmp/wrangler_dev.log 2>&1 & WPID=$!
trap 'kill $WPID 2>/dev/null || true; rm -rf "$STATE_DIR"' EXIT
until curl -sf "http://localhost:$PORT/rail/stats" >/dev/null 2>&1; do sleep 1; done

python3 - "$PORT" "$LOG" <<'PY'
import json, sys, time, urllib.request

port, log = sys.argv[1], sys.argv[2]
base = f"http://localhost:{port}"

def get(path):
    with urllib.request.urlopen(base + path) as r:
        return json.load(r)

def post(path, body):
    req = urllib.request.Request(base + path, data=body.encode(), method="POST",
                                 headers={"content-type": "application/json"})
    try:
        with urllib.request.urlopen(req) as r:
            return r.status, json.load(r)
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")

records = [l for l in open(log).read().splitlines() if l.strip()]
lat = []
rejected = 0
for line in records:
    seq = json.loads(line)["seq"]
    t0 = time.monotonic()
    status, resp = post("/rail/ingest", line)
    if status != 200:
        rejected += 1
        print(f"  seq {seq}: REJECTED {status} {resp.get('error','')[:80]}")
        continue
    # visible on the public read?
    while True:
        vis = get(f"/rail/records?since={seq-1}&limit=1")
        if vis and vis[0]["seq"] == seq:
            break
        time.sleep(0.005)
    lat.append((time.monotonic() - t0) * 1000)

lat.sort()
p = lambda q: lat[min(int(q * (len(lat) - 1)), len(lat) - 1)]
print(f"\ningest -> publicly readable (local): n={len(lat)}, rejected={rejected}")
print(f"  median {p(0.5):.1f} ms   p90 {p(0.9):.1f} ms   max {p(1.0):.1f} ms")

# Chain enforcement: a fork must be refused with the expected tip.
fork = json.loads(records[5]); fork["seq"] = 9_999
status, resp = post("/rail/ingest", json.dumps(fork))
print(f"\nfork attempt   : {status} ({resp.get('error','')[:60]}) — expected 409/400")

# Replay of an already-accepted record must be refused.
status, resp = post("/rail/ingest", records[-1])
print(f"replay attempt : {status} ({resp.get('error','')[:60]}) — expected 409")

stats = get("/rail/stats")
print(f"\nstats: tip_seq={stats['tip_seq']} total={stats['total_accepted']} "
      f"gaps={stats['seq_gaps']} segments={stats['segments_closed']} root={str(stats['latest_root'])[:16]}…")

if stats["segments_closed"] < 1:
    print("FAIL: no segment closed"); sys.exit(1)
proof = get("/rail/proof/10")
print(f"proof(seq=10): root={proof['segment_root'][:16]}… steps={len(proof['proof'])}")
open("/tmp/rail_proof_10.json", "w").write(json.dumps(proof))
PY

# Independent check: the proof the Worker served must verify under the
# native rail crate against the same record in the log.
python3 - <<'PY'
import json
proof = json.load(open("/tmp/rail_proof_10.json"))
rec = next(l for l in open("/tmp/rail_falsifier.jsonl") if json.loads(l)["seq"] == 10)
open("/tmp/rail_proof_check.json", "w").write(json.dumps(
    {"record": json.loads(rec), "proof": proof}))
PY
cargo run -p afterswap-rail --example check_proof --release --quiet -- /tmp/rail_proof_check.json
echo "FALSIFIER DONE"
