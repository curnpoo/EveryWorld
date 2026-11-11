"""Temperate-forest (BIO_FOR) building kit, first 12 parts.

One function per part; each returns a kitlib.Part. No randomness anywhere.
Coordinates are Blender Z-up, part-local, metres. Walls: x in [-1, 1],
z in [0, 3], outer face on y = 0, interior toward +Y, trim out to y = -0.05.
"""

import math

from kitlib import Part

PITCH_DEG = 35.0
SLOPE = math.tan(math.radians(PITCH_DEG))   # rise per metre of plan run
RISE = 2.0 * SLOPE                          # rise over one 2 m roof cell

WALL_ORIGIN = ("bottom centre of the outer face at storey level; x in [-1,1], z in [0,3], "
               "outer face on y=0, interior +Y (y in [0,0.2]), trim out to y=-0.05")
CELL_ORIGIN = ("midpoint of the cell's outer edge; footprint x in [-1,1], y in [0,2], "
               "interior +Y")

# shared wall detail heights
BASE_H = 0.12      # dark metal base flashing
BELT_Z = 2.75      # timber belt (rim) from here to the storey top; hides floor edges
TRIM_D = 0.04      # flashing / belt projection
BAT_D = 0.03      # batten projection
BAT_W = 0.05


def _cladding(p, opening=None, sill_drop=0.0, lintel=0.0):
    """Board-and-batten skin on the outer face: base flashing, belt, battens on
    a 0.25 m rhythm. Half battens sit on x = +-1 so neighbours join into one.
    opening = (x0, x1, z0, z1) breaks the battens and the flashing."""
    if opening and opening[2] < BASE_H:
        p.box(-1.0, opening[0], -TRIM_D, 0.0, 0.0, BASE_H, "metal_dark")
        p.box(opening[1], 1.0, -TRIM_D, 0.0, 0.0, BASE_H, "metal_dark")
    else:
        p.box(-1.0, 1.0, -TRIM_D, 0.0, 0.0, BASE_H, "metal_dark")
    p.box(-1.0, 1.0, -TRIM_D, 0.0, BELT_Z, 3.0, "timber_structure")
    for k in range(9):
        c = -1.0 + 0.25 * k
        a, b = max(c - BAT_W / 2, -1.0), min(c + BAT_W / 2, 1.0)
        if opening:
            x0, x1, z0, z1 = opening
            if abs(c - x0) < 1e-6:
                a, b = x0 - BAT_W, x0            # jamb batten, outside the opening
            elif abs(c - x1) < 1e-6:
                a, b = x1, x1 + BAT_W
            elif x0 < c < x1:
                if z0 - sill_drop > BASE_H + 0.01:
                    p.box(a, b, -BAT_D, 0.0, BASE_H, z0 - sill_drop, "timber_cladding")
                p.box(a, b, -BAT_D, 0.0, z1 + lintel, BELT_Z, "timber_cladding")
                continue
        p.box(a, b, -BAT_D, 0.0, BASE_H, BELT_Z, "timber_cladding")


# 1 ---------------------------------------------------------------------------
def wall_timber_a():
    p = Part("BIO_FOR_WALL_TIMBER_A", "wall_plain",
             ((-1.0, -0.04, 0.0), (1.0, 0.2, 3.0)), (2.0, 0.2, 3.0), WALL_ORIGIN)
    p.box(-1.0, 1.0, 0.0, 0.2, 0.0, 3.0, "timber_cladding")
    _cladding(p)
    p.socket("vine", (-0.625, -0.03, 0.0), (0.0, -1.0, 0.0))
    p.socket("vine", (0.625, -0.03, 0.0), (0.0, -1.0, 0.0))
    return p


