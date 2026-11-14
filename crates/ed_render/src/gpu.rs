//! GPU word layouts shared with `shaders/common.slang` and `shaders/cull.slang`. Change both
//! sides together; the tests at the bottom pin the sizes.

use ed_gpu::DRAW_CONSTANTS_BYTES;

/// `GpuInstance`: 3x4 transform rows, previous transform rows, mesh, material, object_lo,
/// node_flags, 4 pad words.
pub(crate) const INSTANCE_WORDS: usize = 32;
pub(crate) const INST_PREV: usize = 12;
pub(crate) const INST_MESH: usize = 24;
pub(crate) const INST_MATERIAL: usize = 25;
pub(crate) const INST_OBJECT_LO: usize = 26;
pub(crate) const INST_NODE_FLAGS: usize = 27;
/// Flag bit in `node_flags`: the slot holds a live instance.
pub(crate) const INSTANCE_ALIVE: u32 = 1 << 16;

/// `GpuMesh`: sphere, lod_count, bin_base, capacity, pad, first_index[4], index_count[4],
/// base_vertex[4], error[4].
pub(crate) const MESH_WORDS: usize = 24;
/// `GpuMaterial`: base, params, shade, tex (uint4: base, normal, orm, rotation bits), uv
/// (offset.xy, scale.xy).
pub(crate) const MATERIAL_WORDS: usize = 20;
/// Material texture slot without a texture.
pub(crate) const NO_TEXTURE: u32 = u32::MAX;
/// `float2` texture coordinates, parallel to the vertex buffer.
pub(crate) const UV_WORDS: usize = 2;
/// `SkinVtx`: bind-pose position, octahedral normal, 4 x u16 joints (2 words), 4 weights.
pub(crate) const SKIN_SRC_WORDS: usize = 10;
/// One palette joint: 3x4 affine rows.
pub(crate) const PALETTE_WORDS: usize = 12;
/// `Vtx`: position + octahedral normal.
pub(crate) const VERTEX_WORDS: usize = 4;
/// `DrawIndexedArgs`.
pub(crate) const ARGS_WORDS: usize = 5;

/// `FrameData`: 3 float4x4, 5 planes, 8 float4, uint4 cull, 16 HzbLevel, `LightData`, `AtmosData`.
pub(crate) const FRAME_WORDS: usize = 48 + 20 + 32 + 4 + 64 + crate::light::LIGHT_WORDS + crate::atmosphere::ATMOS_WORDS + WIND_WORDS + IMP_WORDS + SEASON_WORDS + GRASS_WORDS;
pub(crate) const FD_PLANES: usize = 48;
pub(crate) const FD_SUN_DIR: usize = 68;
pub(crate) const FD_VIEWPORT: usize = 88;
pub(crate) const FD_PARAMS: usize = 92;
pub(crate) const FD_PROJ: usize = 96;
pub(crate) const FD_CULL: usize = 100;
pub(crate) const FD_HZB: usize = 104;
pub(crate) const MAX_HZB_LEVELS: usize = 16;
/// `FrameData.light` (common.slang `LightData`, [`crate::light::light_words`]).
pub(crate) const FD_LIGHT: usize = FD_HZB + MAX_HZB_LEVELS * 4;
/// `FrameData.atmos` (common.slang `AtmosData`, [`crate::atmosphere::Atmosphere::words`]).
pub(crate) const FD_ATMOS: usize = FD_LIGHT + crate::light::LIGHT_WORDS;
/// `FrameData.wind` (common.slang `WindData`): wind and gust band (8), on, rest distance,
/// near trees on, unused.
pub(crate) const FD_WIND: usize = FD_ATMOS + crate::atmosphere::ATMOS_WORDS;
pub(crate) const WIND_WORDS: usize = 12;
/// `FrameData.imp` (`ImpData`, trees G5 impostors): bufs, band, far, cfg.
pub(crate) const FD_IMP: usize = FD_WIND + WIND_WORDS;
pub(crate) const IMP_WORDS: usize = 16;
/// `FrameData.season` (`SeasonData`, trees G5 seasons): cfg (phase day, autumn gain, far rows),
/// then two float4 per species row (`ed_world::season::gpu_row`).
pub(crate) const FD_SEASON: usize = FD_IMP + IMP_WORDS;
pub(crate) const SEASON_ROWS: usize = 32;
pub(crate) const SEASON_WORDS: usize = 4 + SEASON_ROWS * 8;
/// `FrameData.grass` (`GrassData`, grass style A): bufs, lod, look.
pub(crate) const FD_GRASS: usize = FD_SEASON + SEASON_WORDS;
pub(crate) const GRASS_WORDS: usize = 12;
/// `GpuMesh.flags` (word 7): a tree bank mesh (common.slang `MESH_FOLIAGE`).
pub(crate) const MESH_FOLIAGE: u32 = 1;
/// `GpuInstance.pad[0]`: free per-instance user word (trees: tint, branch drop, wind phase).
pub(crate) const INST_USER: usize = 28;
/// `GpuInstance.pad[1]`: the tree's season word (`ed_world::season::season_word`).
pub(crate) const INST_SEASON: usize = 29;
/// `GpuInstance.pad[2]`: ground-cover word (`ed_world::ground_cover::ground_word`), read when
/// the instance carries `scene::GROUND_COVER_FLAG`.
pub(crate) const INST_GROUND: usize = 30;

