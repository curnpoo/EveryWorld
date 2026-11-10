//! Tests A1 to A9 of `docs/contracts/addressing.md`.
use ed_geo::node::{lattice_point, warp};
use ed_geo::{BodyId, NodeId, ObjectId};
use glam::DVec3;
use std::collections::{HashMap, HashSet};

const B: BodyId = BodyId(3);
const STEPS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Small deterministic generator so the tests need no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let x = self.0;
        (x ^ x >> 33).wrapping_mul(0xff51afd7ed558ccd) >> 11
    }
    fn node(&mut self, level: u8) -> NodeId {
        let n = 1u64 << level;
        NodeId::new(B, (self.next() % 6) as u8, level, (self.next() % n) as u32, (self.next() % n) as u32).unwrap()
    }
    fn unit(&mut self) -> DVec3 {
        loop {
            let mut f = || (self.next() % 2_000_001) as f64 / 1_000_000.0 - 1.0;
            let p = DVec3::new(f(), f(), f());
            if p.length() > 0.1 && p.length() <= 1.0 {
                return p.normalize();
            }
        }
    }
}
fn bits(p: DVec3) -> [u64; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}
/// Number of corners of `a` that are bit-identical to a corner of `b`. Two for a shared edge.
fn shared_corners(a: NodeId, b: NodeId) -> usize {
    (0..4).filter(|&i| (0..4).any(|j| bits(a.corner(i)) == bits(b.corner(j)))).count()
}

#[test]
fn a1_round_trip_and_validity() {
    let id = NodeId::new(BodyId(4), 5, 24, (1 << 24) - 1, 12345).unwrap();
    assert_eq!((id.body(), id.face(), id.level(), id.x(), id.y()), (BodyId(4), 5, 24, (1 << 24) - 1, 12345));
    assert_eq!(id.to_string(), "4/5/24/16777215/12345");
    assert_eq!("4/5/24/16777215/12345".parse(), Ok(id));
    assert_eq!(NodeId::from_bits(id.bits()), Some(id));
    assert!(NodeId::new(B, 6, 0, 0, 0).is_none());
    assert!(NodeId::new(B, 0, 25, 0, 0).is_none());
    assert!(NodeId::new(B, 0, 3, 8, 0).is_none());
    assert!(NodeId::from_bits(id.bits() | 7 << 53).is_none());
    assert!("3/0/3/8/0".parse::<NodeId>().is_err());
    // Sort order is body, face, level, x, y.
    let a = NodeId::new(B, 1, 5, 3, 9).unwrap();
    assert!(a < NodeId::new(B, 1, 5, 4, 0).unwrap());
    assert!(a < NodeId::new(B, 1, 6, 0, 0).unwrap());
    assert!(a < NodeId::new(B, 2, 0, 0, 0).unwrap());
}

#[test]
fn a2_hierarchy() {
    let mut r = Rng(1);
    for level in 0..=24u8 {
        for _ in 0..200 {
            let n = r.node(level);
            assert_eq!(n.parent().is_none(), level == 0);
            assert_eq!(n.children().is_none(), level == 24);
            if let Some(c) = n.children() {
                for (i, c) in c.iter().enumerate() {
                    assert_eq!(c.parent(), Some(n));
                    assert_eq!((c.x() & 1, c.y() & 1), (i as u32 & 1, i as u32 >> 1));
                    assert!(n.contains(*c) && !c.contains(n));
                }
            }
            let up = (r.next() % (level as u64 + 1)) as u8;
            let a = n.ancestor(up);
            assert!(a.level() == up && a.contains(n));
            let mut p = n;
            while p.level() > up {
                p = p.parent().unwrap();
            }
            assert_eq!(p, a);
        }
    }
}

