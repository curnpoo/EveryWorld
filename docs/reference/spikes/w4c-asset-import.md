# W4c — Asset import

Verdict: **partly done. The character file is understood and importable; the viewer, the
VRM, the scans and the Blender prefab are not done.** Date 2026-11-10.
Code: `spikes/asset_import/inspect_glb.py`.

## The owner's character (`assets/characters/Solarcat.glb`, not committed)

| Item | Found |
|---|---|
| File | 4.1 MiB, glTF 2.0, exported from Blender |
| Geometry | 1 mesh, 1 primitive, 45,622 vertices, 30,981 triangles; no morph targets |
| Skin | 16 joints with inverse bind matrices; up to 4 influences; every vertex's weights sum to 1 |
| Material | One baked material: base colour and metal-roughness textures; no normal map; double sided; opaque |
| Textures | Two WebP images (1.3 MiB and 0.36 MiB) |
| Required extensions | `EXT_texture_webp`, `KHR_texture_transform` |
| Animations | None |

Bones map to the standard humanoid by name for 15 of 17 core bones:

| Humanoid | Rig bone |
|---|---|
| hips, spine, chest | `spine.001`, `spine.002`, `spine.003` |
| upper arm, lower arm, hand (left and right) | `upper_arm`, `forearm`, `hand` with `.L` / `.R` |
| upper leg, lower leg, foot (left and right) | `thigh`, `shin`, `foot` with `.L` / `.R` |
| neck, head | **none** |

What this means for `ed_assets` and `ed_anim`:

1. **Retargeting is feasible, with three gaps to handle.** There is no head or neck bone
   (the head is skinned to the chest), no shoulders, fingers or toes, and the thighs are
   children of the armature root, not of the hips. The importer must accept a humanoid with
   optional bones, and re-parent the legs under the hips (or treat the root as the hips'
   parent) so that hip motion carries the legs.
2. The importer must support `EXT_texture_webp` and `KHR_texture_transform`, or the bake
   must reject files that require extensions it does not implement. Decision: support both
   at bake time; nothing ships as WebP (textures are re-encoded to GPU formats).
3. One baked material without a normal map is fine for a first skinned draw. Toon or
   realistic shading per avatar (D5) is a material choice at our side, not in the file.
4. Without head and face bones there is no head look-at and no facial animation. If those
   are wanted, the rig needs a head bone added in Blender; the failure branch "re-skin onto
   the standard skeleton" is not needed.
5. No clips are in the file. Locomotion clips come from elsewhere and are retargeted.

## Not done

| Item | Why | Where it goes |
|---|---|---|
| Render the character skinned in a throwaway viewer | Needs a renderer; the alpha has no skinned path and the new one does not exist yet | First task of Phase 1 W3 (skinned draw), using this file as the test asset |
| Read a sample VRM | No sample chosen | Phase 1, with the importer. VRM 1.0 is glTF plus `VRMC_vrm` humanoid mapping, so the bone table above is the same work |
| Convert two or three Megascans assets with licence metadata | No Megascans files on this machine | Needs the owner to download assets from Fab; then Phase 1 `ew-bake` |
| Export one building prefab from Blender with a manifest | Blender is not installed here | Needs Blender; manifest format is specified in `contracts/pack.md` |

The exit-gate line "owner's character loads skinned; one scan and one Blender prefab
convert" is therefore **open**, and is carried into Phase 1 as the first `ew-bake` tasks.
