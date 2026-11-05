# Sound: everything that should make a noise

Status: plan, nothing built beyond what exists today: the car/jet engine kit (`ed_engine_audio`, `ed_game/engine_audio.rs`) and the synthesized sonic boom (`sonic_boom.rs`).

Same rule as the visuals: **real data decides what you hear, procedural rules decide how it sounds.** The ground type under the foot, the species overhead, the distance to the coast, the weather code, the hour and the biome are all already in the world. Sound reads them and does not keep its own copy.

## How it's built (one paragraph each)

- **Synthesis first, samples second.** Like `sonic_boom.rs`, most of this is noise shaped by filters and envelopes: wind, water, rain, rustle, thrust, footsteps (grain bursts). It's cheap, endless and seed-deterministic, and needs no asset licensing. Samples only where synthesis sounds fake: birdsong and animal calls, voices, train horns. Engine-free DSP goes in a new `ed_audio` crate (or grows `ed_engine_audio::dsp`), with tests. Bevy glue goes in `ed_game/audio/`.
- **Three layers.**
  1. *Beds*: 2D, stereo, always on, crossfaded by data (wind, biome ambience, city hum, ocean).
  2. *Emitters*: 3D, attached to things near the camera (a river reach, a tree chunk, a street light, a car). Built from the same near-camera chunks the renderer already streams.
  3. *One-shots*: events such as a footstep, landing, ignition or thunder.
- **Listener = camera.** Panning follows the camera, and distance follows the player for body sounds, so the third-person camera doesn't make your own footsteps quiet.
- **Distance.** Volume falls as 1/r, and highs roll off with distance (air absorption, stronger in humid air: `Weather::humidity_pct`). Far events arrive late at 343 m/s: thunder, booms, a distant rocket.
- **Doppler** on anything that passes fast: cars, planes, the player passing a waterfall at 60 m/s.
- **Air density gates everything outside the body.** Wind, exhaust, ambience and impacts scale with air density from `ed_flight` atmosphere. Above about 30 km the world fades out and only sounds carried through the body remain. In space, silence plus your own servos.
- **Voice budget** of about 32 voices, ranked by loudness at the listener. Beds don't count.

---

## 1. The player (the robot body)

The player is a procedural robot (`body.rs`), so every human sound gets a mechanical layer underneath.

### Footsteps
Trigger: a foot plant in `body.rs` (`foot_planted`, `Gait`). Loudness scales with impact speed: walk, then sprint, then landing. Surface comes from `surface::Ground` under the foot, or road/bridge/roof when on those.

| Surface | Sound |
|---|---|
| Grass | soft swish plus a muffled thud, blades brushing the shin |
| DryGrass | crisper, brittle crackle, more high end |
| Moss | almost nothing: a damp, spongy press |
| ForestFloor | crunch of dry leaves and twigs, now and then a stick snap |
| Dirt | dull, short, slightly gritty thump |
| Farmland | soft clod crumble; crops rustle against the legs between steps |
| Sand | a hushed sinking "hff", grains trickling after the lift |
| Gravel | loud, wide crunch with stones rattling for 100 ms |
| Rock | hard, bright knock with a little ring; the metal heel clicks on it |
| Snow | squeaky compression in the cold (< -5 °C), wet slush when warm |
| Paving | clean tap with a short slap echo between buildings |
| Mud | wet suck on the way out, squelch on the way in |
| Shallow water | splash on the plant, drips on the lift |
| Bridge deck / roof | hollow boom (wood), clang (steel), ceramic clack (tile roof) |

**Robot layer**, under every step:
- a metal heel *clank* that is quieter than the surface sound;
- a short servo whine as the knee loads (higher-pitched when sprinting);
- a hydraulic *tss* when weight shifts.

`crouch` turns the steps into slow, muted shuffles.

### Jump, land, bump
- **Jump charge** (`jump_charge`): servos winding up, a rising hum that climbs in pitch as the charge builds, and a ratchet click at full charge.
- **Launch**: pneumatic *thump* plus whatever the ground throws up: a spray of gravel, a puff of sand, a splash.
- **Landing**, by impact speed:
  - soft: a tap;
  - medium: a crunch plus a servo grunt;
  - hard: a slam with debris, a deep body *boom*, and a metallic strain groan while `landing_recovery` runs.
- **Sliding down a slope**: continuous scrape matched to the surface (scree rattle, snow hiss).
- **Bumping into things**:
  - tree trunk: a woody *thock*;
  - building wall: a dull concrete thud;
  - rock: a crack;
  - car: a metal dent sound.

---

