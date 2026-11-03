#!/usr/bin/env python3
"""Build the lookdev review board.

    python3 tools/make_board.py LOOKDEV_DIR PLACES_DIR AUDIO_DIR OUT_DIR

Converts renders to web JPEGs, engine clips to MP3, and writes OUT_DIR/index.html
from the template in tools/board_template.html, filling in the item manifest.
"""
import json
import subprocess
import sys
from pathlib import Path

from PIL import Image

lookdev, places, audio, out = (Path(p) for p in sys.argv[1:5])
(out / "img").mkdir(parents=True, exist_ok=True)
(out / "audio").mkdir(parents=True, exist_ok=True)


def jpg(src: Path) -> str | None:
    if not src.exists():
        return None
    dst = out / "img" / (src.stem + ".jpg")
    im = Image.open(src).convert("RGB")
    if im.width > 1280:
        im = im.resize((1280, round(im.height * 1280 / im.width)), Image.LANCZOS)
    im.save(dst, "JPEG", quality=82, optimize=True, progressive=True)
    return f"img/{dst.name}"


def mp3(src: Path) -> str | None:
    if not src.exists():
        return None
    dst = out / "audio" / (src.stem + ".mp3")
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(src), "-b:a", "128k", str(dst)], check=True)
    return f"audio/{dst.name}"


REGIONS = [
    ("northamericacity", "North American city", "Brick and brownstone walk-ups, flat roofs, colored awnings on shopfronts, double-yellow center lines, signals on the far side of the junction.", "New York, Chicago, Boston"),
    ("northamericasuburb", "North American suburb", "Siding in soft pastels, shingle gable and hip roofs, wooden utility poles with transformer cans.", "Anywhere US/Canada outside the core"),
    ("latinamerica", "Latin America", "Bright painted stucco, mostly flat roofs with some terracotta hips, concrete poles, ALTO signs.", "Mexico City, Lima, Bogotá"),
    ("mediterranean", "Mediterranean", "Ochre and cream stucco, terracotta hip roofs, cypress and palms on the street.", "Provence, Tuscany, Andalusia, Greece"),
    ("paris", "Paris", "Cream limestone, grey zinc roofs, 5 to 7 storeys, shopfronts on nearly every ground floor.", "Inside the Périphérique"),
    ("westerneurope", "Western Europe", "Cream render and brick, steep red-tile and slate gables.", "Germany, Benelux, northern France"),
    ("alpine", "Alpine", "Timber chalets over white bases, steep dark gables, conifers. Kicks in above 600 m in the Alps.", "Zermatt, Chamonix, Tyrol"),
    ("british", "British", "Red brick terraces, slate gables, drive on the left.", "UK and Ireland"),
    ("nordic", "Nordic", "Falu red, mustard and white timber, steep gables, birch and spruce.", "Norway, Sweden, Finland, Iceland"),
    ("easterneurope", "Eastern Europe", "Pastel panel blocks, 3 to 9 storeys, mixed roofs.", "Poland to Russia"),
    ("northafricamiddleeast", "North Africa and Middle East", "Sand, clay and whitewash, flat roofs, palms.", "Marrakech, Cairo, Dubai"),
    ("subsaharan", "Sub-Saharan Africa", "Painted block walls, corrugated metal hip roofs, wooden poles, acacias.", "Nairobi, Accra, Lagos"),
    ("southasia", "South Asia", "Saturated painted concrete, flat roofs, dense shopfronts, left-hand traffic.", "Delhi, Mumbai, Dhaka"),
    ("eastasia", "East Asia", "Grey and white tile towers, flat roofs, horizontal signals.", "Shanghai, Seoul, Taipei"),
    ("japan", "Japan", "Low houses with dark tiled roofs, concrete utility poles everywhere, horizontal signals, left-hand traffic.", "Tokyo, Kyoto, Osaka"),
    ("southeastasia", "Southeast Asia", "Pastel shophouses, metal and tile hip roofs, palms.", "Bangkok, Hanoi, Manila"),
    ("oceania", "Oceania", "Brick and weatherboard, terracotta and steel hip roofs, left-hand traffic.", "Sydney, Melbourne, Auckland"),
]

items = []

