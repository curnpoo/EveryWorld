"""W4c: read a .glb and report what a skinned-character importer needs.
Usage: python3 inspect_glb.py path/to/file.glb"""
import json, struct, sys, re
path = sys.argv[1]
data = open(path, "rb").read()
magic, ver, total = struct.unpack_from("<4sII", data, 0)
assert magic == b"glTF", "not a GLB"
off, chunks = 12, []
while off < total:
    n, kind = struct.unpack_from("<I4s", data, off)
    chunks.append((kind, data[off + 8: off + 8 + n])); off += 8 + n
g = json.loads(chunks[0][1])
bin_ = chunks[1][1] if len(chunks) > 1 else b""
print(f"file {len(data)/2**20:.1f} MiB, glTF {g['asset'].get('version')}, generator {g['asset'].get('generator')}")
print("extensions used:", g.get("extensionsUsed", []), "required:", g.get("extensionsRequired", []))
nodes, meshes, acc = g.get("nodes", []), g.get("meshes", []), g.get("accessors", [])
tris = verts = 0; attrs = set(); targets = 0
for m in meshes:
    for p in m["primitives"]:
        attrs |= set(p["attributes"]); verts += acc[p["attributes"]["POSITION"]]["count"]
        tris += acc[p["indices"]]["count"] // 3 if "indices" in p else acc[p["attributes"]["POSITION"]]["count"] // 3
        targets = max(targets, len(p.get("targets", [])))
print(f"meshes {len(meshes)}, primitives {sum(len(m['primitives']) for m in meshes)}, vertices {verts}, triangles {tris}")
print("vertex attributes:", sorted(attrs), "| morph targets:", targets)
for i, s in enumerate(g.get("skins", [])):
    names = [nodes[j].get("name", f"node{j}") for j in s["joints"]]
    print(f"skin {i}: {len(names)} joints, inverse bind matrices: {'inverseBindMatrices' in s}")
    print("  joints:", ", ".join(names))
    def side(n):
        m = re.search(r"[._ ]([LR])$|^(left|right)|[._ ](left|right)$", n, re.I)
        return None if not m else ("left" if (m.group(1) or m.group(2) or m.group(3)).lower().startswith("l") else "right")
    PARTS = {"UpperArm": r"upper.?arm", "LowerArm": r"fore.?arm|lower.?arm", "Hand": r"hand", "UpperLeg": r"thigh|up.?leg|upper.?leg", "LowerLeg": r"shin|calf|lower.?leg", "Foot": r"foot"}
    CENTRE = {"hips": r"hip|pelvis|^spine$|spine[._]?0*1$", "spine": r"spine[._]?0*2$|^spine1$", "chest": r"chest|spine[._]?0*3$", "neck": r"neck", "head": r"head"}
    HUMANOID = {**CENTRE, **{sd + k: (sd, rx) for sd in ("left", "right") for k, rx in PARTS.items()}}
    def match(rule):
        if isinstance(rule, tuple): return next((n for n in names if side(n) == rule[0] and re.search(rule[1], n, re.I)), None)
        return next((n for n in names if side(n) is None and re.search(rule, n, re.I)), None)
    found = {k: match(rx) for k, rx in HUMANOID.items()}
    miss = [k for k, v in found.items() if not v]
    print(f"  humanoid bones matched by name: {len(found)-len(miss)}/{len(found)}; missing: {miss}")
    for k, v in found.items(): print(f"    {k:14s} <- {v}")
    parents = {c: i for i, n in enumerate(nodes) for c in n.get("children", [])}
    for j in s["joints"]:
        par = parents.get(j); print(f"    {nodes[j].get('name'):14s} parent {nodes[par].get('name') if par is not None else None}")
if "JOINTS_0" in attrs:
    # Max influences actually used, and weight normalisation, from the first skinned primitive.
    p = next(p for m in meshes for p in m["primitives"] if "WEIGHTS_0" in p["attributes"])
    a = acc[p["attributes"]["WEIGHTS_0"]]; bv = g["bufferViews"][a["bufferView"]]
    comp = {5126: ("f", 4, 1.0), 5121: ("B", 1, 255.0), 5123: ("H", 2, 65535.0)}[a["componentType"]]
    stride = bv.get("byteStride", comp[1] * 4); base = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    used = [0] * 5; bad = 0
    for i in range(a["count"]):
        w = [x / comp[2] for x in struct.unpack_from("<4" + comp[0], bin_, base + i * stride)]
        used[sum(1 for x in w if x > 1e-4)] += 1; bad += abs(sum(w) - 1) > 1e-2
    print(f"influences per vertex (0..4): {used}; weights not summing to 1: {bad}; second set (JOINTS_1): {'JOINTS_1' in attrs}")
for i, m in enumerate(g.get("materials", [])):
    pbr = m.get("pbrMetallicRoughness", {})
    print(f"material {i} '{m.get('name')}': baseColorTex {'baseColorTexture' in pbr}, metalRoughTex {'metallicRoughnessTexture' in pbr}, normal {'normalTexture' in m}, emissive {'emissiveTexture' in m}, alpha {m.get('alphaMode','OPAQUE')}, doubleSided {m.get('doubleSided', False)}, ext {list(m.get('extensions', {}))}")
for i, im in enumerate(g.get("images", [])):
    bv = g["bufferViews"][im["bufferView"]] if "bufferView" in im else None
    dims = ""
    if bv:
        b = bin_[bv.get("byteOffset", 0): bv.get("byteOffset", 0) + 32]
        if b[:8] == b"\x89PNG\r\n\x1a\n": dims = "%dx%d" % struct.unpack(">II", b[16:24])
    print(f"image {i}: {im.get('mimeType')} {dims} {bv['byteLength']//1024 if bv else '?'} KiB")
an = g.get("animations", [])
print(f"animations: {len(an)}", [a.get("name") for a in an][:20])
