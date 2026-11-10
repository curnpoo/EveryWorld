//! Geometry clipmap layout and mesh generator (pure Rust, no dependencies).
//!
//! Level `L` has vertex spacing `s_L = base * 2^L` and a block footprint
//! `G_L = block * s_L`. Level 0 is a full 4x4-block square `R_0`. Every other
//! level `L` is a 4x4-block square `R_L` with a 2x2-block hole `H_L`, and the
//! hole is exactly the square `R_{L-1}` that the next-finer level covers.
//!
//! Block origins of level `L` always sit on the `G_L` grid, so the nesting is
//! exact (no gaps, no overlaps) and every level's vertices lie on the finest
//! vertex lattice. The hole is placed by a per-level block offset in `{0,1,2}`
//! chosen so the finer square always contains the camera.
//!
//! Winding: the block mesh is laid out in the xz plane and its triangles have
//! a +y normal, i.e. counter-clockwise when viewed from +y.

/// Quads per block side is `block`; blocks per level footprint side.
const FOOTPRINT: i64 = 4;
/// Hole size in blocks per side (2x2 blocks filled by the finer level).
const HOLE: i64 = 2;

/// One block of `block x block` quads, local to the block, in grid units.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipmapMesh {
    /// xz in grid units, local to one block, in `0..=block`.
    pub vertices: Vec<[f32; 2]>,
    /// Triangle list, counter-clockwise viewed from +y.
    pub indices: Vec<u32>,
}

/// Clipmap shape: number of levels, block size, and level-0 spacing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipmapLayout {
    pub levels: u32,
    /// Quads per block side, e.g. 64.
    pub block: u32,
    /// Level 0 vertex spacing in metres.
    pub base_spacing_m: f32,
}

/// One placed block. Offsets are relative to the snapped clipmap centre
/// (see [`snapped_centre_m`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockInstance {
    pub level: u32,
    /// World xz of the block origin relative to the snapped centre, metres.
    pub offset_m: [f32; 2],
    /// Vertex spacing at this level (metres per quad).
    pub scale_m: f32,
    /// Neighbours in `-x, +x, -z, +z` that are coarser. The shader morphs
    /// those edges; the mesh itself is flat and shared.
    pub edge_morph: [bool; 4],
}

fn is_valid(layout: &ClipmapLayout) -> bool {
    layout.levels > 0
        && layout.block > 0
        && layout.base_spacing_m.is_finite()
        && layout.base_spacing_m > 0.0
}

fn spacing_m(layout: &ClipmapLayout, level: u32) -> f64 {
    layout.base_spacing_m as f64 * 2f64.powi(level as i32)
}

fn block_size_m(layout: &ClipmapLayout, level: u32) -> f64 {
    layout.block as f64 * spacing_m(layout, level)
}

/// Builds one block of `block x block` quads, shared by every instance.
pub fn build_block(block: u32) -> ClipmapMesh {
    let n = block as usize;
    let stride = n + 1;
    let mut vertices = Vec::with_capacity(stride * stride);
    for z in 0..=n {
        for x in 0..=n {
            vertices.push([x as f32, z as f32]);
        }
    }
    let mut indices = Vec::with_capacity(n * n * 6);
    for z in 0..n {
        for x in 0..n {
            // a=(x,z) b=(x+1,z) c=(x+1,z+1) d=(x,z+1); +y normal, CCW from +y.
            let a = (z * stride + x) as u32;
            let b = a + 1;
            let d = a + stride as u32;
            let c = d + 1;
            indices.extend_from_slice(&[a, d, c, a, c, b]);
        }
    }
    ClipmapMesh { vertices, indices }
}

/// Clipmap centre after snapping to the coarsest level's block grid
/// (`G_top = block * base * 2^(levels-1)`). Offsets of every instance are
/// relative to this point.
pub fn snapped_centre_m(layout: &ClipmapLayout, centre_m: [f64; 2]) -> [f64; 2] {
    if !is_valid(layout) {
        return [0.0, 0.0];
    }
    let g = block_size_m(layout, layout.levels - 1);
    centre_m.map(|c| (c / g).round() * g)
}