## 2. Air: wind, falling, wingsuit, parachute

The key is **dynamic pressure, not speed**: q = ½ ρ v². At 60 m/s near the ground it roars. At the same speed at 25 km it whispers.

### Wind in the ears (every airborne mode)
- Below 10 m/s: low rumble.
- Around 30 m/s: broadband rushing roar.
- Above 55 m/s: a hurricane howl with buffeting.

Your angle to the airflow changes the colour. Face-first into the wind is smooth and bright. Side-on gives choppy buffeting that changes from one ear to the other.

### Freefall (suit off)
- Clothing and plating flutter, and the flutter rate climbs with speed.
- **Spread eagle** (S): louder, more turbulent and lower-pitched, the "big wind".
- **Tuck / somersault** (W): smoother airflow, higher whistle, plus a whoosh that pans around your head once per rotation.
- **Barrel roll** (A/D): the wind roar sweeps left, right, left in time with the spin.

### Wingsuit
- **Suit open** (F): a fabric *whump* as the cells pressurise.
- In flight: a taut membrane buzz whose pitch rises with speed, with a fine flutter on the trailing edges.
- **Pulling hard** (S, turns): pressure builds, the buzz tightens and a low groan comes from the suit.
- **Diving** (W): the wind climbs into a scream.
- **Barrel roll**: the roar sweeps around your head.
- **Stall / over-flare**: the fabric luffs and flaps.
- **Proximity flying**: low over terrain, trees or buildings, add a ground-rush *whoosh* as you pass each object. This sells speed more than anything else (every tree passed within 20 m gets its own whoosh, Doppler-shifted).

### Transonic and supersonic
- Approaching Mach 1, the roar thickens and shock buffeting rattles the body.
- Breaking Mach 1 plays the sonic boom (already built).
- Once supersonic, wind noise from ahead goes eerily quiet; you hear your structure and your own thrust.

### Parachute
- **Deploy**: a rustle as it leaves the pack, a slider zip, then the **opening shock**: a loud *crack-whump* and a hard harness creak.
- **Under canopy**: a gentle wind, the canopy flapping now and then, lines creaking when you steer, and quiet. The quiet is the point: it should feel like a big relief after the wingsuit.
- **Flare and touchdown**: the canopy collapses with a soft rumple.

---

## 3. Rocket shoes

- **Ignition** (Space): a sharp *pop* plus a crackle as each sole lights. The two boots are slightly out of time and panned left and right.
- **Sustain** (1.5 g): two hoarse torch hisses over a low rumble, with crackle. Each boot sits at a slightly different pitch, and the pitch wobbles as `leg_steer` swings the legs.
- **Gimbal**: small servo chirps as the ankles steer.
- **Afterburner** (Shift, slow ramp): the pitch spools up with the ramp, a deep thunder builds underneath, and loud random crackle pops in. Each `boost_stage` step adds a *kick*: a low thump with a pressure punch.
- **Cutoff**: a dying puff, then cooling metal *tinks* for a few seconds after a long burn.
- **Exhaust hitting the ground** (low AGL): the jet blasts whatever is underneath.
  - dust storm roar on dirt or sand;
  - grass thrashing;
  - water spray plus a steam hiss;
  - snow blown off in a hush;
  - pavement scouring.
- **In vacuum**: no exhaust sound at all, only a low-passed *thrum* carried up through the legs. Combined with silence everywhere else, that's how space should feel.

---

## 4. Vehicles (the engine kit exists; everything around it doesn't)

**Car** (engine plus shifts already done):
- Tyre roll by surface: an asphalt hum that rises with speed, a gravel crunch-rattle on `Track`, a soft drum on dirt.
- Skid squeal when the friction circle saturates (`vehicles.rs` already knows).
- Suspension thumps from `compression` spikes.
- Wind noise above 30 m/s, and brake squeal at low speed.
- Door open and slam on enter/exit, a horn, and a collision crunch with glass tinkle.

**Plane** (jet kit done):
- Airframe wind, flap motor whine, a gear thunk.
- Stall warning horn near the stall angle.
- Touchdown tyre *chirp*, then runway rumble.

---

## 5. The environment (beds and emitters, all data-driven)

### Wind in the world
Source: `Weather::{wind_speed_ms, wind_gusts_ms, wind_direction_deg}`, plus exposure.
- **Exposure**: on a ridge or rooftop the wind is stronger; in a valley or a street it's sheltered. Derive it from local terrain height against its surroundings.
- **Gusts**: random swells that sweep through the scene in the wind's direction. Vegetation upwind sounds first, then the gust reaches you.
- **Altitude**: thin and whistling on high peaks.
- **Desert**: sand hiss when the wind is above about 8 m/s.

