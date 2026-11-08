# Plan: GPU Building Instancing

Status: PLANNED (2026-11-07). Targets 60 fps in dense cities (Shibuya, Manhattan, Paris) under Native Metal renderer.

---

## 1. Context & Motivation

In cities, buildings dominate scene complexity:
- **Draw Call Overhead**: Even when meshes share identical materials and pipelines, `ed_metal::world::WorldRenderer` currently issues individual `re.drawIndexedPrimitives` calls per item / cluster.
- **Micro-Part Triangulation**: In `ed_world::bparts`, every window frame, pane, mullion, sill, door, AC unit, and balcony is triangulated and welded into the tile vertex buffer on the CPU. A single dense tile generates **20,000+ individual window polygons**, pushing mesh sizes to 400k–800k triangles and consuming dozens of megabytes per tile.
- **Far Skyline Overload**: Extruding unique polygonal footprints all the way to 16 km results in 15M–25M triangles submitted per frame in cities.

GPU instancing addresses all three bottlenecks:
1. **Low-overhead draws**: Combining identical geometry into instanced batches (`drawIndexedPrimitives:instanceCount:`).
2. **Micro-part instancing**: Replacing welded window/door geometry with canonical instanced components.
3. **Massing / Prism instancing**: Rendering skyline buildings (1.5 km – 20 km) as instanced boxes/prisms from compact per-instance buffers.

---

## 2. Architecture & Implementation Tiers

```
  Tier 1: Metal Renderer Instancing (world.metal + ed_metal::world.rs)
          │  Support `instanceCount`, instance buffer (model, tint, emissive)
          ▼
  Tier 2: Far Building Box / Prism Instancing (1.5 km – 20 km)
          │  Unit cube / prism prototype, instanced OBBs per tile (pos, scale, yaw, color)
          ▼
  Tier 3: Micro-Part Instancing (bparts.rs + library)
             Canonical window/door/AC prototypes, per-tile instance arrays
```

---

### Tier 1: Renderer-Level Instancing (`ed_metal` + `ed_game::native`)

Enable true hardware instancing in the native Metal pipeline.

#### Data Structures (`ed_metal::world`)
```rust
#[repr(C)]
pub struct InstanceU {
    pub model: [[f32; 4]; 4],
    pub tint: [f32; 4],
    pub emissive: [f32; 4],
}
```

#### Shader Changes (`crates/ed_metal/shaders/world.metal`)
- In `world_vs`, bind `device const InstanceU* instances [[buffer(4)]]`:
```metal
vertex VOut world_vs(uint vid [[vertex_id]],
                     uint iid [[instance_id]],
                     device const Vtx* verts [[buffer(0)]],
                     constant FrameU& F [[buffer(1)]],
                     constant DrawU& D [[buffer(2)]],
                     device const InstanceU* insts [[buffer(4)]]) {
    Vtx v = verts[vid];
    float4x4 model = (D.flags.y > 0.5) ? insts[iid].model : D.model;
    float4 tint = (D.flags.y > 0.5) ? insts[iid].tint : D.tint;
    float4 w = model * float4(float3(v.p), 1.0);
    // ... rest of projection
}
```

#### Renderer Changes (`crates/ed_metal/src/world.rs`)
- Allocate a dynamic `MTLBuffer` ring for frame instance data (`iarena`).
- In `WorldRenderer::encode`:
  - Group draws by `(mesh_id, pipeline_mode, textures, blend_mode)`.
  - Where group size > 1, copy `InstanceU` records into the instance arena.
  - Dispatch via:
    ```rust
    re.setVertexBuffer_offset_atIndex(Some(&self.iarena), offset, 4);
    re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
        MTLPrimitiveType::Triangle,
        m.index_count,
        MTLIndexType::UInt32,
        self.iarena.buf(m.ib),
        self.iarena.offset(m.ib),
        instances.len(),
    );
    ```

---

### Tier 2: Far Building Box / Prism Instancing (1.5 km – 20 km)

For mid-to-far distances, replace unique footprint meshes with instanced geometric primitives.

#### Data Model (`ed_world::buildings`)
- When generating building massing for tiles beyond `far_lod_m` (or during coarse tile generation):
  - Extract Oriented Bounding Box (OBB):
    - `center: Vec3`
    - `half_extents: Vec3` (width, height, depth)
    - `yaw: f32`
    - `wall_color: [f32; 4]`
    - `roof_color: [f32; 4]`
- Emit as `BuildingPrisms { instances: Vec<PrismInstance> }` instead of a merged `MeshData`.

#### Prototypes
- 1 shared canonical unit cube mesh (12 triangles, centered at origin or base).
- Optional pitched/gable prism prototype (16 triangles).

#### Payoff
- A 500-building city tile requires **one single draw call** and **tens of kilobytes** of data instead of a multi-megabyte mesh.
- Can extend building render distance smoothly to 15–20 km with negligible draw call cost.

---

### Tier 3: Canonical Micro-Part Instancing (`ed_world::bparts`)

Decouple windows, doors, and rooftop equipment from the wall mesh.

#### Prototype Palette (baked once in `library`)
- Standard sash window (1.0 m × 1.5 m)
- Tall arched window (1.2 m × 2.2 m)
- Modern casement / ribbon window (2.0 m × 1.2 m)
- Residential entry door (1.0 m × 2.1 m)
- Commercial shopfront bay (3.0 m × 2.8 m)
- Rooftop AC / chiller unit (1.5 m × 1.5 m × 1.0 m)
- Fire escape landing + rail

#### Generator Refactoring (`crates/ed_world/src/bparts.rs`)
- Walls retain openings in the facade polygon (`mk::wall_with_openings`).
- Instead of procedural vertex extrusion inside `bparts::window()`:
  - Select nearest matching prototype from the palette.
  - Emit an `InstancePlacement`:
    - `basis: Basis` (facade orientation and translation)
    - `scale: Vec2` (minor stretch to fit bay width/height)
    - `trim_color: [f32; 4]`
    - `glass_color: [f32; 4]`
- The tile builder collects instances per prototype and passes them to `ed_game` alongside the wall/roof shell meshes.

#### Payoff
- Reduces detailed city tile triangle counts from **~800k to ~200k** (a 60–75% reduction).
- Reduces background worker CPU time for tile generation by 3–4×.
- Massive memory savings in GPU vertex buffers.

---

## 3. Order of Work

1. **Phase 1: Metal Renderer Instancing (Tier 1)**
   - Update `world.metal` with `InstanceU` buffer and `[[instance_id]]` support in `world_vs`.
   - Update `ed_metal::world.rs` with `InstanceArena` and `drawIndexedPrimitives...instanceCount`.
   - Update `ed_game::native.rs` to batch identical mesh entities.
   - Verify with existing library placements (rocks, street props).

2. **Phase 2: Far Building Prism Instancing (Tier 2)**
   - Add unit box prototype to `ed_metal` or `library`.
   - Update `buildings::build_massing` to emit prism instance buffers for far tiles.
   - Benchmark Shibuya skyline: verify draw calls and triangle reduction.

3. **Phase 3: Micro-Part Instancing (Tier 3)**
   - Define canonical window/door/HVAC prototype meshes.
   - Refactor `bparts::window`, `door`, `ac_unit` to emit instance placement records.
   - Connect instance lists through `tilegen` and `streaming` into the native renderer.
   - Visual inspection across Paris, Tokyo, and New York architecture kits.
