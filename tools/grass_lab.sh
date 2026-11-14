#!/bin/zsh
# Grass look lab: four cameras on the synthetic rolling-hills place (`--path grasslab`).
#
#   tools/grass_lab.sh OUT_DIR
#
# Env: BIN (default target/release/ew), SUN=az,el (default 200,40), W/H (default 2560x1440),
# F (frames, default 12; the first 2 are warm-up). Any other env (EW_GROUND_COVER=0, ...)
# passes through, so an A/B is two calls with different OUT_DIRs. Prints gpu_frame mean per
# camera and, with python3 + PIL, writes OUT_DIR/sheet.png (2x2 contact sheet, labelled).
#
# Cameras (name: --ground lat,lon,heading,pitch,agl  horizontal fov):
#   eye   1.7 m, pitch -3      fov 60
#   down  2 m,   pitch -25     fov 60
#   o30   30 m,  pitch -15     fov 50
#   o150  150 m, pitch -20     fov 45
set -u
cd "$(dirname "$0")/.."
export EARTHDROP_CACHE=$PWD/.earthdrop-cache EW_OFFLINE=1
out=${1:?usage: tools/grass_lab.sh OUT_DIR}
bin=${BIN:-target/release/ew}; w=${W:-2560}; h=${H:-1440}; f=${F:-12}
mkdir -p "$out"
spec() { case $1 in
  eye)  echo "47.0500,7.6000,0,-3,1.7 60";;
  down) echo "47.0500,7.6000,0,-25,2 60";;
  o30)  echo "47.0485,7.6000,0,-15,30 50";;
  o150) echo "47.0450,7.6000,0,-20,150 45";;
  *) echo "";; esac; }
views=(eye down o30 o150)
for v in $views; do
  s=($(spec $v)); d=$out/$v; mkdir -p $d
  env EW_FOV=$s[2] timeout 400 $bin --headless --path grasslab --ground $s[1] --sun ${SUN:-200,40} \
    --width $w --height $h --frames $f --out $d ${=EXTRA:-} > $d/log.txt 2>&1 || echo "  $v exit $?"
done
python3 -I - "$out" $views <<'PYEOF'
import sys, csv, os
out, views = sys.argv[1], sys.argv[2:]
print("gpu_frame ms (mean over frames 2..)")
for v in views:
    try:
        rows = list(csv.reader(open(f"{out}/{v}/perf.csv")))[1:]
        g = [float(r[2]) for r in rows[2:] if r[2]]
        print(f"  {v:5} {sum(g)/len(g):6.2f}" if g else f"  {v:5}      -")
    except Exception:
        print(f"  {v:5}      -")
try:
    from PIL import Image, ImageDraw
    tw = 1280
    ims = []
    for v in views:
        pngs = sorted(p for p in os.listdir(f"{out}/{v}") if p.endswith(".png"))
        im = Image.open(f"{out}/{v}/{pngs[-1]}").convert("RGB")
        im = im.resize((tw, round(im.height * tw / im.width)))
        ImageDraw.Draw(im).text((12, 10), v, fill=(255, 255, 0))
        ims.append(im)
    th = ims[0].height
    sheet = Image.new("RGB", (2 * tw, 2 * th))
    for i, im in enumerate(ims):
        sheet.paste(im, ((i % 2) * tw, (i // 2) * th))
    sheet.save(f"{out}/sheet.png")
    print(f"sheet: {out}/sheet.png")
except Exception as e:
    print("no sheet:", e)
PYEOF
