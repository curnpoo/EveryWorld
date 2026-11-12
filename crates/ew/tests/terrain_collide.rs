//! P-slice S3: the `ed_sim` character mover walks on the surface the renderer draws.
//! Synthetic slope (always) and real Zermatt data (`--ignored`, needs the disk cache).

use ed_geo::NodeId;
use ed_planet::collide::node_surface;
use ed_planet::{Anchor, Body, FakeResidency, HeightPage, Residency};
use ed_sim::{Command, CommandKind, Entity, MoverDesc, MoverInput, SimConfig, SimInput, World};
use ew::terrain_collide::{COLLISION_LEVEL, TerrainCollision};
use glam::DVec3;
use std::collections::BTreeMap;

const SPEED: f64 = 2.0;
/// The mover's ground ray (ed_sim mover.rs) can miss on locally steeper triangles and snap the
/// feet to the sphere contact, up to ~0.17 m high; heights are held to 5 cm on slopes up to this.
const GENTLE_DEG: f64 = 30.0;

struct Walk {
    lo: f64,
    hi_gentle: f64,
}

struct Surfaces {
    cache: BTreeMap<NodeId, (DVec3, Vec<glam::Vec3>, Vec<[u32; 3]>)>,
}

impl Surfaces {
    /// Surface height at sim (x, y): barycentric on the render triangle containing it.
    fn height(&mut self, anchor: &Anchor, res: &dyn Residency, id: NodeId, x: f64, y: f64) -> Option<f64> {
        let s = self.cache.entry(id).or_insert_with(|| node_surface(anchor, id, &res.height_page(id).unwrap(), 1));
        let (o, v, t) = (s.0, &s.1, &s.2);
        for tr in t {
            let p: Vec<DVec3> = tr.iter().map(|&k| o + v[k as usize].as_dvec3()).collect();
            let d = (p[1].y - p[2].y) * (p[0].x - p[2].x) + (p[2].x - p[1].x) * (p[0].y - p[2].y);
            let a = ((p[1].y - p[2].y) * (x - p[2].x) + (p[2].x - p[1].x) * (y - p[2].y)) / d;
            let b = ((p[2].y - p[0].y) * (x - p[2].x) + (p[0].x - p[2].x) * (y - p[2].y)) / d;
            let c = 1.0 - a - b;
            if a >= -1e-9 && b >= -1e-9 && c >= -1e-9 {
                return Some(a * p[0].z + b * p[1].z + c * p[2].z);
            }
        }
        None
    }

    fn at(&mut self, anchor: &Anchor, res: &dyn Residency, x: f64, y: f64) -> f64 {
        let dir = anchor.sim_to_body(DVec3::new(x, y, 0.0)).normalize();
        let id = NodeId::from_unit(anchor.body.id, dir, COLLISION_LEVEL);
        // The node under (x, y) may differ slightly from the one found by the ellipsoid direction
        // (heights shift the xy); try it and its neighbours.
        for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
            let n = id.neighbour(dx, dy);
            if res.height_page(n).is_some() {
                if let Some(h) = self.height(anchor, res, n, x, y) {
                    return h;
                }
            }
        }
        panic!("no surface under ({x}, {y})");
    }
}

fn step(world: &mut World, cmds: &[Command]) {
    let tick = world.tick();
    world.step(&SimInput { tick, commands: cmds, utc: 0.0 });
}

fn input(e: Entity, seq: u32, v: DVec3) -> Command {
    Command { entity: e, sequence: seq, kind: CommandKind::MoverInput(MoverInput { velocity: v, jump_speed: 0.0 }) }
}

