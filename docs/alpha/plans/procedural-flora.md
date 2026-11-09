# Procedural flora: every tree and plant different

Goal: any species, anywhere on Earth, with no two trees alike, chosen by real
climate/geology/land cover, all deterministic. Today a species is **code** (one
builder fn per `TreeKind` in `trees/mod.rs`) with 3 baked variants shared by
every instance. This plan makes species **data**, draws a **genome** per tree,
and pays for uniqueness in tiers by distance.

Status: **step 1 landed** (2026-11-04). Steps 2-6 are open: pick up the next one.

## Pieces (where they live)

| Piece | File | State |
|---|---|---|
| `SpeciesDef` rows (leaf palette, far-card budget, trunk collider, genome ranges) | `ed_world/src/trees/species.rs` `SPECIES`, `def(kind)` | landed |
| `Genome` (height, width, leaf tint), `Genome::sample(kind, seed, Conditions)` | same | landed, not yet used per instance |
| `build_genome(kind, variant, far, &Genome)`; `build()` = default genome (identity, tested) | `trees/mod.rs` | landed |
| Trunk colliders (one static compound per tree chunk, full-detail tier only) | `ed_game/src/foliage.rs` `tree_colliders` | landed, unverified in game |
| Rock colliders (trimesh from the rock chunk mesh, within 90 m) | `ed_game/src/near_chunks.rs` | landed, unverified in game |
| Niche-based selection | `ed_world/src/foliage.rs` `species()` is still a biome table | step 2 |

## Design

1. **Species are rows.** Growth ranges, climate niche (temp, precip, elevation,
   wetness, lithology, shade tolerance), generator choice (branching, palm,
   conifer whorl, cactus, buttressed emergent). Redwood, kapok, rainforest
   canopy trees are rows, not code paths. Art tables stay data.
2. **Selection by niche.** Score every species against `Climate::at`, elevation,
   WorldCover canopy fraction and Macrostrat lithology, draw by weight. Mixed
   stands and tree lines fall out; nobody writes "redwood goes here".
3. **Genome per instance.** Deterministic draw (hash of tile/cell) from the
   species ranges, shifted by `Conditions { vigor, crowding }` (`foliage::vigor`
   already gives stunting by altitude and biome): stunted at the treeline, tall
   and narrow in dense stands, spreading in the open.
4. **Uniqueness in tiers.**

   | Distance | Representation | Uniqueness |
   |---|---|---|
   | < ~30 m | mesh grown from the tree's own genome on a worker, LRU cached | true |
   | leaf-card tier (< 800 m) | per-chunk merged mesh (`bake_far`), built on the CPU | true, no extra draw calls |
   | far | impostors from a bank of 8-16 genome buckets per species; per-instance scale/tint/lean in the shader | hidden by jitter |

   GPU instancing needs shared meshes, which is why 3 variants was the limit.
   Merged chunk meshes are one draw call and built on the CPU, so each tree
   can have its own geometry there for build-time cost only.
5. **Lazy baking.** Models and impostor layers on demand with a cache, not all at
   startup (30 species x 16 buckets is ~500 models).
6. **Ground cover and rocks use the same system.** `ground_scatter` kinds become
   species rows with niche + genome; rocks get a lithology-driven genome
   (`surface.lith`). One `LocalConditions` lookup (climate, elevation, slope,
   wetness, lithology, canopy) feeds trees, cover, rocks, later buildings.
   Weather only modulates it (season, snow, drought); it never decides what grows.

## Steps

1. **Landed.** `SpeciesDef`/`Genome`, palettes and card budgets moved out of code, identity-tested.
2. Niche scoring replaces `foliage::species()`; test against Sahara, Manaus, Aix, Zermatt, NYC, Pacific NW.
3. Use the genome per instance: add `genome`/seed to `TreeInstance`, pass non-uniform scale in `bake_far`/`append_transformed`, shader jitter for impostors, bucketed bank (`impostor::layer(kind, bucket)`), lazy atlas.
4. Near tier: per-tree grown meshes (`build_genome` on a worker, LRU).
5. Ground cover + rocks onto the same rows/genomes.
6. New species as rows: coast redwood, kapok emergent, rainforest canopy, then regional oaks, eucalypt, etc. Needs generator params exposed (the `BranchSpec` tables inside each builder are the natural genome: ranges for `children`, `angle`, `len`, `leaf.size`).

## Risks

- Genome generation must stay cheap enough for the near tier or it will hitch (budget test: `every_species_grows_within_budget`).
- Data-driven species need art review (`--lookdev` boards).
- `Genome::apply` post-scales a grown mesh; sway amplitudes are not rescaled. Fine for +-25 %, revisit if ranges widen.

## Collision (landed, simple on purpose)

- Trees: a trunk cylinder per tree from `SpeciesDef::trunk`, only for trees in a full-detail chunk (`NEAR_IN` 230 m), one compound `RigidBody::Static` per chunk, despawned with the chunk. Shrubs, ferns, grass have none.
- Rocks: `Collider::trimesh_from_mesh` of the rock chunk, built lazily within 90 m (3 builds/frame), removed beyond ~104 m. Exact fit, no hand-tuned shapes.
- Not covered: tree canopy for aircraft, impostor/card-tier trees (far from the player anyway), speed-scaled range for fast vehicles.
