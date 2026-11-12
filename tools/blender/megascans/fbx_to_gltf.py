"""Megascans FBX -> glTF (GLB), headless and deterministic. Driven by `ew-bake` (ed_assets::megascans_lib),
which embeds this file and runs it as:

    Blender --background --factory-startup --python-exit-code 1 --python fbx_to_gltf.py -- \
        --out DIR in0.fbx [in1.fbx ...]

Writes DIR/0.glb, DIR/1.glb, ... (one per input, in argument order). Each input is imported into a
fresh empty scene, so object names, LOD meshes and their order come from that file alone.

What it keeps / changes:
  - every mesh object stays its own node (LOD0..LODn are separate nodes, named as in the FBX);
  - units: the FBX importer (Blender 5.x: always) applies the file's unit scale (Megascans exports
    centimetres), so the output is in metres;
  - axes: taken from the file's own axis settings (no manual orientation); FBX is normally Y-up,
    Blender imports it Z-up and the glTF exporter writes Y-up again, so a
    model's up stays +Y and Blender's -Y (front) becomes glTF +Z, Blender +Y becomes glTF -Z
    (our "y up, -z north" convention, same mapping as tools/blender/make_prefab.py);
  - geometry only: no materials, images, cameras, lights, animation, skins or morphs (textures go
    through ew-bake's surface path), normals and UV0 kept, no tangents.

Determinism: no timestamps or random ids enter the GLB; the exporter walks objects in name order.
ew-bake only reads the geometry, sorted by LOD and node order, and its tests convert the same FBX
twice in two Blender processes and compare the bytes.
"""

import os
import sys

import bpy


def parse():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out, inputs = None, []
    it = iter(argv)
    for a in it:
        if a == "--out":
            out = next(it)
        else:
            inputs.append(a)
    if not out or not inputs:
        sys.exit("usage: fbx_to_gltf.py -- --out DIR in.fbx [...]")
    return out, inputs


def convert(src, dst):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(
        filepath=src,
        use_custom_normals=True,
        use_image_search=False,
        use_anim=False,
        use_manual_orientation=False,
        global_scale=1.0,
        bake_space_transform=False,
        automatic_bone_orientation=False,
        ignore_leaf_bones=True,
    )
    # Only meshes (and their parent empties) go out.
    for ob in list(bpy.data.objects):
        if ob.type not in {"MESH", "EMPTY"}:
            bpy.data.objects.remove(ob, do_unlink=True)
    meshes = [ob for ob in bpy.data.objects if ob.type == "MESH"]
    if not meshes:
        sys.exit(f"{src}: no mesh objects")
    bpy.ops.export_scene.gltf(
        filepath=dst,
        export_format="GLB",
        use_selection=False,
        export_yup=True,
        export_apply=True,
        export_extras=False,
        export_materials="NONE",
        export_image_format="NONE",
        export_cameras=False,
        export_lights=False,
        export_animations=False,
        export_normals=True,
        export_texcoords=True,
        export_tangents=False,
        export_skins=False,
        export_morph=False,
    )
    tris = sum(sum(len(p.vertices) - 2 for p in ob.data.polygons) for ob in meshes)
    print(f"fbx_to_gltf: {os.path.basename(src)} -> {os.path.basename(dst)}: {len(meshes)} mesh object(s), {tris} triangles")


def main():
    out, inputs = parse()
    os.makedirs(out, exist_ok=True)
    for i, src in enumerate(inputs):
        convert(src, os.path.join(out, f"{i}.glb"))


main()
