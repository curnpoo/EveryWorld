# Contract: addressing

Owner: `ed_geo`. Decisions: F1, F5. Evidence: `../reference/spikes/w4d-addressing.md`.

## 1. NodeId

```rust
/// body:8 | face:3 | level:5 | x:24 | y:24, most significant first.
#[repr(transparent)]
pub struct NodeId(u64);                           // private: only valid ids exist

impl NodeId {
    pub const MAX_LEVEL: u8 = 24;
    pub fn new(body: BodyId, face: u8, level: u8, x: u32, y: u32) -> Option<NodeId>;
    pub fn parent(self) -> Option<NodeId>;          // None at level 0
    pub fn from_bits(bits: u64) -> Option<NodeId>;
    pub fn children(self) -> Option<[NodeId; 4]>;   // order: (0,0) (1,0) (0,1) (1,1); None at level 24
    pub fn ancestor(self, level: u8) -> NodeId;
    pub fn neighbour(self, dx: i32, dy: i32) -> NodeId;   // crosses face edges
    pub fn contains(self, other: NodeId) -> bool;
    pub fn corner(self, i: u8) -> DVec3;            // unit vector, body-fixed
    pub fn centre(self) -> DVec3;
    pub fn grid_point(self, i: u32, j: u32, cells: u32) -> DVec3;   // the node's own vertex grid
    pub fn from_unit(body: BodyId, p: DVec3, level: u8) -> NodeId;
}
pub struct BodyId(pub u8);                           // index into the ed_space body catalog
```

- `x`, `y` are in `0..2^level`. Bits above `level` are zero. A `NodeId` with face > 5 or
  level > 24 is invalid and cannot be constructed.
- Ordering of the `u64` is: body, face, level, x, y. It is used as a sort key in manifests.
- The text form is `body/face/level/x/y` with the body as its catalog index, for example
  `3/2/12/1840/977`. Used in logs, file names and URLs.
- `neighbour` clamps offsets to one face width. A diagonal step across a cube corner has no
  unique answer; the x step crosses and the y step is clamped.

## 2. Faces and the mapping

Faces: 0 `+X`, 1 `-X`, 2 `+Y`, 3 `-Y`, 4 `+Z`, 5 `-Z` in the body-fixed frame (for Earth,
ECEF: `+Z` is the north pole, `+X` is latitude 0, longitude 0).

Face coordinates `(u, v)` in `[-1, 1]` map to the cube as:

| Face | Cube point |
|---|---|
| 0 | `( 1,  u,  v)` |
| 1 | `(-1, -u,  v)` |
| 2 | `(-u,  1,  v)` |
| 3 | `( u, -1,  v)` |
| 4 | `(-v,  u,  1)` |
| 5 | `( v,  u, -1)` |

The lattice coordinate `i` in `0..=n` (`n = 2^level`, or a finer power of two for vertices
inside a node) maps to `u` by the **tangent warp**:

```
warp(i, n) = -1                      if i == 0
           =  1                      if i == n
           =  0                      if 2i == n
           = -warp(n - i, n)         if 2i > n
           = tan((2i/n - 1) * pi/4)  otherwise
```

Rules, each one a source of cracks if broken:

1. The warp is evaluated **from integers** `(i, n)`, never from a float `u`.
2. `tan` is the software implementation in the `libm` crate, not the platform's.
3. The sphere point is `normalize(cube)` with the sum of squares taken in body `x, y, z`
   order, followed by `+ 0.0` on each component to remove negative zero.
4. The ellipsoid surface point is the unit vector scaled by the body's radii; heights are
   added along the geodetic normal. Height truth is `ed_planet`'s, not this contract's.

Node area varies by 1.40x across a face and edge length by 1.41x.

## 3. Edges and ownership

- A lattice point on a face edge or corner must be bit-identical from every face that
  contains it. The same holds between a node and its parent, children and neighbours.
- A node's terrain grid is `(2^k + 1)` vertices per side and shares its border vertices with
  its neighbours.
- A feature that crosses node edges (road, building, parcel) is **owned by one node**: the
  node at the owner level that contains the feature's anchor point. The anchor is the first
  vertex of the source geometry in source order; for a polygon, the vertex with the smallest
  `(lattice x, lattice y)` at level 24. Other nodes do not emit it.
- A generator may read source data within a margin outside its node. The margin per chunk
  kind is fixed in `generation.md`.

## 4. ObjectId

```rust
/// Stable identity of a generated or authored object. 128 bits.
#[repr(transparent)]
pub struct ObjectId(pub u128);

impl ObjectId {
    /// First 16 bytes of BLAKE3 over the fields below, each length-prefixed.
    pub fn generated(dataset: &str, feature: &str, generator: &str, sub: u32) -> ObjectId;
    /// 128 random bits with the top bit set. Stored in the layer file that creates it.
    pub fn authored(random: u128) -> ObjectId;
    pub fn is_authored(self) -> bool;   // top bit
}
```

- `dataset` is a registry id (`overture-buildings`). `feature` is the source's stable id (the
  Overture GERS id). `generator` names the rule that made the object (`building.shell`,
  `street.lamp`). `sub` counts objects the same rule made from the same feature, in the
  generator's deterministic order.
- Generated ids have the top bit clear. Procedural fill with no source feature uses
  `dataset = "parcel"` and the parcel's own `ObjectId` in hex as `feature`.
- An `ObjectId` never encodes a `NodeId`, a rung, a data version or a generator version. It
  must survive a re-bake, a data-version update (when the source id survives) and LOD changes.
- The renderer carries the low 32 bits per instance for picking; the editor resolves them to
  the full id through the node's chunk.

## 5. Frames

- **Body-fixed** (f64): the truth for all positions. For Earth, ECEF.
- **Node-local** (f32): origin at the node's centre on the ellipsoid, axes east, north, up
  at that point. All chunk geometry is stored in its node's frame. At L15 (about 300 m)
  f32 gives better than 0.1 mm.
- **Camera-relative** (f32): one origin per view per frame; the renderer converts node
  origins in f64 and concatenates in f32 (P1-D7).
- **Sim region** (f64): defined in `sim.md`.
- Slippy tiles, latitude and longitude appear only inside `ed_data` and at the editor's
  go-to box.

## 6. Tests

A1 to A9 are implemented in `crates/ed_geo/tests/addressing.rs`. A10 arrives with the first
generator that emits per node (Phase 3).

| # | Test |
|---|---|
| A1 | Round trip: `new` then field accessors; invalid inputs return `None` |
| A2 | `parent`/`children`/`ancestor`/`contains` agree for random nodes at every level |
| A3 | `neighbour` across all 12 cube edges and 8 corners returns a node that shares the edge |
| A4 | Edge agreement: every border lattice point of every face has a bit-identical twin on a neighbouring face, at 1,025 points per edge |
| A5 | Parent and child lattice points are bit-identical from L0 to L24 |
| A6 | `from_unit(corner(i))` and `from_unit(centre())` return the node or an edge neighbour; random points land in exactly one node per level |
| A7 | Node area and edge ratios across a face stay under 1.45 |
| A8 | Golden vectors: 64 fixed `(NodeId, corner)` pairs with their expected bits, identical on macOS, Windows and Linux |
| A9 | `ObjectId::generated` golden values; changing any field changes the id; `authored` ids never collide with generated ids |
| A10 | Ownership: a segment crossing four nodes is emitted by exactly one |
