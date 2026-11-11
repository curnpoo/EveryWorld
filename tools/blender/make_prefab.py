"""Planter bench prefab (M3): a bench with a planter box and a solar panel on a post.

Run headless:
    /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup -P tools/blender/make_prefab.py -- \
        --out target/blender [--variant oak|teal|slate] [--colour R,G,B] [--render preview.png]

Writes (all generated, under target/, never committed):
    <out>/planter_bench.glb          two materials: "bench_wood" (variant colour) and "solar_panel"
    <out>/planter_bench.prefab.toml  footprint polygon and ground anchor (read by ew-bake)

Conventions (docs/contracts/pack.md, prefab kind):
    - metres; the object origin is the ground anchor: centre of the footprint at ground level,
      the lowest vertex sits at z = 0 (Blender, z up) = y = 0 (glTF, y up);
    - the bench faces -Y in Blender (= +Z in glTF), the seat is in front of the planter;
    - footprint = [x, z] corners in glTF/prefab space (y up), counter-clockwise seen from above;
    - the export is deterministic: same arguments -> byte-identical .glb (checked by exporting
      twice and comparing).

Variant/colour only change the wood material's base colour, so a re-export with another
variant changes one material chunk in the baked pack (the hot-reload test relies on that).
"""

import argparse
import hashlib
import math
import os
import sys

import bpy
import bmesh

VARIANTS = {
    "oak": (0.55, 0.36, 0.20),
    "teal": (0.12, 0.42, 0.40),
    "slate": (0.30, 0.32, 0.36),
}

# Bench dimensions (metres, Blender z up).
LEN = 1.8         # along X
DEPTH = 0.9       # along Y (planter at the back, seat at the front)
PLANTER_H = 0.55
SEAT_H = 0.45
POST_H = 2.3
PANEL = (1.1, 0.7, 0.04)
PANEL_TILT_DEG = 28.0


def parse():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(prog="make_prefab.py")
    ap.add_argument("--out", default="target/blender")
    ap.add_argument("--variant", default="oak", choices=sorted(VARIANTS))
    ap.add_argument("--colour", default=None, help="linear R,G,B for the wood (overrides --variant)")
    ap.add_argument("--render", default=None, help="also render a preview PNG (Workbench)")
    a = ap.parse_args(argv)
    if a.colour:
        c = tuple(float(x) for x in a.colour.split(","))
        if len(c) != 3 or not all(0.0 <= x <= 1.0 for x in c):
            sys.exit("--colour: expected R,G,B in 0..1")
        a.wood = c
    else:
        a.wood = VARIANTS[a.variant]
    return a


def fresh_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    return bpy.context.scene


def material(name, colour, metallic, roughness):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*colour, 1.0)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    m.diffuse_color = (*colour, 1.0)   # viewport / Workbench preview colour (not exported)
    return m


