# Planetary Reference Library — master geospatial data & reference registry

Provenance: contributed by the owner 2026-11-08 as a complete ChatGPT research output
("SECOND EARTH — Master Geospatial Data & Reference Registry"), saved here as the master
catalog. The GREEN/YELLOW/RED statuses are **screening categories, not legal determinations**;
every dataset must be checked against its actual release version and terms before packaging.
Our implementation rules, current-source audit and acquisition plan: `docs/data/README.md`.

> **Corrections (2026-11-10).** The catalogue below is kept as provided. These points
> override it; the working rules and chosen sources are in `docs/data/README.md`.
>
> - **Overture Maps** buildings and transportation themes are **ODbL** (they include
>   OpenStreetMap data), not CDLA Permissive. Baked road and building chunks are probably
>   derivative databases; see `README.md` §4.
> - **Google Photorealistic 3D Tiles** are excluded: the Map Tiles API policies prohibit
>   caching, storing and offline use. (Google *Open Buildings* footprints are a separate
>   dataset and remain YELLOW.)
> - **GlobalBuildingAtlas** offers global building heights but is listed as CC BY-NC; treat
>   as RED until verified.
> - **Roads** come from Overture transportation plus raw OpenStreetMap tags, not from
>   OpenMapTiles vector tiles (`docs/plans/streets.md`).
> - **Imagery** is used for far-field views only; near-field ground is synthesized.
> - "Priority: Microsoft + OSM + PLATEAU + Overture" for buildings is replaced by: Overture
>   buildings as the base (stable ids), PLATEAU and 3D BAG for hero cities, heights inferred
>   elsewhere by the rule in `README.md` §2.

Starting point (as provided): the project has OpenStreetMap and satellite imagery. Everything
below is a proposed expansion of the world-data library. Two jobs for the data:
1. Reconstruct the real Earth's physical geography as the foundation.
2. Give the game a realistic starting point for transforming that geography into a thriving,
   rewilded, technologically advanced civilization.

Project principles:
- Global geographic coverage.
- Prefer free, public and openly licensed sources.
- Prioritize redistribution and commercial game-use permissions.
- Preserve source attribution and licensing.
- Separate real-world reference data from fictional future development.
- Keep original data separate from processed game assets.
- Use procedural generation to fill gaps and create future environments.

---

## 1. Licensing rules for a free, packaged game

A dataset being free to download does NOT automatically mean it can be packaged inside a game.

| License | General game-use position |
|---|---|
| CC0 | Preferred. Generally unrestricted, including commercial use |
| Public Domain | Preferred. Generally unrestricted |
| CC BY | Usable. Attribution required |
| CC BY 4.0 | Usable. Attribution and license notice required |
| ODbL | Review. Database obligations and possible share-alike requirements |
| CC BY-SA | Review. Share-alike obligations may apply |
| CC BY-NC | Avoid. Noncommercial restriction |
| CC BY-ND | Avoid. Adaptation restrictions |
| Custom government terms | Verify. Depends on jurisdiction and source |
| Research-only | Avoid. Unless separate game distribution permission exists |
| Commercial API | Avoid. Unless redistribution rights are explicitly granted |

Important distinctions:
- Free access and free redistribution are different.
- A public API does not necessarily grant permission to redistribute its underlying data.
- Government data is not automatically public domain outside the United States.
- Attribution requirements still apply to many open datasets.
- ODbL requires particular attention when combining and distributing databases.
- Satellite imagery can have different rights depending on provider, sensor and processor.
- Some datasets contain third-party components with separate licenses.

Internal classifications:
- **GREEN**: explicitly suitable for game redistribution under documented terms.
- **YELLOW**: potentially usable, but license, attribution, database combination or
  redistribution needs review.
- **RED**: don't package until permission is obtained.

## 2. Master data-source catalog

### 01. Global geography, terrain and elevation

| Dataset | Description | License |
|---|---|---|
| Copernicus DEM | Global elevation and surface models, useful for terrain reconstruction | YELLOW |
| USGS 3DEP | US elevation, bare-earth terrain, lidar point clouds and 3D surface data | GREEN |
| NASA SRTM | Global elevation data from the Shuttle Radar Topography Mission | GREEN |
| ALOS World 3D | Japanese satellite-derived global elevation model | YELLOW |
| ASTER GDEM | Global digital elevation model from ASTER satellite observations | GREEN |
| FABDEM | Terrain elevation with estimated building and forest height effects removed | YELLOW |
| TanDEM-X | Radar-derived global elevation and surface models | RED |
| OpenTopography | Lidar/terrain discovery and downloads; individual datasets have their own terms | YELLOW |
| ArcticDEM | High-resolution Arctic surface elevation | YELLOW |
| REMA | High-resolution Antarctic elevation model | YELLOW |
| Natural Earth | Global terrain-related cartographic features, coastlines, boundaries | GREEN |
| ETOPO | Global land and ocean relief model | GREEN |