# 2 ---------------------------------------------------------------------------
def wall_timber_window_a():
    p = Part("BIO_FOR_WALL_TIMBER_WINDOW_A", "wall_opening",
             ((-1.0, -0.05, 0.0), (1.0, 0.2, 3.0)), (2.0, 0.2, 3.0), WALL_ORIGIN,
             notes="window opening 1.5 x 1.5, sill at 0.9")
    x0, x1, z0, z1 = -0.75, 0.75, 0.9, 2.4
    p.grid_solid([-1.0, x0, x1, 1.0], [0.0, z0, z1, 3.0], 0.0, 0.2, "timber_cladding",
                 absent=[(1, 1)])
    _cladding(p, (x0, x1, z0, z1), sill_drop=0.06, lintel=0.08)
    # dark metal frame with one centre mullion
    f = 0.05
    p.grid_solid([x0, x0 + f, -0.02, 0.02, x1 - f, x1], [z0, z0 + f, z1 - f, z1],
                 -0.03, 0.09, "metal_dark", absent=[(1, 1), (3, 1)])
    p.box(x0 + f, x1 - f, 0.02, 0.04, z0 + f, z1 - f, "glass")
    # timber sill and lintel
    p.box(x0 - 0.08, x1 + 0.08, -0.05, 0.0, z0 - 0.06, z0, "timber_structure")
    p.box(x0 - 0.08, x1 + 0.08, -TRIM_D, 0.0, z1, z1 + 0.08, "timber_structure")
    p.socket("planter", (0.0, -0.05, z0 - 0.06), (0.0, -1.0, 0.0))
    p.socket("vine", (-0.875, -0.03, 0.0), (0.0, -1.0, 0.0))
    return p


# 3 ---------------------------------------------------------------------------
def wall_timber_door_a():
    p = Part("BIO_FOR_WALL_TIMBER_DOOR_A", "wall_opening",
             ((-1.0, -0.05, 0.0), (1.0, 0.2, 3.0)), (2.0, 0.2, 3.0), WALL_ORIGIN,
             notes="door opening 1.2 x 2.4, leaf recessed 0.05")
    x0, x1, z1 = -0.6, 0.6, 2.4
    p.grid_solid([-1.0, x0, x1, 1.0], [0.0, z1, 3.0], 0.0, 0.2, "timber_cladding",
                 absent=[(1, 0)])
    _cladding(p, (x0, x1, 0.0, z1), lintel=0.08)
    f = 0.06
    p.grid_solid([x0, x0 + f, x1 - f, x1], [0.0, z1 - f, z1], -0.03, 0.12, "metal_dark",
                 absent=[(1, 0)])
    # stone threshold between the jambs
    p.box(x0 + f, x1 - f, -0.05, 0.12, 0.0, 0.03, "stone")
    # timber leaf with a tall glazed lite
    p.grid_solid([x0 + f, -0.36, -0.14, x1 - f], [0.03, 1.0, 2.1, z1 - f], 0.05, 0.10,
                 "timber_cladding", absent=[(1, 1)])
    p.box(-0.36, -0.14, 0.065, 0.085, 1.0, 2.1, "glass")
    # pull handle on two standoffs
    p.box(0.36, 0.39, 0.0, 0.02, 0.92, 1.38, "metal_dark")
    p.box(0.36, 0.39, 0.02, 0.05, 0.97, 1.00, "metal_dark")
    p.box(0.36, 0.39, 0.02, 0.05, 1.30, 1.33, "metal_dark")
    p.box(x0 - 0.08, x1 + 0.08, -TRIM_D, 0.0, z1, z1 + 0.08, "timber_structure")
    p.socket("light", (0.8, -0.03, 2.1), (0.0, -1.0, 0.0))
    p.socket("vine", (-0.875, -0.03, 0.0), (0.0, -1.0, 0.0))
    return p


# 4 ---------------------------------------------------------------------------
def wall_glass_a():
    p = Part("BIO_FOR_WALL_GLASS_A", "wall_glass",
             ((-1.0, -0.04, 0.0), (1.0, 0.2, 3.0)), (2.0, 0.2, 3.0), WALL_ORIGIN,
             notes="frame is 0.12 deep, head beam is the full 0.2; edge mullions are "
                   "half width and pair up with a neighbouring glass wall")
    p.box(-1.0, 1.0, -TRIM_D, 0.2, BELT_Z, 3.0, "timber_structure")
    p.grid_solid([-1.0, -0.975, -0.02, 0.02, 0.975, 1.0],
                 [0.0, 0.08, 2.10, 2.14, BELT_Z - 0.05, BELT_Z],
                 0.0, 0.12, "metal_dark", absent=[(1, 1), (3, 1), (1, 3), (3, 3)])
    p.box(-0.975, 0.975, 0.05, 0.07, 0.08, BELT_Z - 0.05, "glass")
    return p


