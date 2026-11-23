#!/bin/bash
# Pictures of each screen with the demo data, for the guides: runs against `make preview` (port 1420) with headless
# Chrome at 1200 by 750 points, twice the pixels, and saves app/src/assets/guide/<screen>.jpg.
# scripts/guide-shots.sh [out-dir] [flags]   e.g. scripts/guide-shots.sh build/shots "setup" for the setup states.
set -euo pipefail
OUT=${1:-app/src/assets/guide}
FLAGS=${2:-}
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
SCREENS=${SCREENS:-"overview tasks projects github devices vpn server sites agents cloud printer"}
mkdir -p "$OUT"
curl -sf -o /dev/null http://localhost:1420/ || { echo "Start the preview first: make preview"; exit 1; }
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
for s in $SCREENS; do
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$TMP/profile" \
    --window-size=1200,750 --force-device-scale-factor=2 --timeout=12000 --disable-features=IsolateOrigins,site-per-process \
    --screenshot="$TMP/$s.png" "http://localhost:1420/#screen=$s&still${FLAGS:+&$FLAGS}" >/dev/null 2>&1
  sips -s format jpeg -s formatOptions 82 "$TMP/$s.png" --out "$OUT/$s.jpg" >/dev/null
  echo "$OUT/$s.jpg"
done
