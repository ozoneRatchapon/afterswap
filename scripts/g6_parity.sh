#!/usr/bin/env bash
# GOAT G6 — wasm parity: the browser engine must produce byte-identical
# simulate() output to the native binary on the bundled corpus.
# Needs: wasm32 target, wasm-bindgen CLI, wrangler, headless Chrome.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p afterswap-wasm --target wasm32-unknown-unknown --release --quiet
wasm-bindgen "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/afterswap_wasm.wasm" \
  --target web --out-dir web-wasm/public/pkg
cargo run -p afterswap-engine --example parity_ref --release --quiet > /tmp/g6_native.json
npx wrangler dev --port 8789 >/dev/null 2>&1 & WPID=$!
sleep 6
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless --disable-gpu \
  --dump-dom --virtual-time-budget=20000 "http://localhost:8789/parity.html" 2>/dev/null \
  | python3 -c "
import sys, re, html
m = re.search(r'<pre id=\"out\">(.*?)</pre>', sys.stdin.read(), re.S)
open('/tmp/g6_wasm.json', 'w').write(html.unescape(m.group(1)).strip() if m else 'EXTRACT_FAIL')
"
kill $WPID 2>/dev/null || true
if cmp -s /tmp/g6_native.json <(cat /tmp/g6_wasm.json; echo); then echo "G6 PASS"; else
  diff <(cat /tmp/g6_native.json) <(cat /tmp/g6_wasm.json) | head -3; echo "G6 FAIL"; exit 1; fi
