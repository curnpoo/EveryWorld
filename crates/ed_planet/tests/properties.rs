//! Phase 2 W1 acceptance properties (`docs/phases/phase-2.md` §6, W1 row).

use ed_geo::{BodyId, NodeId};
use ed_planet::*;
use glam::{DQuat, DVec3};
use std::collections::BTreeSet;
use std::time::Instant;

struct Rng(u64);
impl Rng {
    fn u64(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f()
    }
    fn unit(&mut self) -> DVec3 {
        loop {
            let v = DVec3::new(self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0));
            let l = v.length();
            if l > 1e-3 && l <= 1.0 {
                return v / l;
            }
        }
    }
}

fn hash(id: NodeId, salt: u64) -> u64 {
    Rng(id.bits() ^ salt.wrapping_mul(0x2545_F491_4F6C_DD1D)).u64()
}

const MOON_AT: DVec3 = DVec3::new(3.844e8, 0.0, 0.0);

fn earth() -> BodyId {
    Body::earth().id
}
fn moon() -> BodyId {
    Body::moon().id
}

fn poses() -> Vec<BodyPose> {
    vec![BodyPose::identity(earth()), BodyPose { id: moon(), rotation: DQuat::from_rotation_z(0.3), position: MOON_AT }]
}

fn planet() -> Planet {
    Planet::new(vec![Body::earth(), Body::moon()])
}

fn bytes_of(id: NodeId) -> u64 {
    100_000 + hash(id, 1) % 300_000
}

fn residency() -> FakeResidency {
    let mut r = FakeResidency::new(0);
    r.bytes_fn = Some(Box::new(bytes_of));
    r
}

/// A camera from the ground to 400,000 km, near Earth or the Moon, looking at the body
/// half of the time and anywhere otherwise.
fn random_view(rng: &mut Rng) -> View {
    let (centre, radius) = if rng.f() < 0.8 { (DVec3::ZERO, ed_geo::WGS84_A) } else { (MOON_AT, Body::moon().equatorial_m) };
    let alt = 10f64.powf(rng.range(0.0, 8.6)); // 1 m .. 400,000 km
    let up = rng.unit();
    let position = centre + up * (radius + alt);
    let look = if rng.f() < 0.5 {
        // Toward the body, tilted.
        (-up + rng.unit() * rng.range(0.0, 1.2)).normalize()
    } else {
        rng.unit()
    };
    let roll = rng.range(-3.0, 3.0);
    let rotation = DQuat::from_rotation_arc(DVec3::NEG_Z, look) * DQuat::from_rotation_z(roll);
    let viewport = if rng.f() < 0.5 { [1920, 1080] } else { [2560, 1440] };
    View {
        position,
        rotation,
        velocity: rng.unit() * rng.range(0.0, 400.0),
        fov_y: rng.range(30f64, 90.0).to_radians(),
        viewport,
        look_ahead_s: 1.0,
    }
}

fn roots_bytes() -> u64 {
    [earth(), moon()].iter().flat_map(|&b| (0..6).map(move |f| bytes_of(NodeId::new(b, f, 0, 0, 0).unwrap()))).sum()
}

/// Restricted tree: every same-level edge neighbour of a split node (level >= 1) is present.
fn assert_restricted(nodes: &BTreeSet<NodeId>, split: impl Iterator<Item = NodeId>, what: &str) {
    for n in split {
        if n.level() == 0 {
            continue;
        }
        for m in edge_neighbours(n) {
            assert!(nodes.contains(&m), "{what}: split {n} has neighbour {m} missing (more than one level apart)");
        }
    }
}

