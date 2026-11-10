//! Cube-sphere node addresses (`docs/contracts/addressing.md`).
//!
//! One address for every system: terrain, chunks, streaming, the editor and the server all key
//! on [`NodeId`]. The mapping from lattice indices to the sphere is the tangent warp, evaluated
//! from integers so that points shared by two faces, or by a parent and a child, are
//! bit-identical.

use glam::DVec3;
use std::f64::consts::FRAC_PI_4;
use std::fmt;

/// Index of a body in the body catalog (`ed_space`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct BodyId(pub u8);

/// `body:8 | face:3 | level:5 | x:24 | y:24`, most significant first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct NodeId(u64);

/// Lattice coordinate `i` in `0..=n` to a face coordinate in `[-1, 1]`.
///
/// Exact at the ends and the centre and exactly odd about the centre. `n` must be a power of
/// two. `tan` is the software one so every platform returns the same bits.
pub fn warp(i: u64, n: u64) -> f64 {
    debug_assert!(n.is_power_of_two() && i <= n);
    if i == 0 {
        return -1.0;
    }
    if i == n {
        return 1.0;
    }
    if 2 * i == n {
        return 0.0;
    }
    if 2 * i > n {
        return -warp(n - i, n);
    }
    // Exact: 2i/n is a dyadic rational in (0, 1).
    let s = (2 * i) as f64 / n as f64 - 1.0;
    libm::tan(s * FRAC_PI_4)
}

fn cube(face: u8, u: f64, v: f64) -> [f64; 3] {
    match face {
        0 => [1.0, u, v],
        1 => [-1.0, -u, v],
        2 => [-u, 1.0, v],
        3 => [u, -1.0, v],
        4 => [-v, u, 1.0],
        _ => [v, u, -1.0],
    }
}

/// Integer form of [`cube`]: face coordinates in `-n..=n`, major axis at `±n`.
fn cube_i(face: u8, u: i64, v: i64, n: i64) -> [i64; 3] {
    match face {
        0 => [n, u, v],
        1 => [-n, -u, v],
        2 => [-u, n, v],
        3 => [u, -n, v],
        4 => [-v, u, n],
        _ => [v, u, -n],
    }
}

/// Inverse of [`cube_i`] for a point whose component on `face`'s axis is `±n`.
fn uncube_i(face: u8, c: [i64; 3]) -> (i64, i64) {
    match face {
        0 => (c[1], c[2]),
        1 => (-c[1], c[2]),
        2 => (-c[0], c[2]),
        3 => (c[0], c[2]),
        4 => (c[1], -c[0]),
        _ => (c[1], c[0]),
    }
}

fn uncube(face: u8, c: [f64; 3]) -> (f64, f64) {
    match face {
        0 => (c[1], c[2]),
        1 => (-c[1], c[2]),
        2 => (-c[0], c[2]),
        3 => (c[0], c[2]),
        4 => (c[1], -c[0]),
        _ => (c[1], c[0]),
    }
}

/// Unit vector of lattice point `(i, j)` on `face`, with `n` cells per side (a power of two).
///
/// The sum of squares is taken in body x, y, z order and `+ 0.0` removes negative zero, so the
/// result does not depend on which face asked.
pub fn lattice_point(face: u8, i: u64, j: u64, n: u64) -> DVec3 {
    let c = cube(face, warp(i, n), warp(j, n));
    let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    DVec3::new(c[0] / l + 0.0, c[1] / l + 0.0, c[2] / l + 0.0)
}

impl NodeId {
    pub const MAX_LEVEL: u8 = 24;

    /// `None` if the face, level or coordinates are out of range.
    pub fn new(body: BodyId, face: u8, level: u8, x: u32, y: u32) -> Option<NodeId> {
        if face > 5 || level > Self::MAX_LEVEL {
            return None;
        }
        let n = 1u32 << level;
        if x >= n || y >= n {
            return None;
        }
        Some(NodeId((body.0 as u64) << 56 | (face as u64) << 53 | (level as u64) << 48 | (x as u64) << 24 | y as u64))
    }

    /// Rebuilds a node from its raw bits, checking validity.
    pub fn from_bits(bits: u64) -> Option<NodeId> {
        let id = NodeId(bits);
        (NodeId::new(id.body(), id.face(), id.level(), id.x(), id.y()) == Some(id)).then_some(id)
    }