/// Visible-list entry: instance index bits.
pub(crate) const ENTRY_INSTANCE_BITS: u32 = 20;

/// Cull statistics words (cull.slang `STAT_*`).
pub(crate) const STAT_WORDS: usize = 20;

pub(crate) fn bytes(words: &[u32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(words.len() * 4);
    for w in words {
        v.extend_from_slice(&w.to_ne_bytes());
    }
    v
}

pub(crate) fn words(bytes: &[u8]) -> Vec<u32> {
    bytes.as_chunks::<4>().0.iter().map(|c| u32::from_ne_bytes(*c)).collect()
}

/// Packs 32-bit words into a draw-constant block.
pub(crate) fn constants(words: &[u32]) -> [u8; DRAW_CONSTANTS_BYTES] {
    let mut c = [0u8; DRAW_CONSTANTS_BYTES];
    for (i, w) in words.iter().enumerate() {
        c[i * 4..i * 4 + 4].copy_from_slice(&w.to_ne_bytes());
    }
    c
}

/// Octahedral normal encoding into two snorm16 (x low, y high), as `unpack_snorm2x16_to_float`.
pub(crate) fn oct_encode(n: glam::Vec3) -> u32 {
    let n = n.normalize_or(glam::Vec3::Y);
    let l1 = n.x.abs() + n.y.abs() + n.z.abs();
    let (mut x, mut y) = (n.x / l1, n.y / l1);
    if n.z < 0.0 {
        let (ox, oy) = (x, y);
        x = (1.0 - oy.abs()) * if ox >= 0.0 { 1.0 } else { -1.0 };
        y = (1.0 - ox.abs()) * if oy >= 0.0 { 1.0 } else { -1.0 };
    }
    let q = |v: f32| ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
    q(x) | (q(y) << 16)
}

#[cfg(test)]
pub(crate) fn oct_decode(p: u32) -> glam::Vec3 {
    let f = |v: u16| (v as i16 as f32 / 32767.0).max(-1.0);
    let (ex, ey) = (f(p as u16), f((p >> 16) as u16));
    let mut n = glam::Vec3::new(ex, ey, 1.0 - ex.abs() - ey.abs());
    let t = (-n.z).clamp(0.0, 1.0);
    n.x += if n.x >= 0.0 { -t } else { t };
    n.y += if n.y >= 0.0 { -t } else { t };
    n.normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn frame_layout_offsets() {
        assert_eq!(FRAME_WORDS, 388 + SEASON_WORDS + GRASS_WORDS);
        assert_eq!(FD_GRASS + GRASS_WORDS, FRAME_WORDS);
        assert_eq!(FD_SUN_DIR, FD_PLANES + 20);
        assert_eq!(FD_CULL, FD_SUN_DIR + 32);
        assert_eq!(FD_HZB + MAX_HZB_LEVELS * 4, FD_LIGHT);
        assert_eq!(INST_NODE_FLAGS + 5, INSTANCE_WORDS);
    }

    #[test]
    fn oct_round_trip() {
        for n in [Vec3::X, Vec3::NEG_Y, Vec3::new(0.3, -0.4, -0.86), Vec3::new(-0.7, 0.1, 0.7), Vec3::NEG_Z] {
            let n = n.normalize();
            let d = oct_decode(oct_encode(n));
            assert!(d.dot(n) > 0.9999, "{n:?} -> {d:?}");
        }
    }
}
