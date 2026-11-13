//! Tree impostors (trees G5, rung T2): one hemi-octahedral impostor per tree bank entry
//! (species x variant x age), baked on the CPU from the bank mesh's simplified level (the level
//! the impostor replaces, so the hand-off shows the same tree) at rest pose.
//!
//! * [`IMP_FRAMES`]^2 views over the upper hemisphere (octahedral layout, `impostor::oct_*`),
//!   each an orthographic frame of [`ImpostorTier::frame_px`] px whose square holds the tree's
//!   bounding sphere (centre on the trunk axis, so yaw keeps it in place).
//! * Three BC5 textures per entry, every mip down to 4 px frames:
//!   A = (coverage, depth), B = (octahedral normal x, y), C = (value, bark).
//!   Coverage is the supersampled share of the texel the tree covers; the raster dithers it
//!   (stochastic, the mean coverage of the real tree at every mip). Depth is towards the viewer
//!   from the sphere's centre plane, `0.5 + d / (2 R PAD)`. Normal is tree-local (cards face
//!   the viewer). Value is the albedo relative to the bank leaf (or bark) albedo times a mild
//!   crown occlusion, over [`VALUE_MAX`]; bark is the bark share.
//! * Lit at run time by the same foliage model as the meshes (`resolve.slang shade_foliage`),
//!   with the bank material's leaf and bark albedo, so the colour is the meshes' by
//!   construction.
//!
//! Deterministic (no threads inside one bake, fixed iteration order); the BC encoder is our
//! own ([`bc4_block`]), so the bytes are the same on every platform.

use super::bank::{AUX_BARK, AUX_CARD, BankMesh};
use super::impostor::{oct_decode, view_basis};
use super::textures::{LEAF_BASE, Rgba8Image};
use glam::{Vec2, Vec3};

/// Frames per side of the octahedral grid.
pub const IMP_FRAMES: u32 = 8;
/// Frame half size = bounding radius * PAD (a texel of margin for filtering).
pub const IMP_PAD: f32 = 1.04;
/// Value channel range (`C.r * VALUE_MAX`).
pub const VALUE_MAX: f32 = 2.5;
/// Supersampling per frame texel side.
const SS: usize = 3;
/// Bake format version (cache key).
pub const BAKE_VERSION: u32 = 1;

/// Frame size per quality tier (px). The bake and its memory scale with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpostorTier {
    Low,
    Medium,
    High,
}

impl ImpostorTier {
    pub fn frame_px(self) -> u32 {
        match self {
            ImpostorTier::Low => 32,
            ImpostorTier::Medium => 48,
            ImpostorTier::High => 64,
        }
    }
}

/// One baked impostor.
#[derive(Clone, Debug, PartialEq)]
pub struct BankImpostor {
    pub frames: u32,
    pub frame_px: u32,
    /// Sphere centre height above the trunk base and radius (m, bank mesh space).
    pub center_y: f32,
    pub radius: f32,
    /// BC5 mips of the three textures (A coverage/depth, B normal, C value/bark), level 0 first.
    pub tex: [Vec<Vec<u8>>; 3],
}

impl BankImpostor {
    pub fn size(&self) -> u32 {
        self.frames * self.frame_px
    }
    pub fn bytes(&self) -> usize {
        self.tex.iter().flatten().map(|m| m.len()).sum()
    }

