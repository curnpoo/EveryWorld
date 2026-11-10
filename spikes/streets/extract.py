"""W4e: extract Overture transportation segments for the 3x3 block of zoom-14 tiles around a
point, as synthetic per-tile road features in tile units, for the roads v2 solver.
Needs `pip install duckdb`. Usage: python extract.py LAT LON OUT.json [RELEASE]"""
import json, math, sys, time, os
import duckdb

lat, lon, out = float(sys.argv[1]), float(sys.argv[2]), sys.argv[3]
release = sys.argv[4] if len(sys.argv) > 4 else "2026-10-21.0"
Z, EXT = 14, 4096
n = 2 ** Z
def fx(lo): return (lo + 180.0) / 360.0 * n
def fy(la): return (1.0 - math.asinh(math.tan(math.radians(la))) / math.pi) / 2.0 * n
def lon_of(x): return x / n * 360.0 - 180.0
def lat_of(y): return math.degrees(math.atan(math.sinh(math.pi * (1 - 2 * y / n))))
tx, ty = int(fx(lon)), int(fy(lat))
w, e, s, nn = lon_of(tx - 1), lon_of(tx + 2), lat_of(ty + 2), lat_of(ty - 1)
for k in [k for k in os.environ if k.startswith("AWS_")]: del os.environ[k]
con = duckdb.connect(); con.execute("INSTALL spatial; LOAD spatial; INSTALL httpfs; LOAD httpfs; SET s3_region='us-west-2';")
src = f"read_parquet('s3://overturemaps-us-west-2/release/{release}/theme=transportation/type=segment/*', hive_partitioning=1)"
t0 = time.time()
cols = [r[0] for r in con.execute(f"DESCRIBE SELECT * FROM {src}").fetchall()]
print("columns:", cols)
want = [c for c in ["road_surface", "road_flags", "level_rules", "speed_limits", "width_rules", "access_restrictions", "names", "routes", "destinations", "prohibited_transitions", "subclass", "subclass_rules"] if c in cols]
q = f"""SELECT id, subtype, class, ST_AsGeoJSON(geometry) g, {', '.join(f'to_json({c}) AS {c}' for c in want)}
        FROM {src} WHERE bbox.xmin < {e} AND bbox.xmax > {w} AND bbox.ymin < {nn} AND bbox.ymax > {s}"""
rows = con.execute(q).fetchall()
print(f"{len(rows)} segments in {time.time()-t0:.1f} s")
names = ["id", "subtype", "class", "g"] + want
feats, have = [], {c: 0 for c in want}
length_km = 0.0
for r in rows:
    d = dict(zip(names, r))
    if d["subtype"] != "road": continue
    g = json.loads(d["g"])
    if g["type"] != "LineString": continue
    for c in want:
        d[c] = json.loads(d[c]) if d[c] else None
        have[c] += d[c] not in (None, [], {})
    pts = g["coordinates"]
    for a, b in zip(pts, pts[1:]):
        length_km += 111.32 * math.hypot((a[0] - b[0]) * math.cos(math.radians(lat)), a[1] - b[1])
    flags = {f for rule in (d.get("road_flags") or []) for f in rule.get("values", [])}
    level = next((r_.get("value") for r_ in (d.get("level_rules") or []) if r_.get("value")), 0)
    surf = next((r_.get("value") for r_ in (d.get("road_surface") or [])), None)
    feats.append({"id": d["id"], "class": d["class"], "bridge": "is_bridge" in flags, "tunnel": "is_tunnel" in flags,
                  "link": "is_link" in flags, "level": level, "surface": surf,
                  "name": ((d.get("names") or {}).get("primary")),
                  "speed": d.get("speed_limits"), "width": d.get("width_rules"),
                  "ll": pts})
json.dump({"release": release, "tile": [Z, tx, ty], "extent": EXT, "features": feats}, open(out, "w"))
roads = len(feats)
area = (111.32 * (nn - s)) * (111.32 * (e - w) * math.cos(math.radians(lat)))
print(f"{roads} road segments, {length_km:.0f} km of centreline over {area:.1f} km2; file {os.path.getsize(out)/1024:.0f} KiB ({os.path.getsize(out)/1024/area:.0f} KiB per km2 as JSON)")
print("non-empty attribute counts:", {c: f"{v} ({100*v//max(roads,1)}%)" for c, v in have.items()})
from collections import Counter
print("classes:", Counter(f["class"] for f in feats).most_common())
print("bridges", sum(f["bridge"] for f in feats), "tunnels", sum(f["tunnel"] for f in feats), "non-zero level", sum(1 for f in feats if f["level"]))
