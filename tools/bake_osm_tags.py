#!/usr/bin/env python3
"""Bake OpenStreetMap road tags (lanes, width, sidewalks, bike lanes, speed limits) per z14 tile.

The vector tiles the game draws from carry no lane counts, widths, sidewalks, cycle lanes or speed limits. OSM
has them. This tool asks Geofabrik's Postpass (a public PostGIS over OSM) for the highways in each tile and writes

    <cache>/osmtags/v1/14/<x>/<y>.json

    {"version": 1, "baked": "2026-11-06", "extent": 4096,
     "ways": [{"id": 123, "highway": "residential", "tags": {"lanes": "2", ...}, "lines": [[[x, y], ...], ...]}]}

Coordinates are tile units (0..4096, y down), not clipped to the tile. Only the tags the renderer reads are kept.
A tile's file is a snapshot (the date is inside), so the world stays a pure function of the baked data.

    python3 tools/bake_osm_tags.py --lat 35.6595 --lon 139.7005 --radius 1
    python3 tools/bake_osm_tags.py --tile 14/14549/6452

No dependencies beyond the standard library.
"""
import argparse
import datetime
import json
import math
import os
import sys
import time
import urllib.parse
import urllib.request

EXTENT = 4096
Z = 14
URL = "https://postpass.geofabrik.de/api/interpreter"
KEEP = {
    "highway", "oneway", "lanes", "lanes:forward", "lanes:backward", "lanes:both_ways", "width", "maxspeed",
    "maxspeed:forward", "maxspeed:backward", "sidewalk", "sidewalk:left", "sidewalk:right", "sidewalk:both",
    "cycleway", "cycleway:left", "cycleway:right", "cycleway:both", "cycleway:left:lane", "cycleway:right:lane",
    "surface", "junction", "lane_markings", "bridge", "tunnel", "layer", "parking:lane:both", "parking:lane:left",
    "parking:lane:right", "lit", "divider",
}


def lonlat_to_tile(lon, lat, z):
    n = 2 ** z
    x = (lon + 180.0) / 360.0 * n
    lat_r = math.radians(max(-85.0511, min(85.0511, lat)))
    y = (1.0 - math.log(math.tan(lat_r) + 1.0 / math.cos(lat_r)) / math.pi) / 2.0 * n
    return x, y


def tile_bounds(x, y, z):
    n = 2 ** z
    lon = lambda tx: tx / n * 360.0 - 180.0
    lat = lambda ty: math.degrees(math.atan(math.sinh(math.pi * (1 - 2 * ty / n))))
    return lon(x), lat(y + 1), lon(x + 1), lat(y)


def fetch(west, south, east, north):
    sql = (
        "select osm_id, tags, geom from postpass_line where tags ? 'highway' "
        f"and geom && ST_MakeEnvelope({west},{south},{east},{north},4326)"
    )
    req = urllib.request.Request(URL, data=urllib.parse.urlencode({"data": sql}).encode(), method="POST")
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.load(r)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tile")
    ap.add_argument("--lat", type=float)
    ap.add_argument("--lon", type=float)
    ap.add_argument("--radius", type=int, default=1, help="tiles around the centre tile (1 = the 3x3 block the game reads)")
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
    today = datetime.date.today().isoformat()
    for y in range(ty - r, ty + r + 1):
        for x in range(tx - r, tx + r + 1):
            west, south, east, north = tile_bounds(x, y, Z)
            # A little margin so a way that starts just outside still arrives with its tags.
            padx, pady = (east - west) * 0.02, (north - south) * 0.02
            t0 = time.time()
            data = fetch(west - padx, south - pady, east + padx, north + pady)
            ways = []
            for f in data.get("features", []):
                g, p = f["geometry"], f["properties"]
                lines = g["coordinates"] if g["type"] == "MultiLineString" else [g["coordinates"]] if g["type"] == "LineString" else []
                tags = p.get("tags", {})
                units = []
                for line in lines:
                    pts = []
                    for lon, lat in line:
                        fx, fy = lonlat_to_tile(lon, lat, Z)
                        pts.append([round((fx - x) * EXTENT, 1), round((fy - y) * EXTENT, 1)])
                    if len(pts) >= 2:
                        units.append(pts)
                if units:
                    ways.append({"id": p["osm_id"], "highway": tags.get("highway"), "tags": {k: v for k, v in tags.items() if k in KEEP and k != "highway"}, "lines": units})
            ways.sort(key=lambda w: w["id"])
            path = os.path.join(args.cache, "osmtags", "v1", str(Z), str(x), f"{y}.json")
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w") as fh:
                json.dump({"version": 1, "baked": today, "extent": EXTENT, "ways": ways}, fh, separators=(",", ":"))
            print(f"{path}: {len(ways)} ways ({time.time() - t0:.1f}s)", file=sys.stderr)
            time.sleep(0.5)  # be polite to a free public service


if __name__ == "__main__":
    main()
