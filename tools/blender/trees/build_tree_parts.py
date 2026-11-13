"""Tree parts for Norway spruce and European larch (trees G6): authored from scratch.

    Blender --background --factory-startup --python tools/blender/trees/build_tree_parts.py -- \
        --out .earthdrop-cache/tree_parts [--only textures|parts|bark]

(or ``tools/blender/trees/bake_tree_parts.sh``). Writes, all generated and gitignored:

    <out>/<cell>.png        card textures, 512 px, sRGB + alpha (the needles rendered from fine
                            geometry, ``needles.py``): spruce_spray, spruce_curtain,
                            larch_shoot, spruce_leader
    <out>/<cell>_n.png      their normal maps (card space, +Z towards the viewer)
    <out>/bark_spruce.png, bark_larch.png   tileable bark (1.0 m square), procedural, baked
    <out>/tree_parts.glb    the parts: low-poly card assemblies in real metres
    <out>/manifest.json     what is where (cells, parts, triangle counts, hashes)

Part conventions (read by ``ew::trees`` -> ``ed_world::trees::parts``):
    - one glTF node per (part, texture): ``<part>__<cell>`` (``spruce_spray_a__spruce_curtain``);
    - sprays: origin at the point where the spray sits on its branch, +X (glTF) along the
      branch towards its tip, +Y up, +Z to the branch's side; leaders: origin at the base,
      +Y along the leader;
    - UV v = 0 at the end of a card that attaches (twig base), v = 1 at its tip (cell space);
    - ``*_far`` parts are the T1 stand-ins: two cards each.
The engine bends, rotates, scales and instances these per branch from the tree's genome.
Deterministic: fixed seeds, no wall clock.
"""

import argparse
import hashlib
import json
import math
import os
import random
import sys

import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import needles as nd  # noqa: E402
from needles import Vector as V  # noqa: E402,F401

RES = 512

# Card frames (m): the square each texture covers. Card geometry uses the same size, so the
# needles keep their real length on the tree.
FRAME = {"spruce_spray": 0.5, "spruce_curtain": 0.5, "larch_shoot": 0.5, "spruce_leader": 0.6}

# Colours (sRGB, converted to linear). The engine renormalises each texture to the atlas'
# neutral green, so these set the contrast inside a card (needles vs twigs, old vs new).
SPRUCE_NEEDLE = nd.srgb_to_lin((0.17, 0.27, 0.12))
SPRUCE_TWIG = nd.srgb_to_lin((0.36, 0.23, 0.13))
LARCH_NEEDLE = nd.srgb_to_lin((0.36, 0.52, 0.20))
LARCH_TWIG = nd.srgb_to_lin((0.62, 0.48, 0.30))
LARCH_SPUR = nd.srgb_to_lin((0.34, 0.27, 0.22))


def parse():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(prog="build_tree_parts.py")
    ap.add_argument("--out", default=".earthdrop-cache/tree_parts")
    ap.add_argument("--only", default=None, choices=["textures", "parts", "bark"])
    return ap.parse_args(argv)


def fresh():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    return bpy.context.scene


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


# ---------------------------------------------------------------------------------------------
# Needle textures
# ---------------------------------------------------------------------------------------------

def sink_object(scene, sink, name):
    me = bpy.data.meshes.new(name)
    me.from_pydata(sink.verts, [], sink.tris)
    me.update()
    for attr_name, data in (("col", sink.cols), ("nrm", sink.nrms)):
        a = me.color_attributes.new(attr_name, "FLOAT_COLOR", "POINT")
        flat = [c for rgba in data for c in rgba]
        a.data.foreach_set("color", flat)
    ob = bpy.data.objects.new(name, me)
    scene.collection.objects.link(ob)
    return ob, me


def set_active_colour(me, name):
    idx = me.color_attributes.find(name)
    me.color_attributes.active_color_index = idx
    me.color_attributes.render_color_index = idx


