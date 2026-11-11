"""Assemble one small test house from the exported kit parts and render it.

  Blender --background --factory-startup --python tools/blender/kit/assemble_test.py -- \
      --kit target/kit

Only 2 m steps in plan, 3 m storeys and 90 degree rotations are used; roof
cells sit on the wall top and the second roof row one rise higher. The house
is described in layout().
Writes <kit>/previews/_ASSEMBLY_front.png, _ASSEMBLY_back.png and
<kit>/assembly_report.txt (neighbour gap check).
"""

import argparse
import math
import os
import sys

import bpy
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import kitlib  # noqa: E402

P = "BIO_FOR_"
RISE = 2.0 * math.tan(math.radians(35.0))   # roof_rise_per_cell in kit.toml
S, E, N, W = 0, 90, 180, 270      # rotation of a part whose outer face looks south/east/north/west

# (part, x, y, z, rotation_deg, group) ; group: wall | cell | other
PLACEMENTS = []


def add(part, x, y, z, deg, group):
    PLACEMENTS.append((P + part, float(x), float(y), float(z), deg, group))


def layout():
    """Footprint X in [0,8], Y in [0,6]. Ground storey 8 x 6; upper storey 8 x 4
    (Y 2..6); the 8 x 2 south bay is glazed and carries the green roof. Gable
    roof over the upper storey, ridge along Y at X=4, two cells deep per slope:
    solar on the west slope, standing seam on the east."""
    # the plinth top is the ground-floor surface: no FLOOR part at ground level
    for x in (1, 3, 5, 7):
        for y in (0, 2, 4):
            add("FOUNDATION_STONE_A", x, y, 0, S, "cell:foundation")
    # ground storey walls, counter-clockwise from the SW corner
    for x, part in ((1, "WALL_GLASS_A"), (3, "WALL_GLASS_A"), (5, "WALL_GLASS_A"),
                    (7, "WALL_GLASS_A")):
        add(part, x, 0, 0, S, "wall")
    for y, part in ((1, "WALL_GLASS_A"), (3, "WALL_TIMBER_WINDOW_A"), (5, "WALL_TIMBER_A")):
        add(part, 8, y, 0, E, "wall")
    for x, part in ((7, "WALL_TIMBER_A"), (5, "WALL_TIMBER_WINDOW_A"), (3, "WALL_TIMBER_A"),
                    (1, "WALL_TIMBER_WINDOW_A")):
        add(part, x, 6, 0, N, "wall")
    for y, part in ((5, "WALL_TIMBER_WINDOW_A"), (3, "WALL_TIMBER_DOOR_A"), (1, "WALL_GLASS_A")):
        add(part, 0, y, 0, W, "wall")
    for x, y, d in ((0, 0, S), (8, 0, E), (8, 6, N), (0, 6, W)):
        add("CORNER_TIMBER_A", x, y, 0, d, "other")
    add("ENTRY_STEP_A", 0, 3, 0, W, "other")
    # upper floor, 4 x 2 cells: corner variants at the four corners
    add("FLOOR_CORNER_A", 1, 2, 3, S, "cell:floor")
    add("FLOOR_A", 3, 2, 3, S, "cell:floor")
    add("FLOOR_A", 5, 2, 3, S, "cell:floor")
    add("FLOOR_CORNER_A", 8, 3, 3, E, "cell:floor")
    add("FLOOR_CORNER_A", 7, 6, 3, N, "cell:floor")
    add("FLOOR_A", 5, 6, 3, N, "cell:floor")
    add("FLOOR_A", 3, 6, 3, N, "cell:floor")
    add("FLOOR_CORNER_A", 0, 5, 3, W, "cell:floor")
    # green roof over the south bay
    add("ROOF_GREEN_CORNER_A", 1, 0, 3, S, "cell:green")
    add("ROOF_GREEN_A", 3, 0, 3, S, "cell:green")
    add("ROOF_GREEN_A", 5, 0, 3, S, "cell:green")
    add("ROOF_GREEN_CORNER_A", 8, 1, 3, E, "cell:green")
    # upper storey walls
    for x, part in ((1, "WALL_TIMBER_WINDOW_A"), (3, "WALL_GLASS_A"), (5, "WALL_GLASS_A"),
                    (7, "WALL_TIMBER_WINDOW_A")):
        add(part, x, 2, 3, S, "wall")
    for y, part in ((3, "WALL_TIMBER_WINDOW_A"), (5, "WALL_TIMBER_A")):
        add(part, 8, y, 3, E, "wall")
    for x, part in ((7, "WALL_TIMBER_A"), (5, "WALL_TIMBER_WINDOW_A"), (3, "WALL_TIMBER_A"),
                    (1, "WALL_TIMBER_WINDOW_A")):
        add(part, x, 6, 3, N, "wall")
    for y, part in ((5, "WALL_TIMBER_WINDOW_A"), (3, "WALL_TIMBER_DOOR_A")):
        add(part, 0, y, 3, W, "wall")
    for x, y, d in ((0, 2, S), (8, 2, E), (8, 6, N), (0, 6, W)):
        add("CORNER_TIMBER_A", x, y, 3, d, "other")
    add("BALCONY_A", 0, 3, 3, W, "other")
    # roof, two cells deep per slope; row 1 sits one rise higher
    for y in (3, 5):
        add("ROOF_SOLAR_EAVE_A", 0, y, 6, W, "cell:roof")
        add("ROOF_SOLAR_A", 2, y, 6 + RISE, W, "cell:roof")
        add("ROOF_PITCH_EAVE_A", 8, y, 6, E, "cell:roof")
        add("ROOF_PITCH_A", 6, y, 6 + RISE, E, "cell:roof")
    # gable infill: one panel under each row-1 cell on both gable walls
    add("GABLE_INFILL_A", 3, 2, 6, S, "wall")
    add("GABLE_INFILL_A", 5, 2, 6, S, "wall")
    add("GABLE_INFILL_A", 5, 6, 6, N, "wall")
    add("GABLE_INFILL_A", 3, 6, 6, N, "wall")
    # verges: roof cell origin + R*(+-1,0,0), on the two gable planes Y=2 and Y=6
    for y in (2, 6):
        add("VERGE_A", 0, y, 6, W, "other")
        add("VERGE_A", 2, y, 6 + RISE, W, "other")
        add("VERGE_A", 8, y, 6, E, "other")
        add("VERGE_A", 6, y, 6 + RISE, E, "other")