# Real places, streamed live from map data.
PLACES = [
    ("central_park", "Central Park, New York", "Ground level, 10:30 am"),
    ("manhattan_aerial", "Manhattan from 1,500 m", "Late afternoon"),
    ("zermatt", "Zermatt, Switzerland", "Valley floor toward the peaks, 11 am"),
    ("aix_en_provence", "Aix-en-Provence, France", "Old town, 4:30 pm"),
    ("marrakech", "Marrakech, Morocco", "Medina, 9:30 am"),
    ("shibuya", "Shibuya, Tokyo", "3 pm"),
]
for key, title, sub in PLACES:
    src = jpg(places / f"{key}.png")
    if src:
        items.append({"id": f"place-{key}", "section": "places", "title": title, "sub": sub, "images": [src],
                      "about": "Real terrain, buildings, roads and land cover for this exact spot, generated live. Judge the overall mood: light, color, density, haze."})

src = [s for s in (jpg(lookdev / "character_front.png"), jpg(lookdev / "character_back.png")) if s]
if src:
    items.append({"id": "character-courier", "section": "character", "title": "Robot courier", "sub": "Placeholder primitives, procedural IK rig",
                  "images": src,
                  "about": "Cream shell, dark visor with two glowing eyes, orange courier jacket, teal parcel backpack. Built from capsules so the IK rig can be tested; the final model will be sculpted to the direction you pick here."})

for key, title, about, where in REGIONS:
    imgs = [s for s in (jpg(lookdev / f"region_{key}_street.png"), jpg(lookdev / f"region_{key}_aerial.png")) if s]
    if imgs:
        items.append({"id": f"region-{key}", "section": "regions", "title": title, "sub": where, "images": imgs, "about": about})

for key, title, sub, about in [
    ("trees_lineup", "Tree species", "Broadleaf, conifer, birch, palm, acacia, cypress, shrub, cactus",
     "Low-poly, chunky silhouettes with baked underside shading. Species mix and leaf color come from the biome; density from mapped land cover."),
    ("ground_swatches", "Ground colors", "Row 1 biomes, row 2 mapped land covers, row 3 slope and snow rules",
     "Row 1 left to right: rainforest, savanna, hot desert, Mediterranean, temperate, grassland, boreal, tundra, alpine, ice cap. Row 2: wood, grass, farmland, wetland, sand, rock, ice, residential, commercial, industrial, park, sports pitch. Row 3: alpine meadow flat, same but steep (rock shows), just below snowline, above snowline, steep above snowline, beach. Today these are flat vertex colors; next step is a textured ground material."),
    ("clouds", "Cloud styles", "Puffy, towering, stratus",
     "Left: storybook cumulus. Middle: towering storm cell with an anvil, so you can see weather coming. Right: flat overcast sheet. Weather data will pick the mix."),
]:
    src = jpg(lookdev / f"{key}.png")
    if src:
        items.append({"id": f"look-{key}", "section": "nature", "title": title, "sub": sub, "images": [src], "about": about})

tm = [s for s in (jpg(lookdev / f"tonemap_{k}.png") for k in ("agx", "tony", "aces", "blender")) if s]
if tm:
    items.append({"id": "look-tonemapping", "section": "light", "title": "Tone mapping", "sub": "Same street, four film looks",
                  "images": tm, "labels": ["AgX", "TonyMcMapface", "ACES", "Blender Filmic"],
                  "about": "This sets the whole game's color personality. AgX (current) is soft and filmic but mutes saturated colors. TonyMcMapface keeps more color. ACES is punchy and contrasty. Blender Filmic sits between. Pick one, or tell me the reference you want (Great Big World leans saturated)."})

for key, title, about in [
    ("v8_crossplane", "V8 cross-plane", "Uneven firing per bank gives the muscle-car lope."),
    ("inline4", "Inline-4", "Even firing, buzzy and eager."),
    ("flat6", "Flat-6", "Smooth and raspy at high rpm."),
    ("vtwin", "V-twin", "Big uneven gaps between pulses."),
]:
    src = mp3(audio / f"{key}.wav")
    if src:
        items.append({"id": f"sound-{key}", "section": "sound", "title": title, "sub": "2 s idle, rev to 80% redline, back to idle", "audio": src, "about": about + " Synthesized live: each cylinder fires at its real crank angle into a tuned exhaust pipe. No samples."})

template = (Path(__file__).parent / "board_template.html").read_text()
html = template.replace("/*__ITEMS__*/[]", json.dumps(items, ensure_ascii=False, indent=1))
(out / "index.html").write_text(html)
print(f"{len(items)} items -> {out/'index.html'}")