#[test]
fn a3_neighbours() {
    for level in [0u8, 1, 2, 5, 24] {
        let n = 1u32 << level;
        let mut edge_pairs = HashSet::new();
        for face in 0..6u8 {
            // Every cell at small levels; the corners and one mid cell at deep ones.
            let coords: Vec<u32> = if n <= 32 { (0..n).collect() } else { vec![0, n / 2, n - 1] };
            for &x in &coords {
                for &y in &coords {
                    let id = NodeId::new(B, face, level, x, y).unwrap();
                    assert_eq!(id.neighbour(0, 0), id);
                    for (dx, dy) in STEPS {
                        let nb = id.neighbour(dx, dy);
                        assert_eq!(nb.level(), level);
                        assert_ne!(nb, id);
                        assert_eq!(shared_corners(id, nb), 2, "{id} step ({dx},{dy}) gave {nb}");
                        assert!(STEPS.iter().any(|&(a, b)| nb.neighbour(a, b) == id), "{id} -> {nb} not symmetric");
                        if nb.face() != face {
                            edge_pairs.insert((face.min(nb.face()), face.max(nb.face())));
                        }
                    }
                }
            }
        }
        assert_eq!(edge_pairs.len(), 12, "all 12 cube edges crossed at level {level}");
    }
    let id = NodeId::new(B, 0, 4, 14, 7).unwrap();
    assert_eq!(id.neighbour(-3, 2), NodeId::new(B, 0, 4, 11, 9).unwrap());
    // Two cells past the edge is one more step from the first cell on the next face.
    let (one, two) = (id.neighbour(2, 0), id.neighbour(3, 0));
    assert_ne!(one.face(), 0);
    assert_eq!(two.face(), one.face());
    assert_eq!(shared_corners(one, two), 2);
    assert_eq!(shared_corners(two, id.neighbour(1, 0)), 0);
    // A diagonal across a cube corner still returns a node that touches this one.
    for face in 0..6u8 {
        let c = NodeId::new(B, face, 3, 7, 7).unwrap();
        assert!(shared_corners(c, c.neighbour(1, 1)) >= 1);
    }
}

#[test]
fn a4_face_edges_agree() {
    let n = 1024u64;
    let mut seen: HashMap<[u64; 3], HashSet<u8>> = HashMap::new();
    for face in 0..6u8 {
        for k in 0..=n {
            for (i, j) in [(k, 0), (k, n), (0, k), (n, k)] {
                seen.entry(bits(lattice_point(face, i, j, n))).or_default().insert(face);
            }
        }
    }
    // 12 edges with n-1 interior points, each on 2 faces; 8 corners, each on 3.
    assert_eq!(seen.len() as u64, 12 * (n - 1) + 8);
    assert_eq!(seen.values().filter(|f| f.len() == 3).count(), 8);
    assert!(seen.values().all(|f| f.len() == 2 || f.len() == 3));
}

#[test]
fn a5_parent_child_points_agree() {
    let mut r = Rng(5);
    for level in 0..24u8 {
        for _ in 0..50 {
            let p = r.node(level);
            let c = p.children().unwrap();
            for i in 0..4 {
                assert_eq!(bits(p.corner(i)), bits(c[i as usize].corner(i)));
            }
            assert_eq!(bits(p.centre()), bits(c[0].corner(3)));
            // A node's own grid shares its border with every neighbour's grid, across faces too.
            for (dx, dy) in STEPS {
                let nb = p.neighbour(dx, dy);
                let mine: HashSet<_> = (0..=16).flat_map(|k| [(k, 0), (k, 16), (0, k), (16, k)]).map(|(i, j)| bits(p.grid_point(i, j, 16))).collect();
                let theirs = (0..=16).flat_map(|k| [(k, 0), (k, 16), (0, k), (16, k)]).filter(|&(i, j)| mine.contains(&bits(nb.grid_point(i, j, 16)))).count();
                // A full shared edge: 17 points (corner points appear twice in the border walk).
                assert!(theirs >= 17, "{p} and {nb} share {theirs} grid points");
            }
        }
    }
}