/// Drops a mover at (0,0), settles, walks `dist` m along `dir` and back. Returns (min, max) of
/// feet minus surface over the walk, and the settle error.
fn stand_and_walk(anchor: &Anchor, res: &dyn Residency, from: (f64, f64), dir: DVec3, dist: f64) -> Walk {
    let mut surf = Surfaces { cache: BTreeMap::new() };
    let mut world = World::new(&SimConfig::default());
    let mut tc = TerrainCollision::new();
    let h0 = surf.at(anchor, res, from.0, from.1);
    let start = DVec3::new(from.0, from.1, h0 + 3.0);
    tc.update(&mut world, anchor, res, start);
    assert!(tc.nodes().len() >= 4, "bodies: {:?}", tc.nodes());
    let e = world.create_mover(MoverDesc { position: start, ..Default::default() }).unwrap();
    for _ in 0..180 {
        step(&mut world, &[]);
    }
    let s = world.mover_state(e).unwrap();
    assert!(s.grounded, "did not land: {s:?}");
    let settle = s.position.z - surf.at(anchor, res, s.position.x, s.position.y);
    assert!(settle.abs() < 0.03, "settled {settle} m off the surface");

    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let (mut hi_gentle, mut worst_slope) = (f64::MIN, 0.0f64);
    let steps = (dist / SPEED * 60.0) as usize;
    let mut seq = 0;
    for leg in [1.0, -1.0] {
        seq += 1;
        step(&mut world, &[input(e, seq, dir * (SPEED * leg))]);
        for k in 0..(steps * if leg < 0.0 { 2 } else { 1 }) {
            let p = world.mover_state(e).unwrap().position;
            tc.update(&mut world, anchor, res, p);
            step(&mut world, &[]);
            let p = world.mover_state(e).unwrap().position;
            let d = p.z - surf.at(anchor, res, p.x, p.y);
            if k > 30 {
                // Steepest slope within ~2.5 m of the feet.
                let mut slope = 0.0f64;
                for (ox, oy) in [(0.0, 0.0), (2.5, 0.0), (-2.5, 0.0), (0.0, 2.5), (0.0, -2.5)] {
                    let (x, y) = (p.x + ox, p.y + oy);
                    let gx = (surf.at(anchor, res, x + 1.0, y) - surf.at(anchor, res, x - 1.0, y)) / 2.0;
                    let gy = (surf.at(anchor, res, x, y + 1.0) - surf.at(anchor, res, x, y - 1.0)) / 2.0;
                    slope = slope.max(gx.hypot(gy).atan().to_degrees());
                }
                lo = lo.min(d);
                hi = hi.max(d);
                if slope <= GENTLE_DEG {
                    hi_gentle = hi_gentle.max(d);
                }
                if d > 0.05 {
                    worst_slope = worst_slope.max(slope);
                }
            }
        }
    }
    seq += 1;
    step(&mut world, &[input(e, seq, DVec3::ZERO)]);
    let p = world.mover_state(e).unwrap().position;
    println!(
        "walked to ({:.1}, {:.1}); feet - surface: min {lo:.4} m, max {hi:.4} m (max on slopes <= {GENTLE_DEG} deg {hi_gentle:.4} m; floats > 5 cm only on slopes >= {worst_slope:.1} deg), settle {settle:.4} m; bodies {}",
        p.x,
        p.y,
        tc.nodes().len()
    );
    Walk { lo, hi_gentle }
}

/// The disk cache only holds elevation up to z14 around Zermatt: z15 requests are served by
/// bilinear upsampling of the z14 parent (what `elevation::fetch` does above its max zoom).
struct Upsampled<'a> {
    inner: ed_data::node::ElevationSource<'a>,
}

impl ed_data::node::SourceView for Upsampled<'_> {
    fn elevation_tile(&self, z: u8, x: u32, y: u32) -> Option<std::sync::Arc<ed_data::node::Raster>> {
        if z < 15 {
            return self.inner.elevation_tile(z, x, y);
        }
        let parent = self.inner.elevation_tile(z - 1, x / 2, y / 2)?;
        let n = parent.size;
        let at = |px: i64, py: i64| parent.heights[(py.clamp(0, n as i64 - 1) as usize) * n + px.clamp(0, n as i64 - 1) as usize] as f64;
        let mut heights = Vec::with_capacity(n * n);
        for py in 0..n {
            for px in 0..n {
                let fx = (((x & 1) as usize * n + px) as f64 + 0.5) / 2.0 - 0.5;
                let fy = (((y & 1) as usize * n + py) as f64 + 0.5) / 2.0 - 0.5;
                let (x0, y0) = (fx.floor(), fy.floor());
                let (tx, ty) = (fx - x0, fy - y0);
                let (x0, y0) = (x0 as i64, y0 as i64);
                let a = at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx;
                let b = at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx;
                heights.push((a * (1.0 - ty) + b * ty) as _);
            }
        }
        Some(std::sync::Arc::new(ed_data::node::Raster { size: n, heights }))
    }
}

