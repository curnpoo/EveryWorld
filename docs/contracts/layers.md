# Contract: override layers and deltas

Owners: `ed_pack` (format), `ed_editor` (writer), `ed_world` (application), `ed_store`
(deltas). Decisions: F5, F6.

## 1. The rule

```
world(node, rung) = apply(deltas, apply(layers, generate(seed, data version, node, rung)))
```

Layers are authored and ship with the game. Deltas happen at runtime and are stored per
world. Both use one vocabulary of operations, so one piece of code applies them.

## 2. Files

```
layers/
  layers.toml                     order, names, default on or off
  base_hubs/
    earth/2/10/460/244.toml       one file per L10 node that the layer touches
  spaceports/
    ...
```

- Text (TOML), committed to the repository, diffable and mergeable.
- An operation is stored in the file of the L10 node that contains its anchor position.
- The bake compiles each file to a `layer` chunk (`pack.md` kind 32). The editor reads and
  writes the TOML and reloads the affected nodes.
- Positions are body-fixed f64 in metres. Rotations are quaternions relative to the local
  east, north, up frame at the position, so "upright" is the identity anywhere on the planet.

## 3. Operations

Every operation has `id` (an authored `ObjectId` in hex), `op`, and the fields below.

| `op` | Fields | Effect |
|---|---|---|
| `place` | `prefab`, `position`, `rotation`, `scale`, `clear` (footprint margin in metres, default from the prefab) | Adds the prefab. Generated objects whose footprint intersects the prefab footprint plus `clear` are suppressed; the street generator treats the footprint as an obstacle; terrain is flattened under it per the prefab's manifest |
| `replace` | `target` (`ObjectId`), then either `prefab` or `params` (style, height, colour, roof) | Swaps one generated object for a prefab at the same anchor, or regenerates it with changed parameters |
| `suppress` | `targets` (list of `ObjectId`) or `volume` | Removes generated objects |
| `marker` | `name`, `kind` (spawn, landing_pad, poi, trigger, ...), `position`, `rotation`, optional `volume`, `tags` | A named place for gameplay. No visual in play mode |
| `mission` | `name`, `giver`, `steps` (each bound to marker ids), `rewards`, `requires` | A mission definition |
| `road_patch` | `add` (polyline with class and tags), `remove` (segment `ObjectId`s), or `profile` (segment id, cross-section) | Edits the street graph before the solver runs |
| `terrain_patch` | `shape` (circle, polygon or spline), `mode` (set, raise, smooth), `height`, `falloff`, optional `surface` | Local edit of height or surface class |

`volume` is a box (centre, half extents, rotation), a sphere, or a polygon prism.

Example:

```toml
[[op]]
id = "8f3a...c1"
op = "place"
prefab = "hubs/aix_market_hall"
position = [4581204.118, 381377.902, 4407066.540]
rotation = [0.0, 0.0, 0.3827, 0.9239]

[[op]]
id = "8f3a...c2"
op = "marker"
name = "aix_hall_spawn"
kind = "spawn"
position = [4581210.4, 381380.1, 4407070.2]
```

## 4. Application rules

1. Layers apply in the order in `layers.toml`; within a layer, operations apply sorted by
   `id`. The result must not depend on file order or node visiting order.
2. A `target` that no longer exists (the source feature disappeared in a data update) is not
   an error at runtime. The bake reports it in an **orphan report**, and the editor lists
   orphans for the author to fix. Position-based operations keep working regardless.
3. **Every rung honours layers.** A `place` contributes to R1, R2 and R3 through the
   prefab's rung contributions; a `suppress` removes the object from every rung; a
   `terrain_patch` changes the height truth for every rung and for collision.
4. An operation affects only nodes whose bounds intersect its influence bounds (footprint
   plus margin, or the patch shape plus falloff). The bake computes the affected node set;
   only those chunks get new hashes.
5. Layers are generator input: the chunk hash of an affected node covers the layer
   operations that touch it.
6. An override never changes the `ObjectId` of an untouched generated object.

## 5. Deltas

A delta is one operation from section 3 plus `world`, `author` (account), `time` (sim tick)
and `seq` (monotonic per world). Deltas are rows in `ed_store` keyed by the L12 node of the
anchor (`persistence.md`).

