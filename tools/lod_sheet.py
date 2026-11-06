#!/usr/bin/env python3
"""Render the LOD levels of a dumped tile into one contact sheet.

The native renderer draws distant buildings from decimation proxies. This tool dumps a real
tile, runs the renderer's own mesh preparation over it, writes each level as an OBJ, and
renders them side by side so the LOD can be *looked at* instead of guessed at.

    cargo run --release -p ed_world --example dump_tile_lod -- 40.7690 -73.9750 /tmp/tile.bin
    cargo run --release -p ed_metal --example lod_dump -- /tmp/tile.bin /tmp/lod_out
    python3 tools/lod_sheet.py /tmp/lod_out /tmp/lod_sheet.png

Needs numpy and pillow.
"""
import os
import sys

import numpy as np
from PIL import Image, ImageDraw


def load_obj(path):
    vs, fs = [], []
    label = None
    with open(path) as f:
        for line in f:
            if line.startswith("# lod error"):
                label = line.strip()
            elif line.startswith("v "):
                _, x, y, z = line.split()
                vs.append((float(x), float(y), float(z)))
            elif line.startswith("f "):
                p = line.split()[1:]
                if len(p) == 3:
                    fs.append((int(p[0]) - 1, int(p[1]) - 1, int(p[2]) - 1))
    return np.array(vs, dtype=np.float32), np.array(fs, dtype=np.int32), label


def render(V, F, size=(520, 460), eye=None, target=None, fov=55.0):
    W, H = size
    c = V.mean(0)
    r = (V.max(0) - V.min(0)).astype(np.float32)
    if eye is None:
        eye = c + np.array([r[0] * 0.8, r[1] * 0.5, r[0] * 0.8], dtype=np.float32)
    if target is None:
        target = c
    eye = np.asarray(eye, dtype=np.float32)
    target = np.asarray(target, dtype=np.float32)
    f = target - eye
    f /= np.linalg.norm(f)
    up = np.array([0, 1, 0], dtype=np.float32)
    s = np.cross(f, up)
    s /= np.linalg.norm(s)
    u = np.cross(s, f)
    fl = 1.0 / np.tan(np.radians(fov) / 2)
    rel = V - eye
    xs, ys, zs = rel @ s, rel @ u, rel @ f
    ok = zs > 0.05
    sx = xs / zs * fl * W / 2 + W / 2
    sy = -ys / zs * fl * H / 2 + H / 2
    A, B, C = F[:, 0], F[:, 1], F[:, 2]
    keep = ok[A] & ok[B] & ok[C]
    A, B, C = A[keep], B[keep], C[keep]
    depth = np.full((H, W), np.inf, dtype=np.float32)
    col = np.zeros((H, W, 3), dtype=np.float32)
    x1, x2, x3 = sx[A], sx[B], sx[C]
    y1, y2, y3 = sy[A], sy[B], sy[C]
    z1, z2, z3 = zs[A], zs[B], zs[C]
    nz = np.cross(V[B] - V[A], V[C] - V[A])
    ln = np.linalg.norm(nz, axis=1)
    ln[ln == 0] = 1
    nz /= ln[:, None]
    light = np.array([0.4, 0.8, 0.45], dtype=np.float32)
    light /= np.linalg.norm(light)
    sh = np.clip(np.abs(nz @ light), 0, 1) * 0.75 + 0.25
    for t in range(len(A)):
        area = (x2[t] - x1[t]) * (y3[t] - y1[t]) - (y2[t] - y1[t]) * (x3[t] - x1[t])
        if abs(area) < 1e-9:
            continue
        X0 = max(0, int(np.floor(min(x1[t], x2[t], x3[t]))))
        X1 = min(W - 1, int(np.ceil(max(x1[t], x2[t], x3[t]))))
        Y0 = max(0, int(np.floor(min(y1[t], y2[t], y3[t]))))
        Y1 = min(H - 1, int(np.ceil(max(y1[t], y2[t], y3[t]))))
        if X1 < X0 or Y1 < Y0:
            continue
        gx, gy = np.meshgrid(np.arange(X0, X1 + 1, dtype=np.float32), np.arange(Y0, Y1 + 1, dtype=np.float32))
        l0 = ((x2[t] - gx) * (y3[t] - gy) - (y2[t] - gy) * (x3[t] - gx)) / area
        l1 = ((x3[t] - gx) * (y1[t] - gy) - (y1[t] - gy) * (x1[t] - gx)) / area
        l2 = 1 - l0 - l1
        inside = (l0 >= 0) & (l1 >= 0) & (l2 >= 0)
        if not inside.any():
            continue
        zz = l0 * z1[t] + l1 * z2[t] + l2 * z3[t]
        sub = depth[Y0:Y1 + 1, X0:X1 + 1]
        m = inside & (zz < sub)
        if not m.any():
            continue
        sub[m] = zz[m]
        cc = col[Y0:Y1 + 1, X0:X1 + 1]
        cc[m] = np.array([0.88, 0.84, 0.78], dtype=np.float32) * sh[t]
    return (np.clip(col, 0, 1) * 255).astype(np.uint8)


def main():
    src = sys.argv[1] if len(sys.argv) > 1 else "/tmp/lod_out"
    out = sys.argv[2] if len(sys.argv) > 2 else "/tmp/lod_sheet.png"
    names = [("full.obj", "full geometry"), ("proxy0.obj", "LOD 0 (0.5 m)"),
             ("proxy1.obj", "LOD 1 (1.5 m)"), ("proxy2.obj", "LOD 2 (4 m)")]
    tiles = []
    for fname, label in names:
        path = os.path.join(src, fname)
        if not os.path.exists(path):
            continue
        V, F, tag = load_obj(path)
        if len(F) == 0:
            continue
        img = render(V, F)
        name = label if tag is None else f"{label} ({tag.replace('# lod error ', '').replace(' m', ' m error')})"
        tiles.append((name, len(F), Image.fromarray(img)))
    if not tiles:
        print("no OBJ files in", src)
        return
    w, h = tiles[0][2].size
    sheet = Image.new("RGB", (w * len(tiles), h + 22), (18, 18, 20))
    d = ImageDraw.Draw(sheet)
    for i, (label, tris, img) in enumerate(tiles):
        sheet.paste(img, (i * w, 22))
        d.text((i * w + 6, 5), f"{label}  ({tris} tris)", fill=(235, 235, 235))
    sheet.save(out)
    print("wrote", out)


if __name__ == "__main__":
    main()