#[test]
fn wanted_within_budget_and_restricted_10k_cameras() {
    let mut rng = Rng(0x5eed_0001);
    let res = residency();
    let mut p = planet();
    let roots = roots_bytes();
    let (mut total_nodes, mut max_nodes, mut limited) = (0usize, 0usize, 0u32);
    for _ in 0..10_000 {
        let view = random_view(&mut rng);
        let budget = roots + (10f64.powf(rng.range(5.0, 8.7))) as u64; // up to ~500 MB
        let cfg = SelectConfig { budget_bytes: budget, pixel_error: rng.range(1.0, 8.0), ..Default::default() };
        let s = p.select(&view, &poses(), &res, &cfg);
        let sum: u64 = s.wanted.iter().map(|&n| bytes_of(n)).sum();
        assert_eq!(sum, s.wanted_bytes);
        assert!(s.wanted_bytes <= budget, "{} > {budget}", s.wanted_bytes);
        let wanted: BTreeSet<NodeId> = s.wanted.iter().copied().collect();
        let split: BTreeSet<NodeId> = s.split.iter().copied().collect();
        for &n in &s.wanted {
            if let Some(par) = n.parent() {
                assert!(split.contains(&par), "{n} wanted without its parent split");
            }
        }
        for &n in &s.split {
            for c in n.children().unwrap() {
                assert!(wanted.contains(&c));
            }
        }
        assert_restricted(&wanted, s.split.iter().copied(), "wanted");
        total_nodes += s.wanted.len();
        max_nodes = max_nodes.max(s.wanted.len());
        limited += (s.budget_limited > 0) as u32;
    }
    println!("10k cameras: mean wanted {} nodes, max {max_nodes}, budget-limited in {limited}", total_nodes / 10_000);
    assert!(limited > 100, "the budget must actually bind in a good share of cases");
}

#[test]
fn drawn_nodes_are_complete_and_restricted() {
    let mut rng = Rng(0x5eed_0002);
    let mut p = planet();
    for _ in 0..1500 {
        let view = random_view(&mut rng);
        let pc = rng.range(0.3, 1.0);
        let threshold = (pc * u64::MAX as f64) as u64;
        let roots_missing = rng.f() < 0.1;
        let mut res = residency();
        res.complete_fn = Some(Box::new(move |id: NodeId| if id.level() == 0 { !roots_missing || id.face() != 2 } else { hash(id, 7) < threshold }));
        let cfg = SelectConfig { budget_bytes: roots_bytes() + rng.range(1e6, 3e8) as u64, ..Default::default() };
        let s = p.select(&view, &poses(), &res, &cfg);
        let wanted: BTreeSet<NodeId> = s.wanted.iter().copied().collect();
        let drawn: BTreeSet<NodeId> = s.draw.iter().map(|d| d.id).collect();
        for d in &s.draw {
            assert!(res.complete(d.id), "{} drawn without complete chunks", d.id);
            assert!(wanted.contains(&d.id));
            assert!(d.origin.is_finite() && d.basis.is_finite());
            let mut a = d.id;
            while let Some(par) = a.parent() {
                assert!(!drawn.contains(&par), "{} and its ancestor {par} both drawn", d.id);
                a = par;
            }
        }
        // Coverage: drawn area plus holes is every root exactly once.
        let area: f64 = s.draw.iter().map(|d| 0.25f64.powi(d.id.level() as i32)).sum::<f64>() + s.holes.len() as f64;
        assert!((area - 12.0).abs() < 1e-9, "coverage {area}");
        for h in &s.holes {
            assert!(h.level() == 0 && !res.complete(*h));
        }
        // Interior of the draw tree is restricted too.
        let mut interior = BTreeSet::new();
        for d in &drawn {
            let mut a = *d;
            while let Some(par) = a.parent() {
                interior.insert(par);
                a = par;
            }
        }
        let mut present: BTreeSet<NodeId> = drawn.clone();
        present.extend(interior.iter().copied());
        present.extend(s.holes.iter().copied());
        assert_restricted(&present, interior.iter().copied(), "draw");
    }
}