Priority: NASA SRTM, USGS 3DEP, Copernicus DEM and ETOPO.
Note: USGS 3DEP is US-only; international elevation sources are needed for global coverage.
NASA SRTM also has coverage/resolution limits — not a universal high-res terrain solution.

### 02. Ocean floor, seafloor and underwater geography

| Dataset | Description | License |
|---|---|---|
| GEBCO | Global bathymetry, ocean depths and combined land/ocean terrain | GREEN |
| NOAA NCEI Bathymetry | Marine depth surveys, bathymetric grids and ocean floor measurements | YELLOW |
| GMRT | Global Multi-Resolution Topography, especially marine geology and seafloor mapping | YELLOW |
| IBCSO | International Bathymetric Chart of the Southern Ocean | YELLOW |
| EMODnet Bathymetry | European sea floor bathymetry and marine terrain | YELLOW |
| Seabed 2030 | Global initiative and data discovery for mapping the ocean floor | YELLOW |
| NOAA ETOPO | Global topography and bathymetry in one elevation reference | GREEN |

GEBCO is especially valuable: its 2026 global grid is 15 arc-seconds, and its terms explicitly
permit commercial use, modification and inclusion in products, with source acknowledgment.
Priority: GEBCO first, then regional bathymetry to improve detail.

### 03. Buildings, structures and 3D cities

| Dataset | Description | License |
|---|---|---|
| Microsoft Global ML Building Footprints | Global satellite-derived building footprints, some height estimates | GREEN |
| Google Open Buildings | Building footprint data, particularly in underrepresented regions | YELLOW |
| Overture Maps | Global building geometry, addresses, places and transportation features | YELLOW (ODbL for buildings and transportation) |
| OpenStreetMap Buildings | Community-mapped buildings, types, heights and attributes | YELLOW |
| Project PLATEAU | Detailed Japanese 3D city models, buildings and urban features | YELLOW |
| 3D BAG | Detailed 3D building models for the Netherlands | YELLOW |
| GSI Japan | Japanese elevation, maps, aerial imagery and geographic information | YELLOW |
| OpenBuildingMap | Building geometry and attributes from open geographic mapping | YELLOW |
| CityGML | Standard for representing structured 3D city objects | GREEN |
| 3D Tiles | Standard for streaming large-scale 3D geographic content | GREEN |

Microsoft's footprints use CDLA Permissive 2.0 (free use and redistribution) and include some
height estimates. Priority: Microsoft + OSM + PLATEAU + Overture. These are the original
footprints; the game can replace or procedurally transform buildings into solarpunk
architecture.

### 04. Roads, paths and transportation

| Dataset | Description | License |
|---|---|---|
| OpenStreetMap | Global roads, footpaths, trails, railways and transport infrastructure | YELLOW |
| Overture Transportation | Harmonized global road and transportation network | YELLOW (ODbL) |
| Geofabrik | Regional OSM extracts for convenient downloading | YELLOW |
| GRIP | Global Roads Inventory Project road network | YELLOW |
| gROADS | Global road network data | YELLOW |
| US Census TIGER/Line | US roads, transportation and geographic boundaries | GREEN |
| Natural Earth Roads | Generalized global roads and transport features | GREEN |
| OpenRailwayMap | Railway network and infrastructure visualization | YELLOW |

Additional road properties to collect: width, lane counts and surface; bridges, tunnels and
elevated sections; elevation and gradients; footpaths and cycleways; traffic and road usage;
parking and loading; rail electrification and track gauge; ports, canals, shipping routes and
ferry terminals; airports and runway geometry.

OSM is the main road geometry foundation. OSM data is ODbL: the distributed database and
derivative-data architecture must account for share-alike requirements. OSM's own guidance
specifically discusses computer games and simulations.

### 05. Land cover, land use and surface classification

| Dataset | Description | License |
|---|---|---|
| ESA WorldCover | Global 10 m land cover: forests, crops, grassland, wetlands, built-up | GREEN |
| Copernicus Global Land Service | Global land cover, vegetation, water and environmental monitoring | YELLOW |
| JRC Global Land Cover | European Commission global and regional land monitoring products | YELLOW |
| CORINE Land Cover | Detailed European land cover and land-use classification | YELLOW |
| USDA Cropland Data Layer | Crop-specific land classification across the United States | GREEN |
| EU Crop Map / JRC EUCROPMAP | Crop type mapping and agricultural land classification | YELLOW |
| Dynamic World | Near-real-time 10 m land cover classification derived from Sentinel-2 | YELLOW |
| GLAD Land Cover | Global land cover and change monitoring | YELLOW |
| ESA CCI Land Cover | Long-term land cover records for environmental change | YELLOW |
| Global Human Settlement Layer | Built-up areas and settlement classification | YELLOW |

Priority: ESA WorldCover, USDA CDL, Dynamic World and Copernicus.
ESA WorldCover: 10 m global coverage, 11 classes, CC BY 4.0.

### 06. Soil, geology and underground geography