fn anchor_zermatt() -> Anchor {
    Anchor::from_ecef(Body::earth(), ed_geo::Geodetic::new(46.0207, 7.7491, 0.0).to_ecef())
}

/// 5 x 5 nodes around the anchor at the collision level, pages from `page_of`.
fn residency(anchor: &Anchor, mut page_of: impl FnMut(NodeId) -> HeightPage) -> FakeResidency {
    let mut r = FakeResidency::default();
    let c = NodeId::from_unit(anchor.body.id, anchor.dir, COLLISION_LEVEL);
    for dy in -2..=2 {
        for dx in -2..=2 {
            let n = c.neighbour(dx, dy);
            r.insert(n, Some(page_of(n)));
        }
    }
    r
}

#[test]
fn walks_on_a_synthetic_slope_without_sinking_or_floating() {
    let anchor = anchor_zermatt();
    let tan = 25f64.to_radians().tan();
    let field = |x: f64, y: f64| tan * x + 1.0 * (-((x - 20.0).powi(2) + (y - 2.0).powi(2)) / 50.0).exp();
    let res = residency(&anchor, |id| {
        HeightPage::from_fn(|i, j| {
            let b = anchor.body.point(id.grid_point(i as u32, j as u32, 256), 0.0);
            let s = anchor.body_to_sim(b);
            1500.0 + field(s.x, s.y)
        })
    });
    let w = stand_and_walk(&anchor, &res, (0.0, 0.0), DVec3::X, 60.0);
    assert!(w.lo > -0.05, "sank {}", w.lo);
    assert!(w.hi_gentle < 0.05, "floated {}", w.hi_gentle);
}

#[test]
#[ignore = "needs the elevation disk cache (EARTHDROP_CACHE)"]
fn walks_on_real_zermatt_terrain() {
    use ed_data::node::node_heights;
    use ed_data::node::ElevationSource;
    let root = std::env::var("EARTHDROP_CACHE").unwrap_or_else(|_| "/Users/curren/Documents/projects/EveryWorld/.earthdrop-cache".into());
    ed_data::http::set_offline(true);
    let cache = ed_data::DiskCache::new(root);
    let src = Upsampled { inner: ElevationSource { cache: &cache } };
    let anchor = anchor_zermatt();
    let c = NodeId::from_unit(anchor.body.id, anchor.dir, COLLISION_LEVEL);
    let probe = node_heights(&src, c);
    if probe.max_m - probe.min_m < 1.0 {
        println!("SKIP: no cached elevation tiles under Zermatt");
        return;
    }
    let res = residency(&anchor, |n| node_heights(&src, n));
    // A real slope near the anchor: the steepest gradient under 32 degrees on a 10 m scan,
    // then walk uphill along it.
    let mut surf = Surfaces { cache: BTreeMap::new() };
    let e = 3.0;
    let grad = |surf: &mut Surfaces, x: f64, y: f64| {
        let gx = (surf.at(&anchor, &res, x + e, y) - surf.at(&anchor, &res, x - e, y)) / (2.0 * e);
        let gy = (surf.at(&anchor, &res, x, y + e) - surf.at(&anchor, &res, x, y - e)) / (2.0 * e);
        (gx, gy)
    };
    let mut best = (0.0, (0.0, 0.0), (0.0, 0.0));
    for iy in -12..=12 {
        for ix in -12..=12 {
            let (x, y) = (ix as f64 * 10.0, iy as f64 * 10.0);
            let (gx, gy) = grad(&mut surf, x, y);
            let slope = gx.hypot(gy).atan().to_degrees();
            if slope < 32.0 && slope > best.0 {
                best = (slope, (x, y), (gx, gy));
            }
        }
    }
    let (slope, from, (gx, gy)) = best;
    println!("start ({:.0}, {:.0}), slope {slope:.1} deg", from.0, from.1);
    let dir = DVec3::new(gx, gy, 0.0).try_normalize().unwrap_or(DVec3::X);
    let w = stand_and_walk(&anchor, &res, from, dir, 60.0);
    assert!(w.lo > -0.05, "sank {}", w.lo);
    assert!(w.hi_gentle < 0.05, "floated {}", w.hi_gentle);
}
