//! Scripted character route (M4b, no GPU): from the spawn marker run into the nearest
//! building, then to the authored barrel and jump onto it. Every tick checks the capsule
//! against the pack collision analytically ([`crate::collide::capsule_penetration`]); the run
//! is recorded and replayed, and its hashes printed so two processes can be compared
//! (`ew --sim-route`).

use crate::authored::Authored;
use crate::collide::{PackCollision, capsule_penetration};
use crate::fixed_step::Simulation;
use crate::game::{Game, GameInput};
use ed_assets::collision::{centre, surface};
use ed_geo::ObjectId;
use ed_sim::{Recorder, Replay};
use glam::{DVec3, Vec2};
use std::path::Path;

/// Allowed overlap (the mover keeps a small skin; Box3D's slop is ~5 mm).
pub const PENETRATION_TOL: f32 = 0.02;
/// The barrel placed in `assets-src/layers` (route target: 0.9 m tall, so a jump clears it).
pub const CRATE: ObjectId = ObjectId(0x8000_0000_0000_0000_0000_0000_0000_0005);

#[derive(Clone, Debug)]
pub struct RouteReport {
    pub ticks: u64,
    pub static_bodies: usize,
    pub build_ms: f64,
    pub max_penetration: f32,
    pub worst_tick: u64,
    pub lowest_feet: f64,
    /// Feet after the wall leg, distance still to the building's centre line, the surface hit.
    pub wall_stop: DVec3,
    pub wall_target: DVec3,
    pub wall_gap: f32,
    pub wall_surface: u32,
    /// Feet after the crate leg; whether the character stood on the crate at some tick.
    pub crate_stop: DVec3,
    pub stood_on_crate: bool,
    pub final_hash: u64,
    /// BLAKE3 of the recorded per-tick state hashes.
    pub replay_blake3: String,
    pub replay_ok: bool,
    pub step_mean_ms: f64,
    pub step_p99_ms: f64,
}

impl RouteReport {
    pub fn lines(&self) -> String {
        format!(
            "route: {} ticks, {} static bodies (built in {:.0} ms)\n\
             route: wall leg stopped at ({:.2}, {:.2}, {:.2}) heading for the building centred at ({:.1}, {:.1}); clearance {:.3} m to a shape of surface {} (2 = building)\n\
             route: prop leg ended at ({:.2}, {:.2}, {:.2}), stood on the barrel: {}\n\
             route: max penetration {:.4} m (tick {}), lowest feet z {:.3}\n\
             route: sim step mean {:.3} ms, p99 {:.3} ms\n\
             route: replay {} ({} ticks)\n\
             final_hash: {:016x}\nreplay_blake3: {}\n",
            self.ticks,
            self.static_bodies,
            self.build_ms,
            self.wall_stop.x,
            self.wall_stop.y,
            self.wall_stop.z,
            self.wall_target.x,
            self.wall_target.y,
            self.wall_gap,
            self.wall_surface,
            self.crate_stop.x,
            self.crate_stop.y,
            self.crate_stop.z,
            self.stood_on_crate,
            self.max_penetration,
            self.worst_tick,
            self.lowest_feet,
            self.step_mean_ms,
            self.step_p99_ms,
            if self.replay_ok { "re-ran identically" } else { "DIVERGED" },
            self.ticks,
            self.final_hash,
            self.replay_blake3
        )
    }
}

/// Camera yaw whose forward is the ENU direction `d` (renderer convention: 0 looks north).
pub fn yaw_towards(d: DVec3) -> f32 {
    (-d.x).atan2(d.y) as f32
}

