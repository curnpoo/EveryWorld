# SOLARPUNK WORLD
## Modular Environment & Game Asset Production Specification

**Project:** Solarpunk World
**Category:** 3D Environment Art / Procedural Asset Generation
**Art Direction:** Stylized Realism / Optimistic Science Fiction
**Primary Goal:** Build a reusable library of modular architecture, vegetation, infrastructure, and environmental props for a large open-world game.

Companion reference images (saved alongside this spec in this folder):
- `Solarpunk Game Asset Guide.png`
- `Solarpunk World Lookbook.png`

---

# 1. GLOBAL ART DIRECTION

Create a visually cohesive futuristic world where advanced technology and natural ecosystems coexist.

### Visual characteristics
- Stylized 3D geometry with believable proportions.
- Realistic lighting and physically based materials.
- Slightly exaggerated architectural shapes and silhouettes.
- Rounded corners, beveled edges, large openings, and interesting roof profiles.
- Strong environmental storytelling.
- High-quality material detail without excessive surface noise.
- Saturated accent colors against natural earth tones.
- Abundant vegetation integrated into architecture.

Avoid:
- Generic cyberpunk architecture.
- Excessive neon.
- Sterile white futuristic buildings.
- Overly complex mechanical details.
- Photorealism that conflicts with the game's stylized aesthetic.
- Repetitive prefabricated-looking settlements.

Buildings should look functional, inhabited, and purpose-built for their environment.

# 2. WORLD SCALE AND MODULAR GRID

Use a consistent world scale across all assets.

### Base measurements

| Element | Dimension |
|---|---:|
| Human character | 1.8 m |
| Standard grid unit | 2 m |
| Small prop grid | 0.5 m |
| Standard wall module | 2 × 2 m |
| Large wall module | 4 × 2 m |
| Standard floor module | 2 × 2 m |
| Door opening | 1.2 × 2.4 m |
| Large entrance | 2.5 × 3 m |
| Standard floor height | 3 m |
| Small building | 4 to 6 m |
| Medium building | 6 to 12 m |
| Large landmark | 15 to 35 m |

All architectural assets should use a shared coordinate system.

**Grid rules:**
- Primary snapping increment: 2 m.
- Secondary snapping increment: 0.5 m.
- Building origins should be centered or positioned at a clearly documented corner.
- Doors, windows, floors, and roofs must align with their corresponding modules.
- Modular pieces must be reusable across multiple building configurations.

Use a consistent naming convention:

`BIO_RGN_ASSET_VARIANT`

Examples:
- `BIO_DES_WALL_ADOBE_A`
- `BIO_FOR_ROOF_SOLAR_B`
- `BIO_TRO_PILLAR_WOOD_A`
- `BIO_ARC_WINDOW_TRIPLE_A`

# 3. REGIONAL ASSET FAMILIES

Create five distinct architectural families.

Each family should have at least:
- 3 small residential buildings.
- 3 medium residential buildings.
- 2 large residential buildings.
- 1 community building.
- 1 utility or infrastructure building.
- 10 architectural detail props.

## A. TEMPERATE FOREST

**Theme:** Organic architecture, timber, glass, lush vegetation, and integrated renewable energy.

### Architectural language
- Asymmetrical rooflines.
- Large triangular windows.
- Exposed timber structures.
- Elevated walkways.
- Terraced balconies.
- Green roofs and living walls.
- Large overhanging eaves.

### Asset ideas

1. Forest Cabin
   - 6 × 8 m footprint.
   - Two-story structure.
   - Timber frame with large glass panels.
   - Asymmetrical pitched roof.
   - Solar panels integrated into roof.
   - Wraparound balcony.

2. Canopy Residence
   - 8 × 10 m footprint.
   - Elevated 3 m above ground.
   - Four structural support columns.
   - Large central living space.
   - Rooftop garden.
   - Elevated access bridge.

3. Eco Habitat Pod
   - 5 m diameter.
   - Rounded modular shell.
   - Large panoramic window.
   - Integrated planter sections.
   - Small solar canopy.

4. Treehouse Community
   - Multiple elevated residential modules.
   - Shared bridges and platforms.
   - Integrated elevators.
   - Central community garden.

### Materials
- Warm timber.
- Natural stone.
- Dark green metal.
- Transparent glass.
- Moss and climbing vegetation.
- Matte solar panels.

### Vegetation assets
- Large deciduous trees.
- Tall grass.
- Fern clusters.
- Wildflower patches.
- Hanging vines.
- Moss-covered rocks.

---

## B. URBAN REWILDING

**Theme:** High-density sustainable architecture with integrated greenery and advanced public infrastructure.

### Architectural language
- Mixed-height buildings.
- Terraced facades.
- Rooftop gardens.
- Vertical farming structures.
- Elevated transit.
- Large public plazas.
- Solar-glass facades.

