#!/usr/bin/env python3
"""Bake WorldClim 2.1 (10 arc-min) into crates/ed_data/data/climate10.bin.

WorldClim 2.1: Fick & Hijmans 2017, CC BY 4.0. Inputs (from wc2.1_10m_bio.zip and
wc2.1_10m_elev.zip) are passed as a directory: bio_1 (mean T), bio_6 (min T of the
coldest month), bio_12 (annual precip), bio_18 / bio_19 (precip of the warmest /
coldest quarter) and elev.

Output (zlib): "CLM1" u16le w, u16le h, then 5 planes of w*h u8 (row 0 = 90N, col 0 = 180W):
  0 T_sl    mean annual temperature reduced to sea level, (T+40)*3
  1 Tmin_sl coldest-month mean temperature reduced to sea level, (T+40)*3
  2 P       annual precipitation, round(sqrt(mm)*2.4)
  3 Pwarm   share of annual precip that falls in the warmest quarter, *255
  4 Pcold   share in the coldest quarter, *255
255 in plane 0 = no data (open ocean). Coast cells are filled from the nearest land.

  python3 tools/bake_climate.py <dir with the tifs> crates/ed_data/data/climate10.bin
"""
import sys, zlib, struct
import numpy as np
from PIL import Image

src, out = sys.argv[1], sys.argv[2]

def load(name):
    a = np.array(Image.open(f"{src}/{name}.tif"), dtype=np.float32)
    a[a < -1e30] = np.nan
    return a

elev = np.nan_to_num(load("wc2.1_10m_elev"), nan=0.0)
t = load("wc2.1_10m_bio_1")
tmin = load("wc2.1_10m_bio_6")
p = load("wc2.1_10m_bio_12")
pw = load("wc2.1_10m_bio_18")
pc = load("wc2.1_10m_bio_19")
land = ~np.isnan(t)
lift = 0.0065 * np.maximum(elev, 0.0)
planes = [
    np.clip((t + lift + 40) * 3, 0, 254),
    np.clip((tmin + lift + 40) * 3, 0, 254),
    np.clip(np.sqrt(np.maximum(p, 0)) * 2.4, 0, 254),
    np.clip(pw / np.maximum(p, 1) * 255, 0, 255),
    np.clip(pc / np.maximum(p, 1) * 255, 0, 255),
]
planes = [np.where(land, np.nan_to_num(x), 255).astype(np.uint8) for x in planes]

# Fill ocean cells near the coast with the nearest land value (so a coastal
# tile whose centre is over water still gets a climate).
h, w = land.shape
filled = land.copy()
for _ in range(12):
    grow = np.zeros_like(filled)
    for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        grow |= np.roll(filled, (dy, dx), (0, 1))
    new = grow & ~filled
    if not new.any():
        break
    for pl in planes:
        acc = np.zeros(pl.shape, np.float32); cnt = np.zeros(pl.shape, np.float32)
        for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            m = np.roll(filled, (dy, dx), (0, 1))
            acc += np.where(m, np.roll(pl, (dy, dx), (0, 1)), 0)
            cnt += m
        pl[new] = (acc[new] / np.maximum(cnt[new], 1)).astype(np.uint8)
    filled |= new
for pl in planes:
    pl[~filled] = 255 if pl is planes[0] else 0

blob = b"CLM1" + struct.pack("<HH", w, h) + b"".join(pl.tobytes() for pl in planes)
open(out, "wb").write(zlib.compress(blob, 9))
print(w, h, len(blob), "->", len(zlib.compress(blob, 9)))