#[test]
fn same_input_same_output() {
    let run = || {
        let mut rng = Rng(0x5eed_0003);
        let mut p = planet();
        let mut res = residency();
        res.complete_fn = Some(Box::new(|id: NodeId| !hash(id, 9).is_multiple_of(5) || id.level() == 0));
        let mut out = Vec::new();
        for i in 0..400 {
            // Mix smooth motion (hysteresis paths) and jumps.
            let view = if i % 4 == 0 {
                random_view(&mut rng)
            } else {
                let mut v = random_view(&mut Rng(0x77 + i / 4));
                v.position += DVec3::new(10.0, 3.0, -2.0) * (i % 4) as f64;
                v
            };
            let cfg = SelectConfig { budget_bytes: roots_bytes() + 80_000_000, ..Default::default() };
            out.push(p.select(&view, &poses(), &res, &cfg));
        }
        out
    };
    let (a, b) = (run(), run());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.wanted, y.wanted);
        assert_eq!(x.draw.len(), y.draw.len());
        for (d, e) in x.draw.iter().zip(&y.draw) {
            assert_eq!(d.id, e.id);
            assert_eq!(d.origin.to_array().map(f32::to_bits), e.origin.to_array().map(f32::to_bits));
            assert_eq!(d.basis.to_cols_array().map(f32::to_bits), e.basis.to_cols_array().map(f32::to_bits));
            assert_eq!(d.morph.map(f32::to_bits), e.morph.map(f32::to_bits));
        }
        assert_eq!(x, y);
    }
}

/// Unit direction at fractional lattice coordinates of `id`'s grid of `cells` cells: the
/// forward warp of `ed_geo::node`, extended to non-integer positions.
fn dir_at(id: NodeId, fi: f64, fj: f64, cells: f64) -> DVec3 {
    let n = id.side() as f64 * cells;
    let w = |g: f64| libm::tan((2.0 * g / n - 1.0) * std::f64::consts::FRAC_PI_4);
    let (u, v) = (w(id.x() as f64 * cells + fi), w(id.y() as f64 * cells + fj));
    let c = match id.face() {
        0 => [1.0, u, v],
        1 => [-1.0, -u, v],
        2 => [-u, 1.0, v],
        3 => [u, -1.0, v],
        4 => [-v, u, 1.0],
        _ => [v, u, -1.0],
    };
    DVec3::from_array(c).normalize()
}

#[test]
fn height_matches_page_data_to_1mm() {
    let mut rng = Rng(0x5eed_0004);
    let p = planet();
    let body = Body::earth();
    let mut worst: f64 = 0.0;
    for _ in 0..200 {
        let level = (rng.u64() % 18) as u8;
        let dir = rng.unit();
        let leaf = NodeId::from_unit(body.id, dir, level);
        let mut res = residency();
        // Ancestors get flat pages at a recognisable height; the leaf gets rough data.
        let mut a = leaf;
        while let Some(par) = a.parent() {
            res.insert(par, Some(HeightPage::flat(-5000.0 + par.level() as f32)));
            a = par;
        }
        let seed = rng.u64();
        let page = HeightPage::from_fn(|i, j| ((hash(leaf, seed ^ (i * 1000 + j) as u64) % 900_000) as f64) / 100.0 - 400.0);
        res.insert(leaf, Some(page.clone()));
        // Grid points strictly inside the node.
        for _ in 0..50 {
            let (i, j) = (1 + (rng.u64() % 255) as u32, 1 + (rng.u64() % 255) as u32);
            let pos = body.point(leaf.grid_point(i, j, 256), rng.range(-100.0, 10_000.0));
            let (h, l) = p.height(&res, body.id, pos).expect("resident");
            assert_eq!(l, level, "finest resident level");
            let err = (h - page.get(i as usize, j as usize)).abs();
            worst = worst.max(err);
            assert!(err < 1e-3, "{leaf} ({i},{j}) {h} vs {}", page.get(i as usize, j as usize));
        }
        // Arbitrary interior points against the page's own triangle rule.
        for _ in 0..50 {
            let (fi, fj) = (rng.range(0.01, 255.99), rng.range(0.01, 255.99));
            let pos = body.point(dir_at(leaf, fi, fj, 256.0), 50.0);
            let (h, _) = p.height(&res, body.id, pos).unwrap();
            let want = page.sample(fi / 256.0, fj / 256.0);
            let err = (h - want).abs();
            worst = worst.max(err);
            assert!(err < 1e-3, "{leaf} ({fi},{fj}) {h} vs {want}");
        }
        // Without the leaf, the parent answers.
        if level > 0 {
            res.remove(leaf);
            let (h, l) = p.height(&res, body.id, body.point(dir_at(leaf, 128.5, 77.25, 256.0), 0.0)).unwrap();
            assert_eq!(l, level - 1);
            assert!((h - (-5000.0 + (level - 1) as f64)).abs() < 1e-3);
        }
    }
    println!("height: worst error {worst:.3e} m");
    assert!(p.height(&FakeResidency::default(), earth(), DVec3::X).is_none());
}