pub fn run(packs: &Path, layers: &Path) -> Result<RouteReport, String> {
    let col = PackCollision::from_dir(packs)?;
    let auth = Authored::load(layers)?;
    let mut g = Game::new(&col, auth.spawn_feet())?;
    for (id, a, x) in auth.list() {
        g.set_object(id, a, &x)?;
    }
    let spawn = auth.spawn_feet();
    // Shapes near the route (the analytic check walks these every tick).
    let near: Vec<_> = g.all_shapes().into_iter().filter(|s| (centre(&s.shape).as_dvec3() - spawn).truncate().length() < 120.0 || s.surface == surface::GROUND).collect();
    // The nearest building.
    let wall = near
        .iter()
        .filter(|s| s.surface == surface::BUILDING)
        .map(|s| centre(&s.shape).as_dvec3())
        .min_by(|a, b| (a - spawn).truncate().length().total_cmp(&(b - spawn).truncate().length()))
        .ok_or("no building near the spawn")?;
    let crate_c = g.object_shapes(CRATE).and_then(|s| s.first()).map(|s| centre(&s.shape).as_dvec3()).ok_or("the barrel (layer object 5) has no collision")?;

    g.recorder = Some(Recorder::start(&g.world));
    let (r, h) = (g.desc.radius, g.desc.height);
    let mut rep = RouteReport {
        ticks: 0,
        static_bodies: g.static_body_count(),
        build_ms: g.build_ms,
        max_penetration: 0.0,
        worst_tick: 0,
        lowest_feet: f64::INFINITY,
        wall_stop: DVec3::ZERO,
        wall_target: wall,
        wall_gap: 0.0,
        wall_surface: 0,
        crate_stop: DVec3::ZERO,
        stood_on_crate: false,
        final_hash: 0,
        replay_blake3: String::new(),
        replay_ok: false,
        step_mean_ms: 0.0,
        step_p99_ms: 0.0,
    };
    let crate_top = g.object_shapes(CRATE).map(|s| s.iter().map(|s| crate::collide::top_z(&s.shape)).fold(f32::MIN, f32::max)).unwrap_or(0.0) as f64;
    let crate_shape = g.object_shapes(CRATE).and_then(|s| s.first()).map(|s| s.shape.clone());
    let tick = |g: &mut Game, inp: GameInput, rep: &mut RouteReport| {
        g.step(&inp, ed_sim::DT);
        rep.ticks += 1;
        let (pen, _) = capsule_penetration(g.cur, r, h, &near);
        if pen > rep.max_penetration {
            rep.max_penetration = pen;
            rep.worst_tick = rep.ticks;
        }
        rep.lowest_feet = rep.lowest_feet.min(g.cur.z);
        // On the crate: grounded, feet near its top, and the feet's column over its footprint
        // (within the capsule radius).
        let over = crate_shape.as_ref().and_then(|c| crate::collide::distance(DVec3::new(g.cur.x, g.cur.y, crate_top - 0.01).as_vec3(), c)).is_some_and(|d| d < r);
        if over && g.cur.z > crate_top - r as f64 && g.grounded() {
            rep.stood_on_crate = true;
        }
    };
    // Settle.
    for _ in 0..30 {
        tick(&mut g, GameInput::default(), &mut rep);
    }
    // Leg 1: sprint at the building for 5 s (it stops at the wall or whatever is in front).
    for _ in 0..300 {
        let d = (wall - g.cur).truncate().extend(0.0);
        tick(&mut g, GameInput { axes: Vec2::Y, sprint: true, yaw: yaw_towards(d), jump: false }, &mut rep);
    }
    rep.wall_stop = g.cur;
    // Clearance between the capsule and the nearest non-ground shape, and what that shape is.
    let probe = |feet: DVec3| {
        near.iter()
            .filter(|s| s.surface != surface::GROUND)
            .filter_map(|s| (0..=8).filter_map(|k| crate::collide::distance((feet + DVec3::Z * (r as f64 + k as f64 * 0.15)).as_vec3(), &s.shape)).reduce(f32::min).map(|d| (d - r, s.surface)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap_or((f32::INFINITY, 0))
    };
    (rep.wall_gap, rep.wall_surface) = probe(g.cur);
    // Leg 2: back to the middle of the avenue, then to the crate, jumping when close (10 s).
    let waypoint = DVec3::new(spawn.x, g.cur.y, 0.0);
    for k in 0..600 {
        let goal = if k < 120 { waypoint } else { crate_c };
        let d = (goal - g.cur).truncate().extend(0.0);
        let close = k >= 120 && d.length() < 2.2 && g.cur.z < 0.5;
        let axes = if d.length() < 0.15 { Vec2::ZERO } else { Vec2::Y };
        tick(&mut g, GameInput { axes, sprint: false, yaw: yaw_towards(d), jump: close }, &mut rep);
    }
    rep.crate_stop = g.cur;
    for _ in 0..30 {
        tick(&mut g, GameInput::default(), &mut rep);
    }
    rep.final_hash = g.world.state_hash();
    let bytes = g.recorder.take().expect("recording").finish();
    let replay = Replay::from_bytes(&bytes).map_err(|e| format!("replay: {e}"))?;
    // The recording's per-tick state hashes (the raw bytes also carry the backend's world
    // image, which is not byte-stable across worlds in one process).
    let hashes = replay.hashes();
    rep.replay_blake3 = blake3::hash(&hashes.iter().flat_map(|h| h.to_le_bytes()).collect::<Vec<u8>>()).to_hex().to_string();
    rep.replay_ok = hashes.last() == Some(&rep.final_hash) && matches!(replay.run(), Ok(Ok(ref h)) if *h == hashes);
    let (m, p99, _) = g.step_stats();
    rep.step_mean_ms = m;
    rep.step_p99_ms = p99;
    Ok(rep)
}
