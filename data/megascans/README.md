# Megascans index (built 2026-11-11)

Index of the Quixel Megascans seller on Fab, plus a shortlist for EveryWorld. **No asset files are in this repo and none may be committed.** Originals stay outside the repo per `data/registry/megascans.toml` (Fab Standard License, redistribution only inside the packaged game; record the Fab asset id per converted asset).

## Licence findings (read 2026-11-11)
- Announcement: <https://quixel.com/news/update-megascans-are-going-free-again> (2026-11-05): "We're making the full Quixel Megascans library free on Fab." and "We've also updated the terms for all Quixel assets so you can now train AI models on Megascans and Megaplants." It does not name the licence or give an end date.
- Licence: the Fab Standard License (Personal and Professional tiers, shown on each listing; priced 0.0). Summary on <https://www.fab.com/eula>: use "with any compatible tools (usage is not limited to Unreal Engine)"; "Use the assets commercially or privately"; "Commercially distribute your Projects with the Fab assets incorporated into it"; prohibited: "Resell or redistribute the asset for free on a standalone basis"; sharing allowed with collaborators "via a private repository or in the Project"; "You are not required to give credit to the publisher". The page says the summary is "not legally binding"; the full EULA is. Epic's FAQ (<https://forums.unrealengine.com/t/quixel-to-fab-transition-faqs/2033839>): "These assets can be used in any tool or engine according to the Fab Standard License agreement."
- Not found stated: games are not named explicitly (the "Projects" wording covers it); public-repo redistribution is not mentioned, standalone redistribution is barred, so do not commit sources; no Megaplants vs surfaces difference found; no end date found (earlier free window ended 2024-12-31 but claimed items stay usable, per the FAQ). CG Channel notes the listings now show "Allows usage with AI: Yes" (<https://www.cgchannel.com/?p=179945>). Studio licences stay under contract. Have the full EULA text reviewed before shipping.
- Some Fab listings are CC-BY 4.0 instead (the /eula page lists it): those need credit in `CREDITS.md`. Check the licence shown on each listing at download; every Quixel Megascans listing indexed here shows the Fab Standard License.

## How the index was built
- Endpoint: `https://www.fab.com/i/listings/search?seller=Quixel%20Megascans&count=100&cursor=...` (the JSON the seller page itself loads), plain `curl`, 1 request about every 1.1 s, 209 pages (the server caps pages at 24 results), HTTP 200 throughout, no challenge or login. Cursor chain ended with `next: null`, so the sweep is complete.
- `catalog.jsonl`: one line per listing. `type` is derived: Fab `material` -> surface (damage-grunge categories -> imperfection), `decal` -> decal, `3d-model` in plants/trees/mushrooms -> 3d-plant (logs, stumps, branches, bark stay 3d), other `3d-model` -> 3d, one `brush` -> other. `biome_hint` is a keyword guess from title/tags. `free` is true when the starting price is 0.0 (Fab's own `isFree` flag is false for all rows, `price` is 0.0 for all).
- `resolutions_or_formats` holds the formats Fab lists (gltf, fbx, texture-set, ...) and per-format file-size range. Resolutions, LOD counts and displacement are not in the listing JSON; check on the asset page or at download.

## Counts
Total 5016 listings, all price 0.0. By type: 3d 3457, surface 1147, 3d-plant 250, decal 127, imperfection 34, other 1.
By top category: nature-plants 1656 (rocks-stones 1071, trees 248, plants 196), buildings-architecture 965, objects-decor 627, building-human-made 626, nature-terrain 568, furniture-fixtures 244, damage-grunge 84, environments 78, food-drink 45, weapons-combat 32, characters-creatures 29, fabric-clothing 28, electronics-technology 27, other 7. Full list is 108 category paths; derive with `jq`.

## Gaps
- No LOD, resolution, tileability or displacement data (not in public JSON).
- Catalogue has no oak, hickory, tulip poplar, rhododendron or mountain laurel, and no lunar regolith.
- Fab lists far fewer entries (5016) than the old Quixel site; the new listings were all re-published 2026-11.
- Megaplants are not tagged separately in the listing data.
- `shortlist.md`: 305 picks chosen by rule and sampling, not asset-by-asset visual review.