### Vegetation in the wind
Source: `TreeKind` from tree chunks near the camera, plus the WorldCover canopy rasters for distant density.

| Plants | Sound in the wind |
|---|---|
| Pine, Fir, Conifer | soft high *sighing* hiss, a long smooth "shhhh", the classic forest whisper |
| Cypress, Juniper | denser, lower hush |
| Broadleaf, Maple, PlaneTree | full-bodied leaf rustle in waves, dry and crackly in autumn |
| Birch | bright, fast flicker, like light applause |
| Olive | silvery small-leaf patter |
| Palm, DatePalm | dry fronds clacking and rattling, a knocking sound |
| Banana | big leaves slapping, a papery tear sound |
| Acacia, Baobab | sparse, dry, mostly wind through gaps |
| Cactus, PricklyPear | nothing from the plant; the wind whistles on the spines in strong gusts |
| Shrub, Boxwood, Oleander, FloweringBush | low close rustle, only near you |
| GrassTussock, grassland, farmland | waves of soft swish you can hear moving across the field |
| Wetland | reeds: hollow, dry, rattling stems |

In strong wind (> 12 m/s) trunks *creak* and branches knock.

### Water
Source: `water.rs`, `Cover::Water`, the ocean in `sky.rs`, coast distance, and river width and slope.
- **Ocean surf**: a slow 8-12 s rhythm of build, crash and wash-back.
  - Sandy beaches: a long hissing wash.
  - Rocky coasts: booming crashes and spray.
  - Wind raises the size of the surf.
- **Open sea**: low swell rumble, slaps on rocks, wind over water.
- **Lake**: gentle lapping at the shore, near silence in calm air.
- **River**, by width and gradient:
  - flat and wide: a deep quiet flow;
  - a mountain stream: babbling and gurgling;
  - steep gradient: a rapids roar.
  - A steep drop in the river line becomes a **waterfall**: thunderous at the base, with spray hiss, and audible from far away (a long-range emitter).
- **Fountains** in parks and plazas: a splashing loop.
- **Diving into water**: a splash on entry, then everything muffled with bubbles.

### Weather
Source: `Weather`, `weather_code` (WMO).
- **Rain** (`rain_mm` → intensity), sounding different on each surface:
  - pattering on leaves;
  - a hiss on pavement;
  - drumming on roofs and cars;
  - plinking on water;
  - and on **the robot itself**, small metallic pings across the shell (the best rain sound here).
  - Gutters running and dripping after it stops.
- **Drizzle** (codes 51-57): a fine, close hiss, no impacts.
- **Snow** (`snowfall_cm`, codes 71-77): muffles the world (global low-pass, quieter beds, shorter echo), plus snow-squeak footsteps.
- **Hail** (96/99): hard, bouncing ticks on everything.
- **Thunderstorm** (95-99): lightning *crack* close by, or a long rolling rumble that reaches you late at 343 m/s. The delay tells you how far away it was: about 3 s per km.
- **Fog** (45/48): quieter, damped highs, birds sparse.

### Living things: biome × time of day × temperature
Source: `Biome`, `Cover`, sun elevation (`ed_geo::sun`), `temperature_c`, latitude for the season. Mostly samples, scheduled procedurally.

| Biome | Day | Night |
|---|---|---|
| TropicalRainforest | dense layered insect drone, tropical birds, monkey calls, dripping | frog chorus, cicada wall, night birds |
| Savanna | cicada buzz, weaver chatter, distant big-animal rumbles | crickets, hyena whoops, wind in grass |
| HotDesert | near silence, sand hiss, the odd raven | cooler wind, almost nothing; makes the silence a feature |
| Mediterranean | **cicadas when > 25 °C** (louder the hotter it is), swifts, doves | crickets, scops owl beep |
| TemperateForest | songbirds, woodpecker drum, crows, squirrels in leaves | owls, rustling, crickets in summer |
| Grassland / Steppe | skylark overhead, grasshoppers | crickets, wind |
| Boreal | sparse birds, wind in pines, **loon calls near lakes** | wolves very rarely, deep quiet |
| Tundra | wind, ptarmigan, geese overhead | wind |
| Alpine | wind, marmot whistles, choughs, **cowbells on alpine pasture** (Zermatt) | wind, rockfall now and then |
| IceCap | wind, ice creaking and cracking | the same, and colder |

