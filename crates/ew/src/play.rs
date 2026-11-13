//! Walk on real terrain (P-slice S4): `ew --play zermatt|blueridge`.
//!
//! A window on the place's [`TerrainWorld`], the proving character (`characters/solarcat`)
//! driven by the `ed_sim` mover on the S3 terrain collision, and a third-person orbit camera.
//!
//! * Pages: built on background threads ([`TerrainWorld::start_loader`]), coarse levels and
//!   nodes near the camera first. The collision 3 x 3 (level 16) around the player is always
//!   exact: a missing one is built on the spot (rare: a 5 x 5 ring is prefetched first), so the
//!   sim's collision depends only on where the player is, never on load timing.
//! * Spawn: blocks until the first view and the collision around the anchor are resident, then
//!   puts the feet on the ground there.
//! * Camera: orbit (mouse), lagged target, pulled in front of terrain between it and the head,
//!   and never below the ground under it.
//! * Gravity is uniform -z in the sim (anchor frame): fine within a few km of the anchor;
//!   re-basing is later work. The log warns past [`FAR_FROM_ANCHOR_M`].
//!
//! Controls: W A S D walk (relative to the camera), Shift run, Space jump, mouse looks, wheel
//! zooms, V vsync, Esc quits. E gets into the placeholder car parked beside the spawn (within
//! [`crate::car::ENTER_RANGE_M`]) and out again; driving: W throttle, S brake then reverse, A D
//! steer, Space handbrake, mouse orbits (the view swings back behind the car), wheel zooms.
//!
//! `EW_AUTOFLY=S`: walks 2 s, holds Space (rocket thrust) to 7 s, skydives, opens the parachute at
//! 9 s and lands; `fly_*.png` shots to `--out`. F wingsuit, P parachute, G nudge (alpha keys).
//! `EW_AUTOWALK=S`: walks forward for S seconds (Shift held after a third of it) turning a little,
//! writes `play_start.png`, `walk_*.png`, `run_*.png` and `play_end.png` (offscreen render of the same view) to `--out`,
//! and exits. For checks without anyone at the keyboard.
//!
//! `EW_AUTODRIVE=S`: gets into the car, drives for S seconds along the flattest way ahead (the
//! valley floor), turns, brakes to a stop, gets out, writing `drive_*.png` to `--out`.

use crate::args::Args;
use crate::car::{AutoPhase, CarSim, CarView, ChaseCam};
use crate::content::{CHARACTER_BAKE, Content, PackSpec};
use crate::fixed_step::{FixedStep, Simulation};
use crate::game::{CHARACTER, CHARACTER_ASSET, Game, GameInput};
use crate::robot::cam::RobotCam;
use crate::robot::controller::Controller;
use crate::robot::rig::{self, Gait, RigInput};
use crate::robot::view::RobotView;
use crate::terrain::{Place, TerrainWorld};
use crate::terrain_collide::{COLLISION_LEVEL, TerrainCollision};
use ed_geo::NodeId;
use ed_gpu::{BufferDesc, BufferId, Device, DeviceDesc, Format, ResourceState, SurfaceConfig, SurfaceId, TextureDesc, TextureId, TextureKind, Usage, WindowHandle};
use ed_pack::AssetId;
use ed_planet::Anchor;
use ed_render::scene::NodeSlot;
use ed_render::{Camera, FrameParams, Renderer, View, ViewTarget, Viewport};
use ed_sim::{QueryFilter, SimConfig, VehicleInput, layers};
use glam::{Affine3A, DVec3, Quat, Vec2};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

/// Walking and running speeds on terrain, m/s: a brisk hike and a run.
pub const WALK_MPS: f64 = 1.9;
pub const RUN_MPS: f64 = 5.5;
/// Horizontal acceleration, m/s^2: full walking speed in about 0.2 s, a stop in about 0.1 s
/// from a walk (feet, not ice).
pub const ACCEL: f64 = 14.0;
/// Camera: distance from the look point, its height above the feet, its side offset (over the
/// right shoulder), zoom limits, metres.
pub const CAM_DIST: f64 = 4.2;
pub const CAM_HEAD: f64 = 1.55;
pub const CAM_SHOULDER: f64 = 0.35;
pub const CAM_MIN: f64 = 1.5;
pub const CAM_MAX: f64 = 14.0;
/// Camera clearance above the ground under it and in front of terrain between it and the head.
pub const CAM_CLEAR: f64 = 0.45;
/// Look point lag time constants, s (horizontal, vertical: steps and bumps are absorbed).
pub const LAG_XY_S: f64 = 0.06;
pub const LAG_Z_S: f64 = 0.16;
/// Mouse look, radians per pixel, and the pitch range (radians; negative looks down).
pub const LOOK_RAD_PER_PX: f32 = 0.0022;
pub const PITCH_MIN: f32 = -1.25;
pub const PITCH_MAX: f32 = 0.7;
/// Character turn rate toward its velocity (rad/s, rendering only).
pub const TURN_RAD_S: f32 = 10.0;
/// Background page builders and how many pages may be queued or building at once.
pub const LOADER_THREADS: usize = 6;
pub const MAX_INFLIGHT: usize = 24;
/// Past this distance from the anchor the uniform gravity and flat sim frame start to show.
pub const FAR_FROM_ANCHOR_M: f64 = 2000.0;

/// Level-16 nodes around sim position `s`: the (2k+1)^2 block, centre first then by ring.
pub fn collision_block(anchor: &Anchor, s: DVec3, k: i32) -> Vec<NodeId> {
    let dir = anchor.sim_to_body(s).normalize();
    let c = NodeId::from_unit(anchor.body.id, dir, COLLISION_LEVEL);
    let mut v: Vec<(i32, NodeId)> = Vec::new();
    for dy in -k..=k {
        for dx in -k..=k {
            v.push((dx.abs().max(dy.abs()), c.neighbour(dx, dy)));
        }
    }
    v.sort();
    v.dedup_by_key(|x| x.1);
    v.into_iter().map(|x| x.1).collect()
}

/// `ids` and all their ancestors (height queries descend from the root).
fn with_ancestors(ids: &[NodeId]) -> Vec<NodeId> {
    let mut s = BTreeSet::new();
    for &id in ids {
        let mut n = Some(id);
        while let Some(i) = n {
            if !s.insert(i) {
                break;
            }
            n = i.parent();
        }
    }
    s.into_iter().collect()
}

/// The third-person orbit camera (sim frame: ENU, z up).
#[derive(Clone, Copy, Debug)]
pub struct OrbitCam {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f64,
    /// Lagged look point (sim frame); `None` until the first frame.
    pub target: Option<DVec3>,
    /// Current (pulled-in) distance, eased back out after an obstruction.
    pub cur_dist: f64,
}

impl OrbitCam {
    pub fn new(yaw: f32) -> OrbitCam {
        OrbitCam { yaw, pitch: -0.2, dist: CAM_DIST, target: None, cur_dist: CAM_DIST }
    }

    pub(crate) fn look(&self) -> Camera {
        Camera { position: DVec3::ZERO, yaw: self.yaw, pitch: self.pitch, fov_y: Camera::DEFAULT_FOV_Y, near: Camera::DEFAULT_NEAR }
    }