# 5 ---------------------------------------------------------------------------
def corner_timber_a():
    p = Part("BIO_FOR_CORNER_TIMBER_A", "other",
             ((-0.05, -0.05, 0.0), (0.225, 0.225, 3.0)), (0.275, 0.275, 3.0),
             "the footprint's outer corner point at storey level; the two outer wall "
             "planes are y=0 (running +X) and x=0 (running +Y); interior is x>0, y>0",
             notes="place at the LEFT end of a wall: position = wall origin + R*(-1,0,0), "
                   "same rotation R as that wall. Walls keep their outer faces on the "
                   "footprint edge, so two walls overlap in the 0.2 x 0.2 corner column; "
                   "the post encloses that column and ends flush against the first "
                   "batten of each wall")
    p.box(-0.05, 0.225, -0.05, 0.225, 0.0, BASE_H, "metal_dark")
    p.octa_post(-0.04, 0.225, -0.04, 0.225, BASE_H, 3.0, 0.02, "timber_structure")
    p.socket("vine", (-0.05, -0.05, 0.0), (-0.7071, -0.7071, 0.0))
    return p


# 6 ---------------------------------------------------------------------------
def foundation_stone_a():
    p = Part("BIO_FOR_FOUNDATION_STONE_A", "other",
             ((-1.0, 0.0, -1.6), (1.0, 2.0, 0.0)), (2.0, 2.0, 1.6), CELL_ORIGIN,
             notes="top at floor level z=0; grade is z=-0.6; 1.0 m below grade. "
                   "Same on all four sides, so any 90 degree rotation tiles")
    d = 0.025                      # block relief
    top, c0 = -0.08, -0.86         # coursed zone (runs 0.26 below grade for slopes)
    p.ring_stack(-1.0, 1.0, 0.0, 2.0,
                 [(-1.6, 0.0), (c0, 0.0), (c0, d), (top, d), (top, 0.0), (0.0, 0.0)], "stone")
    ch = 0.02
    courses = [[0.0, 0.5, 1.5, 2.0], [0.0, 0.25, 1.0, 1.75, 2.0], [0.0, 0.5, 1.5, 2.0]]
    h = (top - c0) / len(courses)
    # (origin, along-side dir, inward dir) for the four faces
    sides = [((-1.0, 0.0), (1, 0), (0, 1)), ((1.0, 0.0), (0, 1), (-1, 0)),
             ((1.0, 2.0), (-1, 0), (0, -1)), ((-1.0, 2.0), (0, -1), (1, 0))]
    for (ox, oy), (ux, uy), (nx, ny) in sides:
        def pt(u, dep, z):
            return (ox + ux * u + nx * dep, oy + uy * u + ny * dep, z)
        for ci, joints in enumerate(courses):
            za, zb = c0 + ci * h, c0 + (ci + 1) * h
            for a, b in zip(joints, joints[1:]):
                # end blocks run unchamfered to the module edge (they continue into the
                # neighbour, or wrap the corner) and are mitred at the back
                fa = a if a == 0.0 else a + ch
                fb = b if b == 2.0 else b - ch
                ba = a + d if a == 0.0 else a
                bb = b - d if b == 2.0 else b
                p.hexa([pt(fa, 0, za + ch), pt(fb, 0, za + ch), pt(bb, d, za), pt(ba, d, za),
                        pt(fa, 0, zb - ch), pt(fb, 0, zb - ch), pt(bb, d, zb), pt(ba, d, zb)],
                       "stone")
    return p


# 7 ---------------------------------------------------------------------------
def floor_a():
    p = Part("BIO_FOR_FLOOR_A", "other",
             ((-1.0, -0.05, -0.25), (1.0, 2.0, 0.0)), (2.0, 2.0, 0.25), CELL_ORIGIN,
             notes="walking surface at z=0, slab hangs below; the rim beam on the outer "
                   "edge projects 0.05 and covers the belt of the wall below")
    p.layer_stack(-1.0, 1.0, 0.0, 2.0, [(-0.25, -0.04, "timber_structure"),
                                        (-0.04, 0.0, "timber_cladding")])
    p.box(-1.0, 1.0, -0.05, 0.0, -0.25, 0.0, "timber_structure")
    return p


# 8 ---------------------------------------------------------------------------
def _roof_base(p, y0, t):
    """Timber wedge that closes the gable (cheeks on x = +-1) plus the deck."""
    p.prism([(0.0, 0.0), (2.0, 0.0), (2.0, RISE)], "x", -1.0, 1.0, "timber_cladding")
    p.shear_box(-1.0, 1.0, y0, 2.0, 0.0, t, SLOPE, "metal_dark")


