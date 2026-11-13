//! The character (M4b): an `ed_sim` capsule mover driven by input at the fixed step, statics
//! from pack collision ([`crate::collide`]), render interpolation, and a third-person follow
//! camera that ray-casts against the same world so it does not go through walls.
//!
//! Determinism: the world is a pure function of the pack collision, the authored placements and
//! the per-tick input. Commands carry a per-mover sequence number and are only sent when the
//! held input changes; `utc` is derived from the tick. Wall-clock reads only time the step.

use crate::collide::{self, PackCollision, enu_to_yup, yup_to_enu};
use crate::fixed_step::Simulation;
use ed_assets::proving::ORIGIN;
use ed_geo::ObjectId;
use ed_pack::AssetId;
use ed_pack::kinds::CollisionShape;
use ed_render::{Camera, OverlayVertex};
use ed_sim::{Command, CommandKind, DebugDraw, Entity, MoverDesc, MoverInput, QueryFilter, Recorder, SimConfig, SimInput, World, layers};
use glam::{Affine3A, DVec3, Quat, Vec2};
use std::collections::BTreeMap;
use std::time::Instant;

/// Renderer object id of the character (not authored, not generated: never editable).
pub const CHARACTER: ObjectId = ObjectId(0x4000_0000_0000_0000_0000_0000_0000_c47a);
/// Character skinned mesh / prefab, and the placeholder when its pack is absent.
pub const CHARACTER_ASSET: &str = "characters/solarcat";
pub const PLACEHOLDER_ASSET: &str = "proving/parts/capsule";

pub const WALK_MPS: f64 = 4.0;
pub const SPRINT_MPS: f64 = 8.0;
pub const JUMP_MPS: f32 = 5.0;
/// Follow camera: distance behind and height of the look target above the feet.
pub const FOLLOW_DIST: f64 = 4.5;
pub const FOLLOW_HEAD: f64 = 1.6;
/// Simulated UTC at tick 0 (2026-06-21 12:00 UTC); the sim never reads the wall clock.
pub const UTC0: f64 = 1_782_043_200.0;

/// Character input for one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GameInput {
    /// x: right, y: forward; each in -1..1.
    pub axes: Vec2,
    pub jump: bool,
    pub sprint: bool,
    /// Heading the axes are relative to (camera yaw, radians, renderer convention).
    pub yaw: f32,
}

/// The ENU velocity for `input` (z = 0).
pub fn input_velocity(i: &GameInput) -> DVec3 {
    input_velocity_at(i, WALK_MPS, SPRINT_MPS)
}

/// The ENU velocity for `input` at the given walk and sprint speeds (z = 0).
pub fn input_velocity_at(i: &GameInput, walk: f64, sprint: f64) -> DVec3 {
    let cam = Camera { position: DVec3::ZERO, yaw: i.yaw, pitch: 0.0, fov_y: 1.0, near: 0.1 };
    let f = yup_to_enu(cam.forward().as_dvec3());
    let r = yup_to_enu(cam.right().as_dvec3());
    let a = i.axes.as_dvec2();
    let dir = r * a.x + f * a.y;
    let len = dir.length();
    if len < 1e-9 {
        return DVec3::ZERO;
    }
    dir / len.max(1.0) * if i.sprint { sprint } else { walk }
}

/// One authored object's collision: its body and its placed shapes (node frame).
struct ObjectBody {
    body: Option<Entity>,
    shapes: Vec<CollisionShape>,
}

pub struct Game {
    pub world: World,
    statics: Vec<Entity>,
    node_shapes: Vec<CollisionShape>,
    assets: BTreeMap<AssetId, Vec<CollisionShape>>,
    objects: BTreeMap<ObjectId, ObjectBody>,
    pub mover: Entity,
    pub desc: MoverDesc,
    seq: u32,
    held: Option<MoverInput>,
    /// Feet, node frame, before and after the last step.
    pub prev: DVec3,
    pub cur: DVec3,
    /// Render-only facing (radians about y-up; 0 faces +z).
    pub heading: f32,
    /// Wall time of each `World::step`, ms (perf only).
    pub step_ms: Vec<f64>,
    pub recorder: Option<Recorder>,
    /// Statics creation time, ms.
    pub build_ms: f64,
    /// Walk and sprint speeds, m/s (default [`WALK_MPS`], [`SPRINT_MPS`]).
    pub speeds: (f64, f64),
    /// Horizontal acceleration toward the wanted velocity, m/s^2 (`None`: instant, the
    /// proving-ground behaviour). Ramped in the fixed step, so replays stay deterministic.
    pub accel: Option<f64>,
    /// The velocity the ramp has reached (ENU, z = 0).
    ramp: DVec3,
}

