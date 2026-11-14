#!/bin/zsh
# Tree-system bench: whole-frame GPU time per view, trees on / off, repeated.
#
#   tools/trees_bench.sh OUT_DIR [view...]
#
# Env: BIN (default target/release/ew), N (runs per cell, default 3), VARIANTS (default
# "on notrees nogc"; also "noimp", "nowind", "bare"), W/H (default 2560x1440), F (frames,
# default 12; the first 2 are warm-up), PLAY=secs (also run `--play zermatt` with
# EW_AUTOWALK=secs and print its frame and main-thread lines), EXTRA (appended args).
# Any other env (EW_IMP_T1_PX=..., ...) passes through to every run, so an A/B is two calls
# with different OUT_DIRs. Only `gpu_frame` is trusted: per-pass timers misattribute on tile
# GPUs. Trees cost = on - notrees; ground cover cost = on - nogc.
#
# Views (name: place, --ground lat,lon,heading,pitch,agl, horizontal fov):
#   a      zermatt   inside a crown, near casters dominate
#   b      zermatt   meadow at eye height, forest across the valley
#   c1     blueridge 300 m above closed broadleaf canopy
#   n70    zermatt   70 m above a stand
#   n2     zermatt   2 m, forest floor under a closed stand (near casters, ground cover)
#   g2     zermatt   2 m above the meadow, looking down the grass (ground-cover resolve)
#   f1k    zermatt   1 km AGL (impostor band)
#   f5k    blueridge 5 km AGL (far rung only)
#   f15k   blueridge 15 km AGL (far rung only)
set -u
cd "$(dirname "$0")/.."
export EARTHDROP_CACHE=$PWD/.earthdrop-cache EW_OFFLINE=1
out=${1:?usage: tools/trees_bench.sh OUT_DIR [view...]}; shift
bin=${BIN:-target/release/ew}; n=${N:-3}; w=${W:-2560}; h=${H:-1440}; f=${F:-12}
variants=(${=VARIANTS:-on notrees nogc})
views=("$@"); [ ${#views[@]} -eq 0 ] && views=(a b c1 n70 n2 g2 f1k f5k f15k)
mkdir -p "$out"
spec() { case $1 in
  a)    echo "zermatt 46.0200,7.7560,236.2,-1.5,20 45";;
  b)    echo "zermatt 46.0150,7.7650,200,-3,1.7 60";;
  c1)   echo "blueridge 37.95,-78.95,120,-15,300 60";;
  n70)  echo "zermatt 46.0207,7.7691,60,-14,70 45";;
  n2)   echo "zermatt 46.0207,7.7691,60,-2,2 45";;
  f1k)  echo "zermatt 46.0200,7.7560,236.2,-10,1000 45";;
  g2)   echo "zermatt 46.0150,7.7650,200,-18,2 60";;
  f5k)  echo "blueridge 37.95,-78.95,120,-25,5000 45";;
  f15k) echo "blueridge 37.95,-78.95,120,-35,15000 45";;
  *) echo "";; esac; }
for v in $views; do
  s=($(spec $v)); [ ${#s[@]} -eq 3 ] || { echo "unknown view $v"; continue; }
  for var in $variants; do
    for i in $(seq 1 $n); do
      d=$out/$v/$var.$i; mkdir -p $d
      case $var in
        on) pre=();;
        notrees) pre=(EW_TREES_NEAR=0);;
        nogc) pre=(EW_GROUND_COVER=0);;
        noimp) pre=(EW_TREES_IMPOSTOR=0);;
        nowind) pre=(EW_WIND=0);;
        bare) pre=(EW_TREES_NEAR=0 EW_GROUND_COVER=0 EW_TREES_FAR=0);;
        *) echo "unknown variant $var"; continue;;
      esac
      env $pre EW_FOV=$s[3] timeout 400 $bin --headless --path $s[1] --ground $s[2] --sun ${SUN:-200,55} \
        --width $w --height $h --frames $f --out $d ${=EXTRA:-} > $d/log.txt 2>&1 || echo "  $v/$var.$i exit $?"
    done
  done
done
[ -n "${PLAY:-}" ] && { mkdir -p $out/play
  EW_AUTOWALK=$PLAY timeout 300 $bin --play zermatt --out $out/play > $out/play/play.log 2>&1 || echo "  play exit $?"; }
python3 -I - "$out" $views <<'EOF' | tee "$out/summary.txt"
import sys, os, glob, csv, statistics as st, re
out, views = sys.argv[1], sys.argv[2:]
def cell(v, var):
    m = []
    for d in sorted(glob.glob(f"{out}/{v}/{var}.*")):
        try:
            rows = list(csv.reader(open(d + "/perf.csv")))[1:]
            g = [float(r[2]) for r in rows[2:] if r[2]]
            if g: m.append(sum(g) / len(g))
        except Exception: pass
    return m
def fmt(m): return "    -      " if not m else f"{st.mean(m):6.2f}±{(st.stdev(m) if len(m) > 1 else 0):4.2f}"
vars_ = sorted({os.path.basename(d).split(".")[0] for d in glob.glob(f"{out}/*/*.*") if os.path.isdir(d)},
               key=lambda x: ["on", "notrees", "nogc", "noimp", "nowind", "bare"].index(x) if x in ["on", "notrees", "nogc", "noimp", "nowind", "bare"] else 9)
print("gpu_frame ms (mean ± stdev over runs; frames 2.. of each run)")
print(f"{'view':6}" + "".join(f"{x:>12}" for x in vars_) + "   trees    gc   tris(M)  inst-drawn")
for v in views:
    c = {x: cell(v, x) for x in vars_}
    on = c.get("on") or []
    t = f"{st.mean(on) - st.mean(c['notrees']):6.2f}" if on and c.get("notrees") else "     -"
    g = f"{st.mean(on) - st.mean(c['nogc']):6.2f}" if on and c.get("nogc") else "     -"
    tri = drawn = ""
    try:
        log = open(f"{out}/{v}/on.1/log.txt").read()
        tri = "%.2f" % (float(re.search(r"triangles:\s+mean (\d+)", log).group(1)) / 1e6)
        drawn = re.search(r"drawn:\s+mean (\d+)", log).group(1)
    except Exception: pass
    print(f"{v:6}" + "".join(f"{fmt(c[x]):>12}" for x in vars_) + f"  {t} {g}  {tri:>7}  {drawn:>9}")
p = f"{out}/play/play.log"
if os.path.exists(p):
    for l in open(p):
        if re.search(r"ew: play:|update ms|near trees update", l): print(l.rstrip())
EOF
