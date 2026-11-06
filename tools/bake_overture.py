#!/usr/bin/env python3
"""Bake Overture Maps buildings into per-tile files the game reads (docs/plans/roads.md, P8).

Overture's buildings are OSM's wherever OSM has them, plus machine-learned footprints (Microsoft, Google)
where it has gaps, and machine-learned heights. The game reads, per z14 tile, one JSON file:

    <cache>/overture/<release>/14/<x>/<y>.json

    {"version": 1, "release": "...", "extent": 4096,
     "buildings": [{"id", "rings": [[[x, y], ...] outer, hole...], "height", "min_height", "floors",
                    "class", "subtype", "roof_shape", "roof_color", "facade_color", "ml": bool,
                    "part": bool, "has_parts": bool}]}

Coordinates are tile units (0..4096 across the tile, y down), like a vector tile. A building belongs to the
tile that holds its centroid, so nothing is duplicated. The release is pinned (`--release`), which keeps the
world a pure function of (seed, data version): rerunning for the same release gives the same files.

    python3 tools/bake_overture.py --lat 35.6595 --lon 139.7005 --radius 1
    python3 tools/bake_overture.py --tile 14/14549/6452

Needs `pip install duckdb`. Reads the public S3 bucket without credentials (any AWS_* variables in the
environment are ignored on purpose).
"""
import argparse
import json
import math
import os
import sys
import time

DEFAULT_RELEASE = "2026-10-21.0"
EXTENT = 4096
Z = 14


def lonlat_to_tile(lon, lat, z):
    n = 2 ** z
    x = (lon + 180.0) / 360.0 * n
    lat_r = math.radians(max(-85.0511, min(85.0511, lat)))
    y = (1.0 - math.log(math.tan(lat_r) + 1.0 / math.cos(lat_r)) / math.pi) / 2.0 * n
    return x, y


def tile_bounds(x, y, z):
    n = 2 ** z

    def lon(tx):
        return tx / n * 360.0 - 180.0

    def lat(ty):
        return math.degrees(math.atan(math.sinh(math.pi * (1 - 2 * ty / n))))

    return lon(x), lat(y + 1), lon(x + 1), lat(y)  # west, south, east, north


def to_tile_units(ring, tx, ty, z):
    out = []
    for lon, lat in ring:
        fx, fy = lonlat_to_tile(lon, lat, z)
        out.append([round((fx - tx) * EXTENT, 1), round((fy - ty) * EXTENT, 1)])
    return out


def polygons(geom):
    if geom["type"] == "Polygon":
        return [geom["coordinates"]]
    if geom["type"] == "MultiPolygon":
        return geom["coordinates"]
    return []


def clean(v):
    return None if v is None or (isinstance(v, float) and math.isnan(v)) else v


def query(con, release, theme_type, west, south, east, north):
    base = f"s3://overturemaps-us-west-2/release/{release}/theme=buildings/type={theme_type}/*"
    cols = {
        "building": "id, height, num_floors, class, subtype, roof_shape, roof_color, facade_color, min_height, is_underground, has_parts, sources[1].dataset as dataset",
        "building_part": "id, height, num_floors, roof_shape, roof_color, facade_color, min_height, is_underground, building_id, sources[1].dataset as dataset",
    }[theme_type]
    sql = f"""
        select {cols}, ST_AsGeoJSON(geometry) as gj
        from read_parquet('{base}', hive_partitioning=1)
        where bbox.xmin <= {east} and bbox.xmax >= {west} and bbox.ymin <= {north} and bbox.ymax >= {south}
    """
    cur = con.execute(sql)
    names = [d[0] for d in cur.description]
    for row in cur.fetchall():
        yield dict(zip(names, row))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tile", help="z14 tile as 14/x/y")
    ap.add_argument("--lat", type=float)
    ap.add_argument("--lon", type=float)
    ap.add_argument("--radius", type=int, default=0, help="tiles around the centre tile")
    ap.add_argument("--release", default=DEFAULT_RELEASE)
    ap.add_argument("--cache", default=os.environ.get("EW_CACHE", ".earthdrop-cache"))
    args = ap.parse_args()

    if args.tile:
        _, tx, ty = [int(v) for v in args.tile.split("/")]
    elif args.lat is not None and args.lon is not None:
        fx, fy = lonlat_to_tile(args.lon, args.lat, Z)
        tx, ty = int(fx), int(fy)
    else:
        ap.error("give --tile or --lat/--lon")
    r = args.radius
    tiles = [(x, y) for y in range(ty - r, ty + r + 1) for x in range(tx - r, tx + r + 1)]

    for k in list(os.environ):
        if k.startswith("AWS_"):
            os.environ.pop(k)
    import duckdb

    con = duckdb.connect()
    con.execute("INSTALL httpfs; LOAD httpfs; INSTALL spatial; LOAD spatial;")
    con.execute("SET s3_region='us-west-2'; SET s3_access_key_id=''; SET s3_secret_access_key='';")

    west = min(tile_bounds(x, y, Z)[0] for x, y in tiles)
    south = min(tile_bounds(x, y, Z)[1] for x, y in tiles)
    east = max(tile_bounds(x, y, Z)[2] for x, y in tiles)
    north = max(tile_bounds(x, y, Z)[3] for x, y in tiles)
    wanted = set(tiles)
    per_tile = {t: [] for t in tiles}
    t0 = time.time()
    counts = {"building": 0, "building_part": 0}
    for kind in ("building", "building_part"):
        for row in query(con, args.release, kind, west, south, east, north):
            geom = json.loads(row["gj"])
            for poly in polygons(geom):
                outer = poly[0]
                cx = sum(p[0] for p in outer) / len(outer)
                cy = sum(p[1] for p in outer) / len(outer)
                fx, fy = lonlat_to_tile(cx, cy, Z)
                t = (int(fx), int(fy))
                if t not in wanted:
                    continue
                ds = (row.get("dataset") or "")
                height = clean(row.get("height"))
                floors = clean(row.get("num_floors"))
                if row.get("is_underground"):
                    continue
                per_tile[t].append(
                    {
                        "id": row["id"],
                        "rings": [to_tile_units(ring, t[0], t[1], Z) for ring in poly],
                        "height": height,
                        "min_height": clean(row.get("min_height")),
                        "floors": floors,
                        "class": row.get("class"),
                        "subtype": row.get("subtype"),
                        "roof_shape": row.get("roof_shape"),
                        "roof_color": row.get("roof_color"),
                        "facade_color": row.get("facade_color"),
                        "ml": ("Microsoft" in ds or "Google" in ds or "ML" in ds),
                        "part": kind == "building_part",
                        "has_parts": bool(row.get("has_parts")) if kind == "building" else False,
                    }
                )
                counts[kind] += 1
    for (x, y), items in per_tile.items():
        path = os.path.join(args.cache, "overture", args.release, str(Z), str(x), f"{y}.json")
        os.makedirs(os.path.dirname(path), exist_ok=True)
        items.sort(key=lambda b: b["id"])  # deterministic file content
        with open(path, "w") as f:
            json.dump({"version": 1, "release": args.release, "extent": EXTENT, "buildings": items}, f, separators=(",", ":"))
        print(f"{path}: {len(items)} features", file=sys.stderr)
    print(f"{counts['building']} buildings, {counts['building_part']} parts in {time.time() - t0:.1f}s", file=sys.stderr)


if __name__ == "__main__":
    main()