/// Places every block of every level. Order: level ascending, then block
/// rows (z) and columns (x) ascending. Deterministic for equal inputs.
pub fn build_level_instances(layout: &ClipmapLayout, centre_m: [f64; 2]) -> Vec<BlockInstance> {
    if !is_valid(layout) {
        return Vec::new();
    }
    let top = layout.levels - 1;
    let snap = snapped_centre_m(layout, centre_m);

    // Top-down pass: origin of R_L (min corner, metres) and, for L >= 1, the
    // hole offset (in level-L blocks) of H_L, which equals R_{L-1}.
    let g_top = block_size_m(layout, top);
    let mut origin = [snap[0] - 2.0 * g_top, snap[1] - 2.0 * g_top];
    // (level, origin of R_L, hole offset in blocks for L >= 1)
    let mut frames: Vec<(u32, [f64; 2], [i64; 2])> = Vec::with_capacity(layout.levels as usize);
    for level in (0..=top).rev() {
        let g = block_size_m(layout, level);
        if level == 0 {
            frames.push((0, origin, [0, 0]));
            break;
        }
        let mut hole = [0i64; 2];
        for a in 0..2 {
            // Camera cell inside R_L, in level-L blocks, in 0..FOOTPRINT.
            let k = ((centre_m[a] - origin[a]) / g).floor() as i64;
            // Hole covers blocks [h, h+2); the camera cell k must be inside.
            hole[a] = (k - 1).clamp(0, FOOTPRINT - HOLE);
        }
        frames.push((level, origin, hole));
        origin = [
            origin[0] + hole[0] as f64 * g,
            origin[1] + hole[1] as f64 * g,
        ];
    }

    let mut out = Vec::new();
    for &(level, org, hole) in frames.iter().rev() {
        let g = block_size_m(layout, level);
        let scale = spacing_m(layout, level) as f32;
        let has_hole = level >= 1;
        for j in 0..FOOTPRINT {
            for i in 0..FOOTPRINT {
                if has_hole
                    && (hole[0]..hole[0] + HOLE).contains(&i)
                    && (hole[1]..hole[1] + HOLE).contains(&j)
                {
                    continue;
                }
                let ox = org[0] + i as f64 * g - snap[0];
                let oz = org[1] + j as f64 * g - snap[1];
                // Neighbour -x, +x, -z, +z. Outside R_L means a coarser level
                // (if any); inside R_L it is either same-level ring or finer.
                let dirs = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)];
                let mut edge_morph = [false; 4];
                for (e, (di, dj)) in dirs.iter().enumerate() {
                    let ni = i + di;
                    let nj = j + dj;
                    let outside = !(0..FOOTPRINT).contains(&ni) || !(0..FOOTPRINT).contains(&nj);
                    edge_morph[e] = outside && level < top;
                }
                out.push(BlockInstance {
                    level,
                    offset_m: [ox as f32, oz as f32],
                    scale_m: scale,
                    edge_morph,
                });
            }
        }
    }
    out
}

/// Total triangles drawn for a full clipmap: level 0 has 16 blocks, every
/// other level 12 (4x4 minus the 2x2 hole), each block `2 * block^2` tris.
pub fn total_triangles(layout: &ClipmapLayout) -> u64 {
    if !is_valid(layout) {
        return 0;
    }
    let blocks = 16 + 12 * (layout.levels as u64 - 1);
    blocks * 2 * (layout.block as u64) * (layout.block as u64)
}

