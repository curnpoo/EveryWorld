"""Fine needle geometry for the tree part textures (trees G6).

Everything here is authored from scratch: shoots are thin tubes along smooth curves, needles
are thin ridged blades placed by phyllotaxis. The geometry is never exported; it is rendered
orthographically to small alpha cards (``build_tree_parts.py``).

Blender axes: the card camera looks down -Z, image up = +Y. A shoot grows from (0, 0) at the
bottom of the frame (where the card attaches, cell v = 0) towards +Y (the tip, v = 1).
Colours are linear RGB in a float colour attribute ``col``; ``nrm`` holds the needle normal
encoded 0..1 (the normal-map pass).
"""

import math
import random

from mathutils import Vector

UP = Vector((0.0, 0.0, 1.0))


def srgb_to_lin(c):
    return tuple(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


class Sink:
    """Collects triangles with per-vertex colour and normal (flat lists, then one mesh)."""

    def __init__(self):
        self.verts = []
        self.cols = []
        self.nrms = []
        self.tris = []

    def vert(self, p, col, n):
        self.verts.append(tuple(p))
        self.cols.append(tuple(col) + (1.0,))
        n = Vector(n)
        if n.length < 1e-9:
            n = UP.copy()
        n.normalize()
        self.nrms.append((n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0))
        return len(self.verts) - 1

    def tri(self, a, b, c):
        self.tris.append((a, b, c))


def shade(col, n, k=0.35):
    """Bakes a little relief into the albedo: light from the upper left, above."""
    light = Vector((-0.35, 0.45, 0.82)).normalized()
    s = 1.0 - k + k * max(0.0, Vector(n).normalized().dot(light)) * 1.25
    return tuple(min(1.0, x * s) for x in col)


def curve(start, direction, length, bend, rng, segs=8, wiggle=0.02):
    """A smooth shoot: `segs` steps, bending by `bend` (vector per unit length)."""
    pts = [Vector(start)]
    d = Vector(direction).normalized()
    step = length / segs
    for _ in range(segs):
        d = (d + Vector(bend) * step + Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-0.3, 0.3))) * wiggle).normalized()
        pts.append(pts[-1] + d * step)
    return pts


def frame_at(pts, t):
    f = max(0.0, min(1.0, t)) * (len(pts) - 1)
    i = min(int(f), len(pts) - 2)
    k = f - i
    p = pts[i].lerp(pts[i + 1], k)
    d = (pts[i + 1] - pts[i]).normalized()
    return p, d


def path_len(pts):
    return sum((pts[i + 1] - pts[i]).length for i in range(len(pts) - 1))


def tube(sink, pts, r0, r1, col, sides=5):
    """A shoot axis: tapered tube, flat-shaded colour (bark of the shoot)."""
    n = len(pts)
    rings = []
    for i, p in enumerate(pts):
        d = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
        side = d.cross(UP)
        if side.length < 1e-4:
            side = Vector((1, 0, 0))
        side.normalize()
        up = side.cross(d).normalized()
        r = r0 + (r1 - r0) * i / (n - 1)
        ring = []
        for j in range(sides):
            a = j / sides * math.tau
            nn = side * math.cos(a) + up * math.sin(a)
            ring.append(sink.vert(p + nn * r, shade(col, nn, 0.5), nn))
        rings.append(ring)
    for i in range(n - 1):
        for j in range(sides):
            a, b = rings[i][j], rings[i][(j + 1) % sides]
            c, d = rings[i + 1][j], rings[i + 1][(j + 1) % sides]
            sink.tri(a, b, d)
            sink.tri(a, d, c)