def place(kit, name, x, y, z, deg):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(kit, "glb", name + ".glb"))
    new = [o for o in bpy.data.objects if o not in before]
    roots = [o for o in new if o.parent is None]
    assert len(roots) == 1 and roots[0].name.split(".")[0] == name, [o.name for o in roots]
    root = roots[0]
    root.rotation_mode = "XYZ"
    root.rotation_euler = (0.0, 0.0, math.radians(deg))
    root.location = (x, y, z)
    return root


def world_bbox(obj):
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    return (Vector((min(p[i] for p in pts) for i in range(3))),
            Vector((max(p[i] for p in pts) for i in range(3))))


def gap_check(placed, tol=1e-3):
    """Bounding boxes of neighbours must meet within tol.
    walls: same storey, facing and line, origins 2 m apart, tested along the run.
    cells: same layer, footprint centres 2 m apart along one axis (roof rows may
    differ by one rise), tested along that axis."""
    lines, worst, bad, pairs = [], 0.0, 0, 0

    def record(kind, axis, a, b, na, nb, where):
        nonlocal worst, bad, pairs
        gap = world_bbox(b)[0][axis] - world_bbox(a)[1][axis]
        pairs += 1
        worst = max(worst, abs(gap))
        flag = "ok" if abs(gap) <= tol else "BAD"
        bad += flag == "BAD"
        lines.append(f"{flag:3} {kind:16} {where} {na[len(P):]} | {nb[len(P):]}  "
                     f"gap {gap * 1000:+.4f} mm")

    runs = {}
    cells = []
    for (name, x, y, z, deg, group), obj in placed:
        if group == "wall":
            axis = 0 if deg in (S, N) else 1
            line = y if axis == 0 else x
            runs.setdefault((round(z, 3), deg, round(line, 3)), []).append(
                ((x, y)[axis], name, obj))
        elif group.startswith("cell"):
            r = math.radians(deg)
            cells.append((group, x - math.sin(r), y + math.cos(r), z, name, obj))
    for key in sorted(runs):
        axis = 0 if key[1] in (S, N) else 1
        run = sorted(runs[key], key=lambda r: r[0])
        for (pa, na, oa), (pb, nb, ob) in zip(run, run[1:]):
            if abs((pb - pa) - 2.0) < 1e-6:
                record("wall", axis, oa, ob, na, nb, f"z={key[0]:.2f} rot={key[1]:3}")
    for i, (ga, xa, ya, za, na, oa) in enumerate(cells):
        for gb, xb, yb, zb, nb, ob in cells[i + 1:]:
            if ga != gb or abs(za - zb) > (RISE + 1e-6 if ga == "cell:roof" else 1e-6):
                continue
            dx, dy = xb - xa, yb - ya
            for axis, d, other in ((0, dx, dy), (1, dy, dx)):
                if abs(abs(d) - 2.0) < 1e-6 and abs(other) < 1e-6:
                    first, second = ((oa, na), (ob, nb)) if d > 0 else ((ob, nb), (oa, na))
                    record(ga, axis, first[0], second[0], first[1], second[1],
                           f"z={za:.2f} axis={'XY'[axis]}")
    head = (f"GAPCHECK {'ok' if not bad else 'FAILED'}: {pairs} neighbour pairs, "
            f"{bad} over {tol * 1000:.0f} mm, worst |gap| {worst * 1000:.4f} mm")
    return head, lines, bad


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument("--kit", default="target/kit")
    a = ap.parse_args(argv)
    kit = os.path.abspath(a.kit)

    layout()
    report = None
    for view, az in (("front", 305.0), ("back", 125.0)):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        scene = bpy.context.scene
        placed = [(pl, place(kit, *pl[:5])) for pl in PLACEMENTS]
        bpy.context.view_layer.update()
        if report is None:
            head, lines, bad = gap_check(placed)
            counts = {}
            for pl in PLACEMENTS:
                counts[pl[0]] = counts.get(pl[0], 0) + 1
            report = [head] + lines + ["", "parts used:"] + [
                f"  {n} x{c}" for n, c in sorted(counts.items())]
            with open(os.path.join(kit, "assembly_report.txt"), "w") as f:
                f.write("\n".join(report) + "\n")
            print(head)
            for ln in lines:
                if ln.startswith("BAD"):
                    print("  " + ln)
        # azimuth 0 looks at the south face; 305 = camera in the south-west,
        # 125 = camera in the north-east
        hx, hy = (-1.4, 5.0) if view == "front" else (9.2, 1.0)
        kitlib.render_preview(scene, os.path.join(kit, "previews", f"_ASSEMBLY_{view}.png"),
                              (-1.8, -0.3, -0.6), (9.0, 6.4, 9.2), azimuth_deg=az,
                              elevation_deg=16.0, res=1024, human_at=(hx, hy, -0.6),
                              ground_z=-0.6, lens=50.0)
        print(f"ASSEMBLY rendered {view}")
    if bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