| Dataset | Description | License |
|---|---|---|
| SoilGrids | Global soil properties at 250 m and six depth intervals | GREEN |
| OpenLandMap | Soil, terrain, environmental and land-surface information | YELLOW |
| WoSIS | Global soil profile observations and soil properties | YELLOW |
| Harmonized World Soil Database | Global soil classification and properties | YELLOW |
| Macrostrat | Geological units, rock types, formations and stratigraphic information | YELLOW |
| OneGeology | International geological map services | YELLOW |
| USGS National Geologic Map Database | US geological formations, bedrock and surficial geology | GREEN |
| Global Lithological Map / GLiM | Global rock-type classification | YELLOW |
| Permafrost Zonation Index | Frozen ground and permafrost distribution research | YELLOW |
| PEATMAP | Global peatland distribution reference | YELLOW |
| GPlates | Plate tectonics and geological reconstruction software/data | YELLOW |

SoilGrids supplies pH, organic carbon, nitrogen, texture, density and more; CC BY 4.0.

### 07. Forests, trees and vegetation

| Dataset | Description | License |
|---|---|---|
| NASA GEDI | Forest height, vertical structure and biomass from spaceborne lidar | YELLOW |
| Global Forest Watch | Forest extent, tree cover, forest loss and gain | YELLOW |
| Hansen Global Forest Change | Global tree cover and forest disturbance history | YELLOW |
| NASA MODIS Vegetation Indices | NDVI, EVI and vegetation health observations | GREEN |
| Sentinel-2 | Multispectral vegetation, forest and plant observations | GREEN |
| Global Mangrove Watch | Mangrove distribution and changes | YELLOW |
| TRY Plant Trait Database | Plant functional traits and ecological characteristics | YELLOW |
| BIEN | Botanical information, species observations and plant ecology | YELLOW |
| GBIF Plants | Georeferenced plant occurrence records | YELLOW |
| World Flora Online | Plant taxonomy and species reference information | YELLOW |
| iNaturalist | Geolocated observations of plants, fungi and wildlife | YELLOW |
| Global Biodiversity Information Facility | Global biological occurrence records | YELLOW |

Additional ecological data to seek: individual species and ranges; tree height and canopy
diameter; forest density and biomass; grasslands, shrubs and ground vegetation; seasonal
foliage; rooting depths; flowering/fruiting seasons; pollinator habitats; forest succession;
invasive species; wild plant distributions; crop and orchard species.

Important: GBIF records are individually licensed (CC0 / CC BY / CC BY-NC). Filter out
CC BY-NC records; keep attribution at the individual dataset/publisher level.

### 08. Agriculture, crops and food systems (solarpunk-specific)

| Dataset | Description | License |
|---|---|---|
| USDA NASS CDL | Crop-specific land-use classification | GREEN |
| FAOSTAT | Agricultural production, food, land use and livestock statistics | YELLOW |
| FAO GAEZ | Global agroecological suitability and crop productivity | YELLOW |
| MapSPAM | Spatial distribution of agricultural production | YELLOW |
| EarthStat | Global agricultural land use and crop distributions | YELLOW |
| Global Crop Water Model | Agricultural water-use and crop requirements | YELLOW |
| WorldCereal | Global crop type and irrigation mapping | YELLOW |
| Global Pasture Watch | Grasslands, pasture and rangeland monitoring | YELLOW |

Game applications: community gardens, food forests, agroforestry, vertical farming,
greenhouses, orchards, aquaponics, regenerative agriculture, rooftop farms, permaculture
zones, crop rotations, irrigation networks, agricultural biodiversity.
Use real crop maps as references for current agricultural geography, then generate future
food systems procedurally.

### 09. Rivers, lakes, wetlands and groundwater

| Dataset | Description | License |
|---|---|---|
| HydroSHEDS | Global rivers, drainage, elevation-derived hydrology and watersheds | RED |
| HydroRIVERS | Global river centerlines and river attributes | RED |
| HydroLAKES | Global lake polygons and characteristics | RED |
| HydroBASINS | Global watershed and drainage basin boundaries | RED |
| JRC Global Surface Water | Historical surface water occurrence and seasonal water extent | YELLOW |
| NASA SWOT | River, lake and ocean surface elevation observations | GREEN |
| GRACE / GRACE-FO | Changes in groundwater, ice and total water storage | GREEN |
| Global Lakes and Wetlands Database | Global wetlands, lakes and marshes | YELLOW |
| Wetlands International | Wetland inventories, conservation and ecological data | YELLOW |
| Aqueduct | Water risk, scarcity and freshwater stress | YELLOW |
| IGRAC | International groundwater resource information | YELLOW |
| WHYMAP | World groundwater resources and aquifer mapping | YELLOW |

Important licensing exception: HydroSHEDS' website terms restrict use of site materials to
personal, noncommercial purposes and prohibit redistribution without permission. Do not
assume its downloadable river products are safe to bundle. Obtain explicit permission or
substitute separately licensed hydrography data.

