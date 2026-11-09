# World data — implementation, licensing and acquisition

The catalog of every source we may use is `docs/data/registry.md` (saved as provided).
This file is **how we work with it**: current-source audit, licensing process, registry
format, acquisition order mapped to `docs/ROADMAP.md`, and storage/distribution rules.

Vision context: `docs/VISION.md` §10. The data has two jobs — real Earth reference, then the
solarpunk transformation. Three states stay separate:

| State | Meaning | Code home |
|---|---|---|
| 01 Real Earth reference | Original terrain, geology, hydrology, climate, vegetation, infrastructure | `ed_data` (fetch/cache/decode) |
| 02 Solarpunk transformation | Rewilding, retrofits, renewable infrastructure, future settlements | `ed_world` + `ed_planet` (generation), authored assets (`ed_assets`) |
| 03 Living simulation | Weather, seasons, water, wildlife, populations | `ed_climate` / `ed_sim` / `ed_server` |

State 01 inputs are **pinned and versioned**; state 02 is a pure function of (seed, data
version); state 03 is a pure function of (seed, data version, UTC) plus persisted deltas.
Never mix a simulated change back into a reference dataset.

---

## 1. Current-source audit (the alpha's actual inputs)

| Source | Used for | Screening | Action |
|---|---|---|---|
| OpenStreetMap via OpenFreeMap (OpenMapTiles) | Vector map: roads, buildings, water, land use | YELLOW (ODbL) | Keep. Attribution + ODbL review of shipped derivative databases; document produced-by structure |
| Mapterhorn (Copernicus GLO-30 + national DEMs) | Elevation | YELLOW per provider | Keep; record per-tile provider in registry (USGS/swisstopo open; verify Copernicus terms) |
| AWS Terrain Tiles (fallback) | Elevation + ETOPO1 bathymetry | YELLOW | Keep as fallback; verify per-source terms before shipping baked derivatives |
| **EOX Sentinel-2 cloudless mosaic** | Ground/orbit imagery | **RED for commercial (CC BY-NC-SA 4.0)** | **Must replace or license before selling.** Path: raw Sentinel-2 (open) + NAIP/national orthophotos; see `alpha/plans/beamng-earth-lessons.md` §2.1 |
| **WorldClim 2.1 (baked into `ed_data`)** | Climate normals → biomes | **RED (commercial + redistribution restricted)** | **Replace.** Candidates: CHELSA / TerraClimate / ERA5 / NASA MERRA-2 + NASA POWER (all in registry §12). Until replaced: development-only, never shipped |
| ESA WorldCover 2021 v200 | Land cover, canopy fractions | GREEN (CC BY 4.0) | Keep; attribution |
| Macrostrat | Geology/lithology | YELLOW (CC BY 4.0 per project docs) | Keep; attribution |
| Open-Meteo | Weather, geocoding | YELLOW (CC BY 4.0; live API) | Dev convenience only. Weather becomes `ed_climate` (seed, UTC); geocoding ships as a packaged GeoNames/Natural Earth gazetteer |
| JPL Keplerian elements + lunar almanac | Ephemeris | GREEN (computed locally) | Keep |

**P0 licensing actions:** (1) WorldClim replacement task before any public/commercial build;
(2) EOX replacement plan; (3) `CREDITS.md` + in-game attribution screen generated from the
registry; (4) no RED-classified dataset in any distribution manifest — enforced by the
license gate below.

## 2. Dataset registry (machine-readable)

One TOML file per dataset in `data/registry/` (new directory, created in Phase 0). Fields
follow registry §8: id, name, organization, urls, coverage, resolution, temporal coverage,
update frequency, original format, CRS, license name + URL, commercial/modification/
redistribution permissions, attribution text, third-party restrictions, screening
(GREEN/YELLOW/RED), processing status, integration status, quality/confidence, version +
acquisition date, and **consuming systems** (which `ed_*` crate / sim system reads it).