- Additional runtime-only operations: `damage` (`target`, state), `harvest` (`target`,
  regrow tick), `park` (a parked vessel record, see `sim.md`).
- Deltas apply after layers, in `seq` order.
- A delta that conflicts with a later layer change (the plot it stood on is now a hub) is
  kept and flagged; it is never silently dropped.
- Only deltas cross the network and enter saves. Generated content and layers never do.

## 6. Tests

| # | Test |
|---|---|
| L1 | Parse and re-serialise every operation; the file is byte-stable after a round trip |
| L2 | `place` suppresses intersecting generated buildings, re-routes street furniture and flattens terrain under the footprint |
| L3 | The placed prefab appears in R1, R2 and R3 output of its node; its emissive is in R3 |
| L4 | `replace` and `suppress` by id survive a generator-version change and a data-version change where the source id survives |
| L5 | A missing target produces an orphan report entry and no failure |
| L6 | Operation order independence: shuffled files and shuffled node order give identical chunks |
| L7 | Only nodes inside the influence bounds change hash |
| L8 | A delta applied after layers equals the same operation authored in a last layer |
| L9 | Editor round trip: place, move, save, reload, undo, redo leave the file as expected |
| L10 | `road_patch` add and remove produce a connected lane graph across a node edge |

## Decisions made during implementation (`ed_pack::layers`)

- **The section 3 example uses elided ids** (`8f3a...c1`), which are not valid `ObjectId`s, so
  that block fails to parse with `InvalidId` on line 2 (this is tested). Below is the same
  example with full ids, written in canonical form: it parses, and writing it back gives the
  same bytes.

```toml
[[op]]
id = "8f3a00000000000000000000000000c1"
op = "place"
prefab = "hubs/aix_market_hall"
position = [4581204.118, 381377.902, 4407066.54]
rotation = [0.0, 0.0, 0.3827, 0.9239]

[[op]]
id = "8f3a00000000000000000000000000c2"
op = "marker"
name = "aix_hall_spawn"
kind = "spawn"
position = [4581210.4, 381380.1, 4407070.2]
```

- **Canonical form** (the writer's output; reading it and writing it back gives the same bytes):
  - File-level keys other than `op` come first, sorted, followed by a blank line.
  - Operations are sorted by `id`. Each is `[[op]]`, then `id`, then `op`, then its fields in
    the order of the section 3 table.
  - `params`, `volume` and the fields of untyped operations are written as inline tables
    with sorted keys; `volume` puts `shape` first.
  - Floats use the shortest form that round-trips and always have a decimal point (`4.0`,
    `4407066.54`). Integers given where a float is expected become floats.
  - Operations are separated by one blank line, lines end in `\n`, and the file ends with a
    newline.
  - Hand-written files in any order and style parse fine; only the writer's output is
    canonical.
- **Typed operations**: `place`, `replace`, `suppress` and `marker`. `mission`, `road_patch` and
  `terrain_patch` are kept as raw TOML values, written back with sorted keys, until their
  appliers exist.
- **Validation**:
  - Unknown fields on typed operations are errors.
  - `id` must be 32 hex digits and authored (top bit set).
  - Ids must be unique within a file.
  - Quaternions must have unit length within 1e-3.
  - Numbers must be finite; `clear`, `radius` and `height` must not be negative.
  - `replace` takes exactly one of `prefab` and `params`, and `params` must not be empty.
  - `suppress` needs `targets`, `volume` or both.
  - Every error carries a 1-based line number.
- **`volume` shapes**:
  - `{ shape = "box", centre, half_extents, rotation? }`
  - `{ shape = "sphere", centre, radius }`
  - `{ shape = "prism", points (at least 3 body-fixed points), height }`
- **`scale`** is either a number (uniform) or `[x, y, z]`.
- **`layers.toml`** is a list of `[[layer]]` entries with `name` (`[a-z0-9_-]+`, unique) and
  `enabled`. List order is application order.
- **Node file path**: `<body>/<face>/10/<x>/<y>.toml`, with canonical decimals only (no
  leading zeros). Any other file in a layer directory is an error rather than being skipped
  silently.
- **Layer chunk**: a kind-32 chunk holds the canonical TOML text as UTF-8. Its pack key is
  `(L10 node, layer)`, where the layer id is the first 8 bytes of the BLAKE3 hash of the layer
  name (see the `pack.md` decisions).
