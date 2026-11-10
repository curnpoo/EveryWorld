# Contract: generation

Owner: `ed_world`. Decisions: F2, F4, F5, F8, D10. Evidence:
`../reference/determinism.md`, `../reference/spikes/w4e-streets.md`.

## 1. The function

```rust
pub struct GenInput<'a> {
    pub node: NodeId,
    pub rung: Rung,                    // R0..R4
    pub kinds: ChunkKinds,             // which chunk kinds to produce
    pub seed: u64,                     // world seed
    pub version: GenVersion,           // generator version + every switch that changes output
    pub data_version: u32,             // data/versions/vN.toml
    pub source: &'a dyn SourceView,    // pinned inputs for this node plus margin
}
pub fn generate(input: &GenInput) -> Result<Vec<Chunk>, GenError>;
```

`generate` is a pure function of `GenInput`. The client, the tile service and the server
call the same function and must get the same bytes.

`SourceView` gives node-space data only: elevation samples, land cover, climate, road
segments with tags, building footprints, water, parcels. It is filled by `ed_data` from a
source extract chunk (`pack.md`) or from the raw datasets during a bake. Nothing in
`ed_world`'s public API names a slippy tile.

## 2. Determinism rules

A generator must not:

1. Iterate an unordered container where the order can reach the output. Use `BTreeMap`,
   sorted vectors, or sort before emitting.
2. Read environment variables, the clock, thread ids, addresses or global mutable state.
   Every switch is a field of `GenVersion`.
3. Continue after a missing input. A missing required input is `GenError::MissingSource`;
   there is no fallback world. Optional inputs are declared optional in the data version.
4. Use randomness that is not derived from `(seed, ObjectId or NodeId, purpose tag)`.
5. Depend on the order in which nodes are generated, or on any other node's output. Shared
   results (a road's solved heights) are computed from the same inputs by both nodes or
   owned by one (see `addressing.md` section 3).
6. Reduce floats in thread order. Parallel work is merged in index order.
7. Call platform maths functions for values that affect topology or quantised output across
   platforms; use `libm`.

Output ordering: instances sorted by `ObjectId`; mesh vertices in generator order, which must
itself follow sorted feature order.

## 3. Output per rung

| Rung | Node levels | Chunk kinds | Content | Source margin |
|---|---|---|---|---|
| R0 parts | L13 to L15 | `instances`, `collision`, `nav`, `ground` | Part instances (prefab or kit part id, transform, material variant, `ObjectId`); collision shapes; lane graph; ground cover parameters | 150 m |
| R1 shells | L10 to L13 | `shells` | One mesh per building (footprint extrusion with roof form, about 50 to 500 triangles) with a facade material id and style row; tree clusters | 50 m |
| R2 proxy | L8 to L11 | `proxy` | One merged simplified mesh per node with a small atlas; canopy mesh | 0 |
| R3 terrain-baked | L0 to L12 | `terrain`, `layers` | Height grid; albedo, roughness and emissive layers with buildings and roads baked in as tint, bump and light | 0 |
| R4 imagery | L0 to L8 | `layers` | Albedo from the imagery composite; night-light emissive | 0 |

Level ranges overlap so the selector can cross-fade. Exact ranges are constants in
`ed_world` and part of `GenVersion`.

Consistency between rungs, each tested:

- **Colour:** a building's style row fixes one albedo. R1's facade, R2's atlas texel and
  R3's tint use that value.
- **Silhouette:** R1 and R2 use the same footprint and the same height from section 4.
- **Light:** R3 and R4 emissive are computed from the same density that places R0 street
  lights, normalised so that summed radiance is continuous across the R2/R3 switch.
- **Ground:** every rung samples the same height truth; an R0 instance sits on the R3 height
  grid of its own node within 2 cm.

## 4. Building height rule

Order of preference, first that applies:

1. Measured height from the source (`height`), minus `min_height`.
2. `num_floors` x floor height. Floor height by region table (default 3.1 m; ground floor
   4.2 m for commercial classes).
3. Class and footprint-area table per region family (five families, `solarpunk/spec.md`).
4. Density default: the median of rule 1 to 3 heights of buildings within 250 m; if fewer
   than five, the region family's default.

The tables are data files versioned with `GenVersion`. A height is quantised to 0.1 m before
use. The rule that produced a height is stored with the building for debugging.

## 5. Streets

Input is whole Overture segments with connectors, split at every partial-segment rule
boundary, joined with OSM tags by way id (`plans/streets.md`). The solver output per owner
node: cross-sections, junction polygons, solved heights, decks and tunnels, lane graph,
parcels. Segments are owned per `addressing.md` section 3; a solved height on a node edge
must be identical from both sides.

## 6. Interiors hook

R0 buildings carry, per storey, a floor polygon and door positions in the `instances` chunk
metadata. Nothing generates interiors before Phase 8; the hook exists so that building
shells are not redesigned later (D11).

## 7. Tests

| # | Test |
|---|---|
| G1 | Golden hashes per chunk kind for the six reference places and two synthetic inputs, on macOS and Windows (the Phase 0 suite, moved to chunk output) |
| G2 | Same input twice in one process, and across processes: identical bytes |
| G3 | Generating node A then B equals B then A |
| G4 | Edge: terrain border heights and road heights are bit-identical between neighbours and between a node and its parent's border |
| G5 | Ownership: every source feature in a 3 x 3 block appears in exactly one node's R0 output |
| G6 | Rung consistency: colour, silhouette height, emissive sum and ground height agree as in section 3 |
| G7 | A missing required input returns `MissingSource` |
| G8 | `ObjectId`s are unchanged when only `GenVersion` changes, and when the data version changes but the source id survives |
| G9 | No generator source file contains `HashMap` iteration, `std::env`, `Instant` or `SystemTime` (a lint in `xtask`) |
| G10 | Height rule: table-driven cases for each of the four rules |