    /// Serialised form (disk cache): header then the mips.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.bytes() + 64);
        for w in [BAKE_VERSION, self.frames, self.frame_px, self.center_y.to_bits(), self.radius.to_bits(), self.tex[0].len() as u32] {
            out.extend(w.to_le_bytes());
        }
        for t in &self.tex {
            for m in t {
                out.extend((m.len() as u32).to_le_bytes());
                out.extend(m);
            }
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<BankImpostor> {
        let mut at = 0usize;
        fn word(b: &[u8], at: &mut usize) -> Option<u32> {
            let w = u32::from_le_bytes(b.get(*at..*at + 4)?.try_into().ok()?);
            *at += 4;
            Some(w)
        }
        let h: Vec<u32> = (0..6).map(|_| word(b, &mut at)).collect::<Option<_>>()?;
        if h[0] != BAKE_VERSION {
            return None;
        }
        let mut tex: [Vec<Vec<u8>>; 3] = Default::default();
        for t in &mut tex {
            for _ in 0..h[5] {
                let n = word(b, &mut at)? as usize;
                t.push(b.get(at..at + n)?.to_vec());
                at += n;
            }
        }
        (at == b.len()).then_some(BankImpostor { frames: h[1], frame_px: h[2], center_y: f32::from_bits(h[3]), radius: f32::from_bits(h[4]), tex })
    }
}

/// View direction (tree to camera, tree-local, y up) of frame `(i, j)` of an `n` x `n` grid.
pub fn frame_dir(i: u32, j: u32, n: u32) -> Vec3 {
    oct_decode(Vec2::new((i as f32 + 0.5) / n as f32, (j as f32 + 0.5) / n as f32))
}

/// Full-sphere octahedral encoding of a unit vector (y is the pole axis), 0..1.
pub fn oct_sphere(n: Vec3) -> Vec2 {
    let n = n / (n.x.abs() + n.y.abs() + n.z.abs()).max(1e-9);
    let mut p = Vec2::new(n.x, n.z);
    if n.y < 0.0 {
        p = Vec2::new((1.0 - p.y.abs()) * p.x.signum(), (1.0 - p.x.abs()) * p.y.signum());
    }
    p * 0.5 + 0.5
}

/// Inverse of [`oct_sphere`].
pub fn oct_sphere_decode(e: Vec2) -> Vec3 {
    let f = e * 2.0 - 1.0;
    let mut n = Vec3::new(f.x, 1.0 - f.x.abs() - f.y.abs(), f.y);
    let t = (-n.y).max(0.0);
    n.x += if n.x >= 0.0 { -t } else { t };
    n.z += if n.z >= 0.0 { -t } else { t };
    n.normalize()
}

/// The leaf atlas as linear floats per mip (built once, shared by every bake).
pub struct LeafTex {
    levels: Vec<(usize, usize, Vec<[f32; 4]>)>,
}

impl LeafTex {
    pub fn new(img: &Rgba8Image) -> LeafTex {
        let mut lut = [0.0f32; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *v = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        }
        let mut levels = Vec::new();
        let (mut w, mut h, mut o) = (img.width as usize, img.height as usize, 0usize);
        for _ in 0..img.mip_levels.max(1) {
            let px = img.data[o..o + w * h * 4].chunks_exact(4).map(|p| [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize], p[3] as f32 / 255.0]).collect();
            levels.push((w, h, px));
            o += w * h * 4;
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        LeafTex { levels }
    }

    fn sample(&self, uv: Vec2, lod: f32) -> [f32; 4] {
        let l = (lod.max(0.0) as usize).min(self.levels.len() - 1);
        let (w, h, px) = &self.levels[l];
        let x = ((uv.x * *w as f32) as isize).clamp(0, *w as isize - 1) as usize;
        let y = ((uv.y * *h as f32) as isize).clamp(0, *h as isize - 1) as usize;
        px[y * w + x]
    }