Additional information to collect: river width/depth; flow direction; velocity; seasonal
discharge; floodplain extent; riverbed material; water table depth; aquifer type; springs;
wetland vegetation; water quality; seasonal availability; waterfall locations; estuaries
and deltas.

### 10. Oceans, marine ecosystems and coastal geography

| Dataset | Description | License |
|---|---|---|
| NOAA World Ocean Atlas | Global ocean temperature, salinity, oxygen and nutrients | GREEN |
| Copernicus Marine Service | Ocean currents, temperature, sea level, ice and marine observations | YELLOW |
| HYCOM | Ocean circulation, currents, temperature and salinity models | YELLOW |
| Argo | Global ocean profiling floats measuring temperature and salinity | GREEN |
| NASA OceanColor | Chlorophyll, ocean color, phytoplankton and coastal water observations | GREEN |
| EMODnet | European marine bathymetry, geology, habitats and human activities | YELLOW |
| OBIS | Marine biodiversity and species occurrence records | YELLOW |
| Global Coral Reef Monitoring Network | Coral reef condition and distribution | YELLOW |
| Allen Coral Atlas | High-resolution coral reef habitat mapping | YELLOW |
| Global Mangrove Watch | Mangrove forests and coastal ecosystems | YELLOW |
| Sea Around Us | Fisheries, marine ecosystems and human fishing pressure | YELLOW |

Additional layers: current direction/speed; surface and deep temperature; salinity; depth;
seafloor sediment; coral reefs; kelp forests; seagrass meadows; mangroves; phytoplankton;
marine biodiversity; sea ice; tidal patterns; wave height/direction; coastal erosion;
upwelling; acidification; dissolved oxygen; marine protected areas.

For the ocean simulation, Argo + World Ocean Atlas + HYCOM + GEBCO provide a useful
combination of observations, climatology, circulation and physical structure.

### 11. Animals, wildlife and biodiversity

| Dataset | Description | License |
|---|---|---|
| GBIF | Worldwide georeferenced animal, plant and fungal observations | YELLOW |
| iNaturalist | Community wildlife observations with location and species identification | YELLOW |
| IUCN Red List | Species distribution, conservation status and habitat information | RED |
| Map of Life | Global species distributions and biodiversity information | YELLOW |
| eBird | Bird sightings, seasonal distribution and migration | YELLOW |
| Movebank | Animal tracking, movement and migration records | YELLOW |
| OBIS | Marine species occurrence and biodiversity | YELLOW |
| Living Planet Index | Wildlife population trends | YELLOW |
| eMammal | Camera-trap wildlife observations | YELLOW |
| Paleobiology Database | Fossil occurrence records and extinct species | YELLOW |

What to extract: species names/taxonomy; geographic distribution; habitat preferences;
elevation range; temperature tolerance; food sources; predators/prey; migration routes;
breeding seasons; nesting/spawning locations; population estimates; conservation status;
seasonal activity; ecological relationships.

Approach: create our own species database from records that permit redistribution, then
attach ecological characteristics and procedural behavior per species. For a solarpunk
future: wildlife corridors, rewilded urban environments, restored wetlands, thriving
pollinator populations.

### 12. Climate classifications and historical climate

| Dataset | Description | License |
|---|---|---|
| Köppen-Geiger | Global climate zone classification | YELLOW |
| WorldClim | Historical climate normals and bioclimatic variables | RED |
| CHELSA | High-resolution climate surfaces and climatologies | YELLOW |
| TerraClimate | Monthly temperature, precipitation, water balance and drought variables | YELLOW |
| ERA5 | Global hourly atmospheric reanalysis | YELLOW |
| ERA5-Land | Land-focused historical weather and environmental conditions | YELLOW |
| NASA MERRA-2 | Atmospheric reanalysis and composition | GREEN |
| NASA GPM | Global precipitation and rainfall observations | GREEN |
| NASA POWER | Solar and meteorological data | GREEN |
| NOAA GHCN | Historical weather station measurements | GREEN |
| HadISST | Historical sea surface temperature and sea ice | YELLOW |
| Berkeley Earth | Historical global temperature observations and reconstructions | YELLOW |

WorldClim warning: its published terms restrict commercial use and redistribution without
prior permission. Do not make it a default source for a packaged game. (Owner note: the
alpha currently bakes WorldClim into `ed_data` — replacement is tracked in
`docs/data/README.md`.)

Climate classification categories to support: tropical rainforest/monsoon/savanna; hot and
cold desert; Mediterranean; humid subtropical; oceanic; humid continental; subarctic; tundra;
ice cap; highland/alpine; semi-arid steppe. Also consider Holdridge Life Zones for relating
temperature, precipitation and ecological zones.

### 13. Weather, clouds, wind and atmospheric conditions