    /// Advances the lag toward `feet` (sim frame) and returns the render-frame camera.
    /// `blocked(head, back)` is the fraction of the ray `head -> head + back` that is free;
    /// `ground(p)` the terrain height (sim z) under sim point `p`.
    pub fn update(&mut self, feet: DVec3, dt: f64, blocked: impl Fn(DVec3, DVec3) -> Option<f64>, ground: impl Fn(DVec3) -> Option<f64>) -> Camera {
        let want = feet + DVec3::Z * CAM_HEAD;
        let t = match self.target {
            None => want,
            Some(t) => {
                let kxy = 1.0 - (-dt / LAG_XY_S).exp();
                let kz = 1.0 - (-dt / LAG_Z_S).exp();
                let mut n = t + (want - t) * DVec3::new(kxy, kxy, kz);
                // Never trail more than half a metre (fast falls, teleports).
                let d = n - want;
                if d.length() > 0.5 {
                    n = want + d * (0.5 / d.length());
                }
                n
            }
        };
        self.target = Some(t);
        let look = self.look();
        let fwd = Anchor::render_to_sim(look.forward().as_dvec3());
        let right = Anchor::render_to_sim(look.right().as_dvec3());
        let pivot = t + right * CAM_SHOULDER * (self.dist / CAM_DIST).min(1.0);
        let back = -fwd * self.dist;
        let free = blocked(pivot, back).map_or(self.dist, |f| (f * self.dist - CAM_CLEAR).max(0.4));
        // Pull in at once, ease back out (no pumping on bumpy ground).
        self.cur_dist = if free < self.cur_dist { free } else { self.cur_dist + (free - self.cur_dist) * (1.0 - (-dt / 0.25).exp()) };
        let mut eye = pivot - fwd * self.cur_dist;
        if let Some(g) = ground(eye) {
            eye.z = eye.z.max(g + CAM_CLEAR);
        }
        Camera { position: Anchor::sim_to_render(eye), ..look }
    }
}

/// Character input from held keys, relative to the camera yaw.
fn game_input(keys: &BTreeSet<KeyCode>, yaw: f32) -> GameInput {
    let k = |c| if keys.contains(&c) { 1.0 } else { 0.0 };
    GameInput {
        axes: Vec2::new(k(KeyCode::KeyD) - k(KeyCode::KeyA), k(KeyCode::KeyW) - k(KeyCode::KeyS)),
        jump: keys.contains(&KeyCode::Space),
        sprint: keys.contains(&KeyCode::ShiftLeft) || keys.contains(&KeyCode::ShiftRight),
        yaw,
    }
}

/// The robot character: the floating-capsule controller (`None` while driving), the gait that
/// solves its pose, the drawing, and the camera. The rig runs in the render frame relative to
/// `origin` so its f32 maths stays precise far from the anchor.
struct RobotPlay {
    ctl: Option<Controller>,
    gait: Gait,
    view: RobotView,
    origin: DVec3,
    cam: RobotCam,
    planted: [bool; 2],
    feet_at: [glam::Vec3; 2],
}

/// Rig-local distance from `origin` beyond which the rig is re-based (the gait restarts).
const REBASE_M: f64 = 2000.0;

fn to_render_rot(q: glam::DQuat) -> Quat {
    glam::DQuat::from_xyzw(q.x, q.z, -q.y, q.w).as_quat()
}

/// The play state: terrain, collision, character, camera (no GPU surface).
pub struct Play {
    pub place: &'static Place,
    pub world: TerrainWorld,
    pub collision: TerrainCollision,
    pub game: Game,
    pub cam: OrbitCam,
    pub fixed: FixedStep,
    content: Option<Content>,
    node: Option<NodeSlot>,
    heading: f32,
    /// Procedural gait on the character's skins (`None`: the pack's skeleton is unknown).
    anim: Option<(crate::walker::Walker, Vec<ed_render::scene::SkinId>)>,
    pub sync_loads: u64,
    pub far_warned: bool,
    /// Worst planted-foot miss (m) since the last log, and steps taken.
    pub slip_max: f32,
    /// Near trees (`EW_TREES_NEAR`, default on): built off the main thread, applied under a budget.
    pub near: Option<crate::trees::NearTrees>,
    pub steps: u64,
    /// The car (sim and drawing), whether the character is in it, its camera, a pending E.
    pub car: Option<CarSim>,
    car_view: Option<CarView>,
    pub driving: bool,
    pub chase: ChaseCam,
    want_use: bool,
    robot: Option<RobotPlay>,
}

/// One fixed step of play: the car's forces, then the world (with or without the character).
struct PlaySim<'a> {
    game: &'a mut Game,
    car: Option<&'a mut CarSim>,
    robot: Option<&'a mut Controller>,
    driving: bool,
    drive: VehicleInput,
    focus: DVec3,
}

impl Simulation for PlaySim<'_> {
    type Input = GameInput;
    fn step(&mut self, input: &GameInput, dt: f64) {
        if let Some(car) = self.car.as_deref_mut() {
            let vin = if self.driving { self.drive } else { VehicleInput::default() };
            car.pre_step(&mut self.game.world, &vin, self.focus, self.driving);
        }
        if self.driving {
            self.game.step_world();
        } else if let Some(ctl) = self.robot.as_deref_mut() {
            ctl.pre_step(&mut self.game.world, input, dt);
            self.game.step_world();
            ctl.post_step(&self.game.world, dt);
            self.game.prev = self.game.cur;
            self.game.cur = ctl.feet();
        } else {
            self.game.step(input, dt);
        }
        if let Some(car) = self.car.as_deref_mut() {
            car.post_step(&self.game.world);
        }
    }
}

