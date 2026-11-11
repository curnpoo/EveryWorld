# Solastra — core loop and missions

Gameplay plan, added Nov 9–10, 2026. Built from the EveryWorld docs as of Nov 8, 2026 (commit `7cea773`) plus curren's own words from Nov 9–10. Where the docs already say something, the source is named. Everything else is a proposal to shoot at. Ping radius updated to ~1,000 km Nov 9 (vehicles travel fast).

> Note, 2026-11-10: section 1 quotes the documents as they were at commit `7cea773`. The
> plans were reworked since; the old files it names are under `docs/alpha/`. Persistence is
> now `docs/contracts/persistence.md`, authored content `docs/contracts/layers.md`, and the
> part of this loop that is in the solo version is listed in `path-to-solo.md` §1.

---

## 1. What the repo already says about gameplay

The docs are heavy on world tech and thin on game design. Here's everything they actually commit to:

**Missions — delivery and racing only, and barely that**
- `GOALS.md` §10 ("Living world and play"): "Missions: finish delivery, add racing, then more types (community input)." Status: prototypes exist (cars, planes, delivery missions).
- `ROADMAP.md` Stage F ("Play"): "Missions: finish delivery missions, add racing, then select more mission types with community input." Also: "Mission ideas beyond delivery and racing are open for suggestions."
- `ARCHITECTURE.md` ("Missions"): `missions.rs` plans multi-modal deliveries over real hubs — drive to an airport, fly to a launch site, rocket to the launch site nearest the destination, fly, drive to the door. Example route: Brooklyn to Kyoto via JFK, Kennedy Space Center, Tanegashima, Osaka Itami.
- `plans/engine.md` §6: "Missions. Data-driven (a script or graph DSL) on top of placements, agents and `ed_store`. A mission is a set of goals bound to real places (OSM POIs). Generated missions come from the world's data ('deliver to this real address')."
- `plans/roads-remaining.md`: gameplay is explicitly deferred — "Phase 4 — Gameplay (deferred to Stage F)."

**Living world — NPCs exist as a plan, not a system**
- `GOALS.md` §10: "Living world: NPCs, AI traffic, parked cars, biome wildlife (start with urban birds like pigeons)."
- `plans/assets-and-agents.md` §2: one agent framework for animals, NPCs, traffic, player vehicles. Population is deterministic (density = biome, road class, hour, weather, seed). Agents spawn in a ring around the player and despawn far away; beyond that the world only keeps counts — "simulation LOD, not render LOD." Behaviours are data: schedules, needs, routes.
- `plans/engine.md` §6: "Agents (NPCs, animals, traffic). Full sim within ~1 km of any player; simplified 'statistical' sim per cell beyond that."

**Persistence — the tech for it already exists on paper**
- `plans/engine.md` §7 (`ed_store`): world edits are ops (`Add`, `Remove`, `Transform`, `SetState`, `Damage`, ...) keyed by `(cell_id, object_id)`, stored as an append-only log per cell. "When a cell loads: generate the base, apply the deltas." This is the exact mechanism a destroyed-then-rebuilt house needs.
- `GOALS.md` §4 (vehicle persistence): a parked vehicle becomes a parked data record (f64 ECEF state, no live physics) and rehydrates when you come back.
- `plans/multiplayer.md`: only deltas and entity state travel — the base world is never sent. Mission state syncs reliably. Interest management by speed (walking small radius, jet large, orbit coarse). "Space is its own shard space."

**World events that can generate work**
- `plans/world-sim.md`: procedural climate with storm records — hurricanes, tornadoes, sandstorms, blizzards, tsunamis — as pure functions of (seed, UTC). "Pick a storm -> spawn near it."
- `docs/solarpunk/spec.md`: art/asset spec only (modular architecture, five regional families, community buildings and gardens). No economy, no trust, no missions in it.