| Dataset | Description | License |
|---|---|---|
| NOAA GOES | Geostationary weather satellite imagery and atmospheric observations | GREEN |
| NASA CERES | Earth's radiation budget, cloud and energy observations | GREEN |
| ISCCP | Global historical cloud observations | YELLOW |
| NASA CloudSat | Cloud vertical structure and precipitation information | GREEN |
| NASA CALIPSO | Atmospheric aerosols and cloud vertical profiles | GREEN |
| NASA LIS | Lightning observations | YELLOW |
| NOAA GOES GLM | Geostationary lightning mapping | GREEN |
| NOAA NOMADS | Numerical weather prediction and atmospheric model data | YELLOW |
| GEWEX | Global energy and water exchange research | YELLOW |

Physical variables for our own weather engine: air temperature and gradients; pressure;
humidity/dew point; wind speed/direction/turbulence; cloud cover and altitude; precipitation
intensity and phase; solar radiation; evaporation/transpiration; snow accumulation/melt;
atmospheric stability; jet streams; fronts and pressure systems; thunderstorm development;
lightning; fog/visibility; hail/freezing rain.
The engine generates the actual weather events; these sources give historical patterns and
validation targets.

### 14. Nighttime imagery, artificial lighting and light pollution

| Dataset | Description | License |
|---|---|---|
| NASA Black Marble | Nighttime Earth imagery and artificial illumination | GREEN |
| VIIRS Day/Night Band | Global nighttime radiance measurements | GREEN |
| DMSP-OLS | Historical global nighttime lights | YELLOW |
| World Atlas of Artificial Night Sky Brightness | Artificial sky brightness and light pollution | YELLOW |
| Light Pollution Map | Global light pollution visualization and observations | YELLOW |

Additional information: light intensity/color/spectra; urban illumination; industrial
lighting; road lighting; population density; seasonal changes in nighttime activity; sky
brightness; electricity access; nighttime energy consumption.
For the setting: a comparison reference — cities brightly lit where appropriate, but darker
ecological corridors, reduced light pollution and carefully designed public lighting.

### 15. Pollution, emissions and environmental quality

| Dataset | Description | License |
|---|---|---|
| Sentinel-5P | Atmospheric NO₂, SO₂, CO, ozone and aerosol observations | GREEN |
| Copernicus CAMS | Atmospheric composition, pollution and air quality models | YELLOW |
| OpenAQ | Ground-based air quality observations | YELLOW |
| NASA MAIAC | Aerosol optical depth and atmospheric particles | GREEN |
| EDGAR | Global emissions of greenhouse gases and air pollutants | YELLOW |
| Global Carbon Project | Carbon emissions, budgets and global carbon cycle research | YELLOW |
| WHO Air Quality Database | Air quality measurements and health-related environmental data | YELLOW |
| Global Fire Emissions Database | Biomass burning, wildfire emissions and fire history | YELLOW |
| NASA FIRMS | Active fires and thermal anomalies | GREEN |

Also collect: PM2.5/PM10; CO₂ and methane; NO₂; SO₂; CO; ozone; aerosols; industrial
emissions; wildfire smoke; water/soil/ocean contamination; plastic pollution; noise and
thermal pollution; acidification.

### 16. Population, settlements and human geography

| Dataset | Description | License |
|---|---|---|
| WorldPop | High-resolution population distribution | GREEN |
| GHSL | Global built-up areas, population and settlement patterns | YELLOW |
| LandScan | Ambient population distribution | YELLOW |
| Natural Earth | Countries, administrative boundaries and geographic features | GREEN |
| GeoNames | Geographic names, settlements and places | YELLOW |
| HDX | Global humanitarian geographic datasets and population information | YELLOW |
| FAOSTAT | Agricultural and food-production statistics | YELLOW |
| Protected Planet / WDPA | Protected areas, conservation zones and parks | YELLOW |

Additional layers: urban density; rural settlement patterns; population density; cities,
towns, villages; human mobility; industrial zones; agricultural areas; parks/protected areas;
land parcels; energy infrastructure; water infrastructure; urban growth/redevelopment.
For the solarpunk setting these are primarily reference layers; we generate our own future
population and settlement distribution.

### 17. Natural hazards and geological processes

| Dataset | Description | License |
|---|---|---|
| USGS Earthquake Catalog | Historical earthquakes, locations, depths and magnitudes | GREEN |
| ISC-GEM | Historical large earthquake catalog | YELLOW |
| Smithsonian Global Volcanism Program | Volcanoes, eruptions and geological history | YELLOW |
| NASA FIRMS | Active fires and thermal anomalies | GREEN |
| Global Wildfire Information System | Global wildfire monitoring and statistics | YELLOW |
| Global Landslide Catalog | Historical landslide observations | YELLOW |
| Global Flood Database | Satellite-observed flood events and extents | YELLOW |
| ThinkHazard | Geographic natural hazard exposure | YELLOW |
| World Bank GAR Atlas | Global disaster risk and exposure information | YELLOW |

Include: fault lines; plate boundaries; volcanoes; earthquake history; tsunami exposure;
landslides; flooding; storm surges; wildfires; drought; avalanche zones; coastal erosion;
subsidence.

### 18. Snow, glaciers, ice and permafrost