impl Play {
    /// Loads the spawn area (blocking), places the character on the ground at the anchor.
    pub fn new(r: &mut Renderer, dev: &dyn Device, place: &'static Place, packs: &std::path::Path, viewport: [u32; 2], args_character: crate::args::Character) -> Result<Play, String> {
        let t0 = Instant::now();
        let root = crate::terrain::cache_root();
        let mut world = TerrainWorld::new(&root, place);
        if world.source().zooms().is_empty() {
            return Err(format!("no Mapterhorn tiles under {} (set EARTHDROP_CACHE to an absolute .earthdrop-cache path)", root.display()));
        }
        r.set_terrain(dev, Some(world.provider(dev).map_err(|e| format!("terrain provider: {e}"))?));
        // Spawn: the collision block at the anchor with its ancestors, then the first view.
        let block = collision_block(&world.anchor, DVec3::ZERO, 1);
        world.load(&with_ancestors(&block), usize::MAX);
        let g = world.ground_point(DVec3::ZERO).ok_or("spawn: no ground under the anchor")?;
        let feet = Anchor::render_to_sim(g) + DVec3::Z * 0.05;
        let yaw = crate::terrain::path_pose(place, 0.0).yaw;
        let mut cam = OrbitCam::new(yaw);
        let robot_mode = args_character == crate::args::Character::Robot;
        if robot_mode {
            cam.dist = crate::robot::cam::DIST_DEFAULT;
            cam.cur_dist = cam.dist;
        }
        let first = cam.update(feet, 0.0, |_, _| None, |_| None);
        cam.target = None;
        world.update(&first, viewport, usize::MAX, true);
        let cfg = SimConfig { origin_ecef: world.anchor.origin, ..Default::default() };
        let mut game = Game::with_config(&cfg, &Default::default(), feet)?;
        game.speeds = (WALK_MPS, RUN_MPS);
        game.accel = Some(ACCEL);
        game.heading = yaw + std::f32::consts::PI;
        let mut collision = TerrainCollision::new();
        collision.update(&mut game.world, &world.anchor, world.residency(), feet);
        world.start_loader(LOADER_THREADS);
        // The character: only the character pack (the proving pack would bring the proving
        // ground along). Its renderer node follows the feet; the instance only turns.
        let specs: &[PackSpec] = if robot_mode { &[] } else { &[PackSpec { name: "character", required: false, bake: CHARACTER_BAKE }] };
        let mut content = Content::load_specs(r, dev, packs, specs)?;
        for n in &content.notes {
            eprintln!("ew: {n}");
        }
        let id = AssetId::from_path(CHARACTER_ASSET);
        let node = r.instances().nodes().iter().rposition(|n| *n == ed_assets::proving::ORIGIN).map(|i| NodeSlot(i as u16));
        let content = if robot_mode {
            None
        } else if content.has_asset(id) && node.is_some() {
            content.set_authored(r, dev, &[(CHARACTER, id, Affine3A::IDENTITY, CHARACTER_ASSET.to_string())])?;
            Some(content)
        } else {
            eprintln!("ew: play: no {CHARACTER_ASSET} (character.ewpack in {}); walking without a body. Build it with: {CHARACTER_BAKE}", packs.display());
            None
        };
        // The gait: the character pack's skeleton on the renderer's skins. This renderer is
        // fresh and the character is the only skinned content, so its skins are 0..n.
        let anim = content.as_ref().and_then(|c| {
            let sk = pack_skeletons(&packs.join("character.ewpack")).into_iter().next()?;
            let a = crate::walker::Walker::new(&sk)?;
            let skins: Vec<_> = (0..c.skinned_parts() as u32).map(ed_render::scene::SkinId).collect();
            let pal = a.palette();
            skins.iter().all(|s| r.set_skin_pose(*s, &pal).is_ok()).then_some((a, skins))
        });
        if !robot_mode && content.is_some() && anim.is_none() {
            eprintln!("ew: play: character skeleton not matched; standing in the rest pose");
        }
        // The car: parked beside the spawn (to the right, a little ahead), facing the view.
        let car_at = |off: DVec3| {
            let (s, c) = ((yaw as f64).sin(), (yaw as f64).cos());
            let (fwd, right) = (DVec3::new(-s, c, 0.0), DVec3::new(c, s, 0.0));
            let p = feet + right * off.x + fwd * off.y;
            world.ground_point(Anchor::sim_to_render(p)).map(Anchor::render_to_sim)
        };
        let car = match car_at(DVec3::new(2.8, 1.0, 0.0)) {
            Some(g) => match CarSim::spawn(&mut game.world, g, yaw as f64) {
                Ok(c) => Some(c),
                Err(e) => {
                    eprintln!("ew: play: {e}");
                    None
                }
            },
            None => None,
        };
        let car_view = match &car {
            Some(c) => match CarView::new(r, dev, &c.veh.desc) {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("ew: play: car drawing: {e}");
                    None
                }
            },
            None => None,
        };
        let robot = if robot_mode {
            // The mover is not used: the robot is its own dynamic body.
            game.park_mover();
            let ctl = Controller::spawn(&mut game.world, feet, yaw as f64)?;
            game.prev = feet;
            game.cur = feet;
            let view = RobotView::new(r, dev)?;
            Some(RobotPlay { ctl: Some(ctl), gait: Gait::default(), view, origin: Anchor::sim_to_render(feet), cam: RobotCam::default(), planted: [false; 2], feet_at: [glam::Vec3::ZERO; 2] })
        } else {
            None
        };
        println!(
            "ew: play {}: spawn at {:.4},{:.4}, ground {:.1} m, {} pages resident, {} collision bodies, ready in {:.0} ms",
            place.name,
            place.lat,
            place.lon,
            world.ground(g).unwrap_or(f64::NAN),
            world.residency().len(),
            collision.nodes().len(),
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(Play {
            place,
            world,
            collision,
            game,
            cam,
            fixed: FixedStep::default(),
            content,
            node,
            heading: yaw + std::f32::consts::PI,
            anim,
            sync_loads: 0,
            far_warned: false,
            near: None,
            slip_max: 0.0,
            steps: 0,
            car,
            car_view,
            driving: false,
            chase: ChaseCam::new(yaw as f64),
            want_use: false,
            robot,
        })
    }

    /// E: get in or out of the car (handled at the start of the next update).
    pub fn interact(&mut self) {
        self.want_use = true;
    }

    /// F: wingsuit on/off. P: parachute. G: a nudge (the alpha's test key).
    pub fn flight_key(&mut self, code: KeyCode) {
        if self.driving {
            return;
        }
        if let Some(c) = self.robot.as_mut().and_then(|rb| rb.ctl.as_mut()) {
            match code {
                KeyCode::KeyF => c.toggle_wingsuit(),
                KeyCode::KeyP => c.toggle_parachute(),
                KeyCode::KeyG => c.nudge = true,
                _ => {}
            }
        }
    }