#[test]
fn lowering_budget_lowers_detail() {
    let mut rng = Rng(0x5eed_0005);
    let res = residency();
    for _ in 0..300 {
        let view = random_view(&mut rng);
        let mut prev = u64::MAX;
        for mb in [400u64, 100, 25, 6, 1] {
            let cfg = SelectConfig { budget_bytes: roots_bytes() + (mb << 20), ..Default::default() };
            let s = planet().select(&view, &poses(), &res, &cfg);
            assert!(s.wanted_bytes <= cfg.budget_bytes);
            assert!(s.wanted_bytes <= prev, "lower budget gave more detail: {} > {prev}", s.wanted_bytes);
            prev = s.wanted_bytes;
        }
    }
}

/// Selector time at about 600 draw nodes (P2 §5: 0.5 ms). Run with
/// `cargo test --release -p ed_planet --test properties selector_timing -- --nocapture`.
#[test]
fn selector_timing() {
    let mut res = residency();
    res.complete_fn = Some(Box::new(|_| true));
    let mut p = planet();
    // 2 km over the Alps looking toward the horizon, 1440p, 60 m/s.
    let up = DVec3::new(0.67, 0.12, 0.73).normalize();
    let pos = up * (Body::earth().radius(up) + 2000.0);
    let north = up.cross(DVec3::Z.cross(up).normalize());
    let look = (north - up * 0.15).normalize();
    let right = look.cross(up).normalize();
    let rotation = DQuat::from_mat3(&glam::DMat3::from_cols(right, right.cross(look), -look));
    let mut view = View {
        position: pos,
        rotation,
        velocity: north * 60.0,
        fov_y: 60f64.to_radians(),
        viewport: [2560, 1440],
        look_ahead_s: 1.0,
    };
    // Tune the pixel error so the draw set is about 600 nodes.
    let mut cfg = SelectConfig { budget_bytes: 4 << 30, ..Default::default() };
    let mut draw = p.select(&view, &poses(), &res, &cfg).draw.len();
    println!("selector: {draw} draw nodes at the default {} px error", cfg.pixel_error);
    for _ in 0..40 {
        draw = p.select(&view, &poses(), &res, &cfg).draw.len();
        if (550..=650).contains(&draw) {
            break;
        }
        cfg.pixel_error *= (draw as f64 / 600.0).powf(0.7);
    }
    let frames = 200;
    let t = Instant::now();
    let mut n = 0;
    for i in 0..frames {
        view.position += north * 1.0 * (i % 7) as f64;
        n += p.select(&view, &poses(), &res, &cfg).draw.len();
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    println!("selector: {ms:.3} ms/frame at {} draw nodes (tuned {draw}), pixel error {:.2}", n / frames, cfg.pixel_error);
    if !cfg!(debug_assertions) {
        assert!(ms <= 0.5, "selector {ms:.3} ms over the 0.5 ms budget");
    }
}