- **Dawn chorus**: about 30 minutes either side of sunrise, the bird layer doubles.
- **Crickets chirp faster when warm** (Dolbear's law: chirps per minute ≈ 4·T(°C) + 40 for snowy tree crickets), so a hot night *sounds* hot.
- **Frogs**: near `Wetland` and water at night in warm months.
- **Insects** buzz past now and then when you stand still in vegetation.

### City
Source: `Cover::{Residential, Commercial, Industrial}`, road density and `RoadClass`, buildings, street props.
- **Traffic bed**: built from distance to each road class.
  - Motorway or trunk: a constant roar you can hear 1 km away.
  - Primary: a busy hum with horns.
  - Minor: occasional single cars passing with Doppler.
- **Commercial by day**: crowd murmur, footsteps, shop doors, distant sirens, a bus kneeling hiss.
- **Residential**: dogs, kids far off, a lawnmower in summer, quiet at night.
- **Industrial**: HVAC drone, machinery clank, reversing beeps, metal clangs.
- **Street canyons** (tall buildings both sides, Manhattan): strong short echoes and slap-back on every sound, including your footsteps and rocket shoes.
- **Rooftops and skyscrapers**: wind howling around corners, HVAC units humming.
- **Rail** (`RoadClass::Rail`): trains pass with a rising rumble, wheel clack, flanges screeching on curves and a horn. Crossings ring their bells before a train.
- **Street props**:
  - **Traffic signals**: accessible pedestrian signal beeps or chirps where crossings have them, with styles varying by region.
  - **Street lights**: a faint sodium buzz at night.
  - **Utility poles**: transformer hum at **50 Hz or 60 Hz depending on the country's grid** (Americas mostly 60, Europe, Africa and most of Asia 50, Japan split east/west).
- **Pigeons and sparrows** in plazas; gulls near any coastal city.

### Terrain itself
- **Echo**: shouts and booms bounce back in valleys, canyons and against cliffs. The delay comes from distance to the terrain along the sound's direction (a couple of `TerrainGrid` raycasts).
- **Scree and rockfall**: rare, distant clatter on steep alpine rock.
- **Glaciers**: deep creaks and pops.
- **Beaches**: pebbles rattling in the wash on shingle coasts.

---

## 6. Altitude and space

- **Climbing**: the world bed fades with air density. Above about 15 km you hear only wind; above about 30 km only your body.
- **Space**: true silence outside. Inside: servo whir on every limb move, the rocket thrum carried through the legs, suit-radio telemetry blips, and a quiet electronic hum. Silence is the effect, so don't fill it.
- **On the Moon**: footsteps are felt, not heard: low thuds through the body, no surface layer.
- **Re-entry**: a plasma roar that rises with heating (∝ ρ v³) and crackling, then the boom when you drop through Mach 1.

---

## 7. UI and feedback

- **Menu**: soft clicks; map pan and zoom ticks; a rising *whoosh* when you confirm a place; a drop whoosh at spawn.
- **Mode switches**: wingsuit on/off (F) gives a fabric sound plus a small tone; walk, rocket and parachute each get a short distinct chirp.
- **Audible altimeter**, like real skydivers use: beeps at set heights above ground, then a continuous tone at deployment altitude.
- **Ground-proximity warning** in the wingsuit: fast beeps when time-to-impact drops below about 4 s (uses `agl` and vertical speed).
- **Screenshot** (`capture`): shutter click.

---

## 8. Mixing rules

- **At speed, wind is king**: above about 30 m/s the wind ducks the world beds by up to -12 dB, so a fast dive is a roar and a slow glide lets the forest in.
- **Body sounds always cut through** (footsteps, servos, landings); they never duck.
- **Under canopy and in snow**, everything gets quieter and calmer.
- **One reverb send per place**: forest (short, diffuse), street canyon (slap-back), valley (long, discrete echoes), open plain and sea (none), space (none).

## Build order (each step stands alone)

1. **Wind by dynamic pressure, with the space fade**. Biggest payoff: every flying mode at once. Also the listener and mixer skeleton.
2. **Footsteps by `Ground`, plus landings and jumps**. Needs foot-plant events from `body.rs`.
3. **Rocket shoes**: ignition, sustain, afterburner, cutoff, ground blast.
4. **Wingsuit and parachute** detail, plus the proximity whoosh.
5. **World wind and vegetation beds** (`TreeKind`, canopy rasters), **water** (surf, rivers, waterfalls).
6. **Weather**: rain on surfaces and the robot, thunder with delay, the snow muffle.
7. **Fauna** by biome, hour and temperature (first samples).
8. **City**: traffic bed from road classes, street canyons, rail, signals, grid hum.
9. **Vehicles around the engine kit**, plus UI.
