#!/usr/bin/env bash
# Render real-world scenes for review. Usage: tools/shoot_places.sh OUT_DIR [binary]
set -u
OUT=${1:?out dir}; BIN=${2:-target/debug/earthdrop}
mkdir -p "$OUT"
shoot() { # name lat lon alt heading pitch hour
  echo "== $1"
  timeout 900 "$BIN" --fly --lat "$2" --lon "$3" --alt "$4" --heading "$5" --pitch "$6" --hour "$7" \
    --width 1280 --height 720 --frames 60 --screenshot "$OUT/$1.png" 2>&1 | grep -E "capturing|panic" || true
}
shoot central_park      40.7700 -73.9740  120   20  -14 10.5
shoot zermatt           46.0150   7.7400  300  230  -6  11.0
shoot aix_en_provence   43.5270   5.4450   90  340  -18 16.5
shoot marrakech         31.6260  -7.9890   80   40  -20 9.5
shoot shibuya           35.6590 139.7000  110  300  -16 15.0
shoot manhattan_aerial  40.7000 -74.0300 1500   30  -22 17.0