Tooling (`tools/data_registry.py` or an `ed_assets` subcommand):
- **License gate:** fail any ship/dist manifest that references a RED dataset or a YELLOW
  dataset without a filled `review_notes` field.
- **Attribution generator:** emits `CREDITS.md` + an in-game credits resource from all
  integrated datasets (including asset licenses: Solstice MIT, Box3D MIT, Fab/Megascans).
- **Version pinner:** stamps every bake pack with dataset versions + a plan signature
  (BeamNG `bakepack.py` pattern), so generation stays a pure function of pinned inputs.

## 3. Formats and storage

- Master development library lives **outside the shipped repo/build** (large reference data);
  originals are preserved untouched; derivatives are separate.
- COG/GeoTIFF rasters, GeoParquet vectors, Zarr/NetCDF for climate/ocean, LAZ for lidar,
  PMTiles for packaged reference tiles, our own baked format for cluster geometry
  (`ed_assets`). Retain native resolutions; no silent resampling.
- **Hierarchical streaming** global → continent → region → local → detail, matching
  `ed_planet` quadtree nodes; the whole planet is never loaded at once.
- Distribution = self-contained regional data packs + manifests + integrity checks +
  missing-data fallbacks. **No live external API is required for basic gameplay in the
  shipped game.** Live fetching in the alpha (`ed_data` HTTP + disk cache) remains a
  development path; the cache format and the pack format share decoders.

## 4. Acquisition order (mapped to ROADMAP phases)

Do not download indiscriminately; evaluate coverage, overlap, storage and rights first.

| When | Datasets | Why |
|---|---|---|
| **Phase 0–2 (planet foundation)** | SRTM/Copernicus (have via Mapterhorn), ETOPO + **GEBCO** (bathymetry), ESA WorldCover (have), **SoilGrids**, Natural Earth, **WorldClim replacement (CHELSA/TerraClimate/ERA5/MERRA-2)**, raw **Sentinel-2** + NAIP/national orthos, hydrography from licensed alternatives (JRC Global Surface Water, OSM water, NASA SWOT — **not HydroSHEDS**) | Terrain, coasts, biome basis, imagery without NC terms |
| **Phase 3–4 (looks + roads)** | **Overture Maps (pinned release)** + **Microsoft ML footprints (CDLA Permissive 2.0)**, OSM lanes/sidewalks via Geofabrik/Layercake, US Census TIGER (US), **Meta/WRI 1.2 m canopy height (verify terms)** + **NASA GEDI**, **Project PLATEAU / 3D BAG** for authored-quality cities, USGS 3DEP EPT lidar for bridge decks | Driveable roads, real building coverage/heights, dense real forests |
| **Phase 5–6 (civilization + multiplayer)** | **WorldPop** + GHSL (NPC/population placement reference), **NASA Black Marble / VIIRS** (night lights), **GeoNames** (packaged gazetteer), agriculture (USDA CDL, WorldCereal, MapSPAM) for food-system generation | Settlements, NPC density, night side, solarpunk agriculture |
| **Phase 7+ (living simulation)** | ERA5 + NASA GPM (weather validation), NOAA World Ocean Atlas + Argo + HYCOM (ocean), MODIS snow/vegetation indices, GBIF/iNaturalist (CC0/CC BY records only), Köppen-Geiger/Holdridge cross-checks, **Yale BSC / Gaia** (stars), **NASA LOLA/MOLA** (Moon/Mars terrain) | Weather/ocean/ecosystem sims, space |
| **Later / optional** | Pollution, hazards, cryosphere detail, historical Earth (registry §15–20) | Expansion content only |

## 5. Rules of thumb for agents

1. New dataset → new `data/registry/*.toml` entry **first**, with license URLs; no
   integration without it.
2. RED never ships; YELLOW ships only with documented review. When in doubt, ask.
3. Keep reference data and simulated/future data in separate pipelines and caches.
4. Pin versions; generation must stay reproducible from (seed, data version, UTC).
5. Attribution is generated, never hand-maintained.
6. Ship packs, not API dependencies; keep the live-fetch dev path working.
