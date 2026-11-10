# Contract: persistence (`ed_store`)

Owner: `ed_store`. Decisions: D3, D4, F5, F11. Related: `layers.md` section 5 (deltas),
`sim.md` section 5 (parked records).

## 1. Principle

There is one schema. A solo save is that schema in a SQLite file; a server is the same
schema in SQLite at first and PostgreSQL when shards run as separate processes. Code above
`ed_store` cannot tell which it is using.

Only three kinds of thing are stored: **who** (accounts, progression), **what the world has
that generation does not** (deltas, parked records, plots), and **what is in progress**
(missions, NPC assignments). Generated content and override layers are never stored.

## 2. Interface

```rust
pub trait Store: Send + Sync {
    fn begin(&self) -> Result<Box<dyn Txn + '_>>;
}
pub trait Txn {
    fn world(&mut self) -> Result<WorldMeta>;
    fn deltas_in(&mut self, node: NodeId, since_seq: u64) -> Result<Vec<Delta>>;   // node is L12
    fn append_delta(&mut self, d: &NewDelta) -> Result<u64>;                       // returns seq
    fn parked_in(&mut self, node: NodeId) -> Result<Vec<Parked>>;
    fn put_parked(&mut self, p: &Parked) -> Result<()>;
    fn take_parked(&mut self, id: ObjectId) -> Result<Option<Parked>>;
    fn plot(&mut self, id: ObjectId) -> Result<Option<Plot>>;
    // accounts, progression, missions, npc assignments: same pattern
    fn commit(self: Box<Self>) -> Result<()>;
}
```

All writes happen inside a transaction. The sim thread never waits on the store: it hands
changes to a writer queue and reads from an in-memory view of the loaded nodes.

## 3. Tables

| Table | Key | Columns |
|---|---|---|
| `world` | singleton | world id, seed, data version, generator version at creation, current tick, schema version |
| `account` | account id | name, created tick, last seen tick, auth subject (server only) |
| `progression` | account id, track | rank, trust, merit counters, unlocked tech ids |
| `plot` | `ObjectId` | owner account, L12 node, boundary polygon (body-fixed), state |
| `delta` | world, `seq` | L12 node, author account, tick, operation (the `layers.md` vocabulary, stored as a compact binary encoding of the same fields), flags |
| `parked` | `ObjectId` | L12 node, kind (prefab `AssetId`), body-fixed f64 position and rotation, state blob (fuel, damage, cargo, colours), owner account, parked tick |
| `mission_state` | account id, mission `ObjectId` | step index, step data, started tick |
| `npc_assignment` | npc `ObjectId` | home node, schedule id, current activity, since tick |
| `sky_event` | world, `seq` | kind (launch, re-entry), trajectory elements, start tick, owner |

Indexes: `delta(node, seq)`, `parked(node)`, `plot(node)`.

- **Node key:** rows that have a place are indexed by the L12 node containing their anchor
  (about 2.3 km). Loading a region is a range of node keys.
- **Positions** are body-fixed f64, never a node-local or region-local value, so a row does
  not change meaning when frames move.
- **Ids** are `ObjectId`s. A generated object referred to by a delta keeps its id across
  re-bakes (`addressing.md` section 4).
- **Ticks**, not wall-clock times, order everything inside a world. Wall-clock time is
  stored only for accounts and audit.

## 4. Rules

1. **Append-only deltas.** A delta is never edited. Undoing an action appends the inverse.
   Compaction may merge a node's deltas into fewer equivalent rows offline; the result must
   apply to the same world state.
2. **A parked vessel is in exactly one place:** a `parked` row, or a live entity in one
   shard's sim, never both. `take_parked` and entity creation are one transaction with the
   shard's ownership record.
3. **Versions.** `world.schema version` gates every open. Migrations are forward-only
   scripts, each with a test that migrates a saved fixture. A save from a newer schema is
   refused, not guessed at.
4. **Data-version updates.** When the pinned data changes, deltas whose target `ObjectId` no
   longer exists are kept and flagged (`layers.md` section 5). Plots and parked records are
   positions and are unaffected.
5. **Crash safety.** After any crash the store opens to a state that some tick produced.
   The sim writes a consistent batch per interval (default 2 s) and at shutdown.
6. **Solo equals server.** The solo game runs the server's persistence code in process. No
   save-only format exists.
7. **Privacy.** Account rows hold no personal data beyond a display name and an opaque auth
   subject. Nothing here is sent to third parties.

## 5. What crosses the network

Clients receive, for the nodes in their interest set: delta rows since their last `seq`,
parked rows, plot rows, and sky events. They never receive generated geometry. Authority is
the server's; a client's proposed delta is validated (ownership, range, cost) before
`append_delta`.

## 6. Tests

| # | Test |
|---|---|
| D1 | The conformance suite runs unchanged against SQLite in memory, SQLite on disk and PostgreSQL |
| D2 | Park a car, quit, restart: the car is at the same body-fixed position within 1 mm, with its state |
| D3 | Crash injection during a batch write: the store reopens to a consistent tick |
| D4 | Append 1,000,000 deltas across 10,000 nodes; loading one node's deltas takes under 5 ms |
| D5 | Parked exclusivity: concurrent take from two shards yields exactly one success |
| D6 | Migration: every released schema version's fixture opens and migrates |
| D7 | Data-version update: deltas with missing targets are flagged and reported, none are lost |
| D8 | A solo save file opens as a server database and the reverse |
| D9 | Compaction preserves the applied world state for random delta histories |
