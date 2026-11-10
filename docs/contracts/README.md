# Contracts

Each contract fixes one seam between crates: the types, the invariants and the tests that
prove them. They are the review checklist for Phases 1 to 3. Code that crosses a seam must
match its contract; if the code needs something else, change the contract first, in the same
commit, and note it in `../CHANGES.md`.

| Contract | Seam | Owner crate |
|---|---|---|
| [addressing.md](addressing.md) | `NodeId`, `ObjectId`, cube mapping, edges, frames | `ed_geo` |
| [generation.md](generation.md) | What the generator emits per rung; determinism | `ed_world` |
| [pack.md](pack.md) | `.ewpack` container and chunk kinds | `ed_pack` |
| [layers.md](layers.md) | Override layer files and deltas | `ed_pack`, `ed_editor`, `ed_store` |
| [rhi.md](rhi.md) | The GPU interface | `ed_gpu` |
| [sim.md](sim.md) | Fixed-step simulation | `ed_sim` |
| [persistence.md](persistence.md) | Stored records | `ed_store` |

Conventions: integers are little-endian in files; hashes are BLAKE3, 256-bit, unless a field
says otherwise; "must" is a rule a test enforces; every contract ends with its test list.