    /// The movement mode's name, for the window title and the log.
    pub fn mode_label(&self) -> Option<&'static str> {
        self.robot.as_ref().and_then(|rb| rb.ctl.as_ref()).map(|c| c.mode().label())
    }

    /// Whether the character stands on the ground.
    pub fn grounded(&self) -> bool {
        match &self.robot {
            Some(rb) => rb.ctl.as_ref().is_some_and(|c| c.grounded),
            None => self.game.grounded(),
        }
    }

    /// C: put the car about 4 m ahead of the robot, on the ground, facing its heading (spawns
    /// it if there is none).
    pub fn summon_car(&mut self, r: &mut Renderer, dev: &dyn Device) {
        if self.driving {
            return;
        }
        let Some(ctl) = self.robot.as_ref().and_then(|rb| rb.ctl.as_ref()) else { return };
        let (feet, yaw) = (ctl.feet(), ctl.yaw);
        let at = feet + DVec3::new(-yaw.sin(), yaw.cos(), 0.0) * 4.0;
        let Some(g) = self.world.ground_point(Anchor::sim_to_render(at)).map(Anchor::render_to_sim) else {
            println!("ew: play: no ground there for the car");
            return;
        };
        match &mut self.car {
            Some(c) => c.place(&mut self.game.world, g, yaw),
            None => match CarSim::spawn(&mut self.game.world, g, yaw) {
                Ok(c) => {
                    match CarView::new(r, dev, &c.veh.desc) {
                        Ok(v) => self.car_view = Some(v),
                        Err(e) => eprintln!("ew: play: car drawing: {e}"),
                    }
                    self.car = Some(c);
                }
                Err(e) => {
                    eprintln!("ew: play: {e}");
                    return;
                }
            },
        }
        println!("ew: play: car summoned at {:.1},{:.1},{:.1}", g.x, g.y, g.z);
    }

    /// Where the sim is centred: the car while driving, else the character's feet.
    pub fn focus(&self) -> DVec3 {
        match (&self.car, self.driving) {
            (Some(c), true) => c.cur.0.position,
            _ => self.game.cur,
        }
    }

    fn handle_use(&mut self, r: &mut Renderer) {
        if !std::mem::take(&mut self.want_use) {
            return;
        }
        let Some(car) = &self.car else { return };
        if self.driving {
            let world = &self.game.world;
            let filter = crate::car::scenery_filter();
            let terrain = &self.world;
            let free = |a: DVec3, b: DVec3| world.cast_ray(a, b - a, filter).is_none();
            let ground = |p: DVec3| terrain.ground_point(Anchor::sim_to_render(p)).map(|g| Anchor::render_to_sim(g).z);
            let Some(feet) = car.exit_point(free, ground) else {
                println!("ew: play: no room to get out here");
                return;
            };
            let car_yaw = CarSim::yaw(car.cur.0.rotation);
            if let Some(rb) = &mut self.robot {
                match Controller::spawn(&mut self.game.world, feet, car_yaw) {
                    Ok(c) => {
                        rb.ctl = Some(c);
                        rb.gait = Gait::default();
                        rb.origin = Anchor::sim_to_render(feet);
                        rb.cam.reset();
                        self.game.prev = feet;
                        self.game.cur = feet;
                    }
                    Err(e) => {
                        eprintln!("ew: play: {e}");
                        return;
                    }
                }
            } else if let Err(e) = self.game.unpark_mover(feet) {
                eprintln!("ew: play: {e}");
                return;
            }
            self.driving = false;
            let (yaw, pitch) = self.chase.view();
            self.cam.yaw = yaw;
            self.cam.pitch = pitch.clamp(PITCH_MIN, PITCH_MAX);
            self.cam.target = None;
            self.heading = CarSim::yaw(car.cur.0.rotation) as f32 + std::f32::consts::PI;
            self.game.heading = self.heading;
            println!("ew: play: out of the car at {:.1},{:.1},{:.1}", feet.x, feet.y, feet.z);
        } else if car.footprint_distance(self.game.cur) <= crate::car::ENTER_RANGE_M {
            if let Some(rb) = &mut self.robot {
                if let Some(c) = rb.ctl.take() {
                    c.despawn(&mut self.game.world);
                }
                rb.view.hide(r);
            } else {
                self.game.park_mover();
            }
            self.driving = true;
            self.chase = ChaseCam::new(self.cam.yaw as f64);
            println!("ew: play: in the car");
        }
    }

    /// One frame before the GPU work: collision, fixed steps, character or car, camera, pages.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, r: &mut Renderer, input: GameInput, drive: VehicleInput, look: Vec2, zoom: f32, frame_dt: f64, viewport: [u32; 2]) -> Camera {
        if !self.driving {
            self.cam.yaw -= look.x * LOOK_RAD_PER_PX;
            self.cam.pitch = (self.cam.pitch - look.y * LOOK_RAD_PER_PX).clamp(PITCH_MIN, PITCH_MAX);
            let (lo, hi) = if self.robot.is_some() { (crate::robot::cam::DIST_MIN, crate::robot::cam::DIST_MAX) } else { (CAM_MIN, CAM_MAX) };
            self.cam.dist = (self.cam.dist * (1.0 - zoom as f64 * 0.1)).clamp(lo, hi);
        }
        self.handle_use(r);
        // Collision first: the 3 x 3 must be exact before the sim steps.
        self.world.poll();
        let anchor = self.world.anchor.clone();
        let focus = self.focus();
        let near = collision_block(&anchor, focus, 1);
        let missing: Vec<NodeId> = near.iter().copied().filter(|id| self.world.page(*id).is_none()).collect();
        if !missing.is_empty() {
            self.sync_loads += missing.len() as u64;
            self.world.load(&missing, usize::MAX);
        }
        self.collision.update(&mut self.game.world, &anchor, self.world.residency(), focus);
        // Prefetch: a wider ring at driving speed, so the car never waits for its ground.
        let fast = self.driving && self.car.as_ref().is_some_and(|c| c.speed().abs() > 12.0);
        let ring = collision_block(&anchor, focus, if fast { 3 } else { 2 });
        self.world.request(&ring, MAX_INFLIGHT);

        let yaw = self.cam.yaw;
        if let Some(c) = self.robot.as_mut().and_then(|rb| rb.ctl.as_mut()) {
            // Flight inputs the sim frame cannot know: look pitch, planet anchor, ground height.
            c.cam_pitch = self.cam.pitch as f32;
            c.env.anchor = Some(anchor.clone());
            c.env.ground_alt = self.world.ground(Anchor::sim_to_render(c.feet()));
        }
        let mut sim = PlaySim { game: &mut self.game, car: self.car.as_mut(), robot: self.robot.as_mut().and_then(|rb| rb.ctl.as_mut()), driving: self.driving, drive, focus };
        let alpha = self.fixed.run(frame_dt, &mut sim, |_| GameInput { yaw, ..input });
        let feet = match self.robot.as_ref().and_then(|rb| rb.ctl.as_ref()) {
            Some(c) => c.feet_at(alpha),
            None => self.game.feet(alpha),
        };
        let car_pose = self.car.as_ref().map(|c| (c.pose(alpha), c.wheels(alpha), c.speed()));
        if let (Some(c), Some(v), Some((pose, wheels, _))) = (&self.car, &self.car_view, &car_pose) {
            v.draw(r, &c.veh.desc, *pose, wheels);
        }

        if self.driving {
            // The character rides along out of sight.
            if let (Some(n), Some((pose, _, _))) = (self.node, &car_pose) {
                r.instances().set_node(n, Anchor::sim_to_render(pose.position - DVec3::Z * 50.0));
            }
        } else if self.robot.is_some() {
            self.draw_robot(r, alpha, frame_dt);
        } else {
            // Character: render node at the feet, smoothed facing.
            let want = self.game.heading;
            let d = (want - self.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            let step = TURN_RAD_S * frame_dt as f32;
            self.heading += d.clamp(-step, step);
            // The walk: feet planted on the drawn terrain, the body's root where the legs put it.
            let mut root = feet;
            if let Some((a, skins)) = &mut self.anim {
                let v = (self.game.cur - self.game.prev) / ed_sim::DT;
                let terrain = &self.world;
                let z0 = feet.z;
                let ground = |p: glam::DVec2| terrain.ground_point(Anchor::sim_to_render(DVec3::new(p.x, p.y, z0))).map(|g| Anchor::render_to_sim(g).z);
                a.update(frame_dt as f32, feet, self.heading, v, self.game.grounded(), &ground);
                root = a.root;
                self.slip_max = self.slip_max.max(a.slip);
                self.steps = a.steps;
                let pal = a.palette();
                for s in skins.iter() {
                    let _ = r.set_skin_pose(*s, &pal);
                }
            }
            if let (Some(c), Some(n)) = (&mut self.content, self.node) {
                r.instances().set_node(n, Anchor::sim_to_render(root));
                c.move_object(r, CHARACTER, &Affine3A::from_quat(Quat::from_rotation_y(self.heading)));
            }
        }

        // Camera: blocked by the sim world (terrain bodies), clamped to the ground.
        let world = &self.game.world;
        let terrain = &self.world;
        let ground = |p: DVec3| terrain.ground_point(Anchor::sim_to_render(p)).map(|g| Anchor::render_to_sim(g).z);
        let camera = match (&car_pose, self.driving) {
            (Some((pose, _, speed)), true) => {
                let filter = crate::car::scenery_filter();
                self.chase.update(*pose, *speed, look, zoom, LOOK_RAD_PER_PX, frame_dt, |from, d| world.cast_ray(from, d, filter).map(|h| h.fraction as f64), ground)
            }
            _ if self.robot.as_ref().is_some_and(|rb| rb.ctl.is_some()) => {
                let filter = QueryFilter { category: layers::DEFAULT, mask: layers::ALL & !layers::MOVER };
                let rb = self.robot.as_mut().unwrap();
                let ctl = rb.ctl.as_ref().unwrap();
                let (pelvis, rot) = ctl.pose(alpha);
                let speed = if ctl.grounded { DVec3::new(ctl.vel.x, ctl.vel.y, 0.0).length() } else { ctl.vel.length() };
                let shake = ctl.shake() as f64;
                let t = (self.fixed.steps() as f64 + alpha as f64) / 60.0 * 47.0;
                let look = self.cam.look();
                let mut cam = rb.cam.update(&look, self.cam.dist, pelvis, rot * DVec3::Z, speed, ctl.grounded, frame_dt, |from, d| world.cast_ray(from, d, filter).map(|h| h.fraction as f64), ground);
                // Buffet: the alpha's camera shake from the suit and the Mach crossing.
                cam.position += DVec3::new(t.sin(), (t * 1.3).cos(), (t * 0.7).sin()) * shake * 0.25;
                cam
            }
            _ => {
                let filter = QueryFilter { category: layers::DEFAULT, mask: layers::ALL & !layers::MOVER };
                self.cam.update(feet, frame_dt, |from, d| world.cast_ray(from, d, filter).map(|h| h.fraction as f64), ground)
            }
        };

        // Pages for the view, then the atmosphere on the curved body.
        self.world.update_async(&camera, viewport, MAX_INFLIGHT);
        let g = ed_render::terrain::TerrainProvider::camera_ground(&self.world.anchor.body, &self.world.pose(), camera.position, self.world.ground(camera.position));
        ed_render::atmosphere::apply_camera_ground(r, g);
        self.world.feed(r);
        if let (Some(c), None) = (&self.content, &self.anim) {
            let _ = c.animate(r, (self.fixed.steps() as f64 + alpha as f64) / 60.0);
        }
        let focus = self.focus();
        let dist = DVec3::new(focus.x, focus.y, 0.0).length();
        if dist > FAR_FROM_ANCHOR_M && !self.far_warned {
            self.far_warned = true;
            println!("ew: play: {dist:.0} m from the anchor: gravity is uniform in the anchor frame (re-basing is later work)");
        }
        camera
    }

    /// Solves the robot's pose for this frame and draws it.
    fn draw_robot(&mut self, r: &mut Renderer, alpha: f32, frame_dt: f64) {
        let Some(rb) = &mut self.robot else { return };
        let Some(ctl) = &rb.ctl else { return };
        let (pelvis, rot) = ctl.pose(alpha);
        let mut pel = Anchor::sim_to_render(pelvis) - rb.origin;
        if pel.length() > REBASE_M {
            rb.origin += pel;
            rb.gait = Gait::default();
            pel = DVec3::ZERO;
        }
        let o = rb.origin.as_vec3();
        let f = ctl.feed();
        let input = RigInput {
            airborne: f.airborne,
            yaw: f.yaw,
            cam_pitch: self.cam.pitch,
            strafe: f.strafe,
            jump_charge: f.jump_charge,
            jump_push_timer: f.jump_push_timer,
            landing_recovery: f.landing_recovery,
            rot_err: f.rot_err,
            accel_smooth: f.accel_smooth,
            ground_normal: f.ground_normal,
            foot_ground_hit: [f.foot_ground_hit[0].map(|p| p - o), f.foot_ground_hit[1].map(|p| p - o)],
            vel: f.vel,
            fl: f.fl,
        };
        let world = &self.game.world;
        let filter = crate::car::scenery_filter();
        let origin = rb.origin;
        let ground = |p: glam::Vec3, max: f32| {
            let from = Anchor::render_to_sim(origin + p.as_dvec3());
            world.cast_ray(from, -DVec3::Z * max as f64, filter).map(|h| (h.fraction as f32) * max)
        };
        let t = ((self.fixed.steps() as f64 + alpha as f64) / 60.0) as f32;
        let pose = rig::solve(&input, &mut rb.gait, pel.as_vec3(), to_render_rot(rot), frame_dt as f32, t, &ground);
        // Steps and the worst planted-foot slide, for the log.
        for i in 0..2 {
            if pose.foot_planted[i] {
                if rb.planted[i] {
                    self.slip_max = self.slip_max.max((pose.foot_world_pos[i] - rb.feet_at[i]).length());
                } else {
                    self.steps += 1;
                }
            }
            rb.planted[i] = pose.foot_planted[i];
            rb.feet_at[i] = pose.foot_world_pos[i];
        }
        rb.view.draw(r, &pose, rb.origin);
    }

    pub fn destroy(&mut self, r: &mut Renderer) {
        if let Some(c) = &mut self.content {
            c.clear(r);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Window
// ---------------------------------------------------------------------------------------------

struct Gpu {
    renderer: Option<Renderer>,
    surface: SurfaceId,
    config: SurfaceConfig,
    window: Window,
}

/// Offscreen screenshot target (autowalk).
struct Shot {
    tex: TextureId,
    buf: BufferId,
    w: u32,
    h: u32,
}

const SHOT_FORMAT: Format = Format::Rgba8Srgb;
/// Flight script shots: walk, rocket thrust at three heights, skydive, parachute, landing.
const FLY_SHOTS: [&str; 9] = ["fly_0_walk.png", "fly_1_liftoff.png", "fly_2_thrust.png", "fly_3_high.png", "fly_4_skydive.png", "fly_5_skydive.png", "fly_6_parachute.png", "fly_7_chute.png", "fly_8_end.png"];

const WALK_SHOTS: [&str; 9] = ["play_start.png", "walk_0.png", "walk_1.png", "walk_2.png", "walk_3.png", "run_0.png", "run_1.png", "run_2.png", "play_end.png"];
const DRIVE_SHOTS: [&str; 8] = ["drive_start.png", "drive_in.png", "drive_0.png", "drive_1.png", "drive_turn.png", "drive_brake.png", "drive_stopped.png", "drive_out.png"];

struct App {
    args: Args,
    place: &'static Place,
    dev: Box<dyn Device>,
    gpu: Option<Gpu>,
    play: Option<Play>,
    keys: BTreeSet<KeyCode>,
    look: Vec2,
    zoom: f32,
    pending_size: Option<PhysicalSize<u32>>,
    last: Option<Instant>,
    presented: u64,
    errors: u64,
    result: Result<(), String>,
    done: bool,
    log_t: Instant,
    log_frames: u64,
    log_cpu_ms: f64,
    log_upd_ms: f64,
    /// Near-tree update: ms and nodes applied per frame, and when every request was resident.
    trees_ms: Vec<f64>,
    /// Ground-cover share of the near-tree update (ms per frame).
    ground_ms: Vec<f64>,
    trees_applied: Vec<usize>,
    trees_t0: Option<Instant>,
    trees_resident_s: Option<f64>,
    trees_sec_ms: f64,
    log_worst_ms: f64,
    worst_ms: f64,
    /// Frame intervals (ms) after spawn: the whole run and the current second.
    ft: Vec<f64>,
    ft_sec: Vec<f64>,
    /// Auto-walk: seconds to walk, start time, screenshots written.
    autowalk: Option<f64>,
    /// Auto-fly (`EW_AUTOFLY`): seconds of the flight script, and whether it has opened the parachute.
    autofly: Option<f64>,
    auto_chute: bool,
    /// Auto-drive: seconds of the drive script (`EW_AUTODRIVE`), and E presses it has made.
    autodrive: Option<f64>,
    auto_uses: u32,
    /// Auto-drive heading and the script time it was last re-picked.
    auto_heading: Option<(f64, f64)>,
    /// Auto-walk time: summed frame intervals (screenshot readbacks excluded).
    t_walk: Option<f64>,
    shot: Option<Shot>,
    shots_done: u32,
    out: PathBuf,
}

pub fn run(args: Args) -> Result<(), String> {
    let place = args.play.and_then(|p| p.place()).ok_or("--play: expected zermatt|blueridge")?;
    let dev = ed_gpu::create_device(ed_gpu::default_backend(), &DeviceDesc { validation: args.validation, frames_in_flight: args.frames_in_flight })
        .map_err(|e| format!("device: {e}"))?;
    println!("ew: {}", crate::caps_line(dev.caps()));
    let event_loop = EventLoop::new().map_err(|e| format!("event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let env_secs = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok()).filter(|s| *s > 0.0);
    let autodrive = env_secs("EW_AUTODRIVE");
    let autofly = if autodrive.is_some() { None } else { env_secs("EW_AUTOFLY") };
    let autowalk = if autodrive.is_some() || autofly.is_some() { None } else { env_secs("EW_AUTOWALK") };
    let mut app = App {
        out: args.out.clone(),
        args,
        place,
        dev,
        gpu: None,
        play: None,
        keys: BTreeSet::new(),
        look: Vec2::ZERO,
        zoom: 0.0,
        pending_size: None,
        last: None,
        presented: 0,
        errors: 0,
        result: Ok(()),
        done: false,
        log_t: Instant::now(),
        log_frames: 0,
        log_cpu_ms: 0.0,
        log_upd_ms: 0.0,
        trees_ms: Vec::new(),
        ground_ms: Vec::new(),
        trees_applied: Vec::new(),
        trees_t0: None,
        trees_resident_s: None,
        trees_sec_ms: 0.0,
        log_worst_ms: 0.0,
        worst_ms: 0.0,
        ft: Vec::new(),
        ft_sec: Vec::new(),
        autowalk,
        autofly,
        auto_chute: false,
        autodrive,
        auto_uses: 0,
        auto_heading: None,
        t_walk: None,
        shot: None,
        shots_done: 0,
    };
    event_loop.run_app(&mut app).map_err(|e| format!("event loop: {e}"))?;
    app.shutdown();
    app.result
}

impl App {
    fn create_gpu(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let attrs = Window::default_attributes()
            .with_title(format!("ew - {} (WASD walk, Shift run, Space jump, E car in/out, C car here, F wingsuit, P parachute, mouse look, wheel zoom, Esc quit)", self.place.name))
            .with_inner_size(PhysicalSize::new(self.args.width, self.args.height));
        let window = el.create_window(attrs).map_err(|e| format!("window: {e}"))?;
        let ns_view = match window.window_handle().map_err(|e| format!("window handle: {e}"))?.as_raw() {
            RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
            other => return Err(format!("unsupported window handle {other:?}")),
        };
        let size = window.inner_size();
        let config = SurfaceConfig { width: size.width.max(1), height: size.height.max(1), format: crate::windowed::SURFACE_FORMAT, vsync: self.args.vsync, hdr: false };
        let surface = self.dev.create_surface(WindowHandle { window: ns_view, display: std::ptr::null_mut() }, &config).map_err(|e| format!("create_surface: {e}"))?;
        println!("ew: surface {}x{} vsync={} (scale factor {})", config.width, config.height, config.vsync, window.scale_factor());
        let near = crate::trees::near_from_env();
        let mut renderer = if near { Renderer::with_desc(&*self.dev, &crate::trees::NearTrees::renderer_desc()) } else { Renderer::new(&*self.dev) }.map_err(|e| format!("renderer: {e}"))?;
        self.args.apply_lod(renderer.lod_config());
        let mut play = Play::new(&mut renderer, &*self.dev, self.place, &self.args.packs, [config.width, config.height], self.args.character)?;
        if near {
            let mut n = crate::trees::NearTrees::new(&crate::terrain::cache_root());
            n.set_date(self.args.season_day());
            play.near = Some(n);
        }
        self.play = Some(play);
        if !self.scripted() {
            if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = window.set_cursor_grab(CursorGrabMode::Confined);
            }
            window.set_cursor_visible(false);
        }
        if let Some(s) = self.autowalk.or(self.autodrive).or(self.autofly) {
            let what = if self.autodrive.is_some() { "autodrive" } else if self.autofly.is_some() { "autofly" } else { "autowalk" };
            println!("ew: {what} {s:.0} s, screenshots to {}", self.out.display());
            let (w, h) = (config.width, config.height);
            let tex = self
                .dev
                .create_texture(&TextureDesc {
                    label: "play shot",
                    kind: TextureKind::D2,
                    format: SHOT_FORMAT,
                    width: w,
                    height: h,
                    depth_or_layers: 1,
                    mips: 1,
                    samples: 1,
                    usage: Usage { render_target: true, copy_src: true, ..Default::default() },
                })
                .map_err(|e| format!("shot target: {e}"))?;
            let buf = self
                .dev
                .create_buffer(&BufferDesc { label: "play shot readback", size: SHOT_FORMAT.copy_row_pitch(w) as u64 * h as u64, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } })
                .map_err(|e| format!("shot readback: {e}"))?;
            self.shot = Some(Shot { tex, buf, w, h });
        }
        self.gpu = Some(Gpu { renderer: Some(renderer), surface, config, window });
        Ok(())
    }

    fn reconfigure(&mut self, size: PhysicalSize<u32>, vsync: bool) {
        let Some(gpu) = &mut self.gpu else { return };
        let cfg = SurfaceConfig { width: size.width.max(1), height: size.height.max(1), vsync, ..gpu.config.clone() };
        if (cfg.width, cfg.height, cfg.vsync) == (gpu.config.width, gpu.config.height, gpu.config.vsync) {
            return;
        }
        match self.dev.configure_surface(gpu.surface, &cfg) {
            Ok(()) => {
                println!("ew: surface {}x{} vsync={}", cfg.width, cfg.height, vsync);
                gpu.config = cfg;
            }
            Err(e) => {
                eprintln!("ew: configure_surface failed: {e}");
                self.errors += 1;
            }
        }
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        if let Some(size) = self.pending_size.take() {
            let vsync = self.gpu.as_ref().map_or(self.args.vsync, |g| g.config.vsync);
            self.reconfigure(size, vsync);
        }
        let now = Instant::now();
        let frame_dt = self.last.map_or(0.0, |l| (now - l).as_secs_f64());
        self.last = Some(now);
        if self.presented > 5 {
            self.worst_ms = self.worst_ms.max(frame_dt * 1e3);
            self.log_worst_ms = self.log_worst_ms.max(frame_dt * 1e3);
            self.ft.push(frame_dt * 1e3);
            self.ft_sec.push(frame_dt * 1e3);
        }
        let (Some(gpu), Some(play)) = (&mut self.gpu, &mut self.play) else { return };
        let dev = &*self.dev;
        let (w, h) = (gpu.config.width, gpu.config.height);
        let t_upd = Instant::now();

        // Input (keyboard or the auto-walk / auto-drive script).
        let mut shoot = None;
        let mut drive = VehicleInput::default();
        let (keys, look) = if let Some(total) = self.autodrive {
            let tw = self.t_walk.get_or_insert(0.0);
            *tw += frame_dt;
            let t = *tw;
            let (times, names) = Self::drive_shots(total);
            if let Some(&at) = times.get(self.shots_done as usize)
                && t > at
            {
                shoot = Some(names[self.shots_done as usize]);
            }
            let phase = crate::car::auto_phase(t, total);
            let uses = match phase {
                AutoPhase::WalkToCar => 0,
                AutoPhase::Enter | AutoPhase::Drive | AutoPhase::Turn | AutoPhase::Brake => 1,
                AutoPhase::Exit | AutoPhase::Done => 2,
            };
            if self.auto_uses < uses {
                self.auto_uses += 1;
                play.interact();
            }
            if let (true, Some(car)) = (play.driving, &play.car) {
                let speed = car.speed();
                let terrain = &play.world;
                let ground = |p: DVec3| terrain.ground_point(Anchor::sim_to_render(p)).map(|g| Anchor::render_to_sim(g).z);
                let pose = car.pose(0.0);
                if phase == AutoPhase::Drive {
                    // The flattest way from here (any direction at first, then within 35
                    // degrees of the way we are going), re-picked every second.
                    let (around, spread) = self.auto_heading.map_or((CarSim::yaw(pose.rotation), std::f64::consts::PI), |h| (h.0, 0.6));
                    if self.auto_heading.is_none_or(|h| t - h.1 > 1.0) {
                        self.auto_heading = Some((crate::car::flattest_heading(pose.position, around, spread, ground), t));
                    }
                }
                let heading = self.auto_heading.map_or(0.0, |h| h.0);
                drive = match phase {
                    AutoPhase::Drive => VehicleInput { drive: if speed < 16.0 { 1.0 } else { 0.0 }, steer: crate::car::steer_toward(pose, heading), handbrake: false },
                    AutoPhase::Turn => VehicleInput { drive: if speed < 9.0 { 0.6 } else { 0.0 }, steer: -1.0, handbrake: speed > 13.0 },
                    _ => VehicleInput { drive: if speed > 0.3 { -1.0 } else { 0.0 }, ..Default::default() },
                };
            }
            let mut k = BTreeSet::new();
            if phase == AutoPhase::Done {
                k.insert(KeyCode::KeyW);
            }
            (k, Vec2::ZERO)
        } else if let Some(total) = self.autowalk {
            let tw = self.t_walk.get_or_insert(0.0);
            *tw += frame_dt;
            let t = *tw;
            // Start, a walking burst, a running burst, end (a burst shows the gait's phases).
            let walk = total * 0.22;
            let run = total * 0.75;
            let times = [0.8, walk, walk + 0.08, walk + 0.16, walk + 0.24, run, run + 0.06, run + 0.12, total];
            if let Some(&at) = times.get(self.shots_done as usize)
                && t > at
            {
                shoot = Some(WALK_SHOTS[self.shots_done as usize]);
            }
            Self::autowalk_input_static(t, total)
        } else if let Some(total) = self.autofly {
            let tw = self.t_walk.get_or_insert(0.0);
            *tw += frame_dt;
            let t = *tw;
            let times = [1.0, 2.6, 3.8, 6.0, 8.2, 9.5, 11.5, 14.0, total];
            if let Some(&at) = times.get(self.shots_done as usize)
                && t > at
            {
                shoot = Some(FLY_SHOTS[self.shots_done as usize]);
            }
            // Parachute once the skydive has built up speed.
            if t > 9.0 && !self.auto_chute {
                self.auto_chute = true;
                play.flight_key(KeyCode::KeyP);
            }
            // Looking up aims the rocket: pitch up for the thrust, level for the dive.
            play.cam.pitch = if (2.0..7.0).contains(&t) { 0.7 } else if t > 9.0 { 0.45 } else { -0.2 };
            Self::autofly_input_static(t)
        } else {
            let keys = self.keys.clone();
            drive = crate::car::drive_input(&keys);
            (keys, std::mem::take(&mut self.look))
        };
        let renderer = gpu.renderer.as_mut().expect("renderer lives until shutdown");
        let input = game_input(&keys, 0.0);
        let zoom = std::mem::take(&mut self.zoom);
        let camera = play.update(renderer, input, drive, look, zoom, frame_dt, [w, h]);
        if let Some(nt) = &mut play.near {
            let tt = Instant::now();
            let t0 = *self.trees_t0.get_or_insert(tt);
            match nt.update(&play.world, renderer, dev, &camera, [w, h], play.fixed.steps() as f64 / 60.0, false) {
                Ok(()) => {
                    let ms = tt.elapsed().as_secs_f64() * 1e3;
                    self.trees_ms.push(ms);
                    if let Some(g) = &nt.ground {
                        self.ground_ms.push(g.stats.last_ms);
                    }
                    self.trees_sec_ms += ms;
                    self.trees_applied.push(nt.stats.applied_last);
                    if self.trees_resident_s.is_none() && nt.stats.pending == 0 && nt.stats.nodes > 0 {
                        self.trees_resident_s = Some(t0.elapsed().as_secs_f64());
                    }
                }
                Err(e) => {
                    eprintln!("ew: near trees disabled: {e}");
                    play.near = None;
                }
            }
        }
        let upd_ms = t_upd.elapsed().as_secs_f64() * 1e3;
        let tf = Instant::now();
        if let Err(e) = renderer.flush_uploads(dev) {
            eprintln!("ew: flush_uploads: {e}");
            self.errors += 1;
        }
        let flush_ms = tf.elapsed().as_secs_f64() * 1e3;
        let mut enc = match dev.begin_frame() {
            Ok(e) => e,
            Err(e) => {
                self.result = Err(format!("begin_frame: {e}"));
                el.exit();
                return;
            }
        };
        let t0 = Instant::now();
        let shot = shoot.and(self.shot.as_ref());
        let target = match shot {
            Some(s) => ViewTarget { texture: s.tex, format: SHOT_FORMAT, width: s.w, height: s.h, initial: ResourceState::Undefined, final_state: None },
            None => match dev.acquire(gpu.surface) {
                Ok(tex) => ViewTarget { texture: tex, format: gpu.config.format, width: w, height: h, initial: ResourceState::Undefined, final_state: Some(ResourceState::Present) },
                Err(e) => {
                    let _ = dev.end_frame(enc);
                    if !matches!(e, ed_gpu::Error::SurfaceLost) {
                        eprintln!("ew: acquire failed: {e}");
                        self.errors += 1;
                    }
                    self.pending_size = Some(gpu.window.inner_size());
                    return;
                }
            },
        };
        let view = View { camera, viewport: Viewport { x: 0, y: 0, width: target.width, height: target.height }, target, flags: Default::default() };
        let params = FrameParams { readback: shot.map(|s| s.buf), ..Default::default() };
        if let Err(e) = renderer.render(dev, &mut *enc, &view, &params) {
            eprintln!("ew: render: {e}");
            self.errors += 1;
        }
        match dev.end_frame(enc) {
            Ok(()) => self.presented += 1,
            Err(e) => {
                eprintln!("ew: end_frame: {e}");
                self.errors += 1;
            }
        }
        if let (Some(name), Some(s)) = (shoot, &self.shot) {
            dev.wait_idle();
            let pitch = SHOT_FORMAT.copy_row_pitch(s.w) as usize;
            let mut raw = Vec::new();
            match dev.read_buffer(s.buf, &mut raw) {
                Ok(()) => {
                    let mut rgba = Vec::with_capacity((s.w * s.h * 4) as usize);
                    for y in 0..s.h as usize {
                        rgba.extend_from_slice(&raw[y * pitch..y * pitch + s.w as usize * 4]);
                    }
                    let _ = std::fs::create_dir_all(&self.out);
                    let p = self.out.join(name);
                    match crate::headless::write_png(&p, s.w, s.h, &rgba) {
                        Ok(()) => println!("ew: play screenshot {}", p.display()),
                        Err(e) => eprintln!("ew: {e}"),
                    }
                }
                Err(e) => eprintln!("ew: read_buffer: {e}"),
            }
            self.shots_done += 1;
            // The readback wait is not a frame time.
            self.last = Some(Instant::now());
            let total = if self.autodrive.is_some() { DRIVE_SHOTS.len() } else if self.autofly.is_some() { FLY_SHOTS.len() } else { WALK_SHOTS.len() };
            if self.shots_done as usize >= total {
                el.exit();
            }
        }
        let cpu = flush_ms + t0.elapsed().as_secs_f64() * 1e3;
        self.log_cpu_ms += cpu;
        self.log_upd_ms += upd_ms;
        self.log_frames += 1;
        let since = self.log_t.elapsed().as_secs_f64();
        if since >= 1.0 {
            let n = self.log_frames.max(1) as f64;
            let gpu_ms = renderer.timings(dev).map_or("-".into(), |t| format!("{:.2}", t.frame_ms));
            let st = play.world.selection();
            let feet = play.game.cur;
            let ground = play.world.ground_point(Anchor::sim_to_render(feet)).map(|g| feet.z - Anchor::render_to_sim(g).z);
            let lvl = play.world.planet.height(play.world.residency(), play.world.anchor.body.id, play.world.anchor.sim_to_body(feet)).map_or(0, |h| h.1);
            let p99 = pct(&mut self.ft_sec, 0.99);
            self.ft_sec.clear();
            println!(
                "ew: {:.1} fps, gpu {gpu_ms} ms, cpu {:.2} ms (update {:.2}), frame p99 {p99:.1} / worst {:.1} ms | ground level {lvl} | feet {:.1},{:.1},{:.1} ({:.0} m from anchor), {}, above ground {} | pages {} resident, {} loading, {} holes, sync {} | collision {} nodes | walk: {} steps, worst foot miss {:.3} m",
                self.log_frames as f64 / since,
                self.log_cpu_ms / n,
                self.log_upd_ms / n,
                self.log_worst_ms,
                feet.x,
                feet.y,
                feet.z,
                DVec3::new(feet.x, feet.y, 0.0).length(),
                if play.grounded() { "grounded" } else { "airborne" },
                ground.map_or("-".into(), |g| format!("{g:.3} m")),
                play.world.residency().len(),
                play.world.inflight(),
                st.holes.len(),
                play.sync_loads,
                play.collision.nodes().len(),
                play.steps,
                play.slip_max,
            );
            if let (true, Some(c)) = (play.driving, &play.car) {
                let p = c.cur.0.position;
                let up = (c.cur.0.rotation * DVec3::Z).z;
                let wheels = c.veh.wheels.iter().filter(|w| w.contact).count();
                println!(
                    "ew: car {:.1} km/h, gear {}, {:.0} rpm, steer {:.2}, at {:.1},{:.1},{:.1}, up.z {:.3}, {wheels}/4 wheels down",
                    c.speed() * 3.6,
                    c.veh.gear,
                    c.veh.rpm,
                    c.veh.steer,
                    p.x,
                    p.y,
                    p.z,
                    up
                );
            }
            if let Some(nt) = &play.near {
                println!(
                    "ew: near trees: {} nodes resident, {} pending, {} instances, {} bank, update {:.2} ms/frame, last apply {} nodes / {:.2} ms",
                    nt.stats.nodes, nt.stats.pending, nt.stats.instances, nt.stats.bank_meshes, self.trees_sec_ms / n, nt.stats.applied_last, nt.stats.apply_last_ms
                );
            }
            self.trees_sec_ms = 0.0;
            play.slip_max = 0.0;
            self.log_t = Instant::now();
            self.log_frames = 0;
            self.log_cpu_ms = 0.0;
            self.log_upd_ms = 0.0;
            self.log_worst_ms = 0.0;
        }
        if let Some(n) = self.args.frames.filter(|&n| n > 0)
            && self.presented >= n
        {
            el.exit();
        }
    }

    /// Either script is running (no cursor grab, no keyboard E).
    fn scripted(&self) -> bool {
        self.autowalk.is_some() || self.autodrive.is_some() || self.autofly.is_some()
    }

    /// Auto-drive screenshot times for a `total`-second script, with their names.
    fn drive_shots(total: f64) -> ([f64; 8], &'static [&'static str; 8]) {
        let f = |x: f64| x * total;
        ([0.8, 1.7, f(0.3), f(0.45), f(0.62), f(0.8), f(0.88), total], &DRIVE_SHOTS)
    }

    /// Auto-fly input at `t` seconds: walk 2 s, hold Space (jump, then rocket thrust) to 7 s with
    /// a little forward lean, then let go.
    fn autofly_input_static(t: f64) -> (BTreeSet<KeyCode>, Vec2) {
        let mut k = BTreeSet::new();
        if (0.5..2.0).contains(&t) {
            k.insert(KeyCode::KeyW);
        }
        if (2.0..7.0).contains(&t) {
            k.insert(KeyCode::Space);
        }
        (k, Vec2::ZERO)
    }

    /// Auto-walk input at `t` seconds: forward, a slow turn, run after a third, a jump at half.
    fn autowalk_input_static(t: f64, total: f64) -> (BTreeSet<KeyCode>, Vec2) {
        let mut k = BTreeSet::new();
        if t > 1.0 {
            k.insert(KeyCode::KeyW);
        }
        if t > total / 3.0 {
            k.insert(KeyCode::ShiftLeft);
        }
        if (t - total / 2.0).abs() < 0.05 {
            k.insert(KeyCode::Space);
        }
        let turn = if t < total / 2.0 { -2.0 } else { 2.0 };
        (k, Vec2::new(turn, 0.0))
    }

    fn shutdown(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        self.dev.wait_idle();
        if let Some(s) = self.shot.take() {
            self.dev.destroy_texture(s.tex);
            self.dev.destroy_buffer(s.buf);
        }
        if let Some(mut gpu) = self.gpu.take() {
            if let Some(mut r) = gpu.renderer.take() {
                if let Some(p) = &mut self.play {
                    p.destroy(&mut r);
                }
                r.destroy(&*self.dev);
            }
            self.dev.destroy_surface(gpu.surface);
            self.dev.wait_idle();
            drop(gpu.window);
        }
        let (p50, p99) = (pct(&mut self.ft, 0.5), pct(&mut self.ft, 0.99));
        println!("ew: play: {} frames presented, frame interval p50 {p50:.2} / p99 {p99:.2} / worst {:.1} ms, {} errors", self.presented, self.worst_ms, self.errors);
        if !self.trees_ms.is_empty() {
            let (p99, mx) = (pct(&mut self.trees_ms, 0.99), self.trees_ms.iter().copied().fold(0.0, f64::max));
            let amax = self.trees_applied.iter().copied().max().unwrap_or(0);
            let atot: usize = self.trees_applied.iter().sum();
            println!(
                "ew: near trees update ms/frame: p50 {:.2} / p99 {p99:.2} / max {mx:.2}; nodes applied per frame: max {amax}, total {atot}; all requested resident after {}",
                pct(&mut self.trees_ms, 0.5),
                self.trees_resident_s.map_or("never".into(), |s| format!("{s:.1} s"))
            );
        }
        if !self.ground_ms.is_empty() {
            let (p50, p99) = (pct(&mut self.ground_ms, 0.5), pct(&mut self.ground_ms, 0.99));
            let mx = self.ground_ms.iter().copied().fold(0.0, f64::max);
            println!("ew: ground cover update ms/frame (main thread): p50 {p50:.2} / p99 {p99:.2} / max {mx:.2}");
        }
        if self.errors > 0 && self.result.is_ok() {
            self.result = Err(format!("{} GPU/surface errors", self.errors));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_none()
            && let Err(e) = self.create_gpu(el)
        {
            self.result = Err(e);
            el.exit();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => self.pending_size = Some(size),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                let pressed = event.state == ElementState::Pressed;
                if pressed && !event.repeat {
                    match code {
                        KeyCode::Escape => el.exit(),
                        KeyCode::KeyE => {
                            let scripted = self.scripted();
                            if let Some(p) = &mut self.play
                                && !scripted
                            {
                                p.interact();
                            }
                        }
                        KeyCode::KeyC => {
                            let scripted = self.scripted();
                            if !scripted
                                && let (Some(p), Some(gpu)) = (&mut self.play, &mut self.gpu)
                                && let Some(r) = gpu.renderer.as_mut()
                            {
                                p.summon_car(r, &*self.dev);
                            }
                        }
                        KeyCode::KeyF | KeyCode::KeyP | KeyCode::KeyG => {
                            let scripted = self.scripted();
                            if !scripted && let Some(p) = &mut self.play {
                                p.flight_key(code);
                            }
                        }
                        KeyCode::KeyV => {
                            if let Some(gpu) = &self.gpu {
                                let size = PhysicalSize::new(gpu.config.width, gpu.config.height);
                                let vsync = !gpu.config.vsync;
                                self.reconfigure(size, vsync);
                            }
                        }
                        _ => {}
                    }
                }
                if pressed {
                    self.keys.insert(code);
                } else {
                    self.keys.remove(&code);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.zoom += match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 24.0,
                };
            }
            WindowEvent::Focused(false) => self.keys.clear(),
            WindowEvent::Focused(true) => {
                if let Some(gpu) = &self.gpu
                    && !self.scripted()
                    && gpu.window.set_cursor_grab(CursorGrabMode::Locked).is_err()
                {
                    let _ = gpu.window.set_cursor_grab(CursorGrabMode::Confined);
                }
            }
            WindowEvent::RedrawRequested if !el.exiting() => self.frame(el),
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.look += Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() && !el.exiting() {
            self.frame(el);
        }
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.shutdown();
    }
}

/// The `q` quantile of `v` (sorts it); 0 when empty.
fn pct(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

/// Skeletons in a pack file, in key order.
pub fn pack_skeletons(path: &std::path::Path) -> Vec<ed_pack::kinds::Skeleton> {
    let Ok(rd) = ed_pack::PackReader::open_file(path) else { return Vec::new() };
    let keys: Vec<ed_pack::ChunkKey> = rd.entries().iter().map(|e| e.key).filter(|k| k.kind() == ed_pack::ChunkKind::Skeleton).collect();
    keys.iter().filter_map(|k| rd.read(k).ok()?.decode().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn print_character_joints() {
        for sk in pack_skeletons(&crate::content::default_pack_dir().join("character.ewpack")) {
            eprintln!("skeleton for {:?}: {} joints", sk.mesh, sk.joints.len());
            for (i, j) in sk.joints.iter().enumerate() {
                eprintln!("  {i} {} parent {:?} rest t{:?} r{:?}", j.name, j.parent, j.rest.translation, j.rest.rotation);
            }
        }
    }
}