### Asset ideas

1. Vertical Garden Residence
   - 12 × 12 m footprint.
   - 4 to 6 stories.
   - Modular floor sections.
   - Recessed balconies.
   - Vertical vegetation panels.
   - Solar-glass windows.

2. Sky Loft
   - 8 × 10 m footprint.
   - Elevated residential structure.
   - Cantilevered balconies.
   - Large glass walls.
   - Rooftop greenhouse.

3. Community Habitat
   - 20 × 20 m footprint.
   - Shared central courtyard.
   - Multiple residential entrances.
   - Ground-level community gardens.
   - Integrated pedestrian bridges.

4. Transit Hub
   - 15 × 25 m footprint.
   - Elevated light-rail platform.
   - Covered pedestrian areas.
   - Integrated solar canopy.
   - Green roof.

### Materials
- Architectural concrete.
- Brushed aluminum.
- Recycled composite panels.
- Blue-tinted glass.
- Green facade systems.
- Solar panels.

### Vegetation assets
- Vertical ivy systems.
- Rooftop trees.
- Planter modules.
- Street trees.
- Hanging gardens.
- Public park vegetation.

---

## C. ARID DESERT

**Theme:** Passive cooling, solar energy, water harvesting, and architecture shaped by desert landscapes.

### Architectural language
- Rounded adobe structures.
- Thick walls.
- Deep-set windows.
- Large archways.
- Shaded courtyards.
- Wind-catching towers.
- Sculptural solar canopies.

### Asset ideas

1. Desert Adobe Residence
   - 8 × 10 m footprint.
   - Thick exterior walls.
   - Rounded corners.
   - Large arched entrance.
   - Internal courtyard.
   - Rooftop solar array.

2. Solar Dome
   - 12 m diameter.
   - Geodesic or segmented shell.
   - Transparent upper sections.
   - Integrated solar collectors.
   - Central communal living space.

3. Oasis Home
   - 10 × 12 m footprint.
   - Multiple connected structures.
   - Shaded central garden.
   - Water collection roof.
   - Raised cooling vents.

4. Desert Tower
   - 5 × 5 m footprint.
   - 15 m tall.
   - Wind-capture openings.
   - Solar equipment at the top.
   - Water storage integrated into the base.

### Materials
- Sandstone.
- Warm adobe.
- Terracotta.
- Weathered bronze.
- Sand-colored composite panels.
- Dark solar glass.

### Vegetation assets
- Acacia-style trees.
- Desert grasses.
- Succulents.
- Cacti.
- Drought-resistant shrubs.
- Oasis palms.

---

## D. TROPICAL AND COASTAL

**Theme:** Open-air architecture, water-based settlements, lightweight structures, and abundant tropical vegetation.

### Architectural language
- Stilt-mounted buildings.
- Wide overhanging roofs.
- Open-air interiors.
- Elevated decks.
- Bamboo structural elements.
- Curved roof profiles.
- Floating platforms.

### Asset ideas

1. Island Villa
   - 10 × 12 m footprint.
   - Elevated 2 m above water.
   - Timber and bamboo structure.
   - Open central living space.
   - Large shaded deck.
   - Solar roof canopy.

2. Beach Bungalow
   - 6 × 8 m footprint.
   - Single-story construction.
   - Large sliding glass panels.
   - Deep roof overhang.
   - Outdoor shower and garden.

3. Tropical Treehome
   - 8 × 8 m footprint.
   - Elevated structure.
   - Four to six support columns.
   - Wraparound balcony.
   - Integrated rainwater collection.

4. Floating Community
   - Modular floating platforms.
   - Connected residential units.
   - Floating gardens.
   - Small docks.
   - Tidal energy infrastructure.

### Materials
- Bamboo.
- Treated timber.
- Bio-resin.
- Woven composite panels.
- Translucent roofing.
- Weather-resistant metal.

### Vegetation assets
- Palm trees.
- Mangroves.
- Broad-leaf tropical plants.
- Flowering shrubs.
- Coastal grasses.
- Floating vegetation.

---

## E. ARCTIC AND ALPINE

**Theme:** Warm, insulated architecture designed for extreme environments while maintaining a welcoming, sustainable aesthetic.

### Architectural language
- Steep roofs.
- Curved structural profiles.
- Thick insulated walls.
- Large triple-glazed windows.
- Protected entryways.
- Geothermal heating systems.
- Enclosed greenhouse spaces.

### Asset ideas

1. Mountain Chalet
   - 8 × 10 m footprint.
   - Two stories.
   - Steep roof.
   - Large central windows.
   - Timber and stone construction.
   - Integrated solar roofing.

2. Polar Lodge
   - 12 × 15 m footprint.
   - Elevated foundation.
   - Insulated exterior panels.
   - Protected entry vestibule.
   - Geothermal heating equipment.