def _eave_timber(p, over):
    for c in (-0.5, 0.5):
        p.shear_box(c - 0.04, c + 0.04, -over + 0.03, 0.0, -0.14, 0.0, SLOPE, "timber_structure")
    p.shear_box(-1.0, 1.0, -over, -over + 0.03, -0.16, 0.0, SLOPE, "timber_structure")


def _roof_pitch(name, over):
    t, seam = 0.10, 0.045
    zmin = -over * SLOPE - 0.16 if over else 0.0
    p = Part(name, "roof", ((-1.0, -over, zmin), (1.0, 2.0, RISE + t + seam)),
             (2.0, 2.0 + over, RISE), CELL_ORIGIN + "; eave line on y=0 at the wall top (z=0)",
             notes=f"35 degree pitch rising toward +Y; the next cell up-slope sits at "
                   f"y+2, z+{RISE:.4f}. Two cells back to back make the ridge. The timber "
                   f"wedge under the deck closes the gable for a one-cell-deep slope"
                   + (f"; eave overhang {over} m beyond the wall plane" if over else ""))
    _roof_base(p, -over, t)
    for c in (-0.8, -0.4, 0.0, 0.4, 0.8):          # standing seams, 0.4 m pans
        p.shear_box(c - 0.015, c + 0.015, -over, 2.0, t, t + seam, SLOPE, "metal_dark")
    if over:
        _eave_timber(p, over)
    p.socket("pv", (0.0, 1.0, SLOPE + t + seam), (0.0, -math.sin(math.radians(PITCH_DEG)),
                                                  math.cos(math.radians(PITCH_DEG))))
    return p


def roof_pitch_a():
    return _roof_pitch("BIO_FOR_ROOF_PITCH_A", 0.0)


def roof_pitch_eave_a():
    return _roof_pitch("BIO_FOR_ROOF_PITCH_EAVE_A", 0.6)


# 9 ---------------------------------------------------------------------------
def roof_solar_a():
    t, proud = 0.13, 0.012
    p = Part("BIO_FOR_ROOF_SOLAR_A", "roof_solar",
             ((-1.0, 0.0, 0.0), (1.0, 2.0, RISE + t)), (2.0, 2.0, RISE),
             CELL_ORIGIN + "; eave line on y=0 at the wall top (z=0)",
             notes="same pitch and wedge as ROOF_PITCH_A; 3 x 2 PV panels set in a dark "
                   "frame, 0.06 m joints that continue across neighbouring cells")
    _roof_base(p, 0.0, t)
    gap = 0.06
    w = (2.0 - 3 * gap) / 3
    for i in range(3):
        xa = -1.0 + gap / 2 + i * (w + gap)
        for ya, yb in ((0.06, 0.97), (1.03, 1.94)):
            p.shear_box(xa, xa + w, ya, yb, t, t + proud, SLOPE, "pv")
    return p


# 10 --------------------------------------------------------------------------
def roof_green_a():
    p = Part("BIO_FOR_ROOF_GREEN_A", "other",
             ((-1.0, -0.04, 0.0), (1.0, 2.0, 0.30)), (2.0, 2.0, 0.30),
             CELL_ORIGIN + "; underside on the wall top (z=0)",
             notes="parapet on the outer (y=0) edge only; the other three edges show the "
                   "deck / soil / sedum layers and tile with neighbours")
    p.box(-1.0, 1.0, -0.03, 0.06, 0.0, 0.27, "timber_cladding")
    p.box(-1.0, 1.0, -0.04, 0.07, 0.27, 0.30, "metal_dark")
    p.layer_stack(-1.0, 1.0, 0.06, 2.0, [(0.0, 0.08, "timber_structure"),
                                         (0.08, 0.19, "soil"), (0.19, 0.24, "sedum")])
    p.socket("planter", (0.0, 1.0, 0.24), (0.0, 0.0, 1.0))
    p.socket("vine", (0.0, -0.04, 0.27), (0.0, -1.0, 0.0))
    return p