    pub fn bits(self) -> u64 {
        self.0
    }
    pub fn body(self) -> BodyId {
        BodyId((self.0 >> 56) as u8)
    }
    pub fn face(self) -> u8 {
        (self.0 >> 53 & 7) as u8
    }
    pub fn level(self) -> u8 {
        (self.0 >> 48 & 31) as u8
    }
    pub fn x(self) -> u32 {
        (self.0 >> 24 & 0xff_ffff) as u32
    }
    pub fn y(self) -> u32 {
        (self.0 & 0xff_ffff) as u32
    }
    /// Cells per face side at this node's level.
    pub fn side(self) -> u32 {
        1 << self.level()
    }

    fn make(self, level: u8, x: u32, y: u32) -> NodeId {
        NodeId((self.0 >> 53) << 53 | (level as u64) << 48 | (x as u64) << 24 | y as u64)
    }

    pub fn parent(self) -> Option<NodeId> {
        (self.level() > 0).then(|| self.make(self.level() - 1, self.x() >> 1, self.y() >> 1))
    }

    /// Children in the order (0,0) (1,0) (0,1) (1,1). `None` at the deepest level.
    pub fn children(self) -> Option<[NodeId; 4]> {
        if self.level() >= Self::MAX_LEVEL {
            return None;
        }
        let (l, x, y) = (self.level() + 1, self.x() << 1, self.y() << 1);
        Some([self.make(l, x, y), self.make(l, x + 1, y), self.make(l, x, y + 1), self.make(l, x + 1, y + 1)])
    }

    /// The ancestor at `level`, or `self` if `level` is not above this node.
    pub fn ancestor(self, level: u8) -> NodeId {
        if level >= self.level() {
            return self;
        }
        let s = self.level() - level;
        self.make(level, self.x() >> s, self.y() >> s)
    }

    /// True if `other` is this node or a descendant of it.
    pub fn contains(self, other: NodeId) -> bool {
        other.level() >= self.level() && other.ancestor(self.level()) == self
    }

    /// The node `(dx, dy)` cells away at the same level, crossing face edges.
    ///
    /// Offsets are clamped to one face width. A diagonal step across a cube corner has no
    /// unique answer (three faces meet there): the x step crosses and the y step is clamped.
    pub fn neighbour(self, dx: i32, dy: i32) -> NodeId {
        let n = self.side() as i64;
        // Doubled coordinates: cell centres are odd integers in (-n, n).
        let u = 2 * self.x() as i64 + 1 - n + 2 * (dx as i64).clamp(-n, n);
        let mut v = 2 * self.y() as i64 + 1 - n + 2 * (dy as i64).clamp(-n, n);
        if u.abs() > n && v.abs() > n {
            v = v.clamp(-n + 1, n - 1);
        }
        if u.abs() < n && v.abs() < n {
            return self.make(self.level(), ((u + n) / 2) as u32, ((v + n) / 2) as u32);
        }
        // Fold over the edge: d cells past the edge is d cells deep on the adjoining face.
        let over_u = u.abs() > n;
        let d = if over_u { u.abs() - n } else { v.abs() - n };
        let (cu, cv) = if over_u { (u.signum() * n, v) } else { (u, v.signum() * n) };
        let mut c = cube_i(self.face(), cu, cv, n);
        let major = (self.face() / 2) as usize;
        c[major] -= c[major].signum() * d;
        // The other in-face coordinate is a cell centre and never reaches ±n, so this is unique.
        let axis = (0..3).find(|&a| a != major && c[a].abs() == n).expect("fold axis");
        let face = (axis * 2) as u8 + (c[axis] < 0) as u8;
        let (fu, fv) = uncube_i(face, c);
        NodeId((self.body().0 as u64) << 56 | (face as u64) << 53).make(self.level(), ((fu + n) / 2) as u32, ((fv + n) / 2) as u32)
    }

    /// Corner `i` as a body-fixed unit vector. Order: (0,0) (1,0) (0,1) (1,1).
    pub fn corner(self, i: u8) -> DVec3 {
        lattice_point(self.face(), (self.x() + (i & 1) as u32) as u64, (self.y() + (i >> 1 & 1) as u32) as u64, self.side() as u64)
    }

    pub fn centre(self) -> DVec3 {
        lattice_point(self.face(), 2 * self.x() as u64 + 1, 2 * self.y() as u64 + 1, 2 * self.side() as u64)
    }