def render_card(scene, sink, frame, path, normal_path):
    ob, me = sink_object(scene, sink, "needles")
    cam_data = bpy.data.cameras.new("card_cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = frame
    cam_data.clip_start = 0.01
    cam_data.clip_end = 20.0
    cam = bpy.data.objects.new("card_cam", cam_data)
    cam.location = (0.0, frame * 0.5, 5.0)
    scene.collection.objects.link(cam)
    scene.camera = cam
    scene.render.engine = "BLENDER_WORKBENCH"
    sh = scene.display.shading
    sh.light = "FLAT"
    sh.color_type = "VERTEX"
    sh.show_object_outline = False
    sh.show_cavity = False
    sh.show_shadows = False
    sh.show_specular_highlight = False
    sh.show_xray = False
    sh.show_backface_culling = False
    scene.display.render_aa = "32"
    scene.render.film_transparent = True
    scene.render.dither_intensity = 0.0
    scene.render.resolution_x = RES
    scene.render.resolution_y = RES
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.view_settings.look = "None"
    scene.view_settings.exposure = 0.0
    scene.view_settings.gamma = 1.0
    for which, out, vt in (("col", path, "Standard"), ("nrm", normal_path, "Raw")):
        set_active_colour(me, which)
        scene.view_settings.view_transform = vt
        scene.render.filepath = out
        bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(ob)
    bpy.data.meshes.remove(me)
    bpy.data.objects.remove(cam)


def spruce_spray(rng):
    """Top view of a spruce branch end: a leading shoot with alternate side shoots (55 deg,
    shorter towards the tip), second-order shoots on the longer ones, needles all round
    (combed into the plane: a shade shoot), this year's growth a little lighter."""
    s = nd.Sink()
    F = FRAME["spruce_spray"]
    axis = nd.curve((0, 0.004, 0), (rng.uniform(-0.05, 0.05), 1, 0), F * 0.93, (0, 0, -0.05), rng, 10, 0.03)
    nd.brush_shoot(s, axis, rng, SPRUCE_NEEDLE, SPRUCE_TWIG, 0.019, 13.0, 52, 0.55, r=0.0024, width=0.0017, young_tip=0.35)
    for k in range(18):
        t = 0.05 + 0.046 * k + rng.uniform(-0.015, 0.015)
        if t > 0.86:
            break
        p, d = nd.frame_at(axis, t)
        sgn = -1.0 if k % 2 else 1.0
        side = d.cross(nd.UP).normalized() * sgn
        L = F * 0.46 * (1.0 - 0.75 * t) * rng.uniform(0.8, 1.15)
        a = math.radians(rng.uniform(48, 62))
        dirn = (d * math.cos(a) + side * math.sin(a)).normalized()
        sh = nd.curve(p, dirn, L, -side * 0.6 + d * 0.5, rng, 7, 0.04)
        nd.brush_shoot(s, sh, rng, SPRUCE_NEEDLE, SPRUCE_TWIG, 0.017, 12.0, 55, 0.6, r=0.0014, width=0.0016, young_tip=0.35)
        if L > 0.05:
            for j in range(int(L / 0.022)):
                tt = 0.15 + 0.14 * j + rng.uniform(-0.04, 0.04)
                if tt > 0.8:
                    break
                q, dd = nd.frame_at(sh, tt)
                sg = -1.0 if j % 2 else 1.0
                sd = dd.cross(nd.UP).normalized() * sg
                b = math.radians(rng.uniform(45, 60))
                l2 = L * 0.38 * (1.0 - 0.6 * tt) * rng.uniform(0.8, 1.2)
                sh2 = nd.curve(q, (dd * math.cos(b) + sd * math.sin(b)), l2, dd * 0.5, rng, 4, 0.04)
                nd.brush_shoot(s, sh2, rng, SPRUCE_NEEDLE, SPRUCE_TWIG, 0.015, 12.0, 58, 0.6, r=0.001, width=0.0015, young_tip=0.3)
    return s


def spruce_curtain(rng):
    """Side view of the hanging 'comb' branchlets under a spruce branch: a short piece of the
    branch at the attachment (v = 0) and pendulous shoots from it, needles all round."""
    s = nd.Sink()
    F = FRAME["spruce_curtain"]
    stub = [V((-F * 0.3, 0.008, 0)), V((0, 0.012, 0)), V((F * 0.3, 0.006, 0))]
    nd.tube(s, stub, 0.004, 0.003, SPRUCE_TWIG, 5)
    n = 8
    for k in range(n):
        x0 = (k / (n - 1) - 0.5) * F * 0.62 + rng.uniform(-0.02, 0.02)
        L = F * rng.uniform(0.62, 0.9)
        lean = rng.uniform(-0.12, 0.12) + x0 * 0.6
        sh = nd.curve((x0, 0.012, 0), (lean, 1, 0), L, (rng.uniform(-0.2, 0.2), 0, 0), rng, 9, 0.05)
        nd.brush_shoot(s, sh, rng, SPRUCE_NEEDLE, SPRUCE_TWIG, 0.018, 13.0, 60, 0.15, r=0.0016, width=0.0017, young_tip=0.3)
        for j in range(int(L / 0.04)):
            tt = 0.12 + 0.11 * j + rng.uniform(-0.03, 0.03)
            if tt > 0.8:
                break
            q, d = nd.frame_at(sh, tt)
            sg = -1.0 if (j + k) % 2 else 1.0
            sd = d.cross(V((0, 0, 1))).normalized() * sg
            b = math.radians(rng.uniform(30, 45))
            sh2 = nd.curve(q, d * math.cos(b) + sd * math.sin(b), L * rng.uniform(0.12, 0.2), d * 1.5, rng, 4, 0.04)
            nd.brush_shoot(s, sh2, rng, SPRUCE_NEEDLE, SPRUCE_TWIG, 0.015, 12.0, 60, 0.15, r=0.001, width=0.0015)
    return s


def larch_shoot(rng):
    """A larch long shoot with short-shoot rosettes (20-40 soft needles each) every couple of
    centimetres, two side long shoots, and single needles on this year's tip."""
    s = nd.Sink()
    F = FRAME["larch_shoot"]

    def long_shoot(start, d, L, bend, depth):
        sh = nd.curve(start, d, L, bend, rng, 9, 0.03)
        nd.tube(s, sh, 0.0022 if depth == 0 else 0.0015, 0.001, LARCH_TWIG, 4)
        t = 0.04
        k = 0
        while t < 0.82:
            p, dd = nd.frame_at(sh, t)
            sg = -1.0 if k % 2 else 1.0
            sd = dd.cross(nd.UP).normalized() * sg
            ax = (dd * 0.5 + sd * 0.7 + nd.UP * 0.5).normalized()
            nd.rosette(s, p + sd * 0.002, ax, rng, LARCH_NEEDLE, rng.randint(30, 44), 0.036 * rng.uniform(0.85, 1.1), LARCH_SPUR)
            t += rng.uniform(0.012, 0.018) / L
            k += 1
        tip = [nd.frame_at(sh, 0.82 + 0.18 * i / 4)[0] for i in range(5)]
        col = tuple(x * 1.15 for x in LARCH_NEEDLE)
        nd.brush_shoot(s, tip, rng, col, LARCH_TWIG, 0.018, 7.0, 45, 0.3, r=0.0009, width=0.0009, droop=0.2)
        return sh

    main = long_shoot((0, 0.004, 0), (rng.uniform(-0.05, 0.05), 1, 0), F * 0.92, (0, 0, -0.1), 0)
    for t, sg in ((0.18, 1.0), (0.32, -1.0), (0.48, 1.0), (0.62, -1.0)):
        p, d = nd.frame_at(main, t)
        side = d.cross(nd.UP).normalized() * sg
        a = math.radians(rng.uniform(40, 55))
        long_shoot(p, d * math.cos(a) + side * math.sin(a), F * 0.48 * (1.0 - 0.6 * t), d * 0.6, 1)
    return s


def spruce_leader(rng):
    """The top of a spruce: this year's leader standing straight up with needles all round,
    a terminal bud cluster, and the top whorl's short shoots at its base."""
    s = nd.Sink()
    F = FRAME["spruce_leader"]
    lead = nd.curve((0, 0.004, 0), (0, 1, 0), F * 0.92, (0, 0, 0), rng, 10, 0.01)
    nd.brush_shoot(s, lead, rng, SPRUCE_NEEDLE, SPRUCE_TWIG, 0.02, 16.0, 45, 0.0, r=0.003, width=0.0018, young_tip=0.4)
    for k in range(5):
        a = math.radians(-60 + 30 * k + rng.uniform(-8, 8))
        d = V((math.sin(a), math.cos(a) * 0.55 + 0.45, 0)).normalized()
        sh = nd.curve((0, 0.03 + 0.01 * k, 0), d, F * rng.uniform(0.3, 0.4), (0, 0.5, 0), rng, 5, 0.03)
        nd.brush_shoot(s, sh, rng, SPRUCE_NEEDLE, SPRUCE_TWIG, 0.017, 13.0, 50, 0.2, r=0.0015, width=0.0016, young_tip=0.4)
    tip = lead[-1]
    for k in range(4):
        a = k / 4 * math.tau
        nd.tube(s, [tip, tip + V((0.004 * math.cos(a), 0.012, 0.004 * math.sin(a)))], 0.003, 0.001, SPRUCE_TWIG, 5)
    return s


CARDS = {
    "spruce_spray": (spruce_spray, 0x5B7),
    "spruce_curtain": (spruce_curtain, 0xC07),
    "larch_shoot": (larch_shoot, 0x1A7),
    "spruce_leader": (spruce_leader, 0x1EA),
}


def bake_textures(out):
    rows = {}
    for name, (fn, seed) in CARDS.items():
        scene = fresh()
        sink = fn(random.Random(seed))
        p, pn = os.path.join(out, name + ".png"), os.path.join(out, name + "_n.png")
        render_card(scene, sink, FRAME[name], p, pn)
        rows[name] = {"file": name + ".png", "normal": name + "_n.png", "frame_m": FRAME[name], "px": RES,
                      "source_tris": len(sink.tris), "sha256": sha(p)}
        print(f"tree_parts: {name}: {len(sink.tris)} source triangles -> {p}")
    return rows


# ---------------------------------------------------------------------------------------------
# Bark (tileable 1 m squares, procedural shaders baked by Cycles)
# ---------------------------------------------------------------------------------------------

def bark_material(kind):
    m = bpy.data.materials.new("bark_" + kind)
    nt = m.node_tree if m.node_tree else None
    if nt is None:
        m.use_nodes = True
        nt = m.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    N, L = nt.nodes, nt.links
    tc = N.new("ShaderNodeTexCoord")
    sep = N.new("ShaderNodeSeparateXYZ")
    L.new(tc.outputs["UV"], sep.inputs[0])

    def math_node(op, a, b=None):
        n = N.new("ShaderNodeMath")
        n.operation = op
        for i, x in enumerate((a, b)):
            if x is None:
                continue
            if isinstance(x, (int, float)):
                n.inputs[i].default_value = x
            else:
                L.new(x, n.inputs[i])
        return n.outputs[0]

    # Seamless: the square maps onto a torus in 4D (x, y, z, w).
    tau = math.tau
    u, v = sep.outputs[0], sep.outputs[1]
    cu, su = math_node("COSINE", math_node("MULTIPLY", u, tau)), math_node("SINE", math_node("MULTIPLY", u, tau))
    cv, sv = math_node("COSINE", math_node("MULTIPLY", v, tau)), math_node("SINE", math_node("MULTIPLY", v, tau))
    comb = N.new("ShaderNodeCombineXYZ")
    # Plates are taller than wide: the v circle is smaller (fewer cells around it).
    ru, rv = (1.6, 0.7) if kind == "spruce" else (1.1, 0.35)
    L.new(math_node("MULTIPLY", cu, ru), comb.inputs[0])
    L.new(math_node("MULTIPLY", su, ru), comb.inputs[1])
    L.new(math_node("MULTIPLY", cv, rv), comb.inputs[2])
    w = math_node("MULTIPLY", sv, rv)

    vor = N.new("ShaderNodeTexVoronoi")
    vor.voronoi_dimensions = "4D"
    vor.feature = "DISTANCE_TO_EDGE"
    vor.inputs["Scale"].default_value = 5.0 if kind == "spruce" else 3.0
    vor.inputs["Randomness"].default_value = 0.9
    noise = N.new("ShaderNodeTexNoise")
    noise.noise_dimensions = "4D"
    noise.inputs["Scale"].default_value = 9.0
    noise.inputs["Detail"].default_value = 8.0
    noise.inputs["Roughness"].default_value = 0.6
    for n in (vor, noise):
        L.new(comb.outputs[0], n.inputs["Vector"])
        L.new(w, n.inputs["W"])
    # Fissures where the distance to a cell edge is small; plates mottled by the noise.
    edge = math_node("SMOOTH_MAX", math_node("MULTIPLY", vor.outputs["Distance"], 9.0 if kind == "spruce" else 5.0), 0.0)
    plate = math_node("MINIMUM", edge, 1.0)
    shade = math_node("MULTIPLY", plate, math_node("ADD", math_node("MULTIPLY", noise.outputs["Fac"], 0.6), 0.7))
    ramp = N.new("ShaderNodeValToRGB")
    cr = ramp.color_ramp
    if kind == "spruce":
        # Thin reddish-brown scales, grey where weathered.
        stops = [(0.0, (0.05, 0.03, 0.02)), (0.35, (0.20, 0.10, 0.06)), (0.7, (0.33, 0.19, 0.12)), (1.0, (0.40, 0.33, 0.28))]
    else:
        # Thick grey-brown plates split by deep furrows with red-brown inner bark.
        stops = [(0.0, (0.04, 0.02, 0.015)), (0.25, (0.30, 0.10, 0.06)), (0.55, (0.30, 0.22, 0.18)), (1.0, (0.45, 0.40, 0.36))]
    cr.elements[0].position, cr.elements[0].color = stops[0][0], stops[0][1] + (1.0,)
    cr.elements[1].position, cr.elements[1].color = stops[-1][0], stops[-1][1] + (1.0,)
    for pos, c in stops[1:-1]:
        e = cr.elements.new(pos)
        e.color = c + (1.0,)
    L.new(shade, ramp.inputs[0])
    em = N.new("ShaderNodeEmission")
    L.new(ramp.outputs[0], em.inputs[0])
    outn = N.new("ShaderNodeOutputMaterial")
    L.new(em.outputs[0], outn.inputs[0])
    return m, N


def bake_bark(out):
    rows = {}
    for kind in ("spruce", "larch"):
        scene = fresh()
        scene.render.engine = "CYCLES"
        scene.cycles.device = "CPU"
        scene.cycles.samples = 1
        scene.cycles.seed = 0
        bpy.ops.mesh.primitive_plane_add(size=1.0)
        ob = bpy.context.active_object
        m, N = bark_material(kind)
        ob.data.materials.append(m)
        img = bpy.data.images.new("bark_" + kind, RES, RES, alpha=False)
        tex = N.new("ShaderNodeTexImage")
        tex.image = img
        N.active = tex
        for n in N:
            n.select = False
        tex.select = True
        bpy.ops.object.bake(type="EMIT", margin=0)
        path = os.path.join(out, f"bark_{kind}.png")
        img.filepath_raw = path
        img.file_format = "PNG"
        img.save()
        rows[kind] = {"file": f"bark_{kind}.png", "repeat_m": 1.0, "px": RES, "sha256": sha(path)}
        print(f"tree_parts: bark {kind} -> {path}")
    return rows


# ---------------------------------------------------------------------------------------------
# Parts: low-poly card assemblies (Blender axes: +X along the branch, +Y lateral, +Z up)
# ---------------------------------------------------------------------------------------------

def card(verts, uvs, faces, base, along, across, length, width, cols=2, rows=2, sag=None, flip_u=False):
    """A card of `cols` x `rows` vertices from `base` (v = 0, centre of the attaching edge)
    along `along` (v = 1 at `length`), `across` spanning `width`. `sag(u, v)` offsets each
    vertex (a Vector)."""
    along, across = Vector(along).normalized(), Vector(across).normalized()
    first = len(verts)
    for j in range(rows):
        for i in range(cols):
            u, v = i / (cols - 1), j / (rows - 1)
            p = Vector(base) + along * v * length + across * (u - 0.5) * width
            if sag is not None:
                p += sag(u, v)
            verts.append(p)
            uvs.append((1.0 - u if flip_u else u, v))
    for j in range(rows - 1):
        for i in range(cols - 1):
            a = first + j * cols + i
            faces.append((a, a + 1, a + cols + 1, a + cols))


def rot_z(v, deg):
    return Matrix.Rotation(math.radians(deg), 3, "Z") @ Vector(v)


def roll(across, along, deg):
    return Matrix.Rotation(math.radians(deg), 3, Vector(along)) @ Vector(across)


def spray_top(vs, us, fs, L, W, arch, droop, cup):
    card(vs, us, fs, (-0.02, 0, 0), (1, 0, 0), (0, 1, 0), L, W, 2, 3,
         sag=lambda u, v: Vector((0, 0, arch * math.sin(math.pi * v) - droop * v * v - cup * (2 * u - 1) ** 2)))


def side_card(vs, us, fs, x, sgn, yaw, roll_deg, L, W, flip):
    along = rot_z((1, 0, 0), sgn * yaw)
    across = roll(rot_z((0, 1, 0), sgn * yaw), along, -sgn * roll_deg)
    card(vs, us, fs, (x, sgn * 0.03, -0.01), along, across, L, W, flip_u=flip)


def curtain(vs, us, fs, x, y, yaw, L, W, lean=0.25):
    down = Vector((lean, 0, -1)).normalized()
    card(vs, us, fs, (x, y, -0.015), down, rot_z((1, 0, 0), yaw), L, W)


def spruce_spray_a():
    out = {"spruce_spray": ([], [], []), "spruce_curtain": ([], [], [])}
    vs, us, fs = out["spruce_spray"]
    spray_top(vs, us, fs, 0.5, 0.5, 0.03, 0.07, 0.05)
    side_card(vs, us, fs, 0.05, 1, 42, 30, 0.34, 0.34, False)
    side_card(vs, us, fs, 0.09, -1, 38, 34, 0.32, 0.32, True)
    vs, us, fs = out["spruce_curtain"]
    for x, y, yaw, L in ((0.10, 0.07, 18, 0.36), (0.24, -0.06, -14, 0.42), (0.38, 0.04, 10, 0.32)):
        curtain(vs, us, fs, x, y, yaw, L, 0.3)
    return out


def spruce_spray_b():
    out = {"spruce_spray": ([], [], []), "spruce_curtain": ([], [], [])}
    vs, us, fs = out["spruce_spray"]
    spray_top(vs, us, fs, 0.44, 0.46, 0.02, 0.1, 0.06)
    side_card(vs, us, fs, 0.07, -1, 46, 26, 0.3, 0.3, False)
    vs, us, fs = out["spruce_curtain"]
    for x, y, yaw, L in ((0.14, -0.05, -22, 0.40), (0.30, 0.06, 16, 0.36)):
        curtain(vs, us, fs, x, y, yaw, L, 0.28)
    return out


def spruce_spray_c():
    """Brush type (sunlit upper crown): sprays fanned up and out, no curtains."""
    out = {"spruce_spray": ([], [], [])}
    vs, us, fs = out["spruce_spray"]
    spray_top(vs, us, fs, 0.46, 0.44, 0.05, 0.02, 0.02)
    side_card(vs, us, fs, 0.04, 1, 34, 55, 0.32, 0.3, True)
    side_card(vs, us, fs, 0.08, -1, 30, 50, 0.3, 0.3, False)
    return out


def spruce_spray_far():
    out = {"spruce_spray": ([], [], []), "spruce_curtain": ([], [], [])}
    vs, us, fs = out["spruce_spray"]
    card(vs, us, fs, (-0.02, 0, 0), (1, 0, -0.08), (0, 1, 0), 0.5, 0.52)
    vs, us, fs = out["spruce_curtain"]
    curtain(vs, us, fs, 0.2, 0.0, 0, 0.36, 0.34, 0.2)
    return out


def spruce_leader():
    out = {"spruce_leader": ([], [], [])}
    vs, us, fs = out["spruce_leader"]
    for k in range(3):
        a = k / 3 * math.pi
        card(vs, us, fs, (0, 0, -0.04), (0, 0, 1), (math.cos(a), math.sin(a), 0), 0.6, 0.6)
    return out


def larch_main(vs, us, fs, L, W, droop, roll_deg=0.0):
    across = roll((0, 1, 0), (1, 0, 0), roll_deg)
    card(vs, us, fs, (-0.02, 0, 0), (1, 0, 0), across, L, W, 2, 3,
         sag=lambda u, v: Vector((0, 0, 0.02 * math.sin(math.pi * v) - droop * v * v)))


def larch_spray_a():
    out = {"larch_shoot": ([], [], [])}
    vs, us, fs = out["larch_shoot"]
    larch_main(vs, us, fs, 0.5, 0.42, 0.14)
    side_card(vs, us, fs, 0.06, 1, 40, 25, 0.34, 0.32, True)
    curtain(vs, us, fs, 0.22, -0.05, -12, 0.38, 0.3, 0.3)
    return out


def larch_spray_b():
    out = {"larch_shoot": ([], [], [])}
    vs, us, fs = out["larch_shoot"]
    larch_main(vs, us, fs, 0.46, 0.4, 0.1, 15)
    side_card(vs, us, fs, 0.05, 1, 44, 30, 0.3, 0.3, False)
    side_card(vs, us, fs, 0.1, -1, 36, 30, 0.3, 0.3, True)
    return out


def larch_spray_c():
    out = {"larch_shoot": ([], [], [])}
    vs, us, fs = out["larch_shoot"]
    larch_main(vs, us, fs, 0.48, 0.44, 0.2, -20)
    curtain(vs, us, fs, 0.3, 0.05, 15, 0.34, 0.3, 0.35)
    return out


def larch_spray_far():
    out = {"larch_shoot": ([], [], [])}
    vs, us, fs = out["larch_shoot"]
    card(vs, us, fs, (-0.02, 0, 0), (1, 0, -0.15), (0, 1, 0), 0.5, 0.46)
    curtain(vs, us, fs, 0.22, 0.0, 0, 0.34, 0.32, 0.3)
    return out


def larch_leader():
    out = {"larch_shoot": ([], [], [])}
    vs, us, fs = out["larch_shoot"]
    for k in range(2):
        a = k / 2 * math.pi + 0.3
        card(vs, us, fs, (0, 0, -0.04), (0, 0, 1), (math.cos(a), math.sin(a), 0), 0.55, 0.4)
    return out


PARTS = [
    ("spruce_spray_a", spruce_spray_a), ("spruce_spray_b", spruce_spray_b), ("spruce_spray_c", spruce_spray_c),
    ("spruce_spray_far", spruce_spray_far), ("spruce_leader", spruce_leader),
    ("larch_spray_a", larch_spray_a), ("larch_spray_b", larch_spray_b), ("larch_spray_c", larch_spray_c),
    ("larch_spray_far", larch_spray_far), ("larch_leader", larch_leader),
]


def preview_material(cell, out):
    m = bpy.data.materials.new(cell)
    m.use_nodes = True if not m.node_tree else m.use_nodes
    nt = m.node_tree
    bsdf = next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")
    path = os.path.join(out, cell + ".png")
    if os.path.exists(path):
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(path)
        nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        nt.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
        try:
            m.blend_method = "CLIP"
        except Exception:
            pass
    return m


def build_parts(out):
    fresh()
    scene = bpy.context.scene
    mats = {}
    rows = {}
    for name, fn in PARTS:
        tris = 0
        cards = 0
        for cell, (vs, us, fs) in fn().items():
            if not fs:
                continue
            me = bpy.data.meshes.new(f"{name}__{cell}")
            me.from_pydata([tuple(v) for v in vs], [], fs)
            uv = me.uv_layers.new(name="UVMap")
            for poly in me.polygons:
                for li in poly.loop_indices:
                    uv.data[li].uv = us[me.loops[li].vertex_index]
            if cell not in mats:
                mats[cell] = preview_material(cell, out)
            me.materials.append(mats[cell])
            me.update()
            ob = bpy.data.objects.new(f"{name}__{cell}", me)
            scene.collection.objects.link(ob)
            tris += 2 * len(fs)
            # Cards: connected pieces (a curved card is one).
            cards += _islands(fs)
        rows[name] = {"triangles": tris, "cards": cards}
        print(f"tree_parts: part {name}: {tris} triangles, {cards} cards")
    path = os.path.join(out, "tree_parts.glb")
    bpy.ops.export_scene.gltf(
        filepath=path, export_format="GLB", use_selection=False, export_yup=True, export_apply=True,
        export_materials="EXPORT", export_cameras=False, export_lights=False, export_animations=False,
        export_normals=True, export_texcoords=True, export_tangents=False, export_skins=False, export_morph=False)
    return rows, sha(path)


def _islands(faces):
    parent = {}

    def find(x):
        while parent.setdefault(x, x) != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for f in faces:
        for v in f[1:]:
            parent[find(v)] = find(f[0])
    return len({find(f[0]) for f in faces})


def main():
    a = parse()
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    man_path = os.path.join(out, "manifest.json")
    man = {}
    if os.path.exists(man_path):
        with open(man_path) as f:
            man = json.load(f)
    man["generator"] = "tools/blender/trees/build_tree_parts.py"
    man["blender"] = bpy.app.version_string
    if a.only in (None, "textures"):
        man["cells"] = bake_textures(out)
    if a.only in (None, "bark"):
        man["bark"] = bake_bark(out)
    if a.only in (None, "parts"):
        man["parts"], man["glb_sha256"] = build_parts(out)
    with open(man_path, "w") as f:
        json.dump(man, f, indent=2, sort_keys=True)
    print(f"tree_parts: wrote {man_path}")


main()
