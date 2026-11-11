# Architecture references (owner, 2026-11-10)

Twelve images the owner supplied as the direction for the built world, alongside the
lookbook and asset guide in the parent folder and `../spec.md`. They are **reference only**:
third-party images with no licence recorded. They are never baked into a pack, never used
as textures and never shipped. More are expected; add them here with the next number.

## What each one shows

| # | File | What to take from it |
|---|---|---|
| 01 | `01_earth_sheltered_glass_house.png` | A house set into a slope: turf roof continuous with the hill, one long curved glazed wall, solar on the roof, a kitchen garden in front |
| 02 | `02_timber_house_hex_solar_tethered_turbines.webp` | A traditional timber-frame and tile house kept as it is and retrofitted: hexagonal solar on the roof planes, tethered airborne turbines, a greenhouse bay, fruit trees, a fenced garden |
| 03 | `03_bamboo_shingle_shells.png` | Free-form shingled shells over open bamboo structure, stepping stones over ponds, forest pressed up against the building |
| 04 | `04_solar_dome_burrow_house.webp` | A burrow house with a glazed solar dome, round door, cold frame, small wind rotors; rounded plaster forms |
| 05 | `05_cob_house_solar_garden_pond.webp` | A low earthen house with a full glazed south wall and solar roof, standing in a wildflower garden beside a lily pond |
| 06 | `06_earthship_bottle_walls.webp` | Earthship: sculpted adobe, bottle walls, a slanted greenhouse front, a vertical-axis turbine as a landmark |
| 07 | `07_vertical_forest_tower.webp` | A tower with trees and shrubs on every cantilevered balcony |
| 08 | `08_green_facade_tower.png` | A tower wrapped in a climbing-plant mesh facade |
| 09 | `09_alpine_solar_cabin.webp` | An A-frame whose whole roof is solar, a dry-stone wall, a small turbine, meadow and stream to the door |
| 10 | `10_ivy_timber_facade.png` | A timber and glass facade with planted balconies and climbers, a dense planted border, a brick path |
| 11 | `11_biophilic_interior.webp` | An interior of curved plaster and timber with indoor planting, a growing cabinet, cove lighting |
| 12 | `12_geothermal_hillside_house.webp` | A rounded hillside house with terraces, hot pools and steam, a windmill tower, steps cut into rock |

## What the set says as a whole

1. **Buildings grow out of the ground.** Berms, turf roofs, terraces and steps. Five of the
   twelve are partly buried.
2. **Planting is part of the building**, not something beside it: balconies with trees,
   climbers on facades, green roofs, gardens up to the wall.
3. **Energy is visible and designed**: solar as the roof itself (planes, hexagons, a dome),
   small wind in several forms, geothermal steam.
4. **Curves and hand-made materials** sit beside straight timber and glass: plaster, cob,
   adobe, shingle, bamboo, dry stone, reclaimed bottles.
5. **Existing vernacular buildings are kept and retrofitted** (02, 09, 10). The solarpunk
   world is the real one, changed; not a replacement of it.
6. **Water and food are at the door**: ponds, streams, kitchen gardens, cold frames.
7. **Two scales**: the small dwelling in landscape (most of the set) and the planted tower
   (07, 08).

## What this requires of the engine and the kit

These are consequences for the plans; the decisions that carry them are cited.

| Requirement | Why | Where it lands |
|---|---|---|
| **Sockets on parts and prefabs** for planting, solar, wind and climbers, filled by rule per region and per building | Points 2 and 3. A balcony part must be able to carry a tree instance; a roof plane must be able to carry panels | `phases/phase-3.md` P3-D2a |
| **Roof and wall surfaces that take ground cover** | Turf roofs and green walls are the same GPU-instanced cover as the ground, on a building surface | P3-D2a, P3-D7 |
| **A prefab may carry a terrain patch** | Bermed and terraced buildings reshape the ground they sit in | P3-D9 (`terrain_patch` bound to a `place`) |
| **Free-form hero prefabs beside the 2 m grid kit** | Shells, domes and sculpted adobe do not fit a panel grid. They are authored whole in Blender and placed or chosen by rule | P3-D2b |
| **Retrofit as a style layer** | Point 5. The regional vernacular generator stays; a second table adds solar, planting, glazing bays and gardens to it | P3-D6 |
| **Lot dressing**: garden, pond, path, fence, fruit trees generated per parcel | Point 6 | P3-D5 parcels, P3-D7 |
| **Tower planting at city scale** | 07 and 08 across a skyline is many tree instances far from the ground; they need the foliage ladder's cluster and proxy rungs like any forest | P3-D7, Phase 3 budgets |
| **Interiors follow the same language** | 11 | `phases/phase-8.md` P8-D4 |
| **Lighting must flatter planting on buildings**: transmission through leaves, soft ambient in shade, glass that reflects sky and garden | Every image | `phases/phase-4.md` P4-D5, P4-D6, P4-D10 |

## Mapping to the five regional families (`../spec.md` §3)

| Family | References |
|---|---|
| A Temperate forest | 01, 02, 05, 10 |
| B Urban rewilding | 07, 08, 10 |
| C Arid desert | 06, 12 |
| D Tropical and coastal | 03 |
| E Arctic and alpine | 09, 12 |
| Shared forms (burrow, dome) | 04 |
| Interiors | 11 |