/// A placement's node-frame affine (y-up, relative to the proving origin) as the pack ENU
/// transform `collision::place` takes.
pub fn place_shapes(shapes: &[CollisionShape], node_affine: &Affine3A) -> Vec<CollisionShape> {
    let t = ed_assets::proving::to_enu(node_affine);
    shapes.iter().map(|s| ed_assets::collision::place(s, &t)).collect()
}

impl Game {
    /// Builds the world from pack collision, with the mover's feet at `spawn` (node frame ENU).
    pub fn new(col: &PackCollision, spawn: DVec3) -> Result<Game, String> {
        Self::with_config(&SimConfig { origin_ecef: ORIGIN, ..Default::default() }, col, spawn)
    }

    /// [`Game::new`] in any region (`cfg.origin_ecef`): the real-terrain play mode passes its
    /// anchor and no pack collision, and adds the terrain bodies itself.
    pub fn with_config(cfg: &SimConfig, col: &PackCollision, spawn: DVec3) -> Result<Game, String> {
        let t0 = Instant::now();
        let mut world = World::new(cfg);
        let statics = collide::build_statics(&mut world, &col.node)?;
        let desc = MoverDesc { position: spawn, ..Default::default() };
        let mover = world.create_mover(desc).map_err(|e| format!("mover: {e}"))?;
        Ok(Game {
            world,
            statics,
            node_shapes: col.node.clone(),
            assets: col.assets.clone(),
            objects: BTreeMap::new(),
            mover,
            desc,
            seq: 0,
            held: None,
            prev: spawn,
            cur: spawn,
            heading: 0.0,
            step_ms: Vec::new(),
            recorder: None,
            build_ms: t0.elapsed().as_secs_f64() * 1e3,
            speeds: (WALK_MPS, SPRINT_MPS),
            accel: None,
            ramp: DVec3::ZERO,
        })
    }

    /// Rebuilds the world for new pack collision (hot reload), keeping the authored objects and
    /// the mover where it is.
    pub fn rebuild(&mut self, col: &PackCollision, authored: &[(ObjectId, AssetId, Affine3A)]) -> Result<(), String> {
        let mut g = Game::new(col, self.cur)?;
        g.heading = self.heading;
        g.step_ms = std::mem::take(&mut self.step_ms);
        for (id, a, x) in authored {
            g.set_object(*id, *a, x)?;
        }
        *self = g;
        Ok(())
    }

    pub fn static_body_count(&self) -> usize {
        self.statics.len()
    }

    /// Places (or moves) an authored object's collision: its asset's shapes under `node_affine`.
    pub fn set_object(&mut self, id: ObjectId, asset: AssetId, node_affine: &Affine3A) -> Result<(), String> {
        self.remove_object(id);
        let shapes = self.assets.get(&asset).map(|s| place_shapes(s, node_affine)).unwrap_or_default();
        let body = collide::object_body(&mut self.world, &shapes)?;
        self.objects.insert(id, ObjectBody { body, shapes });
        Ok(())
    }

    pub fn remove_object(&mut self, id: ObjectId) {
        if let Some(o) = self.objects.remove(&id)
            && let Some(b) = o.body
        {
            self.world.despawn(b);
        }
    }

    pub fn object_shapes(&self, id: ObjectId) -> Option<&[CollisionShape]> {
        self.objects.get(&id).map(|o| o.shapes.as_slice())
    }

    /// Every collision shape in the world (node + authored), node frame.
    pub fn all_shapes(&self) -> Vec<CollisionShape> {
        let mut v = self.node_shapes.clone();
        for o in self.objects.values() {
            v.extend(o.shapes.iter().cloned());
        }
        v
    }

    pub fn node_shapes(&self) -> &[CollisionShape] {
        &self.node_shapes
    }

    /// Interpolated feet (node frame ENU).
    pub fn feet(&self, alpha: f32) -> DVec3 {
        self.prev.lerp(self.cur, alpha as f64)
    }

    pub fn grounded(&self) -> bool {
        self.world.mover_state(self.mover).is_some_and(|s| s.grounded)
    }

    /// Teleports the mover (a command, so it is part of the replay).
    pub fn teleport(&mut self, feet: DVec3) {
        let c = Command { entity: self.mover, sequence: self.next_seq(), kind: CommandKind::Teleport { position: feet, rotation: glam::DQuat::IDENTITY } };
        self.run(&[c]);
        self.prev = self.cur;
    }

    /// Takes the character's mover out of the world (getting into a vehicle). Its last feet
    /// stay in `cur`; [`Game::step_world`] steps the world without it.
    pub fn park_mover(&mut self) {
        self.world.despawn(self.mover);
        self.held = None;
        self.ramp = DVec3::ZERO;
    }

