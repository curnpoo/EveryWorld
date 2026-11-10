"""W4d: estimate the globe pack size (R3/R4 layers for every L0-L6 node).

Samples points uniformly on the sphere, fetches one 256x256 tile at each (about 600 m per
sample, the L6 node resolution) for height, colour and night lights, and measures compressed
size per layer. Development-only sources are used for measurement; nothing is kept.
Run: python3 measure.py [samples]
"""
import io, math, random, subprocess, sys, zlib, lzma
import numpy as np
from PIL import Image

N = int(sys.argv[1]) if len(sys.argv) > 1 else 80
Z = 8
random.seed(20261110)
UA = {"User-Agent": "globe-pack-size-spike/0.1"}

def get(url):
    # curl, because this Python has no certificate store configured.
    r = subprocess.run(["curl", "-sf", "--max-time", "30", "-A", UA["User-Agent"], url], capture_output=True)
    return r.stdout if r.returncode == 0 and r.stdout else None

def tile(lat, lon):
    n = 2 ** Z
    x = int((lon + 180) / 360 * n) % n
    y = int((1 - math.asinh(math.tan(math.radians(lat))) / math.pi) / 2 * n)
    return x, min(max(y, 0), n - 1)

rows = []
for i in range(N):
    zc = random.uniform(-1, 1)
    lat = max(-84.0, min(84.0, math.degrees(math.asin(zc))))
    lon = random.uniform(-180, 180)
    x, y = tile(lat, lon)
    h = get(f"https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{Z}/{x}/{y}.png")
    c = get(f"https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/BlueMarble_ShadedRelief_Bathymetry/default/2004-08-01/GoogleMapsCompatible_Level8/{Z}/{y}/{x}.jpeg")
    nl = get(f"https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/VIIRS_Black_Marble/default/2016-01-01/GoogleMapsCompatible_Level8/{Z}/{y}/{x}.png")
    if not h or not c:
        continue
    a = np.asarray(Image.open(io.BytesIO(h)).convert("RGB")).astype(np.int32)
    metres = a[..., 0] * 256 + a[..., 1] + a[..., 2] // 128 - 32768          # 1 m steps
    q = np.clip(metres + 12000, 0, 65535).astype(np.uint16)
    # Predict each sample from its left neighbour, then each row from the row above.
    d = q.astype(np.int32); d[:, 1:] -= q[:, :-1].astype(np.int32); d[1:, 0] -= q[:-1, 0].astype(np.int32)
    zz = ((d << 1) ^ (d >> 31)).astype(np.uint16).tobytes()
    height = min(len(zlib.compress(zz, 9)), len(lzma.compress(zz)))
    img = Image.open(io.BytesIO(c)).convert("RGB")
    buf = io.BytesIO(); img.save(buf, "JPEG", quality=90); jpg = buf.tell()
    flat = np.asarray(img).std() < 2.0
    night = 0
    if nl:
        g = np.asarray(Image.open(io.BytesIO(nl)).convert("L").resize((128, 128), Image.BILINEAR))
        night = len(zlib.compress(g.tobytes(), 9))
    ocean = bool((metres <= 0).mean() > 0.98)
    rows.append((lat, lon, ocean, height, jpg, night, flat))
    print(f"{i:3d} lat {lat:7.2f} lon {lon:8.2f} {'ocean' if ocean else 'land '} height {height:6d} B  colour(jpeg90) {jpg:6d} B  night {night:5d} B", flush=True)

n = len(rows)
mean = lambda k: sum(r[k] for r in rows) / n
nodes = sum(6 * 4 ** l for l in range(7))
print(f"\nsamples {n}; ocean share {sum(r[2] for r in rows)/n:.2f}")
print(f"mean per node: height {mean(3)/1024:.1f} KiB, colour jpeg90 {mean(4)/1024:.1f} KiB, night {mean(5)/1024:.1f} KiB")
bc7 = 256 * 256                       # 8 bits per pixel, fixed rate, top mip only
for name, colour in [("colour as JPEG-class (transcoded at load)", mean(4)), ("colour as BC7/ASTC 4x4 fixed rate, uncompressed", bc7), ("colour as BC7 + general compression (assume 55% of fixed rate on land, 5% on flat nodes)", sum((0.05 if r[6] else 0.55) * bc7 for r in rows) / n)]:
    total = nodes * (mean(3) + colour + mean(5))
    print(f"{nodes} nodes, {name}: {total/2**30:.2f} GiB")
