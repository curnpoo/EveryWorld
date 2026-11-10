"""Write positions and indices of a .glb's first primitive, or a generated height grid, as a
raw file for clusterlod_spike: u32 vertex_count, u32 index_count, f32 xyz..., u32 indices...
Usage: export_mesh.py in.glb out.bin | export_mesh.py --grid N out.bin"""
import json, struct, sys, math
if sys.argv[1] == "--grid":
    n = int(sys.argv[2]); pos = []; idx = []
    for z in range(n + 1):
        for x in range(n + 1):
            pos += [x * 0.5, 6 * math.sin(x * 0.05) * math.cos(z * 0.04) + 0.8 * math.sin(x * 0.7 + z * 0.3), z * 0.5]
    for z in range(n):
        for x in range(n):
            a = z * (n + 1) + x; idx += [a, a + n + 1, a + 1, a + 1, a + n + 1, a + n + 2]
    out = sys.argv[3]
elif sys.argv[1].endswith(".obj"):
    pos = []; idx = []
    for line in open(sys.argv[1]):
        t = line.split()
        if t[:1] == ["v"]: pos += [float(x) for x in t[1:4]]
        elif t[:1] == ["f"]:
            f = [int(x.split("/")[0]) - 1 for x in t[1:]]
            for i in range(1, len(f) - 1): idx += [f[0], f[i], f[i + 1]]
    out = sys.argv[2]
else:
    d = open(sys.argv[1], "rb").read(); n0 = struct.unpack_from("<I", d, 12)[0]
    g = json.loads(d[20:20 + n0]); b = d[20 + n0 + 8:]
    p = g["meshes"][0]["primitives"][0]
    def view(i):
        a = g["accessors"][i]; bv = g["bufferViews"][a["bufferView"]]
        return a, bv.get("byteOffset", 0) + a.get("byteOffset", 0), bv.get("byteStride")
    a, o, st = view(p["attributes"]["POSITION"]); st = st or 12
    pos = [v for i in range(a["count"]) for v in struct.unpack_from("<3f", b, o + i * st)]
    a, o, _ = view(p["indices"]); fmt = {5123: "H", 5125: "I", 5121: "B"}[a["componentType"]]
    idx = list(struct.unpack_from(f"<{a['count']}{fmt}", b, o)); out = sys.argv[2]
open(out, "wb").write(struct.pack("<II", len(pos) // 3, len(idx)) + struct.pack(f"<{len(pos)}f", *pos) + struct.pack(f"<{len(idx)}I", *idx))
print(out, len(pos) // 3, "vertices", len(idx) // 3, "triangles")
