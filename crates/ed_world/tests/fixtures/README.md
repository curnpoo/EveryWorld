# Determinism fixtures

Recorded source tiles for the six canonical places in `../golden.rs`, exactly as the alpha
data layer cached them. Recorded 2026-11-10 with
`EW_GOLDEN_RECORD=1 cargo test -p ed_world --test golden --release`.

Test data only; never packaged. Sources and licences (`data/registry/`):

- `openfreemap/` — OpenFreeMap vector tiles, © OpenStreetMap contributors (ODbL).
- `mapterhorn/`, `terrarium/` — elevation (Copernicus DEM and national models; Mapzen terrain tiles).
- `worldcover/` — ESA WorldCover 2021 (CC BY 4.0).
- `macrostrat/` — Macrostrat (CC BY 4.0).
- `roadz/` — surveyed road heights where a national source exists.

The EOX satellite mosaic is deliberately not recorded (CC BY-NC-SA, RED). Imagery and the
baked Overture and OSM-tag tiles therefore miss during the suite, and the generator's
fallbacks run; about 20 requests are attempted and refused per run.
