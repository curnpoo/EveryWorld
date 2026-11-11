"""Assemble one small test house from the exported kit parts and render it.

  Blender --background --factory-startup --python tools/blender/kit/assemble_test.py -- \
      --kit target/kit

Only 2 m steps in plan, 3 m storeys and 90 degree rotations are used (roof
cells sit on the wall top). Footprint X in [0,4], Y in [0,6]:
  ground storey 4 x 6; upper storey 4 x 4 (Y 2..6); the 4 x 2 bay at the south
  end carries a green roof; pitched roof over the upper storey, ridge along Y.
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
S, E, N, W = 0, 90, 180, 270      # rotation of a part whose outer face looks south/east/north/west

# (part, x, y, z, rotation_deg, group) ; group: wall | cell | other
PLACEMENTS = []


def add(part, x, y, z, deg, group):
    PLACEMENTS.append((P + part, float(x), float(y), float(z), deg, group))


def layout():
    # foundation and upper floor cells: west column faces W, east column faces E
    for y in (1, 3, 5):
        add("FOUNDATION_STONE_A", 0, y, 0, W, "cell")
        add("FOUNDATION_STONE_A", 4, y, 0, E, "cell")
    for y in (3, 5):
        add("FLOOR_A", 0, y, 3, W, "cell")
        add("FLOOR_A", 4, y, 3, E, "cell")
    # ground storey walls (counter-clockwise from the SW corner)
    add("WALL_GLASS_A", 1, 0, 0, S, "wall")
    add("WALL_GLASS_A", 3, 0, 0, S, "wall")
    add("WALL_TIMBER_A", 4, 1, 0, E, "wall")
    add("WALL_TIMBER_WINDOW_A", 4, 3, 0, E, "wall")
    add("WALL_TIMBER_A", 4, 5, 0, E, "wall")
    add("WALL_TIMBER_A", 3, 6, 0, N, "wall")
    add("WALL_TIMBER_WINDOW_A", 1, 6, 0, N, "wall")
    add("WALL_TIMBER_WINDOW_A", 0, 5, 0, W, "wall")
    add("WALL_TIMBER_DOOR_A", 0, 3, 0, W, "wall")
    add("WALL_GLASS_A", 0, 1, 0, W, "wall")
    for x, y, d in ((0, 0, S), (4, 0, E), (4, 6, N), (0, 6, W)):
        add("CORNER_TIMBER_A", x, y, 0, d, "other")
    # upper storey walls, Y in [2,6]
    add("WALL_GLASS_A", 1, 2, 3, S, "wall")
    add("WALL_TIMBER_WINDOW_A", 3, 2, 3, S, "wall")
    add("WALL_TIMBER_A", 4, 3, 3, E, "wall")
    add("WALL_TIMBER_WINDOW_A", 4, 5, 3, E, "wall")
    add("WALL_TIMBER_WINDOW_A", 3, 6, 3, N, "wall")
    add("WALL_TIMBER_A", 1, 6, 3, N, "wall")
    add("WALL_TIMBER_WINDOW_A", 0, 5, 3, W, "wall")
    add("WALL_TIMBER_DOOR_A", 0, 3, 3, W, "wall")
    for x, y, d in ((0, 2, S), (4, 2, E), (4, 6, N), (0, 6, W)):
        add("CORNER_TIMBER_A", x, y, 3, d, "other")
    add("BALCONY_A", 0, 3, 3, W, "other")
    # green roof over the south bay, parapet on the south edge
    add("ROOF_GREEN_A", 1, 0, 3, S, "cell")
    add("ROOF_GREEN_A", 3, 0, 3, S, "cell")
    # pitched roof: solar on the west slope, standing seam with eave on the east
    for y in (3, 5):
        add("ROOF_SOLAR_A", 0, y, 6, W, "cell")
        add("ROOF_PITCH_EAVE_A", 4, y, 6, E, "cell")


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
    """Neighbours in a run (same part group, storey, facing and line, origins 2 m
    apart): the bounding boxes must meet along the run within tol."""
    lines, worst, bad, pairs = [], 0.0, 0, 0
    runs = {}
    for (name, x, y, z, deg, group), obj in placed:
        if group not in ("wall", "cell"):
            continue
        axis = 0 if deg in (S, N) else 1            # run direction in the world
        line = y if axis == 0 else x
        runs.setdefault((group, round(z, 3), deg, round(line, 3)), []).append(
            ((x, y)[axis], name, obj))
    for key in sorted(runs):
        axis = 0 if key[2] in (S, N) else 1
        run = sorted(runs[key], key=lambda r: r[0])
        for (pa, na, oa), (pb, nb, ob) in zip(run, run[1:]):
            if abs((pb - pa) - 2.0) > 1e-6:
                continue
            gap = world_bbox(ob)[0][axis] - world_bbox(oa)[1][axis]
            pairs += 1
            worst = max(worst, abs(gap))
            flag = "ok" if abs(gap) <= tol else "BAD"
            bad += flag == "BAD"
            lines.append(f"{flag:3} {key[0]:4} z={key[1]:.0f} rot={key[2]:3} "
                         f"{na[len(P):]} | {nb[len(P):]}  gap {gap * 1000:+.4f} mm")
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
        hx, hy = (-1.4, 0.6) if view == "front" else (5.4, 5.4)
        kitlib.render_preview(scene, os.path.join(kit, "previews", f"_ASSEMBLY_{view}.png"),
                              (-1.2, -0.3, -0.6), (5.0, 6.2, 7.6), azimuth_deg=az,
                              elevation_deg=16.0, res=1024, human_at=(hx, hy, -0.6),
                              ground_z=-0.6, lens=50.0)
        print(f"ASSEMBLY rendered {view}")
    if bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
