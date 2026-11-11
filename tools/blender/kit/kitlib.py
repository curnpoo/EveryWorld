"""Shared helpers for the solarpunk building kit generators (Blender 5.x).

Conventions (fixed by the kit design, see build_kit.py / kit.toml):
- metres, Z-up in Blender, glTF exported +Y up.
- wall modules: x in [-1, 1], z in [0, 3], outer face on y = 0, interior +Y.
- every primitive is a closed shell; shells are not welded to each other.
- UVs are box-projected in metres (1 UV unit = 1 m).
"""

import hashlib
import math
import os

import bmesh
import bpy
from mathutils import Vector

# ----------------------------------------------------------------------------
# Materials: one table, PBR factors only (no textures yet).
# colour is linear RGB.
# ----------------------------------------------------------------------------
MATERIALS = {
    "timber_cladding": dict(color=(0.50, 0.27, 0.11), metallic=0.0, roughness=0.75),
    "timber_structure": dict(color=(0.20, 0.10, 0.045), metallic=0.0, roughness=0.70),
    "glass": dict(color=(0.70, 0.86, 0.90), metallic=0.0, roughness=0.05,
                  transmission=1.0, ior=1.5),
    "metal_dark": dict(color=(0.035, 0.050, 0.045), metallic=1.0, roughness=0.45),
    "stone": dict(color=(0.36, 0.34, 0.30), metallic=0.0, roughness=0.90),
    "pv": dict(color=(0.012, 0.030, 0.085), metallic=0.3, roughness=0.15),
    "sedum": dict(color=(0.16, 0.26, 0.07), metallic=0.0, roughness=0.95),
    "soil": dict(color=(0.085, 0.050, 0.028), metallic=0.0, roughness=1.0),
}

SOCKET_KINDS = ("planter", "vine", "pv", "light")


def get_material(name):
    if name not in MATERIALS:
        raise KeyError(f"material '{name}' is not in the kit material table")
    mat = bpy.data.materials.get(name)
    if mat is not None:
        return mat
    d = MATERIALS[name]
    mat = bpy.data.materials.new(name)
    try:
        mat.use_nodes = True
    except Exception:
        pass
    nt = mat.node_tree
    bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        nt.nodes.clear()
        out = nt.nodes.new("ShaderNodeOutputMaterial")
        bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
        nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    r, g, b = d["color"]
    bsdf.inputs["Base Color"].default_value = (r, g, b, 1.0)
    bsdf.inputs["Metallic"].default_value = d["metallic"]
    bsdf.inputs["Roughness"].default_value = d["roughness"]
    if "transmission" in d:
        bsdf.inputs["Transmission Weight"].default_value = d["transmission"]
        bsdf.inputs["IOR"].default_value = d["ior"]
    mat.diffuse_color = (r, g, b, 1.0)
    mat.metallic = d["metallic"]
    mat.roughness = d["roughness"]
    return mat


