# Plan: multiplayer (ghost → full Stage C)

Status: PLAN ONLY (no implementation). Decisions captured 2026-11-06.
Companion: `docs/ROADMAP.md` Stage C, `docs/plans/engine.md` §7 (persistence) and §9 (multiplayer), `docs/plans/world-sim.md` (climate determinism).

## Decisions (user, 2026-11-06)

1. **First milestone: ghost only.** Thin relay sends player position, pose and boot state at 20-30 Hz; each client generates its own world. Exit: two clients at the same coordinates see the same buildings, trees, clouds and sun, and see each other. Deltas, climate and interest management come later.
2. **Authority for the prototype: listen server.** One player hosts (server logic runs in-process); others connect. No second binary, trivial LAN testing. Migrate to a dedicated authoritative server when deltas/prediction/vehicles land (Stage C). Rationale: ghost only relays transforms, so authority doesn't matter yet; listen server is fastest to first demo. Lightyear supports both, so the protocol doesn't change.
3. **Netcode library: lightyear** (client/server, prediction, reconciliation, interpolation, interest management, WebTransport). Heavy but maps 1:1 to our needs. *Caveat: lightyear tracks Bevy versions; if it doesn't build against Bevy 0.20, prototype on a thin std-UDP transport first and migrate when available. Our message types stay the same either way.*
4. **Prototype environment: LAN first**, then a relay so a friend on another network can connect. Plan: LAN/localhost for iteration; a small relay binary (or a tunnel like cloudflared/ngrok) for friend play; a VPS-hosted dedicated server for Stage C.
5. **Weather in ghost: default to clear.** No Open-Meteo sync, no climate sim yet. Sky is clear/deterministic. The full procedural climate (`ed_climate`) is a Stage C prerequisite and is not started.
6. **Foundations first.** Golden-hash CI, pinned data versions, and sim/render split land *before* netcode. (Data version pinning done 2026-11-06: `ed_data::DATA_VERSION`.)

## Core principle (everything hangs on this)

Terrain, buildings, trees, weather and the sky are **never sent or stored**. Every client generates the base world from the same inputs: world seed, pinned data version (`ed_data::DATA_VERSION`), UTC time. Only **deltas (edits)** and **entity state (players, vehicles, NPCs)** travel. Save files and multiplayer share one delta format (`ed_store`).

## Architecture (target)

- **`ed_net` (new, engine-free crate)** — message types, protocol, (de)serialization. serde + serde_json for the prototype; compact binary later. Lives engine-free so the server runs it headless.
- **`ed_server` (new binary)** — dedicated authoritative server, Stage C. For the ghost prototype, the host player runs an in-process listen server instead.
- **Transport** — QUIC via `quinn` (or WebTransport for web). Ghost prototype may use raw UDP on `std::net::UdpSocket` if lightyear/quinn aren't available offline.
- **`ed_store`** — persistent world deltas (see engine.md §7). Shared by save files and multiplayer.

## Authority and sharding (Stage C)

- Authoritative server per region shard; positions in **f64** on the server. Clients render relative to their own floating origin.
- Interest management by speed: walking → small radius, jet → large, orbit → coarse.
- Shards by region with handoff at borders; global persistent store; **space is its own shard space**.

## What syncs

- World deltas (`ed_store` ops) — **reliable** (Stage C).
- Entity state (players, vehicles, NPCs near players) — **unreliable at 20-30 Hz**, snapshot compression + delta encoding.
- Mission state.
- Never the terrain/world.

## Netcode

- Client-side **prediction + server reconciliation** for owned players and vehicles; **interpolation** for others.
- **Physics note:** Avian is not deterministic across machines. The server owns vehicles and the rocket; clients render and predict them.

## Determinism foundations (Phase 0, before netcode)

These must land first; multiplayer cannot be verified without them.

1. **Golden-hash CI.** Each reference tile generates bit-identical output. CI runs a golden-hash test per reference tile on every platform (macOS + Windows already in CI). No platform-dependent hashing, no thread-order-dependent float reductions, f64 or fixed-point in generators.
   - *Known violation to fix:* `crates/ed_world/src/clouds.rs` `Conditions::at` reads `SystemTime::now()` (wall clock) to decide whether to trust the live report. For multiplayer this must depend on data, not wall clock. With ghost defaulting to clear, this only affects the live-weather path; still fix before golden-hash CI lands (pass the reference "now" in, or store `fetched_unix_s` on `Weather`).
   - *Audit:* many generators use `HashMap` (buildings, roads, props, bparts). Iteration order can affect output ordering where results are collected into vectors. Audit and convert ordering-sensitive ones to `BTreeMap` or sort before emit. `roads/*` already mixes BTreeMap/HashMap.
2. **Pin data versions.** Done: `ed_data::DATA_VERSION` (2026-11-06) stamped into elevation, weather and vector cache keys. Bump on any source/parse change.
3. **Separate sim state from render state.** Fixed timestep for sim, interpolated render, in `ed_game`. Currently player physics and camera are coupled; needs a pass.