| Dataset | Description | License |
|---|---|---|
| GLIMS | Global glacier outlines and observations | YELLOW |
| Randolph Glacier Inventory | Global glacier boundaries and characteristics | YELLOW |
| NSIDC | Snow, sea ice, glaciers, permafrost and cryosphere data | YELLOW |
| NASA ICESat-2 | Ice elevation and surface measurements | GREEN |
| ESA CryoSat-2 | Ice thickness, sea ice and glacier observations | YELLOW |
| MODIS Snow Cover | Snow extent and seasonal changes | GREEN |
| ArcticDEM | Arctic terrain elevation | YELLOW |
| REMA | Antarctic elevation and ice surface models | YELLOW |

Collect: glacier outlines/thickness/movement; snow depth; snow water equivalent; seasonal
snow extent; ice sheets/shelves; sea ice; permafrost extent; frozen lakes; freeze-thaw cycles.

### 19. Solar radiation, atmospheric optics and celestial environment

| Dataset | Description | License |
|---|---|---|
| NASA CERES | Earth's radiation budget and energy balance | GREEN |
| NASA POWER | Solar irradiance and meteorological observations | GREEN |
| NASA Aura | Atmospheric composition, ozone and atmospheric chemistry | GREEN |
| NASA OMI | Ozone, aerosols and atmospheric gases | GREEN |
| NASA MOPITT | Atmospheric carbon monoxide observations | YELLOW |
| ESA Gaia / DPAC | Astronomical star positions, distances and stellar properties | YELLOW |
| GSFC Scientific Visualization Studio | NASA visualizations and scientific imagery; per-asset rights checks | YELLOW |

Additional environmental information: solar elevation/angle; direct and diffuse sunlight;
day length; twilight; UV index; ozone distribution; atmospheric scattering; surface albedo;
cloud optical depth; aurora occurrence; atmospheric refraction.
Gaia is for celestial rendering/sky systems/astronomy features — not a substitute for
terrestrial geography.

### 20. Historical Earth, fossils and geological time

| Dataset | Description | License |
|---|---|---|
| Paleobiology Database | Fossil occurrences, extinct organisms and paleontological records | YELLOW |
| PANGAEA | Earth and environmental science research datasets | YELLOW |
| NOAA Paleoclimatology | Historical climate reconstructions and geological records | YELLOW |
| Deep-Time Digital Earth | Geological information and Earth history | YELLOW |
| GPlates | Continental drift and plate tectonic reconstruction | YELLOW |
| Macrostrat | Geological formations and stratigraphic history | YELLOW |

Potential future features: ancient coastlines; continental drift; ancient ecosystems;
historical biome distributions; fossil locations; extinct species; ancient climates; glacial
periods; sea-level changes; geological time periods. Not essential for the first playable
Earth, but an expansion path.

## 3. Primary satellite and Earth observation programs

| Program | Description | Official access |
|---|---|---|
| Landsat | Long-term land imagery, vegetation, water and thermal observations | USGS Landsat |
| Sentinel-1 | Radar imagery, flooding, forests, surface deformation and terrain | Copernicus |
| Sentinel-2 | Multispectral land imagery, vegetation, crops and water | Copernicus |
| Sentinel-3 | Ocean color, sea surface temperature and land surface temperature | Copernicus |
| Sentinel-5P | Atmospheric gases, pollution and atmospheric composition | Copernicus |
| MODIS Terra | Vegetation, fires, snow, clouds and land surface conditions | NASA MODIS |
| MODIS Aqua | Ocean, atmospheric and land observations | NASA MODIS |
| GEDI | Forest canopy structure, height and biomass | NASA GEDI |
| ICESat-2 | Laser elevation measurements of ice, land and water | NASA |
| SWOT | Surface water elevation, rivers, lakes and oceans | NASA |
| GRACE-FO | Groundwater, ice mass and water storage changes | NASA |
| VIIRS | Nighttime illumination, fires and surface observations | NOAA |
| GOES | Clouds, weather systems and lightning | NOAA |
| GPM | Global precipitation measurements | NASA |
| CloudSat | Cloud structure and precipitation | NASA |
| CALIPSO | Atmospheric aerosols and cloud profiles | NASA |
| CryoSat-2 | Ice thickness and elevation | ESA |
| ALOS | Radar and optical Earth observation | JAXA |

Primary download/discovery portals: NASA Earthdata Search, Copernicus Data Space, USGS
EarthExplorer, AWS Open Data Registry, Microsoft Planetary Computer, Google Earth Engine.

Copernicus Sentinel data has explicit open-use rights covering reproduction, distribution,
modification and combination; other content hosted on the Copernicus portal can have
different terms. NASA also has third-party imagery exceptions — inspect individual asset
credits.

## 4. Data infrastructure and formats