3. Snow Bungalow
   - 6 × 8 m footprint.
   - Rounded exterior shell.
   - Large panoramic window.
   - Snow-resistant roof.
   - Integrated heat recovery ventilation.

4. Glacial Outpost
   - 15 × 15 m footprint.
   - Modular interconnected structures.
   - Elevated walkways.
   - Wind turbines.
   - Greenhouse attachment.

### Materials
- Dark timber.
- Stone.
- Insulated composite panels.
- Frosted glass.
- Brushed metal.
- Snow-resistant roofing.

### Vegetation assets
- Evergreen trees.
- Low shrubs.
- Frost-resistant grasses.
- Alpine flowers.
- Moss.
- Snow-covered rocks.

---

# 4. MODULAR BUILDING KIT

Create a shared library of construction components.

These components should be assembled into different building configurations rather than modeling every building independently.

### Required components

**Structural**
- Wall sections.
- Corner sections.
- Floor panels.
- Foundation blocks.
- Support columns.
- Structural beams.
- Roof supports.

**Architectural**
- Flat roof.
- Pitched roof.
- Curved roof.
- Dome roof.
- Overhanging roof.
- Arched entrance.
- Standard doorway.
- Large panoramic window.
- Circular window.
- Balcony.
- Exterior staircase.
- Railings.

**Functional**
- Solar panels.
- Solar-glass windows.
- Rainwater collectors.
- Ventilation units.
- Geothermal vents.
- Exterior lighting.
- Vertical farming modules.
- Water storage tanks.
- Small wind turbines.

**Decorative**
- Planters.
- Wall-mounted vegetation.
- Exterior pipes.
- Structural braces.
- Signage.
- Fabric shade canopies.
- Decorative lighting.

### Component dimensions

| Component | Size |
|---|---|
| Wall | 2 × 2 m |
| Large wall | 4 × 2 m |
| Floor | 2 × 2 m |
| Column | 0.5 × 0.5 m |
| Standard window | 1.5 × 1.5 m |
| Large window | 2 × 2 m |
| Door | 1.2 × 2.4 m |
| Balcony | 2 × 1 m |
| Roof panel | 2 × 2 m |
| Planter | 1 × 1 m |

All dimensions are starting specifications and can be adjusted for individual architectural styles.

# 5. ASSET ASSEMBLY RULES

The model should construct buildings using a hierarchy of reusable components.

### Example: Forest Cabin

**Root**
- ForestCabin_A

**Foundation**
- Foundation_4x4_A
- Foundation_4x4_B

**Structure**
- Wall_Wood_2x2_A
- Wall_Glass_2x2_A
- Corner_Wood_A
- Support_Beam_A

**Roof**
- Roof_Pitched_A
- Roof_Panel_Solar_A
- Roof_Trim_A

**Details**
- Balcony_Wood_A
- Railing_Wood_A
- Planter_Long_A
- Exterior_Light_A

**Vegetation**
- Vine_Wall_A
- Roof_Moss_A
- Planter_Fern_A

### Assembly process

1. Establish the building footprint.
2. Generate foundation geometry.
3. Place structural columns and beams.
4. Assemble exterior walls.
5. Insert windows and doors.
6. Add roof sections.
7. Attach balconies and exterior stairs.
8. Add functional technology.
9. Add vegetation.
10. Apply regional materials.
11. Generate collision geometry.
12. Generate LOD variants.

Components must align without visible gaps, intersections, or floating geometry.

# 6. VISUAL DESIGN LANGUAGE

Each biome should have its own visual identity.

However, all regions should share a common technological language.

### Shared design elements
- Integrated renewable energy.
- Curved architectural details.
- Large entrances.
- Visible structural systems.
- Environmental lighting.
- Subtle illuminated technology.
- Organic landscaping.

### Accent colors

| Biome | Primary | Accent |
|---|---|---|
| Forest | Deep green | Warm amber |
| Urban | Concrete gray | Cyan |
| Desert | Sandstone | Terracotta |
| Tropical | Natural wood | Turquoise |
| Arctic | Slate blue | Soft white |

Use accent colors sparingly on technology, structural details, lighting, and architectural features.

Avoid making every building look identical through excessive shared accent colors.

# 7. VEGETATION AND ENVIRONMENT ASSETS

Create vegetation as independent reusable assets.

Each vegetation type should have:
- Small variant.
- Medium variant.
- Large variant.
- At least 3 shape variations.
- Seasonal or biome-specific material variants where appropriate.

### Suggested asset library

**Forest**
- Tall deciduous trees.
- Pine trees.
- Ferns.
- Wildflowers.
- Tall grass.
- Hanging vines.

**Urban**
- Street trees.
- Rooftop trees.
- Vertical gardens.
- Decorative grasses.
- Planter clusters.