## Ghost milestone (Phase 1) — concrete tasks

1. Add `ed_net` crate (engine-free) with message types:
   - `Hello { name, protocol_version }`
   - `PlayerState { id, pos: (f64,f64,f64), yaw, pitch, boot_state, t: f64 }` (pos in ECEF f64)
   - `Goodbye { id }`
   - (Later) delta and mission messages reuse the same envelope.
2. Transport: start with raw UDP on `std::net::UdpSocket` on a background thread + crossbeam channels (offline-buildable, no new deps). Migrate to lightyear/quinn when available.
3. Listen server in the host's process: receives `PlayerState`, broadcasts to all other clients at 20-30 Hz. Clients send their own at the same rate.
4. Client: spawn a remote-player avatar entity per known id; update its transform from received state with interpolation. Render as a simple capsule first; reuse the robot body later.
5. Clear sky in multiplayer mode: skip Open-Meteo fetch, use `CloudParams::default()` / clear conditions. Add a `Config`/`GameState` flag (e.g. `--multiplayer` or menu toggle) that forces clear weather.
6. Connection UI: host (shows IP:port), join (enter IP:port), disconnect. Reuse the pause menu tabs.
7. Friend play: document a tunnel (cloudflared/ngrok) and a tiny relay binary for NAT traversal. LAN works directly.
8. Floating origin: remote positions arrive in ECEF f64; convert to the local render frame per client (same as local player). Interest/visibility culling not needed for ghost (few players).

## Remote player rendering

- Ghost: capsule + head, tinted per player. Enough to "see each other".
- Later: full robot body (`body.rs`) with cosmetics, interpolated.

## Testing

- Unit tests in `ed_net` for (de)serialization round-trip.
- Determinism: golden-hash CI (Phase 0) must pass before claiming ghost works.
- Manual: two instances on localhost, same drop coordinates, verify identical world + visible avatars.

## Open questions

- Lightyear vs raw UDP for the prototype (depends on offline availability and Bevy 0.20 compat).
- Whether the host's clock is the authority for "now" in multiplayer (weather is clear in ghost, so moot until Stage C).
- Voice chat: not in scope.

## How to run (testing phase)

Build:
```
cargo build --release -p ed_server
cargo build --release -p ed_game
```

On the host machine (the one acting as server):
```
./target/release/earthdrop-server 7777
```
(Windows: `target\release\earthdrop-server.exe 7777`)

On each client (LAN):
```
./target/release/earthdrop --server <host-ip>:7777 --name alice
```
Open the pause menu → **Multiplayer** tab → **Connect**. Drop in anywhere; the first time a remote player's state arrives you are teleported to stand next to them. Other players show as a blue capsule with a "player" nametag; beyond ~200 m only a ground ring marks their spot. They are not yet drawn on the minimap/worldmap (follow-up).

## Implementation status

- Server relay + protocol: done (`crates/ed_net`, `crates/ed_server`).
- Client networking, avatars, nametag, far ring, spawn-next-to, menu tab: done (`crates/ed_game/src/net.rs`, menu Multiplayer tab).
- Minimap/worldmap dots for remote players: TODO (follow-up).
- Determinism foundations (golden-hash CI, sim/render split, clouds.rs wall-clock): partially done (data version pinning).
## Update 2026-11-06 (netcode v3)

- Binary wire format (no JSON, ~40B/state), protocol v3.
- Server interest management: <1km 60Hz, <10km 15Hz, far 5Hz (ECEF distance).
- Client dead reckoning: extrapolate up to 0.5s on packet loss using velocity.
- Jitter-adaptive interpolation buffer: 50-300ms based on inter-arrival deviation.
- Time sync: server broadcasts shared unix_s at 1Hz; clients set Clock. Time now moves in sync for all players.
- Player list (item 5) shown in Multiplayer tab status. Chat explicitly deferred.

## Procedural world sim (research)

Goal: deterministic weather/climate so all clients see identical sky without Open-Meteo.

Plan from docs/plans/world-sim.md:
1. ed_climate crate (engine-free): climate base (WorldClim normals already embedded), synoptic grid (~1-2deg, advected heat/moisture), storm records (hurricane/tornado/sandstorm/tsunami as pure functions of seed+UTC).
2. WeatherSample API consumed by clouds.rs, sky, foliage, vehicles.
3. Determinism: fixed-point or strictly ordered f64, spin-up from cached climatology + checkpoint fast-forward, golden-hash CI.

Steps:
- Milestone 1: layer 1 + WeatherSample + tests (Sahara dry, Amazon wet, rain shadow).
- Milestone 2: layer 2 grid with deterministic spin-up.
- Milestone 3: storm generator.
- Milestone 4: map overlay + satellite base.
- Milestone 5: local realization (wind/rain/funnel).
- Milestone 6: hook into clouds/sky/ground/vehicles.

Open: grid resolution vs CPU, precomputed spin-up shipped vs computed at launch.
