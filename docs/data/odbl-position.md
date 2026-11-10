# ODbL position for baked world data (for legal review)

Status: **working position written by engineering on 2026-11-10. Not legal advice and not
yet reviewed by a lawyer.** A public build needs that review first.

## The facts

- OpenStreetMap data, and the buildings and transportation themes of Overture Maps, are
  under the Open Database License 1.0. ODbL requires attribution, and requires that a
  *derivative database* that is publicly used is offered under ODbL (share-alike). A
  *produced work* (for example a rendered map image) needs attribution only. A *collective
  database* that merely contains an ODbL database alongside independent ones does not put
  the others under ODbL.
- Our pipeline reads road segments, tags and building footprints, and writes chunks: source
  extracts, lane graphs, building instances, shells, proxies and terrain layers.

## Our reading, most cautious first

| Output | Likely status | Why |
|---|---|---|
| `source` chunks (roads, footprints, tags in node space) | Derivative database | The same data, re-projected and filtered |
| `nav` lane graphs, solved road heights, parcels | Probably derivative database | Structured data derived from the road database and still usable as one |
| `instances` and `shells` for buildings from footprints | Unclear; treat as derivative | Positions and outlines come from the database; styles and parts are ours |
| `proxy`, `terrain` and `layers` with roads and buildings baked into meshes and textures | Probably produced works | Closer to a rendered map than to a database |
| Override layers, prefabs, kit parts, textures, missions | Independent | Authored by us; they refer to the world by position and id |
| Elevation, land cover, imagery, climate | Not ODbL | Separate sources with their own terms |

## What engineering does so that either answer is workable

1. **Attribution** in the credits and in the game, generated from the registry.
2. **The bake can publish the derived databases.** `ew-bake` has an export of the `source`,
   `nav` and building-outline data for any released region, in an open format, separate
   from everything else. If share-alike applies, we publish that export under ODbL.
3. **Separation.** ODbL-derived chunk kinds are distinct chunk kinds and are listed in the
   pack's licence manifest. Authored content never lives in the same chunk as ODbL-derived
   data, so it is not drawn into a derivative database.
4. **No technical restriction** that would prevent a user from extracting the ODbL-derived
   data from a public build, or a parallel unrestricted copy is offered (ODbL section 4.7).
5. **Pinned releases** are recorded per chunk, so we can state exactly which release a
   published export corresponds to.

## Questions for the reviewer

1. Are building `instances` and `shells` derivative databases or produced works?
2. Does serving chunks from our tile service to players count as public use of a derivative
   database, and does offering the export on request satisfy share-alike?
3. Does encrypting or signing packs conflict with section 4.7 if the export is offered?
4. Overture adds non-OSM building sources under other licences; which attribution lines are
   required for the pinned release?
