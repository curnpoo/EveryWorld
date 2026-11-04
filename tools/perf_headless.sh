#!/usr/bin/env bash
# Unattended perf pass: no window, no focus steal, no sound (`--headless`).
# Usage: tools/perf_headless.sh OUT_DIR [scene...]
# Resolution matches the windowed default (1600x900 @2x, render_pct 67).
# Weather is pinned (live weather changes cloud cost run to run).
# Scenes: still cruise spin forest lapaz origin (default: all)
# Each scene writes OUT_DIR/<scene>.csv + .log, then prints a summary.
# Summary stats skip the load phase (until the first second with pending == 0).
set -euo pipefail
out=${1:?usage: tools/perf_headless.sh OUT_DIR [scene...]}
shift || true
mkdir -p "$out"
bin=target/release/earthdrop
cargo build --release -q
scenes=("$@")
[ ${#scenes[@]} -eq 0 ] && scenes=(still cruise spin forest lapaz origin)
common=(--headless --fly --width 2144 --height 1206 --hour 15 --weather fair)
for s in "${scenes[@]}"; do
  case $s in
    still)  args=(--place Zermatt --alt 120 --pitch -12 --quit-after 22) ;;
    cruise) args=(--place Zermatt --alt 120 --pitch -12 --cruise 60 --quit-after 25) ;;
    spin)   args=(--place Zermatt --alt 20 --pitch -5 --spin 30 --quit-after 25) ;;
    forest) args=(--lat 48.0 --lon 8.2 --alt 3000 --pitch -25 --quit-after 22) ;;
    lapaz)  args=(--lat -16.5 --lon -68.15 --alt 300 --pitch 5 --quit-after 22) ;;
    origin) args=(--place Zermatt --alt 400 --pitch -15 --cruise 400 --quit-after 25) ;;
    *) echo "unknown scene $s"; continue ;;
  esac
  echo "== $s"
  EW_PERF=1 EW_PERF_OUT="$out/$s.csv" "$bin" "${common[@]}" "${args[@]}" >"$out/$s.log" 2>&1 || echo "  exit code $?"
  grep -E 'panicked|ERROR' "$out/$s.log" | head -5 || true
  python3 - "$out/$s.csv" <<'EOF'
import csv, sys
rows = list(csv.DictReader(open(sys.argv[1])))
if not rows: print("  no rows"); sys.exit()
load = next((i for i, r in enumerate(rows) if int(r["pending"]) == 0), len(rows))
after = rows[load + 1:] or rows[-1:]
f = lambda k: [float(r[k]) for r in after]
fps = f("fps")
print(f"  load {rows[load-1]['t_s'] if load else '0'} s | fps avg {sum(fps)/len(fps):.1f} min {min(fps):.1f}"
      f" | p99 worst {max(f('p99_ms')):.1f} ms | max frame {max(f('max_ms')):.1f} ms"
      f" | entities {int(min(f('entities')))}-{int(max(f('entities')))} | tiles {int(max(f('tiles')))}"
      f" | tris {max(f('tris_m')):.1f} M | meshes vis {int(max(f('meshes_vis')))}")
EOF
done