/// Guaranteed radius around the snapped centre that the clipmap covers.
/// The top footprint is `4 G_top` wide with the centre within `G_top / 2` of
/// its middle, so the camera always has at least `1.5 G_top` to every edge.
pub fn coverage_radius_m(layout: &ClipmapLayout) -> f32 {
    if !is_valid(layout) {
        return 0.0;
    }
    (1.5 * block_size_m(layout, layout.levels - 1)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(levels: u32, block: u32, base: f32) -> ClipmapLayout {
        ClipmapLayout { levels, block, base_spacing_m: base }
    }

    /// Y component of the normal of triangle (a,b,c) lifted to y=0.
    fn normal_y(p: [[f32; 2]; 3]) -> f32 {
        let (a, b, c) = (p[0], p[1], p[2]);
        let u = [b[0] - a[0], b[1] - a[1]];
        let v = [c[0] - a[0], c[1] - a[1]];
        // 3D cross of u=(ux,0,uz), v=(vx,0,vz) gives y = uz*vx - ux*vz.
        u[1] * v[0] - u[0] * v[1]
    }

    #[test]
    fn clipmap_block_triangle_count() {
        for block in [1u32, 4, 64] {
            let m = build_block(block);
            assert_eq!(m.indices.len() as u64, block as u64 * block as u64 * 6);
            assert_eq!(m.indices.len() / 3, (block * block * 2) as usize);
            assert_eq!(m.vertices.len(), ((block + 1) * (block + 1)) as usize);
        }
    }

    #[test]
    fn clipmap_block_indices_in_range() {
        let m = build_block(64);
        let n = m.vertices.len() as u32;
        assert!(m.indices.iter().all(|&i| i < n));
    }

    #[test]
    fn clipmap_block_winding_ccw_from_plus_y() {
        for block in [1u32, 3, 16] {
            let m = build_block(block);
            for t in m.indices.chunks(3) {
                let p = [
                    m.vertices[t[0] as usize],
                    m.vertices[t[1] as usize],
                    m.vertices[t[2] as usize],
                ];
                assert!(normal_y(p) > 0.0, "triangle {t:?} is not CCW from +y");
            }
        }
    }

    /// Rasterise every block footprint onto the finest grid of a layout with
    /// base spacing 1 and count how many blocks cover each cell.
    fn coverage_counts(l: &ClipmapLayout, centre: [f64; 2]) -> (Vec<u32>, usize) {
        let top = l.levels - 1;
        let g_top = l.block as i64 * (1i64 << top);
        let width = (FOOTPRINT * g_top) as usize;
        let mut counts = vec![0u32; width * width];
        for inst in build_level_instances(l, centre) {
            let size = l.block as i64 * (1i64 << inst.level);
            let x0 = inst.offset_m[0] as f64 + (2 * g_top) as f64;
            let z0 = inst.offset_m[1] as f64 + (2 * g_top) as f64;
            assert!(x0.fract() == 0.0 && z0.fract() == 0.0, "offsets must be whole cells");
            let (x0, z0) = (x0 as i64, z0 as i64);
            assert!(x0 >= 0 && z0 >= 0, "block outside clipmap footprint");
            assert!(x0 + size <= width as i64 && z0 + size <= width as i64);
            for z in z0..z0 + size {
                for x in x0..x0 + size {
                    counts[z as usize * width + x as usize] += 1;
                }
            }
        }
        (counts, width)
    }

    #[test]
    fn clipmap_levels_tile_exactly_once() {
        let l = layout(4, 4, 1.0);
        for centre in [[0.0, 0.0], [13.7, -5.2], [-100.3, 77.9], [1000.5, -999.5]] {
            let (counts, _) = coverage_counts(&l, centre);
            assert!(counts.iter().all(|&c| c == 1), "gap or overlap at centre {centre:?}");
        }
    }

    #[test]
    fn clipmap_single_level_is_full_square() {
        let l = layout(1, 4, 1.0);
        let (counts, _) = coverage_counts(&l, [3.3, 7.1]);
        assert!(counts.iter().all(|&c| c == 1));
        assert_eq!(build_level_instances(&l, [3.3, 7.1]).len(), 16);
    }

    #[test]
    fn clipmap_camera_inside_finest_level() {
        let l = layout(4, 64, 1.0);
        for centre in [[0.0, 0.0], [13.7, -5.2], [-300.3, 201.9], [5000.5, -4999.5]] {
            let snap = snapped_centre_m(&l, centre);
            let rel = [(centre[0] - snap[0]) as f32, (centre[1] - snap[1]) as f32];
            let size = 64.0f32;
            let covered = build_level_instances(&l, centre).iter().any(|b| {
                b.level == 0
                    && rel[0] >= b.offset_m[0]
                    && rel[0] < b.offset_m[0] + size
                    && rel[1] >= b.offset_m[1]
                    && rel[1] < b.offset_m[1] + size
            });
            assert!(covered, "camera not in level 0 at {centre:?}");
        }
    }

    #[test]
    fn clipmap_snapping_stability() {
        let l = layout(4, 64, 1.0);
        for level in 0..l.levels {
            let g = block_size_m(&l, level);
            let s = spacing_m(&l, level);
            // Both points sit in the same G_level cell and the same snap cell
            // of the coarsest level; the move is below 2 * spacing.
            let p = [0.25 * g, 0.25 * g];
            let q = [p[0] + 1.9 * s, p[1] + 1.9 * s];
            let pick = |c: [f64; 2]| -> Vec<BlockInstance> {
                build_level_instances(&l, c)
                    .into_iter()
                    .filter(|b| b.level == level)
                    .collect()
            };
            assert_eq!(pick(p), pick(q), "level {level} offsets changed");
        }
    }

    #[test]
    fn clipmap_instances_deterministic() {
        let l = layout(8, 64, 1.0);
        let a = build_level_instances(&l, [12345.6, -987.6]);
        let b = build_level_instances(&l, [12345.6, -987.6]);
        assert_eq!(a, b);
    }

    #[test]
    fn clipmap_total_triangles_levels8_block64() {
        let l = layout(8, 64, 1.0);
        assert_eq!(total_triangles(&l), 819_200);
        let inst = build_level_instances(&l, [0.0, 0.0]);
        assert_eq!(inst.len() as u64 * 2 * 64 * 64, total_triangles(&l));
    }

    #[test]
    fn clipmap_coverage_radius_contains_camera_margin() {
        let l = layout(4, 64, 1.0);
        let r = coverage_radius_m(&l);
        assert_eq!(r, 768.0);
        for centre in [[0.0, 0.0], [255.0, -255.0], [-1234.5, 777.7]] {
            let snap = snapped_centre_m(&l, centre);
            let rel = [(centre[0] - snap[0]) as f32, (centre[1] - snap[1]) as f32];
            let insts = build_level_instances(&l, centre);
            let mut min = [f32::MAX; 2];
            let mut max = [f32::MIN; 2];
            for b in &insts {
                let size = 64.0 * b.scale_m;
                for a in 0..2 {
                    min[a] = min[a].min(b.offset_m[a]);
                    max[a] = max[a].max(b.offset_m[a] + size);
                }
            }
            for a in 0..2 {
                assert!(rel[a] - min[a] >= r && max[a] - rel[a] >= r);
            }
        }
    }

    #[test]
    fn clipmap_edge_morph_flags() {
        let l = layout(4, 4, 1.0);
        let insts = build_level_instances(&l, [0.0, 0.0]);
        assert!(insts.iter().filter(|b| b.level == 3).all(|b| b.edge_morph == [false; 4]));
        assert!(insts.iter().filter(|b| b.level < 3).any(|b| b.edge_morph.iter().any(|&e| e)));
    }

    #[test]
    fn clipmap_degenerate_layouts_are_empty() {
        assert!(build_level_instances(&layout(0, 64, 1.0), [0.0, 0.0]).is_empty());
        assert_eq!(total_triangles(&layout(0, 64, 1.0)), 0);
        assert_eq!(coverage_radius_m(&layout(0, 64, 1.0)), 0.0);
    }
}