def add_box(bm, lo, hi, mat_index, rot=None, pivot=(0.0, 0.0, 0.0)):
    """Axis-aligned box lo..hi (optionally rotated about X by `rot` degrees around `pivot`)."""
    (x0, y0, z0), (x1, y1, z1) = lo, hi
    corners = [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
               (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]
    if rot is not None:
        a = math.radians(rot)
        ca, sa = math.cos(a), math.sin(a)
        px, py, pz = pivot
        out = []
        for (x, y, z) in corners:
            y, z = y - py, z - pz
            out.append((x, py + y * ca - z * sa, pz + y * sa + z * ca))
        corners = out
    v = [bm.verts.new(c) for c in corners]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    for f in faces:
        face = bm.faces.new([v[i] for i in f])
        face.material_index = mat_index


def build(wood_colour):
    scene = fresh_scene()
    mesh = bpy.data.meshes.new("PlanterBench")
    obj = bpy.data.objects.new("PlanterBench", mesh)
    scene.collection.objects.link(obj)
    mesh.materials.append(material("bench_wood", wood_colour, 0.0, 0.7))
    mesh.materials.append(material("solar_panel", (0.04, 0.07, 0.16), 0.6, 0.18))
    WOOD, SOLAR = 0, 1
    bm = bmesh.new()
    hx, hy = LEN / 2, DEPTH / 2
    # Planter box (back half), with a recessed soil top (the inner box is a separate block).
    add_box(bm, (-hx, 0.0, 0.0), (hx, hy, PLANTER_H), WOOD)
    # Seat slats (front half), three slats with gaps, on two legs.
    for k in range(3):
        y0 = -hy + 0.04 + k * 0.13
        add_box(bm, (-hx + 0.05, y0, SEAT_H - 0.05), (hx - 0.05, y0 + 0.11, SEAT_H), WOOD)
    for sx in (-1, 1):
        x = sx * (hx - 0.25)
        add_box(bm, (x - 0.05, -hy + 0.06, 0.0), (x + 0.05, -0.02, SEAT_H - 0.05), WOOD)
    # Solar post at the back right, and the tilted panel on top (facing -Y, i.e. over the seat).
    px = hx - 0.2
    add_box(bm, (px - 0.04, hy - 0.2, PLANTER_H), (px + 0.04, hy - 0.12, POST_H), SOLAR)
    w, d, t = PANEL
    add_box(bm, (px - w / 2 - 0.2, hy - 0.16 - d / 2, POST_H), (px + w / 2 - 0.2, hy - 0.16 + d / 2, POST_H + t), SOLAR,
            rot=PANEL_TILT_DEG, pivot=(0.0, hy - 0.16, POST_H))
    bm.normal_update()
    bm.to_mesh(mesh)
    bm.free()
    for p in mesh.polygons:
        p.use_smooth = False
    return obj


def export(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=path, export_format="GLB", use_selection=False, export_yup=True,
        export_apply=True, export_extras=False, export_materials="EXPORT",
        export_cameras=False, export_lights=False, export_animations=False,
        export_normals=True, export_texcoords=True, export_tangents=False,
        export_skins=False, export_morph=False)
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def manifest(path, obj):
    # Footprint from the mesh's bounds in glTF space: Blender (x, y, z) -> glTF (x, z, -y).
    xs = [v.co.x for v in obj.data.vertices]
    ys = [v.co.y for v in obj.data.vertices]
    zs = [v.co.z for v in obj.data.vertices]
    assert abs(min(zs)) < 1e-6, f"lowest vertex at z={min(zs)}, expected the ground anchor at 0"
    x0, x1 = min(xs), max(xs)
    z0, z1 = -max(ys), -min(ys)   # glTF z = -Blender y
    r = lambda v: f"{round(v, 4) + 0.0:.4f}"
    # Counter-clockwise seen from above (+Y in glTF looks down at x/z with z towards the viewer).
    fp = [(x0, z1), (x1, z1), (x1, z0), (x0, z0)]
    lines = [
        "# Planter bench prefab manifest (tools/blender/make_prefab.py). Prefab space = glTF:",
        "# metres, y up; footprint = [x, z] corners; anchor = the ground point the prefab is placed by.",
        "footprint = [" + ", ".join(f"[{r(x)}, {r(z)}]" for x, z in fp) + "]",
        "anchor = [0.0, 0.0, 0.0]",
    ]
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def render(path, obj):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.color_type = "MATERIAL"
    scene.render.resolution_x, scene.render.resolution_y = 640, 480
    scene.render.filepath = os.path.abspath(path)
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    cam.location = (4.2, -5.6, 2.9)
    d = (0.0 - cam.location[0], 0.0 - cam.location[1], 1.25 - cam.location[2])
    cam.rotation_euler = (math.atan2(math.hypot(d[0], d[1]), -d[2]), 0.0, math.atan2(d[1], d[0]) - math.pi / 2)
    scene.camera = cam
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(cam)


def main():
    a = parse()
    out = os.path.abspath(a.out)
    obj = build(a.wood)
    glb = os.path.join(out, "planter_bench.glb")
    sha = export(glb)
    again = os.path.join(out, "planter_bench.again.glb")
    sha2 = export(again)
    os.remove(again)
    if sha != sha2:
        sys.exit("PREFAB FAIL: two exports in one session differ (non-deterministic export)")
    manifest(os.path.join(out, "planter_bench.prefab.toml"), obj)
    tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    print(f"PREFAB ok planter_bench: variant {a.variant} wood {a.wood} tris {tris} materials 2 sha {sha[:12]} -> {glb}")
    if a.render:
        render(a.render, obj)
        print(f"PREFAB preview {os.path.abspath(a.render)}")


if __name__ == "__main__":
    main()