#[test]
fn a6_from_unit() {
    let mut r = Rng(6);
    for level in 0..=24u8 {
        for _ in 0..100 {
            let n = r.node(level);
            assert_eq!(NodeId::from_unit(B, n.centre(), level), n);
            // A corner belongs to this node or to one that touches it.
            for i in 0..4 {
                let at = NodeId::from_unit(B, n.corner(i), level);
                assert!(at == n || shared_corners(at, n) >= 1, "{n} corner {i} -> {at}");
            }
            // A random point lands in a chain of nested nodes, and its node brackets it.
            let p = r.unit();
            let leaf = NodeId::from_unit(B, p, level);
            for up in 0..level {
                assert_eq!(NodeId::from_unit(B, p, up), leaf.ancestor(up));
            }
            let c = leaf.centre();
            let radius = (0..4).map(|i| leaf.corner(i).distance(c)).fold(0.0, f64::max);
            assert!(p.distance(c) <= radius * 1.0001 + 1e-15);
        }
    }
}

#[test]
fn a7_distortion() {
    let n = 64u32;
    let (mut amin, mut amax, mut emin, mut emax) = (f64::MAX, 0f64, f64::MAX, 0f64);
    for x in 0..n {
        for y in 0..n {
            let id = NodeId::new(B, 0, 6, x, y).unwrap();
            let c: Vec<DVec3> = (0..4).map(|i| id.corner(i)).collect();
            let tri = |a: DVec3, b: DVec3, c: DVec3| 2.0 * a.dot(b.cross(c)).abs().atan2(1.0 + a.dot(b) + b.dot(c) + c.dot(a));
            let area = tri(c[0], c[1], c[3]) + tri(c[0], c[3], c[2]);
            amin = amin.min(area);
            amax = amax.max(area);
            for (a, b) in [(0, 1), (0, 2), (1, 3), (2, 3)] {
                let e = c[a].angle_between(c[b]);
                emin = emin.min(e);
                emax = emax.max(e);
            }
        }
    }
    assert!(amax / amin < 1.45, "area ratio {}", amax / amin);
    assert!(emax / emin < 1.45, "edge ratio {}", emax / emin);
}

/// One hash over 64 fixed corners and centres. The value must be the same on every platform.
#[test]
fn a8_golden_bits() {
    let mut r = Rng(8);
    let mut h = blake3::Hasher::new();
    for k in 0..64u32 {
        let id = r.node((k % 25) as u8);
        for w in bits(id.corner((k % 4) as u8)).into_iter().chain(bits(id.centre())) {
            h.update(&w.to_le_bytes());
        }
    }
    assert!((warp(1, 4) + (std::f64::consts::SQRT_2 - 1.0)).abs() < 1e-15);
    assert_eq!(&h.finalize().to_hex()[..16], "2fa9a77e19dafa33");
}

#[test]
fn a9_object_ids() {
    let gers = "08b2a100d2ca1fff0200a5b5c8f1c1d2";
    let a = ObjectId::generated("overture-buildings", gers, "building.shell", 0);
    assert_eq!(a.to_string(), "387ef0a1fec25350119f17c455a698e9");
    assert_eq!(a.to_string().parse(), Ok(a));
    assert!(!a.is_authored());
    for other in [
        ObjectId::generated("overture-building", gers, "building.shell", 0),
        ObjectId::generated("overture-buildings", &gers[1..], "building.shell", 0),
        ObjectId::generated("overture-buildings", gers, "building.shel", 0),
        ObjectId::generated("overture-buildings", gers, "building.shell", 1),
        // Length prefixes keep field boundaries from sliding.
        ObjectId::generated("overture-buildings0", &gers[1..], "building.shell", 0),
    ] {
        assert_ne!(other, a);
    }
    let mut r = Rng(9);
    for _ in 0..1000 {
        let authored = ObjectId::authored((r.next() as u128) << 64 | r.next() as u128);
        assert!(authored.is_authored());
        assert!(!ObjectId::generated("parcel", &authored.to_string(), "fill.tree", r.next() as u32).is_authored());
    }
}