**What's nowhere in the docs:** no core loop, no trust/reputation system, no money of any kind (the docs never mention currency at all), no roles or professions, no hub-city concept, no destruction/rebuild mechanics, no NPC population scale, no free-to-play or cosmetics model. That's the gap this draft fills.

---

## 2. The core loop (proposed)

One sentence: **see what the world needs, go do it, get thanked, get trusted, get invited to bigger things.**

1. **Live anywhere.** The whole Earth is open. Put your house in a hub city or alone in the Mongolian steppe. Your choice changes what reaches you, not what you're allowed to do.
2. **The world tells you what it needs.** Your device pings you about needs near you (~1,000 km): a house went down, a solar array is dead, a festival needs a pilot, a storm is coming. Hub cities have boards with more of it. You can also just wander — needs exist whether or not you look.
3. **Go do it, in person, with the right vehicle.** Drive, fly, rocket, walk. The trip is part of the mission — that's why the world is real-scale and seamless. A delivery to the next town and a delivery to the Moon are the same kind of job with a different vehicle.
4. **Get thanked, not paid.** No money changes hands. The people or community you helped give you trust — and sometimes stuff, because that's how this world works: people give things to people they trust.
5. **Trust opens the world up.** More trust = better vehicles lent to you, access to spaceports and rockets, invitations to events (moon basketball), and the standing to start bigger things yourself, like founding a settlement or organizing a festival.
6. **The world keeps breaking and growing.** Storms hit, things wear out, NPCs have needs they can't cover. ~1.5 billion NPCs live their lives, but there aren't enough hands — that's the gap players fill. The loop never ends because the world never stops needing upkeep.

The fantasy isn't "get rich." It's "be the person everyone wants around."

---

## 3. Mission categories

### A. Keep the world running (maintenance and repair)
The daily bread. Things break; players fix them.
- Restart a dead solar array on a farm after a storm knocked it out.
- Fix a water pump for a village before their tanks run dry.
- Repair a jetpack charging station at a trailhead so stranded hikers can get home.
- Clear a landslide off a mountain road before the morning traffic hits.
- Reboot a comms relay after a sandstorm, while the storm is still going.

### B. Move people and things (transport and delivery)
The repo's one real mission type, grown up.
- Multi-modal delivery: drive, fly, rocket, drive — the Brooklyn-to-Kyoto route from `ARCHITECTURE.md`, but for anything: medicine, parts, a birthday cake.
- Space uber: ferry passengers from a hub city to the spaceport, then to the Moon, and back.
- Rescue run: pick up a stranded pilot whose rocket boots died halfway across nowhere.
- Haul building materials to a rebuild site (pairs with category C).
- Courier for a hub city: a day of short hops that ends with the whole neighborhood knowing your name.

### C. Build and rebuild
The signature mechanic. Destruction is persistent outside hub cities; rebuilding is a social event.
- A house got flattened in rural France. Everyone within ~1,000 km gets a ping: "house down — help rebuild?" Show up, bring materials or hands, earn trust and thanks from the neighboring city.
- Raise a community garden for a settlement that asked for one.
- Build a footbridge over a river crossing the locals have been fording for years.
- Help a new player settle their first house — first build is a mission, not a tutorial.
- Restore a landmark after a storm (a lighthouse, a temple roof, a stadium stand).

### D. Culture and connection (social missions)
Not fixing, not delivering. Making life worth living. This is where the game stops being a handyman sim.
- Organize the first zero-gravity music festival on the Moon, with instruments built from scrap.
- Mediate a feud between two farming communes — judge whose tomatoes taste more like the future.
- Teach a group of kids to fly with rocket boots without killing themselves.
- Run a storytelling night where people trade memories for trust.
- Start a rumor campaign to convince the region that the best party is in the middle of nowhere — then make it true.

### E. Race and play (sport and competition)
Chaos with a finish line instead of a body count.
- Moon basketball: pickup games in the dome, low gravity, dunks that hang in the air forever.
- Cross-country rally across a real continent, no roads required.
- Jetpack time trials through a canyon system.
- Wingsuit race down a mountain range at dawn.
- Orbital slalom: slingshot around the Moon and back before the others.