    /// Lattice point `(i, j)` of this node's own grid of `cells` cells per side (a power of
    /// two, at most `2^30 / side`). Border points are shared with neighbours bit for bit.
    pub fn grid_point(self, i: u32, j: u32, cells: u32) -> DVec3 {
        let c = cells as u64;
        lattice_point(self.face(), self.x() as u64 * c + i as u64, self.y() as u64 * c + j as u64, self.side() as u64 * c)
    }

    /// The node at `level` that contains direction `p` (need not be normalised).
    ///
    /// Consistent with the forward mapping: a point on a shared edge belongs to the node on
    /// the higher-coordinate side, and on a face edge to the lower-numbered face.
    pub fn from_unit(body: BodyId, p: DVec3, level: u8) -> NodeId {
        let level = level.min(Self::MAX_LEVEL);
        let a = p.abs();
        let axis = if a.x >= a.y && a.x >= a.z { 0 } else if a.y >= a.z { 1 } else { 2 };
        let m = [p.x, p.y, p.z][axis];
        let face = (axis * 2) as u8 + (m < 0.0) as u8;
        let k = m.abs();
        let (u, v) = uncube(face, [p.x / k, p.y / k, p.z / k]);
        let n = 1u64 << level;
        let cell = |t: f64| -> u32 {
            let s = libm::atan(t.clamp(-1.0, 1.0)) / FRAC_PI_4;
            let mut i = (((s + 1.0) * 0.5 * n as f64) as u64).min(n - 1);
            // The estimate can be one cell off near a boundary; settle it against the forward warp.
            while i > 0 && t < warp(i, n) {
                i -= 1;
            }
            while i + 1 < n && t >= warp(i + 1, n) {
                i += 1;
            }
            i as u32
        };
        NodeId((body.0 as u64) << 56 | (face as u64) << 53).make(level, cell(u), cell(v))
    }
}

/// `body/face/level/x/y`, for logs, file names and URLs.
impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}/{}/{}", self.body().0, self.face(), self.level(), self.x(), self.y())
    }
}
impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({self})")
    }
}
impl std::str::FromStr for NodeId {
    type Err = ();
    fn from_str(s: &str) -> Result<NodeId, ()> {
        let mut it = s.split('/').map(|p| p.parse::<u32>());
        let mut next = || it.next().and_then(Result::ok).ok_or(());
        let (b, f, l, x, y) = (next()?, next()?, next()?, next()?, next()?);
        if it.next().is_some() || b > 255 || f > 5 || l > 24 {
            return Err(());
        }
        NodeId::new(BodyId(b as u8), f as u8, l as u8, x, y).ok_or(())
    }
}

/// Stable identity of a generated or authored object. Never encodes a node, a rung or a
/// version, so it survives re-bakes and LOD changes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ObjectId(pub u128);

impl ObjectId {
    const AUTHORED: u128 = 1 << 127;

    /// First 16 bytes of BLAKE3 over the length-prefixed fields, top bit clear.
    pub fn generated(dataset: &str, feature: &str, generator: &str, sub: u32) -> ObjectId {
        let mut h = blake3::Hasher::new();
        for f in [dataset, feature, generator] {
            h.update(&(f.len() as u32).to_le_bytes());
            h.update(f.as_bytes());
        }
        h.update(&sub.to_le_bytes());
        let mut b = [0u8; 16];
        b.copy_from_slice(&h.finalize().as_bytes()[..16]);
        ObjectId(u128::from_be_bytes(b) & !Self::AUTHORED)
    }

    /// An authored id from 128 random bits. The layer file that creates the object stores it.
    pub fn authored(random: u128) -> ObjectId {
        ObjectId(random | Self::AUTHORED)
    }

    pub fn is_authored(self) -> bool {
        self.0 & Self::AUTHORED != 0
    }

    /// The 32 bits the renderer carries per instance for picking.
    pub fn pick_bits(self) -> u32 {
        self.0 as u32
    }
}
impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}
impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({self})")
    }
}
impl std::str::FromStr for ObjectId {
    type Err = ();
    fn from_str(s: &str) -> Result<ObjectId, ()> {
        if s.len() != 32 {
            return Err(());
        }
        u128::from_str_radix(s, 16).map(ObjectId).map_err(|_| ())
    }
}