# ----------------------------------------------------------------------------
# Part builder
# ----------------------------------------------------------------------------
class Part:
    """Accumulates closed shells into one mesh, plus sockets and metadata."""

    def __init__(self, name, category, expect, size, origin, notes=""):
        self.name = name
        self.category = category          # key into the triangle budget table
        self.expect = expect              # ((x0,y0,z0),(x1,y1,z1)) convention bbox
        self.size = size                  # nominal module size (x, y, z)
        self.origin = origin              # origin convention, text
        self.notes = notes
        self.bm = bmesh.new()
        self.mats = []
        self.sockets = []                 # (name, kind, pos, normal)
        self._socket_n = {}

    # -- low level ----------------------------------------------------------
    def _mi(self, mat):
        if mat not in MATERIALS:
            raise KeyError(f"{self.name}: material '{mat}' not in table")
        if mat not in self.mats:
            self.mats.append(mat)
        return self.mats.index(mat)

    def _face(self, verts, mi):
        f = self.bm.faces.new(verts)
        f.material_index = mi
        return f

    def hexa(self, pts, mat):
        """8 points: bottom quad then the matching top quad."""
        mi = self._mi(mat)
        v = [self.bm.verts.new(p) for p in pts]
        for idx in ((0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4),
                    (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)):
            self._face([v[i] for i in idx], mi)

    def box(self, x0, x1, y0, y1, z0, z1, mat):
        if not (x1 - x0 > 1e-5 and y1 - y0 > 1e-5 and z1 - z0 > 1e-5):
            raise ValueError(f"{self.name}: degenerate box {(x0, x1, y0, y1, z0, z1)}")
        self.hexa([(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
                   (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], mat)

    def shear_box(self, x0, x1, y0, y1, o0, o1, slope, mat):
        """Box whose bottom/top follow z = y * slope + offset (vertical ends)."""
        if not (x1 - x0 > 1e-5 and y1 - y0 > 1e-5 and o1 - o0 > 1e-5):
            raise ValueError(f"{self.name}: degenerate shear box")
        za, zb = y0 * slope, y1 * slope
        self.hexa([(x0, y0, za + o0), (x1, y0, za + o0), (x1, y1, zb + o0), (x0, y1, zb + o0),
                   (x0, y0, za + o1), (x1, y0, za + o1), (x1, y1, zb + o1), (x0, y1, zb + o1)],
                  mat)

    def prism(self, poly, axis, a0, a1, mat):
        """Convex 2D polygon extruded along an axis.
        axis 'x': poly is (y, z); axis 'z': poly is (x, y)."""
        mi = self._mi(mat)

        def pt(u, v, a):
            return (a, u, v) if axis == "x" else (u, v, a)

        lo = [self.bm.verts.new(pt(u, v, a0)) for u, v in poly]
        hi = [self.bm.verts.new(pt(u, v, a1)) for u, v in poly]
        n = len(poly)
        self._face(lo[::-1], mi)
        self._face(hi, mi)
        for i in range(n):
            j = (i + 1) % n
            self._face([lo[i], lo[j], hi[j], hi[i]], mi)

    def octa_post(self, x0, x1, y0, y1, z0, z1, c, mat):
        """Square post with chamfered vertical edges."""
        poly = [(x0 + c, y0), (x1 - c, y0), (x1, y0 + c), (x1, y1 - c),
                (x1 - c, y1), (x0 + c, y1), (x0, y1 - c), (x0, y0 + c)]
        self.prism(poly, "z", z0, z1, mat)

    def grid_solid(self, xs, zs, y0, y1, mat, absent=()):
        """Closed solid made of the cells of an x/z grid extruded y0..y1,
        minus the `absent` cells (i, j). Used for walls with openings and
        for frames. Vertices are shared, so the result is one manifold shell."""
        mi = self._mi(mat)
        nx, nz = len(xs) - 1, len(zs) - 1
        absent = set(absent)
        cache = {}

        def pres(i, j):
            return 0 <= i < nx and 0 <= j < nz and (i, j) not in absent

        def v(i, j, back):
            k = (i, j, back)
            if k not in cache:
                cache[k] = self.bm.verts.new((xs[i], y1 if back else y0, zs[j]))
            return cache[k]

        for i in range(nx):
            for j in range(nz):
                if not pres(i, j):
                    continue
                self._face([v(i, j, 0), v(i + 1, j, 0), v(i + 1, j + 1, 0), v(i, j + 1, 0)], mi)
                self._face([v(i, j, 1), v(i, j + 1, 1), v(i + 1, j + 1, 1), v(i + 1, j, 1)], mi)
                if not pres(i - 1, j):
                    self._face([v(i, j, 0), v(i, j + 1, 0), v(i, j + 1, 1), v(i, j, 1)], mi)
                if not pres(i + 1, j):
                    self._face([v(i + 1, j, 0), v(i + 1, j, 1), v(i + 1, j + 1, 1), v(i + 1, j + 1, 0)], mi)
                if not pres(i, j - 1):
                    self._face([v(i, j, 0), v(i, j, 1), v(i + 1, j, 1), v(i + 1, j, 0)], mi)
                if not pres(i, j + 1):
                    self._face([v(i, j + 1, 0), v(i + 1, j + 1, 0), v(i + 1, j + 1, 1), v(i, j + 1, 1)], mi)

    def ring_stack(self, x0, x1, y0, y1, rings, mat):
        """Rectangular rings (z, inset) joined bottom to top, capped. One shell."""
        mi = self._mi(mat)
        loops = []
        for z, i in rings:
            loops.append([self.bm.verts.new(p) for p in (
                (x0 + i, y0 + i, z), (x1 - i, y0 + i, z), (x1 - i, y1 - i, z), (x0 + i, y1 - i, z))])
        self._face(loops[0][::-1], mi)
        self._face(loops[-1], mi)
        for a, b in zip(loops, loops[1:]):
            for k in range(4):
                m = (k + 1) % 4
                self._face([a[k], a[m], b[m], b[k]], mi)

    def layer_stack(self, x0, x1, y0, y1, layers):
        """Stacked slabs [(z0, z1, mat), ...] sharing one footprint, as ONE shell:
        each band of the side takes its layer's material, no internal faces."""
        zs = [layers[0][0]] + [l[1] for l in layers]
        loops = [[self.bm.verts.new(p) for p in ((x0, y0, z), (x1, y0, z), (x1, y1, z), (x0, y1, z))]
                 for z in zs]
        self._face(loops[0][::-1], self._mi(layers[0][2]))
        self._face(loops[-1], self._mi(layers[-1][2]))
        for (a, b), layer in zip(zip(loops, loops[1:]), layers):
            mi = self._mi(layer[2])
            for k in range(4):
                m = (k + 1) % 4
                self._face([a[k], a[m], b[m], b[k]], mi)

    def socket(self, kind, pos, normal=(0.0, 0.0, 1.0)):
        if kind not in SOCKET_KINDS:
            raise KeyError(f"{self.name}: socket kind '{kind}'")
        n = self._socket_n.get(kind, 0)
        self._socket_n[kind] = n + 1
        self.sockets.append((f"SOCKET_{kind}_{n}", kind, tuple(pos), tuple(normal)))

    # -- finish -------------------------------------------------------------
    def finalize(self, scene):
        """Clean, UV, validate-measure and create the Blender objects.
        Returns (root object, report dict)."""
        bm = self.bm
        bmesh.ops.triangulate(bm, faces=bm.faces[:], quad_method="FIXED", ngon_method="EAR_CLIP")
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
        bm.verts.ensure_lookup_table()
        bm.faces.ensure_lookup_table()

        # outward check per shell (signed volume); flip any inverted shell
        seen = set()
        shells = 0
        flipped = 0
        for f0 in bm.faces:
            if f0.index in seen:
                continue
            stack, island = [f0], []
            seen.add(f0.index)
            while stack:
                f = stack.pop()
                island.append(f)
                for vv in f.verts:
                    for g in vv.link_faces:
                        if g.index not in seen:
                            seen.add(g.index)
                            stack.append(g)
            vol = 0.0
            for f in island:
                a, b, c = (vv.co for vv in f.verts)
                vol += a.dot(b.cross(c)) / 6.0
            shells += 1
            if vol < 0.0:
                flipped += 1
                for f in island:
                    f.normal_flip()
        bm.normal_update()

        uv = bm.loops.layers.uv.new("UVMap")
        zero_area = 0
        dup = 0
        keys = set()
        for f in bm.faces:
            f.smooth = False
            if f.calc_area() < 1e-8:
                zero_area += 1
            k = tuple(sorted((round(v.co.x, 5), round(v.co.y, 5), round(v.co.z, 5)) for v in f.verts))
            if k in keys:
                dup += 1
            keys.add(k)
            n = f.normal
            ax, ay, az = abs(n.x), abs(n.y), abs(n.z)
            for lp in f.loops:
                co = lp.vert.co
                if az >= ax and az >= ay:
                    lp[uv].uv = (co.x, co.y)
                elif ay >= ax:
                    lp[uv].uv = (co.x, co.z)
                else:
                    lp[uv].uv = (co.y, co.z)

        nonmanifold = sum(1 for e in bm.edges if not e.is_manifold)
        loose = sum(1 for v in bm.verts if not v.link_faces)
        lo = Vector((min(v.co.x for v in bm.verts), min(v.co.y for v in bm.verts),
                     min(v.co.z for v in bm.verts)))
        hi = Vector((max(v.co.x for v in bm.verts), max(v.co.y for v in bm.verts),
                     max(v.co.z for v in bm.verts)))
        used = sorted({self.mats[f.material_index] for f in bm.faces})
        tris = len(bm.faces)

        me = bpy.data.meshes.new(self.name)
        bm.to_mesh(me)
        bm.free()
        for m in self.mats:
            me.materials.append(get_material(m))
        obj = bpy.data.objects.new(self.name, me)
        scene.collection.objects.link(obj)

        socks = []
        for sname, kind, pos, normal in sorted(self.sockets):
            e = bpy.data.objects.new(sname, None)
            e.empty_display_type = "ARROWS"
            e.empty_display_size = 0.15
            e.parent = obj
            e.location = pos
            e.rotation_euler = Vector(normal).normalized().to_track_quat("Z", "Y").to_euler()
            e["socket_kind"] = kind
            scene.collection.objects.link(e)
            socks.append(dict(name=sname, kind=kind, position=pos, normal=normal))

        report = dict(tris=tris, bbox_min=tuple(lo), bbox_max=tuple(hi), shells=shells,
                      flipped_shells=flipped, nonmanifold_edges=nonmanifold, loose_verts=loose,
                      zero_area_faces=zero_area, duplicate_faces=dup, materials=used,
                      uv_layers=len(me.uv_layers), sockets=socks,
                      unused_slots=sorted(set(self.mats) - set(used)))
        return obj, report


# ----------------------------------------------------------------------------
# Validation
# ----------------------------------------------------------------------------
TRI_BUDGET = {
    "wall_plain": 300, "wall_opening": 1500, "wall_glass": 1200, "roof": 600,
    "roof_solar": 1500, "balcony": 2500, "other": 800,
}


def validate(part, report, closed=True, tol=1e-3):
    """Returns (errors, warnings). Errors fail the build."""
    err, warn = [], []
    for axis in range(3):
        for which, got, want in (("min", report["bbox_min"][axis], part.expect[0][axis]),
                                 ("max", report["bbox_max"][axis], part.expect[1][axis])):
            if abs(got - want) > tol:
                err.append(f"bbox {'xyz'[axis]} {which}: got {got:.4f}, convention {want:.4f}")
    budget = TRI_BUDGET[part.category]
    if report["tris"] > budget:
        err.append(f"tris {report['tris']} > budget {budget}")
    if report["nonmanifold_edges"]:
        msg = f"{report['nonmanifold_edges']} non-manifold edges"
        (err if closed else warn).append(msg)
    if report["loose_verts"]:
        err.append(f"{report['loose_verts']} loose verts")
    if report["zero_area_faces"]:
        err.append(f"{report['zero_area_faces']} zero-area faces")
    if report["duplicate_faces"]:
        err.append(f"{report['duplicate_faces']} duplicate faces")
    if report["flipped_shells"]:
        warn.append(f"{report['flipped_shells']} shells had inverted normals (fixed)")
    for m in report["materials"]:
        if m not in MATERIALS:
            err.append(f"material '{m}' not in table")
    if report["unused_slots"]:
        err.append(f"unused material slots {report['unused_slots']}")
    if report["uv_layers"] < 1:
        err.append("no UV layer")
    return err, warn


# ----------------------------------------------------------------------------
# Export
# ----------------------------------------------------------------------------
def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def export_glb(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=path, export_format="GLB", use_selection=False, export_yup=True,
        export_apply=True, export_extras=True, export_materials="EXPORT",
        export_cameras=False, export_lights=False, export_animations=False,
        export_normals=True, export_texcoords=True, export_tangents=False,
        export_skins=False, export_morph=False)
    return sha256_file(path)


# ----------------------------------------------------------------------------
# Preview rendering (never affects the exported file: call after export)
# ----------------------------------------------------------------------------
def _preview_material(name, color, alpha=1.0):
    mat = bpy.data.materials.new(name)
    try:
        mat.use_nodes = True
    except Exception:
        pass
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is not None:
        bsdf.inputs["Base Color"].default_value = (*color, 1.0)
        bsdf.inputs["Roughness"].default_value = 0.9
        bsdf.inputs["Alpha"].default_value = alpha
    mat.diffuse_color = (*color, alpha)
    return mat


def _box_object(scene, name, lo, hi, mat):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co = Vector((lo[i] + (v.co[i] + 0.5) * (hi[i] - lo[i]) for i in range(3)))
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    me.materials.append(mat)
    ob = bpy.data.objects.new(name, me)
    scene.collection.objects.link(ob)
    return ob


def soften_glass_for_preview():
    """Glass is a transmission material in the file; for the preview make it a
    tinted alpha blend so the frame and interior read without ray tracing."""
    for mat in bpy.data.materials:
        if not mat.name.startswith("glass") or mat.node_tree is None:
            continue
        bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
        if bsdf is None:
            continue
        bsdf.inputs["Transmission Weight"].default_value = 0.0
        bsdf.inputs["Base Color"].default_value = (0.30, 0.50, 0.58, 1.0)
        bsdf.inputs["Alpha"].default_value = 0.45
        bsdf.inputs["Roughness"].default_value = 0.08
        try:
            mat.surface_render_method = "BLENDED"
        except Exception:
            pass


def render_preview(scene, path, lo, hi, azimuth_deg=35.0, elevation_deg=20.0, res=768,
                   human_at=None, ground_z=None, ground_size=60.0, lens=60.0):
    """3/4 view of the box lo..hi. azimuth 0 looks at the -Y (outer) face from
    the front; positive swings the camera toward +X."""
    lo, hi = Vector(lo), Vector(hi)
    soften_glass_for_preview()
    if human_at is not None:
        hx, hy, hz = human_at
        h_lo, h_hi = (hx - 0.225, hy - 0.125, hz), (hx + 0.225, hy + 0.125, hz + 1.8)
        _box_object(scene, "_scale_human_1p8m", h_lo, h_hi,
                    _preview_material("_scale_ref", (0.85, 0.30, 0.25)))
        lo = Vector((min(lo[i], h_lo[i]) for i in range(3)))
        hi = Vector((max(hi[i], h_hi[i]) for i in range(3)))
    if ground_z is not None:
        g = ground_size
        _box_object(scene, "_ground", (-g, -g, ground_z - 0.2), (g, g, ground_z),
                    _preview_material("_ground", (0.22, 0.30, 0.14)))

    centre = (lo + hi) * 0.5
    radius = (hi - lo).length * 0.5
    az, el = math.radians(azimuth_deg), math.radians(elevation_deg)
    d = Vector((math.sin(az) * math.cos(el), -math.cos(az) * math.cos(el), math.sin(el)))
    cam_data = bpy.data.cameras.new("_cam")
    cam_data.lens = lens
    cam_data.sensor_width = 36.0
    fov = 2.0 * math.atan(18.0 / lens)
    dist = radius / math.sin(fov * 0.5) * 1.04
    cam_data.clip_start = 0.05
    cam_data.clip_end = dist + radius * 4.0 + 200.0
    cam = bpy.data.objects.new("_cam", cam_data)
    cam.location = centre + d * dist
    cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    scene.collection.objects.link(cam)
    scene.camera = cam

    sun_data = bpy.data.lights.new("_sun", "SUN")
    sun_data.energy = 2.4
    sun_data.angle = math.radians(3.0)
    sun = bpy.data.objects.new("_sun", sun_data)
    saz = az - math.radians(55.0)
    sdir = Vector((math.sin(saz) * 0.75, -math.cos(saz) * 0.75, 0.9)).normalized()
    sun.rotation_euler = sdir.to_track_quat("Z", "Y").to_euler()
    scene.collection.objects.link(sun)

    world = bpy.data.worlds.new("_world")
    try:
        world.use_nodes = True
    except Exception:
        pass
    bg = next((n for n in world.node_tree.nodes if n.type == "BACKGROUND"), None)
    if bg is not None:
        bg.inputs["Color"].default_value = (0.62, 0.68, 0.74, 1.0)
        bg.inputs["Strength"].default_value = 0.45
    scene.world = world

    engine = None
    for name in ("BLENDER_EEVEE", "BLENDER_EEVEE_NEXT", "BLENDER_WORKBENCH"):
        try:
            scene.render.engine = name
            engine = name
            break
        except TypeError:
            continue
    try:
        scene.eevee.taa_render_samples = 32
    except Exception:
        pass
    scene.view_settings.view_transform = "Standard"
    scene.render.resolution_x = res
    scene.render.resolution_y = res
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = False
    scene.render.image_settings.file_format = "PNG"
    scene.render.filepath = path
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.render.render(write_still=True)
    return engine