| Technology | Purpose |
|---|---|
| STAC | Catalog and discover satellite imagery and geospatial assets |
| Cloud Optimized GeoTIFF | Stream large elevation, imagery and land-cover rasters |
| GeoParquet | Store large geographic vector datasets efficiently |
| FlatGeobuf | Efficient geographic vector data and spatial indexing |
| GeoPackage | Package regional vector and raster GIS information |
| NetCDF | Climate, ocean and atmospheric multidimensional data |
| Zarr | Large multidimensional scientific data and cloud access |
| LAS / LAZ | Lidar point cloud storage |
| 3D Tiles | Stream 3D buildings, terrain and geographic objects |
| CityGML | Structured 3D city and building representation |
| Parquet | Efficient structured and tabular data |
| GeoJSON | Lightweight geographic feature exchange |
| PMTiles | Single-file, cloud-friendly tiled geographic datasets |
| MBTiles | Packaged raster or vector map tiles |

For the game itself:
- COG for terrain and land-cover source data.
- GeoParquet for roads, buildings and geographic features.
- Zarr for climate and ocean simulation inputs.
- LAZ for detailed terrain and vegetation scans.
- 3D Tiles or our own engine-specific format for streamed world geometry.
- PMTiles for packaged geographic reference layers.

## 5. Critical licensing notes

- **OpenStreetMap (ODbL).** Usable in games, but attribution and database share-alike
  obligations matter. Keep the OSM-derived database clearly documented and have the
  combined-data structure reviewed before distribution. (OSM licensing; game and simulation
  guidance.)
- **Satellite imagery (source-dependent).** A map viewer allowing screenshots or offline
  viewing doesn't necessarily authorize packaging tiles in the game. Prefer raw Sentinel and
  Landsat products with documented redistribution rights.
- **WorldClim (restricted).** Free academic access does not grant commercial redistribution.
  Seek permission or use another climate dataset.
- **HydroSHEDS (restricted).** Published website terms prohibit commercial use and
  redistribution without permission. Find a separately licensed alternative.
- **OpenTopography (API restrictions).** Downloaded datasets and API access have different
  conditions; API terms prohibit integrating API keys into commercial products. Download and
  license eligible source data separately.
- **GBIF (record-dependent).** Filter out noncommercial occurrence records; attribution at
  the individual dataset/publisher level.

Other useful licensing references: NASA media usage guidelines, USGS 3DEP, ESA WorldCover
license, Microsoft building footprints license, WorldPop license, GEBCO terms.
USGS 3DEP is free without use restrictions, WorldPop uses CC BY 4.0, Natural Earth is public
domain — good examples of straightforward sources.

## 6. How this translates into the solarpunk world

The game shouldn't simply reproduce modern Earth and add futuristic buildings. The geographic
data becomes the foundation of an evolving future planet. Three states:

- **STATE 01 — Real Earth Reference.** Original terrain, geology, hydrology, climate records,
  vegetation, biodiversity and existing infrastructure.
- **STATE 02 — Solarpunk Transformation.** Rewilding, green cities, ecological restoration,
  renewable energy, regenerative agriculture, clean transportation, human-centered
  architecture.
- **STATE 03 — Living Planet Simulation.** Dynamic seasons, vegetation growth, water cycles,
  weather, wildlife populations, ocean circulation, ecosystems, environmental changes.

| Real-world reference | Future-world interpretation |
|---|---|
| Existing roads | Green corridors, bicycle infrastructure, electric transit |
| Existing buildings | Retrofitted structures, vertical gardens, solar architecture |
| Deforested land | Rewilded forests and restored ecosystems |
| Agricultural land | Regenerative farms, food forests and agroforestry |
| Industrial zones | Clean manufacturing and circular production |
| Urban rooftops | Solar panels, gardens and greenhouses |
| Rivers | Restored waterways and ecological river corridors |
| Parking lots | Parks, gardens and pedestrian spaces |
| Existing power grids | Renewable microgrids and distributed energy |
| Abandoned infrastructure | Reclaimed habitats and adaptive reuse |

Preserve the underlying geography while allowing the civilization to transform.

## 7. Suggested acquisition priorities

Avoid downloading everything at once; many datasets overlap and some are enormous.

**Phase 1: Establish the planet**
- [ ] NASA SRTM / Copernicus DEM
- [ ] GEBCO
- [ ] ESA WorldCover
- [ ] SoilGrids
- [ ] Microsoft Global Building Footprints
- [ ] OSM roads and infrastructure
- [ ] Natural Earth boundaries
- [ ] Hydrography from appropriately licensed sources

**Phase 2: Make the planet alive**
- [ ] GEDI forest structure
- [ ] MODIS vegetation indices
- [ ] GBIF plant and animal records
- [ ] NASA GPM precipitation
- [ ] ERA5 atmospheric history
- [ ] NOAA ocean climatology
- [ ] Argo ocean observations
- [ ] MODIS snow and ice

**Phase 3: Reconstruct civilization**
- [ ] 3D building models
- [ ] Population distribution
- [ ] Agriculture and crop classification
- [ ] Power and utility infrastructure
- [ ] Nighttime lights
- [ ] Transportation infrastructure
- [ ] Industrial and land-use classification