# 11 --------------------------------------------------------------------------
def balcony_a():
    p = Part("BIO_FOR_BALCONY_A", "balcony",
             ((-1.0, -1.0, -0.15), (1.0, -0.05, 1.04)), (2.0, 1.0, 1.19),
             "bottom centre of the host wall's outer face at floor level; the balcony "
             "projects outward to y=-1; deck surface at z=0",
             notes="starts at y=-0.05: the 0.05 m strip against the wall is left to wall "
                   "trim and the floor rim beam. Railing on three sides, open to the wall")
    back, front = -0.05, -1.0
    ts, md = "timber_structure", "metal_dark"
    # frame
    p.box(-1.0, 1.0, front, front + 0.08, -0.15, -0.03, ts)
    for xa, xb in ((-1.0, -0.92), (-0.04, 0.04), (0.92, 1.0)):
        p.box(xa, xb, front + 0.08, back, -0.15, -0.03, ts)
    for xa, xb in ((-0.92, -0.04), (0.04, 0.92)):
        p.box(xa, xb, back - 0.08, back, -0.15, -0.03, ts)
    # deck boards
    n, g = 8, 0.01
    bw = ((back - front) - (n - 1) * g) / n
    for i in range(n):
        ya = front + i * (bw + g)
        p.box(-1.0, 1.0, ya, ya + bw, -0.03, 0.0, "timber_cladding")
    # posts
    ps = 0.04
    for xa in (-1.0, -ps / 2, 1.0 - ps):
        p.box(xa, xa + ps, front, front + ps, 0.0, 1.0, md)
    for xa in (-1.0, 1.0 - ps):
        p.box(xa, xa + ps, back - ps, back, 0.0, 1.0, md)
    # timber handrail
    p.box(-1.0, 1.0, front, front + 0.08, 1.0, 1.04, ts)
    p.box(-1.0, -0.92, front + 0.08, back, 1.0, 1.04, ts)
    p.box(0.92, 1.0, front + 0.08, back, 1.0, 1.04, ts)
    # bottom rails and slim bars
    yl = front + ps / 2
    spans = ((-1.0 + ps, -ps / 2), (ps / 2, 1.0 - ps))
    b = 0.007
    for xa, xb in spans:
        p.box(xa, xb, yl - 0.01, yl + 0.01, 0.08, 0.11, md)
        k = 7
        for i in range(1, k + 1):
            c = xa + (xb - xa) * i / (k + 1)
            p.box(c - b, c + b, yl - b, yl + b, 0.11, 1.0, md)
    ya, yb = front + ps, back - ps
    for xl in (-1.0 + ps / 2, 1.0 - ps / 2):
        p.box(xl - 0.01, xl + 0.01, ya, yb, 0.08, 0.11, md)
        k = 6
        for i in range(1, k + 1):
            c = ya + (yb - ya) * i / (k + 1)
            p.box(xl - b, xl + b, c - b, c + b, 0.11, 1.0, md)
    p.socket("planter", (-0.5, front + 0.04, 1.04), (0.0, 0.0, 1.0))
    p.socket("planter", (0.5, front + 0.04, 1.04), (0.0, 0.0, 1.0))
    p.socket("vine", (0.0, front, -0.15), (0.0, -1.0, 0.0))
    return p


# 12 --------------------------------------------------------------------------
def column_timber_a():
    p = Part("BIO_FOR_COLUMN_TIMBER_A", "other",
             ((-0.25, -0.25, 0.0), (0.25, 0.25, 3.0)), (0.5, 0.5, 3.0),
             "bottom centre of the column (free-standing, so centred, not face-anchored); "
             "z in [0,3]")
    h = 0.25
    p.ring_stack(-h, h, -h, h, [(0.0, 0.0), (0.32, 0.0), (0.42, 0.05)], "stone")
    p.box(-0.19, 0.19, -0.19, 0.19, 0.42, 0.50, "metal_dark")
    p.octa_post(-0.17, 0.17, -0.17, 0.17, 0.50, 2.80, 0.03, "timber_structure")
    p.box(-0.19, 0.19, -0.19, 0.19, 2.80, 2.86, "metal_dark")
    p.box(-h, h, -h, h, 2.86, 3.0, "timber_structure")
    p.socket("vine", (0.0, -0.25, 0.0), (0.0, -1.0, 0.0))
    p.socket("light", (0.0, -0.17, 2.4), (0.0, -1.0, 0.0))
    return p


