# Claude handoff: tile culling

## Change

`crates/ed_game/src/streaming.rs` now filters the quadtree's desired tile set against the active perspective camera before starting tile builds. A conservative tile bounding sphere is tested against the camera view cone. The sphere includes 500 m of vertical allowance for buildings, and tiles within 1.5 km of the observer are retained so the nearby world remains available when looking away from the ground.

This avoids loading and keeping off-screen terrain and city tiles. Tiles outside the filtered desired set flow through the existing retirement path, which retains coarse coverage while visible finer replacements load.

## Related space visibility change

`crates/ed_game/src/sky.rs` and `crates/ed_game/src/hud.rs` use a 100 km altitude threshold for celestial body meshes and their HUD target rings and labels. That change is committed locally as `fe18450` (`Hide celestial targets until spaceflight`).

## Verification and follow-up

- `cargo check --offline -p ed_game` passes.
- No gameplay run or FPS measurement was performed. Check loaded tile counts and frame rate while rotating the camera at ground level and while flying high; tune the 1.5 km keep radius or the conservative sphere if scenery pops too close to the view edge.
- This change is committed as `2b4a41c` (`Cull off-screen terrain tiles`). Both this and the celestial visibility commit are local and have not been pushed to `origin/main`.