    /// Puts the character's mover back with its feet at `feet` (getting out).
    pub fn unpark_mover(&mut self, feet: DVec3) -> Result<(), String> {
        let desc = MoverDesc { position: feet, ..self.desc };
        self.mover = self.world.create_mover(desc).map_err(|e| format!("mover: {e}"))?;
        self.prev = feet;
        self.cur = feet;
        self.held = None;
        self.ramp = DVec3::ZERO;
        Ok(())
    }

    /// One tick with no character input (the mover parked or idle).
    pub fn step_world(&mut self) {
        self.run(&[]);
    }

    fn next_seq(&mut self) -> u32 {
        self.seq += 1;
        self.seq
    }

    fn run(&mut self, commands: &[Command]) {
        let tick = self.world.tick();
        let input = SimInput { tick, commands, utc: UTC0 + tick as f64 * ed_sim::DT };
        let t0 = Instant::now();
        self.world.step(&input);
        self.step_ms.push(t0.elapsed().as_secs_f64() * 1e3);
        if let Some(r) = &mut self.recorder {
            r.record(&input, self.world.state_hash());
        }
        self.prev = self.cur;
        if let Some(s) = self.world.mover_state(self.mover) {
            self.cur = s.position;
            let v = DVec3::new(s.velocity.x, s.velocity.y, 0.0);
            if v.length() > 0.2 {
                let d = enu_to_yup(v);
                self.heading = d.x.atan2(d.z) as f32;
            }
        }
    }

    /// The character's renderer transform (node frame, y-up).
    pub fn character_affine(&self, alpha: f32) -> Affine3A {
        Affine3A::from_rotation_translation(Quat::from_rotation_y(self.heading), enu_to_yup(self.feet(alpha)).as_vec3())
    }

    /// Third-person camera behind the character, pulled in front of whatever is between it and
    /// the character's head.
    pub fn follow_camera(&self, alpha: f32, yaw: f32, pitch: f32) -> Camera {
        let head = self.feet(alpha) + DVec3::Z * FOLLOW_HEAD;
        let look = Camera { position: DVec3::ZERO, yaw, pitch, fov_y: Camera::DEFAULT_FOV_Y, near: Camera::DEFAULT_NEAR };
        let back = -yup_to_enu(look.forward().as_dvec3()) * FOLLOW_DIST;
        let filter = QueryFilter { category: layers::DEFAULT, mask: layers::ALL & !layers::MOVER };
        let dist = match self.world.cast_ray(head, back, filter) {
            Some(h) => (h.fraction as f64 * FOLLOW_DIST - 0.25).max(0.3),
            None => FOLLOW_DIST,
        };
        let eye = head + back.normalize() * dist;
        Camera { position: ORIGIN + enu_to_yup(eye), ..look }
    }

    /// Collision debug draw near `around` (node frame), camera-relative to `cam` (world y-up),
    /// at most `max_lines` lines.
    pub fn debug_lines(&self, around: DVec3, radius: f64, cam: DVec3, max_lines: usize) -> Vec<OverlayVertex> {
        let dd = DebugDraw::from_world(&self.world);
        let off = ORIGIN - cam;
        let mut out = Vec::new();
        for ([a, b], c) in &dd.lines {
            if out.len() / 2 >= max_lines {
                break;
            }
            if (*a - around).length() > radius && (*b - around).length() > radius {
                continue;
            }
            for p in [a, b] {
                out.push(OverlayVertex { pos: (enu_to_yup(*p) + off).as_vec3().to_array(), color: c.0 });
            }
        }
        out
    }

    /// (mean, p99, max) of the step times, ms.
    pub fn step_stats(&self) -> (f64, f64, f64) {
        if self.step_ms.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let mut v = self.step_ms.clone();
        v.sort_by(f64::total_cmp);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        (mean, v[((v.len() - 1) as f64 * 0.99).round() as usize], v[v.len() - 1])
    }
}

impl Simulation for Game {
    type Input = GameInput;
    fn step(&mut self, input: &GameInput, _dt: f64) {
        let mut v = input_velocity_at(input, self.speeds.0, self.speeds.1);
        if let Some(a) = self.accel {
            let d = v - self.ramp;
            let max = a * ed_sim::DT;
            self.ramp += if d.length() > max { d * (max / d.length()) } else { d };
            v = self.ramp;
        }
        let want = MoverInput { velocity: v, jump_speed: if input.jump { JUMP_MPS } else { 0.0 } };
        let mut cmds = Vec::new();
        if self.held != Some(want) {
            self.held = Some(want);
            cmds.push(Command { entity: self.mover, sequence: self.next_seq(), kind: CommandKind::MoverInput(want) });
        }
        self.run(&cmds);
    }
}