# ---------------------------------------------------------------------------
# Round 2
# ---------------------------------------------------------------------------
def gable_infill_a():
    p = Part("BIO_FOR_GABLE_INFILL_A", "other",
             ((-1.0, -BAT_D, 0.0), (1.0, 0.2, RISE)), (2.0, 0.2, RISE),
             "wall convention: bottom centre of the outer face; x in [-1,1], outer face "
             "on y=0, interior +Y; z in [0, roof_rise_per_cell]",
             notes="RECTANGULAR, one rise tall. The roof wedge already supplies the "
                   "triangle of every cell, so what a deeper roof leaves open is the "
                   "rectangle under row k: stack k panels (z = wall top + 0, 1, .. k-1 "
                   "rises) under each row-k roof cell, on the gable wall line. A triangle "
                   "would be coplanar with the wedge cheek and would need a mirrored twin")
    p.box(-1.0, 1.0, 0.0, 0.2, 0.0, RISE, "timber_cladding")
    for k in range(9):
        c = -1.0 + 0.25 * k
        p.box(max(c - BAT_W / 2, -1.0), min(c + BAT_W / 2, 1.0), -BAT_D, 0.0, 0.0, RISE - 0.12,
              "timber_cladding")
    # collar band: the battens stop here, the plain wedge cheek starts above it
    p.box(-1.0, 1.0, -BAT_D, 0.0, RISE - 0.12, RISE, "timber_structure")
    return p


def entry_step_a():
    tops = [0.0, -0.15, -0.30, -0.45]
    edge = [0.0, -0.7, -1.0, -1.3, -1.6]
    slab, nose, foot = 0.05, 0.02, -0.9
    p = Part("BIO_FOR_ENTRY_STEP_A", "other",
             ((-1.0, edge[-1] - nose, foot), (1.0, 0.0, 0.0)), (2.0, 1.62, 0.6),
             "bottom centre of the wall's outer face line at floor level z=0 (same "
             "transform as the door wall above it); steps project toward -Y",
             notes="landing 0.7 deep at z=0 then three 0.3 treads, four 0.15 risers down "
                   "to grade z=-0.6; the body runs 0.3 below grade for slopes")
    prof = [(0.0, foot), (0.0, tops[0] - slab)]
    for i, t in enumerate(tops):
        prof.append((edge[i + 1], t - slab))
        if i + 1 < len(tops):
            prof.append((edge[i + 1], tops[i + 1] - slab))
    prof.append((edge[-1], foot))
    p.prism(prof, "x", -1.0, 1.0, "stone")
    for i, t in enumerate(tops):
        p.box(-1.0, 1.0, edge[i + 1] - nose, edge[i], t - slab, t, "stone")
    p.socket("planter", (-0.8, -0.35, 0.0), (0.0, 0.0, 1.0))
    p.socket("planter", (0.8, -0.35, 0.0), (0.0, 0.0, 1.0))
    return p


def roof_solar_eave_a():
    over, t, proud = 0.6, 0.13, 0.012
    p = Part("BIO_FOR_ROOF_SOLAR_EAVE_A", "roof_solar",
             ((-1.0, -over, -over * SLOPE - 0.16), (1.0, 2.0, RISE + t)),
             (2.0, 2.0 + over, RISE), CELL_ORIGIN + "; eave line on y=0 at the wall top (z=0)",
             notes="ROOF_SOLAR_A with the 0.6 m eave of ROOF_PITCH_EAVE_A; 3 x 2 longer "
                   "panels cover the overhang too. Use it for the eave row and "
                   "ROOF_SOLAR_A for rows above")
    _roof_base(p, -over, t)
    gap = 0.06
    w = (2.0 - 3 * gap) / 3
    for i in range(3):
        xa = -1.0 + gap / 2 + i * (w + gap)
        for ya, yb in ((-over + 0.06, 0.67), (0.73, 1.94)):
            p.shear_box(xa, xa + w, ya, yb, t, t + proud, SLOPE, "pv")
    _eave_timber(p, over)
    return p


CORNER_NOTE = ("second finished edge is x=-1, the LEFT end of the outer edge seen from "
               "outside. Rotation 0 (outer edge facing south) fits the SW corner, 90 "
               "(east) the SE corner, 180 (north) the NE corner, 270 (west) the NW "
               "corner: the same rule as BIO_FOR_CORNER_TIMBER_A")


