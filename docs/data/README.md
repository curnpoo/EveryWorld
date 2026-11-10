# World data — sources, licensing and acquisition

How we choose, license, pin and package source data. The long catalogue of candidate
sources is `registry.md` (kept as provided, with dated corrections at its top). Where this
file and the catalogue disagree, this file wins. Decisions: F8, F13 in `../DECISIONS.md`.

## 1. Principles

1. **Three states, never mixed.** 01 real-Earth reference data (pinned, read by `ed_data`);
   02 the generated solarpunk world (a pure function of seed and data version, `ed_world`);
   03 the living simulation (also takes UTC; plus persisted deltas).
2. **Pinned.** A "data version" names one release of every source. Chunks record the data
   version that produced them.
3. **Packaged.** The shipped game needs no third-party API. Players fetch chunks only from
   our own tile service or from packs (`../plans/world.md` §3).
4. **Screened.** GREEN ships. YELLOW ships only with written review notes. RED never ships.
   These are screening categories, not legal advice.
5. **Imagery is far-field only.** Satellite colour is used from aircraft height upward.
   Near-field ground is synthesized from land cover, biome and materials, because present-day
   photographs up close would show car parks and bare lots in a solarpunk world.

## 2. Chosen sources (data version 1)

| Need | Source | Screening | Notes |
|---|---|---|---|
| Elevation | Copernicus GLO-30 via Mapterhorn; USGS 3DEP and other open national models where present | YELLOW per provider | Record provider per area |
| Bathymetry | GEBCO | YELLOW | Verify terms for redistribution of derivatives |
| Far-field imagery | A Sentinel-2 composite: Copernicus global mosaics, or our own from Level-2A scenes | YELLOW | Replaces the EOX mosaic. Verify mosaic terms |
| Land cover | ESA WorldCover 2021 | GREEN (CC BY 4.0) | Drives near-field ground synthesis and vegetation |
| Roads | **Overture transportation** (pinned release) + raw OSM tags | YELLOW (**ODbL**) | `../plans/streets.md` |
| Buildings | **Overture buildings** (pinned release) | YELLOW (**ODbL**) | Stable GERS ids are used in `ObjectId` |
| Building heights | Overture where present; PLATEAU (Japan) and 3D BAG (Netherlands) for hero cities; otherwise the inference rule below | per source | |
| Canopy height | A global canopy-height product | YELLOW | Licence to verify before use |
| Climate normals | CHELSA or TerraClimate | YELLOW | Replaces WorldClim. Choose in Phase 0 with licence URL |
| Geology | Macrostrat | YELLOW (CC BY 4.0) | |
| Night lights | NASA Black Marble (VIIRS) | GREEN (NASA open data) | Feeds R3/R4 emissive |
| Place names | GeoNames, Natural Earth | GREEN / CC BY | Packaged gazetteer for go-to and labels |
| Moon, Mars | LOLA and MOLA elevation, NASA mosaics | GREEN | |
| Ephemeris | JPL elements, computed locally | GREEN | |
| Weather, geocoding (development only) | Open-Meteo | not shipped | Replaced by the simulated climate and the packaged gazetteer |

**Building-height inference rule.** Most footprints have no height. Order of preference:
measured height; `building:levels` × regional floor height; class and footprint-area table
per region; a density-based default from surrounding heights. The rule and its tables are
written in `../contracts/generation.md` and are deterministic.

## 3. Excluded or restricted

| Source | Status | Reason |
|---|---|---|
| WorldClim 2.1 | RED | Redistribution and commercial use restricted. Currently baked into `ed_data`; development only until replaced |
| EOX Sentinel-2 cloudless | RED | CC BY-NC-SA |
| HydroSHEDS | RED | Licence restricts commercial redistribution |
| Google Photorealistic 3D Tiles | RED | Map Tiles API policies prohibit caching, storing and offline use |
| GlobalBuildingAtlas | RED until verified | Listed as CC BY-NC |
| OpenMapTiles vector tiles as the road source | Retired | Clipped and simplified, tags dropped (F8). May remain for the 2D map view |

## 4. ODbL

OpenStreetMap, and Overture's buildings and transportation themes, are ODbL. Our baked road
and building chunks are probably derivative databases. Working position, to be confirmed by
legal review before any public build (the full note for the reviewer is
`odbl-position.md`):

- Attribute in the credits and in the game.
- Keep the bake pipeline able to publish the derived road and footprint data, separately
  from authored content and art, if share-alike requires it.
- Keep authored override layers and prefabs separate from ODbL-derived chunks so they are
  not drawn into the share-alike scope.

## 5. Registry and gate

One TOML file per dataset in `data/registry/` (created in Phase 0): id, name, organisation,
URLs, coverage, resolution, format, licence name and URL, commercial / modification /
redistribution permissions, attribution text, screening, review notes, version and
acquisition date, and the systems that consume it.

Tools (`xtask`):

- **Licence gate:** fails any pack manifest that names a RED dataset or a YELLOW dataset
  without review notes.
- **Attribution generator:** writes `CREDITS.md` and the in-game credits resource, including
  code and asset licences (Box3D, meshoptimizer, Fab/Megascans).
- **Version pinner:** stamps every chunk with the data version.

## 6. Formats and storage

- Source data lives outside the repo and the shipped build; originals are kept untouched.
- Rasters as COG/GeoTIFF; vectors as GeoParquet read with DuckDB in the bake tools; our own
  `.ewpack` chunks for everything the game loads.
- The game never parses source formats at runtime except through `ed_data` in development.

## 7. Acquisition order

| Phase | Data | For |
|---|---|---|
| 0 | Fixtures for six reference places; one city from Overture; sample L0–L6 layers; registry entries for everything in §2 | Spikes and golden hashes |
| 2 | Global elevation and bathymetry at globe-pack resolution; Sentinel-2 composite; night lights; land cover; Moon elevation | Globe pack, far field |
| 3 | Overture buildings and transportation with OSM tags for reference regions; canopy height; climate normals; hero-city heights | Streets, buildings, forests |
| 5–6 | Gazetteer; airports and launch sites; Mars elevation | Missions, navigation, space |
| 8 | Population grids, agriculture, ocean and weather validation sets | Living world |

Do not download a dataset before its registry entry exists.

## 8. Rules for anyone adding data

1. Registry entry first, with licence URLs.
2. RED never ships; YELLOW needs review notes.
3. Reference data and generated data stay in separate pipelines and caches.
4. Pin versions; generation must be reproducible.
5. Attribution is generated, never written by hand.
