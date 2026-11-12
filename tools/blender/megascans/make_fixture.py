"""Synthetic Megascans-style FBX for ed_assets tests (no Megascans data in the repository).

    Blender --background --factory-startup --python-exit-code 1 --python make_fixture.py -- \
        --out rock_LODs.fbx [--split DIR]

Builds a lumpy "rock" at three LODs (UV spheres, 48x24 / 24x12 / 12x6 segments, radius 50 cm,
bumps baked into the vertices so coarser LODs really lose shape), named <stem>_LOD0..2, and
exports them in centimetres the way Megascans FBX files are (scene unit scale 0.01, so the FBX
says "centimetres" and 50 Blender units = 50 cm). With --split DIR it also writes one FBX per
LOD (DIR/<stem>_LOD<k>.fbx), the other Megascans layout.
"""

import math
import os
import sys

import bpy
import bmesh


def parse():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out, split = None, None
    it = iter(argv)
    for a in it:
        if a == "--out":
            out = next(it)
        elif a == "--split":
            split = next(it)
    if not out:
        sys.exit("usage: make_fixture.py -- --out file.fbx [--split DIR]")
    return out, split


def rock(name, segs, rings):
    me = bpy.data.meshes.new(name)
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=segs, v_segments=rings, radius=50.0, calc_uvs=True)
    for v in bm.verts:
        x, y, z = v.co
        r = 1.0 + 0.12 * math.sin(3.0 * x / 50.0 + 1.0) * math.cos(2.0 * y / 50.0) + 0.08 * math.sin(5.0 * z / 50.0)
        v.co = (x * r, y * r * 0.9, z * r * 0.7 + 35.0)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def export(path, objs):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.ops.export_scene.fbx(
        filepath=path,
        use_selection=True,
        object_types={"MESH"},
        apply_unit_scale=True,
        apply_scale_options="FBX_SCALE_UNITS",
        axis_forward="-Z",
        axis_up="Y",
        bake_anim=False,
        add_leaf_bones=False,
        mesh_smooth_type="FACE",
    )


def main():
    out, split = parse()
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.unit_settings.system = "METRIC"
    bpy.context.scene.unit_settings.scale_length = 0.01
    stem = os.path.splitext(os.path.basename(out))[0].replace("_LODs", "")
    objs = [rock(f"{stem}_LOD{k}", s, r) for k, (s, r) in enumerate([(48, 24), (24, 12), (12, 6)])]
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    export(out, objs)
    if split:
        os.makedirs(split, exist_ok=True)
        for k, o in enumerate(objs):
            export(os.path.join(split, f"{stem}_LOD{k}.fbx"), [o])
    print(f"make_fixture: wrote {out}")


main()