def floor_corner_a():
    p = Part("BIO_FOR_FLOOR_CORNER_A", "other",
             ((-1.05, -0.05, -0.25), (1.0, 2.0, 0.0)), (2.0, 2.0, 0.25), CELL_ORIGIN,
             notes="FLOOR_A with the rim beam on two adjacent edges; " + CORNER_NOTE)
    p.layer_stack(-1.0, 1.0, 0.0, 2.0, [(-0.25, -0.04, "timber_structure"),
                                        (-0.04, 0.0, "timber_cladding")])
    p.box(-1.05, 1.0, -0.05, 0.0, -0.25, 0.0, "timber_structure")
    p.box(-1.05, -1.0, 0.0, 2.0, -0.25, 0.0, "timber_structure")
    return p


def roof_green_corner_a():
    p = Part("BIO_FOR_ROOF_GREEN_CORNER_A", "other",
             ((-1.04, -0.04, 0.0), (1.0, 2.0, 0.30)), (2.0, 2.0, 0.30),
             CELL_ORIGIN + "; underside on the wall top (z=0)",
             notes="ROOF_GREEN_A with the parapet on two adjacent edges; " + CORNER_NOTE)
    p.box(-1.03, 1.0, -0.03, 0.06, 0.0, 0.27, "timber_cladding")
    p.box(-1.03, -0.94, 0.06, 2.0, 0.0, 0.27, "timber_cladding")
    p.box(-1.04, 1.0, -0.04, 0.07, 0.27, 0.30, "metal_dark")
    p.box(-1.04, -0.93, 0.07, 2.0, 0.27, 0.30, "metal_dark")
    p.layer_stack(-0.94, 1.0, 0.06, 2.0, [(0.0, 0.08, "timber_structure"),
                                          (0.08, 0.19, "soil"), (0.19, 0.24, "sedum")])
    p.socket("planter", (0.0, 1.0, 0.24), (0.0, 0.0, 1.0))
    p.socket("vine", (0.0, -0.04, 0.27), (0.0, -1.0, 0.0))
    p.socket("vine", (-1.04, 1.0, 0.27), (-1.0, 0.0, 0.0))
    return p


def verge_a():
    p = Part("BIO_FOR_VERGE_A", "other",
             ((-0.15, 0.0, -0.10), (0.15, 2.0, RISE + 0.19)), (0.30, 2.0, RISE),
             "the point where a pitched cell's eave line meets the gable plane, at the "
             "wall top: position = roof cell origin + R*(+-1, 0, 0), same rotation and "
             "height as that roof cell; slope rises toward +Y",
             notes="EXCEPTION to the x = +-1 limit: projects 0.15 m outside the gable "
                   "plane (and 0.15 m back over the roof edge, so one part serves both "
                   "gable ends without mirroring). Covers y in [0,2] of its cell only: "
                   "the 0.6 m eave overhang beyond the wall has no verge")
    p.shear_box(-0.13, 0.13, 0.0, 2.0, -0.10, 0.15, SLOPE, "timber_structure")
    p.shear_box(-0.15, 0.15, 0.0, 2.0, 0.15, 0.19, SLOPE, "metal_dark")
    return p


PARTS = {
    "BIO_FOR_WALL_TIMBER_A": wall_timber_a,
    "BIO_FOR_WALL_TIMBER_WINDOW_A": wall_timber_window_a,
    "BIO_FOR_WALL_TIMBER_DOOR_A": wall_timber_door_a,
    "BIO_FOR_WALL_GLASS_A": wall_glass_a,
    "BIO_FOR_CORNER_TIMBER_A": corner_timber_a,
    "BIO_FOR_FOUNDATION_STONE_A": foundation_stone_a,
    "BIO_FOR_FLOOR_A": floor_a,
    "BIO_FOR_ROOF_PITCH_A": roof_pitch_a,
    "BIO_FOR_ROOF_PITCH_EAVE_A": roof_pitch_eave_a,
    "BIO_FOR_ROOF_SOLAR_A": roof_solar_a,
    "BIO_FOR_ROOF_GREEN_A": roof_green_a,
    "BIO_FOR_BALCONY_A": balcony_a,
    "BIO_FOR_COLUMN_TIMBER_A": column_timber_a,
    "BIO_FOR_GABLE_INFILL_A": gable_infill_a,
    "BIO_FOR_ENTRY_STEP_A": entry_step_a,
    "BIO_FOR_ROOF_SOLAR_EAVE_A": roof_solar_eave_a,
    "BIO_FOR_FLOOR_CORNER_A": floor_corner_a,
    "BIO_FOR_ROOF_GREEN_CORNER_A": roof_green_corner_a,
    "BIO_FOR_VERGE_A": verge_a,
}