### F. Explore and discover
For the players who just want to go see things.
- First footfall on a new procedural planet — plant a flag, name something, log it for everyone.
- Chart a storm: fly into a hurricane's edge and map it for the climate network.
- Photograph rare wildlife in a biome nobody visits.
- Track down a crashed satellite and salvage it before it burns up.
- Map an unmapped cave system or a deep-ocean trench.

---

## 4. Trust — the progression currency

There is no money. Trust is the only number that matters, and it works like this:

- **Trust is per community and per faction, not one global score.** The farmers in Mongolia trust you; the Moon dome crew hasn't met you yet. That keeps the map meaningful — you're a legend in one place and a stranger in the next.
- **You earn it by showing up.** Completing missions, helping rebuilds, and being thanked by NPCs and players all add trust with the community involved. Big jobs (rebuilds, rescues, festivals) pay more than small ones.
- **People give you stuff because they trust you.** More trust = better vehicles lent to you, parts, outfits, a place to stay, a seat on a rocket. Nobody sells you anything. The world runs on gifts, and gifts follow trust.
- **Trust gates access, not power.** A spaceport lets you park your car in a rocket because the crew trusts you with it. A hub city invites you to organize its festival because you've shown up before. There are no levels and no stat grind — the "progression" is the world opening up to you.
- **Trust can be lost.** Destroy something and walk away, and the local community remembers. Ignore a place for a long time and your standing there fades (slowly — this isn't a chore, it's just that trust is a relationship). Hub cities are untouchable, so the worst griefing can't break the game — but outside them, your reputation is your record.
- **Players stand out.** NPCs dress normal; players wear wacky custom outfits. Trust is visible: the more a community trusts you, the more they greet you by name and wave you through. Other players can read at a glance who's reliable and who's the chaos gremlin.

---

## 5. Owner's answers (2026-11-10)

These settle most of the questions this draft ended with. Where a section above disagrees,
this section wins.

1. **Trust is two things.** One global trust that follows you everywhere, and one trust per
   hub. A hub is any very large city. Smaller communities count toward the hub they are
   nearest to.
2. **Trust decays only in played time.** Time away from the game costs nothing. While you
   are playing, a hub you ignore slowly forgets you.
3. **Destroying a house** costs trust with the nearby hub. Nothing else: no cooldown, no
   forced repair.
4. **Nobody requests or signs off a rebuild.** A house counts as rebuilt when it is built
   again. Whoever did the work is thanked.
5. **NPCs ask for help only when you are near**, within about 10 to 20 km, or when you walk
   up to them. This replaces the ~1,000 km ping of section 2.
6. **No spam.** Requests go to a message log on the device (an inbox the player opens when
   they want). Only urgent ones raise a notification.
7. **Skills, not classes.** You never pick an identity. What you do feeds skill types, and
   those make you a better farmer, mechanic, racer, pilot, electrician and so on. The list
   is meant to be wide, well beyond the examples here.
8. **Moon basketball is for later.**
9. **The fourth world is a procedural exoplanet**: heavily built over, more industrial,
   heavier work, fewer laws, more chaotic. To be workshopped.
10. **Cosmetics:** vehicle paint, trails, emotes, outfits assembled from pieces, and items
    given by a community you are involved with, so you can represent it.
11. **Character:** the existing robot stands in as the player until character art exists.

## 6. Still open

1. How global trust is derived: its own number, or computed from hub trust?
2. What the skill types are, and what a higher skill changes (speed, quality, access to
   harder jobs), given that trust gates access and not power (section 4).
3. How fast trust decays, and whether global trust decays at all.
4. Whether cosmetics from a community are earned by trust only or can also be bought.
5. Who is thanked for a rebuild when several players worked on it.
6. The fourth world's design.