**Phase 4: Environmental simulation**
- [ ] Historical climate validation
- [ ] Ocean currents and temperatures
- [ ] Water storage and groundwater
- [ ] Atmospheric composition
- [ ] Pollution
- [ ] Wildfires and geological hazards
- [ ] Seasonal vegetation and ecosystem references

**Phase 5: Solarpunk transformation**
- [ ] Regional architectural generation
- [ ] Rewilding and vegetation regeneration
- [ ] Renewable energy infrastructure
- [ ] Future transportation systems
- [ ] Restored waterways and wetlands
- [ ] Regenerative agriculture
- [ ] Ecological corridors and wildlife habitats

## 8. Geospatial data acquisition & integration specification (implementation directive)

**Objective:** build a comprehensive, globally consistent geographic reference database.
The game is set in a solarpunk abundance-future civilization; real-world geographic data
provides the physical foundation for a transformed future Earth.

**Core requirements:**
1. Prioritize free, public, open-license datasets.
2. Prefer sources that explicitly permit commercial use, modification and redistribution.
3. The game must be distributable as a packaged product.
4. Do not assume free downloads, public APIs or public websites grant redistribution rights.
5. Do not package restricted, noncommercial or research-only datasets without permission.
6. Preserve attribution, license notices and source metadata.
7. Do not make live API access a requirement for basic gameplay.
8. Store and distribute eligible data locally in optimized formats.
9. Support global coverage with higher-resolution regional detail where available.
10. Never silently replace source data or lose provenance.

**Dataset registry fields:** identifier; name; organization; official website; download
endpoint; API endpoint if applicable; geographic coverage; spatial resolution; vertical
resolution if applicable; temporal coverage; update frequency; original format; coordinate
reference system; license name; license URL; commercial-use permission; modification
permission; redistribution permission; attribution requirements; third-party restrictions;
processing status; integration status; data quality and confidence; version and acquisition
date; simulation systems that consume the dataset.

**Data classification:**
1. Static geography: terrain, elevation, geology, coastlines, bathymetry, rivers, soil,
   boundaries.
2. Observed geography: satellite imagery, vegetation observations, species observations,
   weather records, pollution, nighttime lights, environmental surveys.
3. Human geography: buildings, roads, transportation, agriculture, settlements, utilities,
   population.
4. Simulated geography: future architecture, vegetation growth, climate, weather, hydrology,
   ecosystems, wildlife, civilization.

**Data processing:** preserve original downloads; create processed derivatives separately;
COG/GeoTIFF for rasters, GeoParquet for vectors, Zarr/NetCDF for multidimensional simulation
data, LAZ for lidar, 3D Tiles for streamed models, PMTiles for packaged tiles, Parquet for
structured metadata. Retain native source resolutions; do not unnecessarily upscale,
downscale or merge incompatible datasets.

**Solarpunk transformation:** real-world geography is a reference state, not a requirement to
recreate modern civilization exactly. Build transformation systems for rewilding, forest
regeneration, ecological restoration, renewable energy, green architecture, regenerative
agriculture, sustainable transportation, restored waterways, wildlife corridors, circular
manufacturing, community infrastructure. Maintain the distinction between original
observations and future simulated changes.

**Simulation integration:** design the database so future systems consume geographic
information without depending on individual source formats. Examples: terrain informs
elevation/water flow; soil informs vegetation suitability; climate informs ecosystem
conditions; vegetation affects evapotranspiration; rivers influence surrounding ecosystems;
ocean circulation influences coastal climate; buildings/infrastructure inform settlement
generation; biodiversity records inform species habitat models.

**Distribution requirements:** a self-contained distributable data package; no external
geographic APIs required for the basic world. Implement data manifests, version tracking,
attribution manifests, license manifests, regional data packages, compression, streaming and
caching, missing-data fallbacks, data integrity checks. Keep licensing records for every
distributed source and processed derivative.

**Implementation approach:** audit the existing OSM and satellite integration first; then
build the dataset registry and license validation; acquire the physical geography foundation
before detailed ecology/climate/infrastructure; do not download indiscriminately — evaluate
coverage, resolution, quality, overlap, storage and redistribution rights first.

## 9. Storage warning

A global Earth assembled from all these sources can become enormous:
- A global 10 m raster can require hundreds of gigabytes depending on bands/compression.
- Global building footprints can contain billions of geometries.
- Multi-year satellite imagery can consume terabytes.
- Lidar point clouds are extremely large.
- Climate simulations generate enormous time series.

Build hierarchical geographic streaming: global overview → continent → region → local area →
detailed environment. Separate the master development dataset library from the actual game
distribution package: maintain a large reference library in development, ship only the
optimized geographic information needed for gameplay.

The end goal: a planet that feels geographically authentic, ecologically interconnected and
full of possibility, with a future civilization that feels like it belongs there. Not just a
map of Earth — a world where the natural environment and human civilization have learned to
coexist beautifully.