    fn texels(&self) -> f32 {
        (self.levels[0].0 * self.levels[0].1) as f32
    }
}

fn lin(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[derive(Clone, Copy, Default)]
struct Sample {
    depth: f32,
    n: Vec3,
    value: f32,
    bark: f32,
    hit: bool,
}

/// One texel: [coverage, depth01, normal oct x, y, value01, bark].
type Texel = [f32; 6];

/// Bakes the impostor of `m` from its LOD `lod` (the simplified level, 1, normally).
pub fn bake(m: &BankMesh, lod: usize, leaves: &LeafTex, tier: ImpostorTier) -> BankImpostor {
    let n = IMP_FRAMES as usize;
    let fp = tier.frame_px() as usize;
    let idx = &m.lods[lod.min(m.lods.len() - 1)].0;
    let used: Vec<u32> = {
        let mut u: Vec<u32> = idx.clone();
        u.sort_unstable();
        u.dedup();
        u
    };
    let pos = |i: u32| Vec3::from(m.positions[i as usize]);
    let (mut ylo, mut yhi) = (f32::MAX, f32::MIN);
    for &i in &used {
        ylo = ylo.min(pos(i).y);
        yhi = yhi.max(pos(i).y);
    }
    if used.is_empty() {
        (ylo, yhi) = (0.0, m.height.max(0.1));
    }
    let c = Vec3::new(0.0, (ylo + yhi) * 0.5, 0.0);
    let radius = used.iter().map(|&i| pos(i).distance(c)).fold(0.1f32, f32::max);
    // Crown ellipsoid of the leaf vertices (mild baked occlusion towards its core).
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for &i in &used {
        if m.words[i as usize][1] & AUX_BARK == 0 {
            lo = lo.min(pos(i));
            hi = hi.max(pos(i));
        }
    }
    if lo.x > hi.x {
        (lo, hi) = (c - radius, c + radius);
    }
    let cc = (lo + hi) * 0.5;
    let rh = ((hi.x - lo.x).max(hi.z - lo.z) * 0.5).max(0.1);
    let rv = ((hi.y - lo.y) * 0.5).max(0.1);
    let occl = |p: Vec3| {
        let e = (Vec2::new(p.x - cc.x, p.z - cc.z).length_squared() / (rh * rh) + (p.y - cc.y).powi(2) / (rv * rv)).sqrt();
        0.72 + 0.28 * smooth(0.25, 1.0, e)
    };
    let leaf_base = Vec3::new(lin(LEAF_BASE[0]), lin(LEAF_BASE[1]), lin(LEAF_BASE[2]));
    let side = n * fp;
    let mut level0: Vec<Texel> = vec![[0.0; 6]; side * side];
    let ss = fp * SS;
    let mut buf = vec![Sample::default(); ss * ss];
    for j in 0..n {
        for i in 0..n {
            let v = frame_dir(i as u32, j as u32, n as u32);
            let (right, up) = view_basis(v);
            buf.iter_mut().for_each(|s| *s = Sample { depth: f32::MIN, ..Default::default() });
            let scale = ss as f32 * 0.5 / (radius * IMP_PAD);
            let proj = |p: Vec3| {
                let q = p - c;
                Vec3::new(ss as f32 * 0.5 + q.dot(right) * scale, ss as f32 * 0.5 - q.dot(up) * scale, q.dot(v))
            };
            for t in idx.chunks_exact(3) {
                let (a, b, cpt) = (proj(pos(t[0])), proj(pos(t[1])), proj(pos(t[2])));
                let area = (b.x - a.x) * (cpt.y - a.y) - (b.y - a.y) * (cpt.x - a.x);
                if area.abs() < 1e-7 {
                    continue;
                }
                let w = [m.words[t[0] as usize], m.words[t[1] as usize], m.words[t[2] as usize]];
                let aux = w[0][1];
                let card = aux & AUX_CARD != 0;
                let bark = aux & AUX_BARK != 0;
                let uv = w.map(|x| Vec2::new((x[0] & 0xFFFF) as f32 / 65535.0, (x[0] >> 16) as f32 / 65535.0));
                let lod = if card {
                    let ua = ((uv[1] - uv[0]).perp_dot(uv[2] - uv[0])).abs() * leaves.texels();
                    0.5 * (ua / area.abs()).max(1.0).log2()
                } else {
                    0.0
                };
                let vals = w.map(|x| (x[1] & 127) as f32 / 127.0 * 2.0);
                let nrm = [t[0], t[1], t[2]].map(|i| Vec3::from(m.normals[i as usize]));
                let x0 = a.x.min(b.x).min(cpt.x).floor().max(0.0) as usize;
                let x1 = (a.x.max(b.x).max(cpt.x).ceil().max(0.0) as usize).min(ss);
                let y0 = a.y.min(b.y).min(cpt.y).floor().max(0.0) as usize;
                let y1 = (a.y.max(b.y).max(cpt.y).ceil().max(0.0) as usize).min(ss);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((b.x - px) * (cpt.y - py) - (b.y - py) * (cpt.x - px)) / area;
                        let w1 = ((cpt.x - px) * (a.y - py) - (cpt.y - py) * (a.x - px)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let depth = a.z * w0 + b.z * w1 + cpt.z * w2;
                        let dst = &mut buf[y * ss + x];
                        if depth <= dst.depth {
                            continue;
                        }
                        let mut value = vals[0] * w0 + vals[1] * w1 + vals[2] * w2;
                        if card {
                            let s = leaves.sample(uv[0] * w0 + uv[1] * w1 + uv[2] * w2, lod);
                            if s[3] < 0.5 {
                                continue;
                            }
                            let r = Vec3::new(s[0], s[1], s[2]) / leaf_base;
                            value *= r.clamp(Vec3::splat(0.2), Vec3::splat(2.5)).dot(Vec3::new(0.25, 0.6, 0.15));
                        }
                        let mut nn = (nrm[0] * w0 + nrm[1] * w1 + nrm[2] * w2).normalize_or(Vec3::Y);
                        if card && nn.dot(v) < 0.0 {
                            nn = -nn;
                        }
                        let p3 = pos(t[0]) * w0 + pos(t[1]) * w1 + pos(t[2]) * w2;
                        let o = if bark { 0.8 * occl(p3).min(0.9) } else { occl(p3) };
                        *dst = Sample { depth, n: nn, value: value * o, bark: if bark { 1.0 } else { 0.0 }, hit: true };
                    }
                }
            }
            // Box-filter the supersampled frame into the level-0 texels.
            for y in 0..fp {
                for x in 0..fp {
                    let (mut cnt, mut d, mut nn, mut val, mut bk) = (0.0f32, 0.0f32, Vec3::ZERO, 0.0f32, 0.0f32);
                    for sy in 0..SS {
                        for sx in 0..SS {
                            let s = buf[(y * SS + sy) * ss + x * SS + sx];
                            if s.hit {
                                cnt += 1.0;
                                d += s.depth;
                                nn += s.n;
                                val += s.value;
                                bk += s.bark;
                            }
                        }
                    }
                    let t = &mut level0[(j * fp + y) * side + i * fp + x];
                    if cnt > 0.0 {
                        let e = oct_sphere(nn.normalize_or(v));
                        *t = [cnt / (SS * SS) as f32, 0.5 + d / cnt / (2.0 * radius * IMP_PAD), e.x, e.y, val / cnt / VALUE_MAX, bk / cnt];
                    }
                }
            }
        }
    }
    dilate(&mut level0, side, fp);
    let mut levels = vec![level0];
    let mut s = side;
    while s / n > 4 {
        let prev = levels.last().unwrap();
        let ns = s / 2;
        let mut next = vec![[0.0f32; 6]; ns * ns];
        for y in 0..ns {
            for x in 0..ns {
                let mut t = [0.0f32; 6];
                let mut wsum = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = prev[(y * 2 + dy) * s + x * 2 + dx];
                    let w = p[0] + 1e-4;
                    for k in 1..6 {
                        t[k] += p[k] * w;
                    }
                    wsum += w;
                    t[0] += p[0] * 0.25;
                }
                for k in 1..6 {
                    t[k] /= wsum;
                }
                next[y * ns + x] = t;
            }
        }
        levels.push(next);
        s = ns;
    }
    let mut tex: [Vec<Vec<u8>>; 3] = Default::default();
    let mut s = side;
    for lv in &levels {
        for (k, t) in tex.iter_mut().enumerate() {
            t.push(bc5(lv, s, k * 2));
        }
        s /= 2;
    }
    BankImpostor { frames: IMP_FRAMES, frame_px: fp as u32, center_y: c.y, radius, tex }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Spreads depth, normal, value and bark into empty texels (inside each frame), so filtering
/// at crown edges never pulls in zeros.
fn dilate(px: &mut [Texel], side: usize, frame: usize) {
    for _ in 0..frame / 4 {
        let src = px.to_vec();
        let mut changed = false;
        for y in 0..side {
            for x in 0..side {
                if src[y * side + x][0] > 0.0 || src[y * side + x][1] > 0.0 {
                    continue;
                }
                let (fx, fy) = (x / frame * frame, y / frame * frame);
                let mut acc = [0.0f32; 6];
                let mut k = 0.0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < fx as i32 || ny < fy as i32 || nx >= (fx + frame) as i32 || ny >= (fy + frame) as i32 {
                        continue;
                    }
                    let q = src[ny as usize * side + nx as usize];
                    if q[1] > 0.0 {
                        for i in 1..6 {
                            acc[i] += q[i];
                        }
                        k += 1.0;
                    }
                }
                if k > 0.0 {
                    let t = &mut px[y * side + x];
                    for i in 1..6 {
                        t[i] = acc[i] / k;
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// BC5 (two BC4 blocks) of channels `ch`, `ch + 1` of a `side` x `side` level.
fn bc5(px: &[Texel], side: usize, ch: usize) -> Vec<u8> {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let mut out = Vec::with_capacity(side * side);
    for by in (0..side).step_by(4) {
        for bx in (0..side).step_by(4) {
            for c in [ch, ch + 1] {
                let mut blk = [0u8; 16];
                for y in 0..4 {
                    for x in 0..4 {
                        blk[y * 4 + x] = q(px[(by + y) * side + bx + x][c]);
                    }
                }
                out.extend(bc4_block(&blk));
            }
        }
    }
    out
}

/// One BC4 block (8 bytes) of 16 values, row-major: the 8-value mode between the block's
/// min and max, each texel its nearest palette entry.
pub fn bc4_block(v: &[u8; 16]) -> [u8; 8] {
    let (lo, hi) = (*v.iter().min().unwrap(), *v.iter().max().unwrap());
    let mut out = [0u8; 8];
    out[0] = hi;
    out[1] = lo;
    if hi == lo {
        return out;
    }
    let (r0, r1) = (hi as u32, lo as u32);
    let pal: [u32; 8] = [r0, r1, (6 * r0 + r1) / 7, (5 * r0 + 2 * r1) / 7, (4 * r0 + 3 * r1) / 7, (3 * r0 + 4 * r1) / 7, (2 * r0 + 5 * r1) / 7, (r0 + 6 * r1) / 7];
    let mut bits = 0u64;
    for (i, &x) in v.iter().enumerate() {
        let mut best = (u32::MAX, 0u64);
        for (k, &p) in pal.iter().enumerate() {
            let d = (p as i32 - x as i32).unsigned_abs();
            if d < best.0 {
                best = (d, k as u64);
            }
        }
        bits |= best.1 << (3 * i);
    }
    out[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
    out
}

/// Decodes one BC4 block (tests and debugging).
pub fn bc4_decode(b: &[u8; 8]) -> [u8; 16] {
    let (r0, r1) = (b[0] as u32, b[1] as u32);
    let pal: [u32; 8] = if r0 > r1 {
        [r0, r1, (6 * r0 + r1) / 7, (5 * r0 + 2 * r1) / 7, (4 * r0 + 3 * r1) / 7, (3 * r0 + 4 * r1) / 7, (2 * r0 + 5 * r1) / 7, (r0 + 6 * r1) / 7]
    } else {
        [r0, r1, (4 * r0 + r1) / 5, (3 * r0 + 2 * r1) / 5, (2 * r0 + 3 * r1) / 5, (r0 + 4 * r1) / 5, 0, 255]
    };
    let mut w = [0u8; 8];
    w[..6].copy_from_slice(&b[2..8]);
    let bits = u64::from_le_bytes(w);
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = pal[((bits >> (3 * i)) & 7) as usize] as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trees::bank;

    #[test]
    fn bc4_roundtrip_is_close() {
        let mut v = [0u8; 16];
        for (i, x) in v.iter_mut().enumerate() {
            *x = (i as u32 * 37 % 200 + 20) as u8;
        }
        let d = bc4_decode(&bc4_block(&v));
        for i in 0..16 {
            assert!((d[i] as i32 - v[i] as i32).abs() <= 15, "{i}: {} vs {}", d[i], v[i]);
        }
        let flat = [77u8; 16];
        assert_eq!(bc4_decode(&bc4_block(&flat)), flat);
        let two = [0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255];
        assert_eq!(bc4_decode(&bc4_block(&two)), two);
    }

    #[test]
    fn oct_sphere_roundtrip() {
        for k in 0..200 {
            let a = k as f32 * 0.7;
            let b = (k as f32 * 0.37).sin();
            let n = Vec3::new(a.cos() * (1.0 - b * b).sqrt(), b, a.sin() * (1.0 - b * b).sqrt());
            let r = oct_sphere_decode(oct_sphere(n));
            assert!(r.distance(n) < 1e-4, "{n} -> {r}");
        }
    }

    #[test]
    fn bakes_a_bank_tree_deterministically() {
        let atlas = crate::trees::textures::leaf_atlas();
        let leaves = LeafTex::new(&atlas);
        let m = bank::bank_mesh(0, 0, 2);
        let a = bake(&m, 1, &leaves, ImpostorTier::Low);
        let b = bake(&m, 1, &leaves, ImpostorTier::Low);
        assert_eq!(a, b);
        assert_eq!(a.size(), 256);
        // 256, 128, 64, 32 (4 px frames).
        assert_eq!(a.tex[0].len(), 4);
        assert_eq!(a.tex[0][0].len(), 256 * 256);
        assert!(a.radius > m.height * 0.45 && a.radius < m.height * 1.2, "{} {}", a.radius, m.height);
        // The side frames carry a crown: some coverage in every frame.
        let side = a.size() as usize;
        let fp = a.frame_px as usize;
        let cov = |fi: usize, fj: usize| {
            let mut c = 0usize;
            for by in (fj * fp..(fj + 1) * fp).step_by(4) {
                for bx in (fi * fp..(fi + 1) * fp).step_by(4) {
                    let o = ((by / 4) * (side / 4) + bx / 4) * 16;
                    let blk: [u8; 8] = a.tex[0][0][o..o + 8].try_into().unwrap();
                    c += bc4_decode(&blk).iter().filter(|&&x| x > 128).count();
                }
            }
            c as f32 / (fp * fp) as f32
        };
        for j in 0..IMP_FRAMES as usize {
            for i in 0..IMP_FRAMES as usize {
                let c = cov(i, j);
                assert!(c > 0.03 && c < 0.9, "frame {i},{j}: coverage {c}");
            }
        }
        let back = BankImpostor::from_bytes(&a.to_bytes()).expect("roundtrip");
        assert_eq!(back, a);
    }
}