def needle(sink, base, direction, length, width, col, droop=0.0):
    """A ridged needle: base, two shoulders and a tip (a flat diamond with a raised midrib),
    curving by `droop` (soft larch needles bend down)."""
    d = Vector(direction).normalized()
    side = d.cross(UP)
    if side.length < 1e-4:
        side = Vector((1, 0, 0))
    side.normalize()
    nrm = side.cross(d).normalized()
    if nrm.z < 0:
        nrm = -nrm
    mid = base + d * length * 0.45 - UP * droop * length * 0.15
    tip = base + d * length - UP * droop * length * 0.5
    ridge = nrm * width * 0.35
    tip_col = tuple(min(1.0, x * 1.12) for x in col)
    b = sink.vert(base, shade(col, nrm), nrm)
    l = sink.vert(mid + side * width * 0.5, shade(col, nrm + side * 0.8), nrm + side * 0.8)
    r = sink.vert(mid - side * width * 0.5, shade(col, nrm - side * 0.8), nrm - side * 0.8)
    m = sink.vert(mid + ridge, shade(col, nrm), nrm)
    t = sink.vert(tip, shade(tip_col, nrm), nrm)
    sink.tri(b, l, m)
    sink.tri(b, m, r)
    sink.tri(m, l, t)
    sink.tri(m, t, r)


def vary(col, rng, k):
    return tuple(max(0.0, x * (1.0 + rng.uniform(-k, k))) for x in col)


def brush_shoot(sink, pts, rng, needle_col, twig_col, needle_len, per_cm, spread_deg, plane_bias=0.6,
                r=0.0012, width=0.0011, droop=0.0, young_tip=0.0):
    """A spruce shoot: tube plus needles all around it on a spiral (137.5 deg), pointing
    forward by `spread_deg`, combed towards the shoot's plane by `plane_bias` (shade shoots
    flatten), shorter at both ends. `young_tip` lightens the outer needles (this year's)."""
    tube(sink, pts, r, r * 0.4, twig_col, 4)
    total = path_len(pts)
    count = max(4, int(total * 100.0 * per_cm))
    golden = math.radians(137.508)
    for i in range(count):
        t = (i + rng.random()) / count
        p, d = frame_at(pts, t)
        side = d.cross(UP)
        if side.length < 1e-4:
            side = Vector((1, 0, 0))
        side.normalize()
        up = side.cross(d).normalized()
        th = i * golden + rng.uniform(-0.3, 0.3)
        # Comb towards the shoot plane (sides) and up: needles underneath are pushed sideways.
        s, c = math.sin(th), math.cos(th)
        c2 = c * (1.0 + plane_bias)
        radial = (side * c2 + up * (s if s > 0 else s * (1.0 - plane_bias))).normalized()
        a = math.radians(spread_deg + rng.uniform(-12, 12))
        nd = (d * math.cos(a) + radial * math.sin(a)).normalized()
        taper = 0.55 + 0.45 * math.sin(math.pi * min(1.0, 0.15 + 0.85 * t))
        L = needle_len * taper * rng.uniform(0.85, 1.15)
        col = vary(needle_col, rng, 0.18)
        if young_tip > 0.0 and t > 0.8:
            k = (t - 0.8) / 0.2 * young_tip
            col = tuple(x * (1.0 + 0.5 * k) + 0.02 * k for x in col)
        # Inner needles are older and darker.
        col = tuple(x * (0.75 + 0.3 * t) for x in col)
        needle(sink, p + radial * r, nd, L, width, col, droop)


def rosette(sink, centre, axis, rng, needle_col, count, length, spur_col):
    """A larch short shoot: a little brown spur with a tuft of soft needles fanning out."""
    axis = Vector(axis).normalized()
    side = axis.cross(UP)
    if side.length < 1e-4:
        side = Vector((1, 0, 0))
    side.normalize()
    up = side.cross(axis).normalized()
    tube(sink, [centre - axis * 0.002, centre + axis * 0.004], 0.0018, 0.0014, spur_col, 4)
    tip = centre + axis * 0.004
    golden = math.radians(137.508)
    for i in range(count):
        th = i * golden
        a = math.radians(rng.uniform(18, 62))
        radial = side * math.cos(th) + up * math.sin(th)
        nd = (axis * math.cos(a) + radial * math.sin(a)).normalized()
        col = vary(needle_col, rng, 0.15)
        needle(sink, tip, nd, length * rng.uniform(0.7, 1.15), 0.0012, col, droop=rng.uniform(0.1, 0.4))