**Desert**
- Acacia trees.
- Succulents.
- Cacti.
- Dry grasses.
- Desert shrubs.

**Tropical**
- Palm trees.
- Mangroves.
- Broad-leaf plants.
- Flowering trees.
- Coastal grasses.

**Arctic**
- Snow-covered pines.
- Alpine shrubs.
- Frost-resistant grasses.
- Moss clusters.

Vegetation should be distributed in natural clusters rather than evenly spaced rows.

# 8. SETTLEMENT LAYOUT GENERATION

Buildings should be assembled into settlements that feel organically planned.

### Forest settlement
- Small residential structures around clearings.
- Elevated walkways between buildings.
- Central community garden.
- Dense tree coverage.
- Small water features.
- Solar infrastructure integrated into rooftops.

### Urban settlement
- Mixed building heights.
- Dense residential clusters.
- Elevated transit.
- Large public green spaces.
- Rooftop farming.
- Pedestrian corridors.

### Desert settlement
- Structures clustered around water.
- Shaded connecting pathways.
- Solar farms outside residential areas.
- Central communal courtyard.
- Wind-capture towers.
- Sparse, drought-resistant vegetation.

### Tropical settlement
- Structures following the coastline.
- Elevated walkways.
- Floating platforms.
- Mangrove protection zones.
- Open public gathering spaces.
- Tidal energy infrastructure.

### Arctic settlement
- Compact building clusters.
- Wind-protected courtyards.
- Covered walkways.
- Greenhouse structures.
- Geothermal infrastructure.
- Wind turbines positioned outside residential areas.

# 9. GAME ASSET TECHNICAL REQUIREMENTS

Create assets with real-time game performance in mind.

### Geometry
- Use clean, efficient topology.
- Apply bevels to important silhouette edges.
- Avoid unnecessary subdivisions.
- Use consistent scale.
- Separate moving components from static geometry.

### Materials
- Use physically based materials.
- Share materials wherever possible.
- Create texture atlases when beneficial.
- Use vertex colors for secondary variation.
- Keep roughness variation subtle and believable.

### Lighting
- Support real-time global illumination where available.
- Use realistic sunlight and shadow direction.
- Include emissive elements only where functionally justified.
- Avoid excessive bloom.
- Ensure architecture remains readable at different times of day.

### Collision
- Use simplified collision meshes.
- Keep walkable surfaces accessible.
- Ensure doorways and passages have adequate clearance.
- Separate decorative geometry from collision geometry.

### LOD
- LOD0: Full detail.
- LOD1: Reduced secondary geometry.
- LOD2: Simplified structure and materials.
- LOD3: Distant silhouette representation.

Prioritize LOD for large buildings, trees, and infrastructure visible across long distances.

### Asset organization

Use a consistent folder structure:

Assets/
- Architecture/
  - Shared/
  - Forest/
  - Urban/
  - Desert/
  - Tropical/
  - Arctic/
- Vegetation/
  - Forest/
  - Urban/
  - Desert/
  - Tropical/
  - Arctic/
- Infrastructure/
- Props/
- Materials/
- Textures/
- Prefabs/
- Collision/
- LOD/

# 10. PRODUCTION ORDER

Build the asset library in stages.

**Stage 1: Foundation**
- Establish the coordinate system.
- Create modular wall, floor, and roof pieces.
- Create shared materials.
- Establish scale references.

**Stage 2: First Biome**
- Build the Forest Cabin.
- Build the Canopy Residence.
- Build the Forest Community.
- Create associated vegetation.
- Validate modular assembly.

**Stage 3: Regional Expansion**
- Build the five architectural families.
- Create regional materials.
- Create biome-specific vegetation.
- Create infrastructure assets.

**Stage 4: Settlement Generation**
- Create reusable settlement layouts.
- Add modular roads and pathways.
- Add environmental dressing.
- Introduce procedural placement variation.

**Stage 5: Optimization**
- Generate LODs.
- Generate collision.
- Consolidate materials.
- Test asset draw calls.
- Test visibility at different viewing distances.

# 11. FINAL QUALITY REQUIREMENTS

Every completed asset should meet the following criteria:

- Clearly identifiable silhouette.
- Correct real-world scale.
- Consistent visual language.
- Regionally appropriate construction.
- Functional architectural logic.
- Reusable modular components.
- Clean material assignments.
- Correct pivots and origins.
- Optimized geometry.
- Collision and LOD support.
- No obvious floating or intersecting components.

**Important:** Do not generate an entire settlement as one unique, monolithic model. Build a reusable asset library first, then assemble buildings and settlements from those components.

The goal is to create a world that can expand indefinitely through modular construction while maintaining a consistent visual identity.

**Creative north star: A future where every region has its own architectural personality, where technology supports nature, and where the world feels alive at every scale.**