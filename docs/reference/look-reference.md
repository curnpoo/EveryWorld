# Look reference (owner-supplied, 2026-11-12)

The look target is **real life** (P1-D18, revised). These are the owner's reference
images. The files live in `docs/reference/photos/`, which is **gitignored**: most are
third-party images and this repo is public, so never commit them, and never copy them
into captures or packs. A fresh checkout won't have them; ask the owner for the set.

Use them as a checklist. Every look pass names the images it compared against and says
which of the "what to match" points it moved.

## Photos (ground truth)

| # | File | Conditions | What to match |
|---|---|---|---|
| 01 | `01-blueridge-noon-road.jpg` | Clear autumn, high sun, Blue Ridge (owner's photo) | Sky goes from deep blue overhead to pale near the horizon. Asphalt is mid-grey and bright in sun. Shadows are crisp and only slightly cool, with readable albedo. The distant ridge is softened by light haze. Foliage is mixed (red, yellow, green), never one green. |
| 02 | `02-blueridge-dusk-ridges.webp` | Dusk / thin overcast (owner's photo) | Aerial perspective: ridges go bluer and lower-contrast with each layer. Pink-cream sky. Low global contrast, no hard shadows. Grass is dull and dry, not neon. |
| 03 | `03-zermatt-winter-backlit.webp` | Winter sun into the lens | Snow is very bright but keeps form (highlight roll-off, not clip). Blue skylight in snow shadows. Bloom/flare around the sun. Warm wood vs cool snow. |
| 04 | `04-zermatt-summer.webp` | Clear summer, altitude | High-altitude clarity (thin haze). Saturated greens on lit slopes, dark conifer bands. Dense village massing; dark slate roofs. |
| 05 | `05-zermatt-rooftops.webp` | Clear, near snowline | Material close-ups: weathered dark timber, stone-slate roofs, snow on roofs, metal flashing. Phone HDR: lifted shadows, very saturated sky. |
| 06 | `06-nyc-avenue.webp` | Bright high overcast, street canyon | Haze builds down the avenue. Soft, low-contrast light in the canyon. Mixed facade materials (brick, limestone, glass). Street clutter: lights, signals, awnings, people, taxis. |
| 07 | `07-central-park-aerial.webp` | Aerial, clear summer | Haze and blue shift at multi-km distance. Grid density. Park canopy texture vs city. Water reflecting the sky. |
| 08 | `08-chefchaouen-low-sun.webp` | Low warm sun, broken cloud | Strong warm key vs cool fill. Painted lime-wash blues stay blue in shadow. Hazy blue mountains behind. Clouds lit warm from below. |
| 09 | `09-shibuya-day.png` | Bright day, oblique high view | Emissive signage that still reads in daylight. Road markings. Crowd density. Glass and metal facades. |
| 14 | `14-fjord-twilight.*` (file pending from owner) | Civil twilight, sun below horizon, snowy fjord | Sky goes deep blue at the zenith, through pale cyan, to an orange horizon band (no green band). Snow lit only by skylight reads blue-violet. Water mirrors the sky gradient. Terrain silhouettes go near-black against the bright band. |
| 15 | `15-sunrise-sunset-night.*` (file pending from owner) | One mountain view at sunrise, sunset and night | Sunrise: small hard sun disk, saturated orange haze, ridges in silhouette. Sunset: layered blue ridges fading into haze, with a warm band only near the sun. Night: an orange afterglow strip over dark ridges, a deep blue-grey sky, a planet visible, and town lights as point emissives. This is the time-of-day sweep the atmosphere must reproduce. |
| 10 | `10-tokyo-bay-orbit.webp` | From orbit | Space view: blue atmospheric veil over the ground, Earth's limb, sun glint on water, ground colours seen through atmosphere. Target for the later space work. |

## Rendered benchmarks (quality bar, not physics)

The bar for **detail density and material fidelity**. Don't calibrate light from these.

| # | File | What to take from it |
|---|---|---|
| 11 | `11-bench-village-render.webp` | Density: every metre has props, wear, mud/snow mixing, crowds. Weathered timber and thatch. |
| 12 | `12-bench-rock-canyon-render.webp` | Megascans-grade rock at close range: lichen, layered fractures, soft sky fill in shade. Phase 3 W7 placement target. |
| 13 | `13-bench-desert-render.webp` | Grass meeting rock, haze toward the sun, soft forward scatter. |

## Notes for look passes

- Phone photos (01, 05) use **local tone mapping**: shadows lifted, sky deepened. Match the
  scene's ratios through the tone curve rather than copying the phone's local contrast. The
  AgX-style curve (planned for Phase 2) is the nearer match.
- Haze and aerial perspective show in 02, 04, 06, 07 and 08, and none of it exists yet.
  This is the biggest gap in the M4e captures, and it is Phase 2 work (atmosphere).
- Sky-visibility occlusion in street canyons (06, 09) is the next biggest gap. Today the
  renderer lights a canyon floor like open ground.
