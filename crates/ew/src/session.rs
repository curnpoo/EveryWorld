//! App session (M4b): the character game, the editor, the mode switch and the views, shared by
//! the windowed shell and the headless harness.
//!
//! Per frame: [`Session::update`] (outside the GPU frame: fixed steps, character, camera,
//! editor UI + its paint upload, overlay batch, view slots), then [`Session::record`] inside
//! the frame (main view, optional inset, egui over the target, optional readback), then
//! [`Session::after_frame`] (pick answers, stats).
//!
//! P1-D17: the simulation steps in both modes on the same fixed-step boundary. In editor mode
//! the character gets zero input (it stands; gravity and collision still run), so switching
//! modes never pauses or forks the world: a session's replay is the same whether the editor
//! was open or not, and the editor never inspects a frozen, stale sim.

use crate::authored::{self, Authored};
use crate::camera::{CameraInput, FreeCamera, Pose};
use crate::content::Content;
use crate::fixed_step::{FixedStep, Simulation};
use crate::game::{CHARACTER, CHARACTER_ASSET, Game, GameInput, PLACEHOLDER_ASSET};
use ed_assets::bake::proving_node;
use ed_assets::proving::ORIGIN;
use ed_editor::editor::{EditorHost, EditorInput, FrameStats, InputEvent, LogLevel, LogLine, ObjectInfo, PassTiming, PickState, ViewportSize};
use ed_editor::{Editor, PaintTarget, Painter, Transform};
use ed_geo::{BodyId, ObjectId};
use ed_gpu::{BufferId, CommandEncoder, Device, LoadOp, ResourceState, Subresource, Transition};
use ed_pack::AssetId;
use ed_render::{Camera, FrameParams, OverlayVertex, Renderer, View, ViewFlags, ViewTarget, Viewport};
use glam::{Affine3A, DQuat, DVec3, Vec2};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

/// What input drives (P1-D17: the sim steps in both).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// The character: WASD runs, mouse turns the follow camera.
    Game,
    /// Fly camera + editor UI; the character gets zero input but the sim keeps stepping.
    #[default]
    Editor,
}

impl Mode {
    pub fn toggled(self) -> Mode {
        match self {
            Mode::Game => Mode::Editor,
            Mode::Editor => Mode::Game,
        }
    }
}

/// Collision debug lines within this distance of the character.
pub const DEBUG_RADIUS_M: f64 = 40.0;
/// Mouse look in game mode, radians per pixel.
pub const LOOK_RAD_PER_PX: f32 = 0.0025;

/// One frame's input.
#[derive(Clone, Debug, Default)]
pub struct FrameIn {
    /// Wall time since the last frame (s); headless passes exactly one step.
    pub dt: f64,
    /// Monotonic UI clock (s).
    pub time: f64,
    /// Character input (game mode; `yaw` is ignored, the session owns the look).
    pub game: GameInput,
    /// Mouse delta, pixels (game: turns the follow camera; editor: free camera look).
    pub look: Vec2,
    /// Free camera axes (editor mode).
    pub cam: CameraInput,
    /// Editor input events (editor mode).
    pub events: Vec<InputEvent>,
    pub size: [u32; 2],
    pub pixels_per_point: f32,
    /// Use this camera instead of the mode's own (headless scripted paths).
    pub camera: Option<Camera>,
}

/// What [`Session::update`] decided.
#[derive(Clone, Copy, Debug)]
pub struct FrameOut {
    pub camera: Camera,
    /// The editor UI owns the pointer / keyboard: do not fly the camera with them.
    pub wants_pointer: bool,
    pub wants_keyboard: bool,
    /// Fixed steps run this frame.
    pub steps: u32,
}

/// The async pick ([`EditorHost::pick`]): requested -> sent with a frame -> answered from
/// that frame's readback.
#[derive(Debug, Default)]
struct PickSlot {
    pixel: Option<[u32; 2]>,
    /// Device frame index the request went out with.
    sent: Option<u64>,
    answer: Option<PickState>,
}

pub struct Session {
    pub mode: Mode,
    pub game: Option<Game>,
    pub editor: Option<Editor>,
    painter: Option<Painter>,
    /// A UI frame has been prepared (the painter repaints it until the next one).
    painted: bool,
    pub free: FreeCamera,
    /// Follow camera look (game mode).
    pub yaw: f32,
    pub pitch: f32,
    pub fixed: FixedStep,
    pub collision_debug: bool,
    pub inset: bool,
    /// Draw the editor panels (egui) in editor mode.
    pub editor_ui: bool,
    pub layer_root: PathBuf,
    pub authored: Authored,
    /// Authored objects as placed now: asset, node affine (collision rebuilds use it).
    placed: BTreeMap<ObjectId, (AssetId, Affine3A)>,
    collision_gen: u64,
    log: Vec<LogLine>,
    pick: PickSlot,
    pub stats: FrameStats,
    pub camera: Camera,
    size: [u32; 2],
    ppp: f32,
    views_ready: usize,
    t0: Instant,
    /// Editor frame + paint upload, ms per editor-mode frame (perf).
    pub ui_ms: Vec<f64>,
    /// Which asset the character uses (`characters/solarcat` or the placeholder).
    pub character_asset: String,
    last_frame: Option<Instant>,
    /// Files written by editor saves (Cmd+S), all frames.
    pub saved: Vec<PathBuf>,
    /// Transform changes the editor applied, all frames.
    pub edits: u64,
    overlay_empty: bool,
}

impl Session {
    /// Loads the layers, places the authored objects and the character, builds the sim.
    pub fn new(r: &mut Renderer, dev: &dyn Device, content: &mut Content, layer_root: PathBuf, mode: Mode, editor_ui: bool) -> Result<Session, String> {
        let authored = Authored::load(&layer_root)?;
        let spawn = authored.spawn_feet();
        let mut game = Game::new(&content.collision, spawn)?;
        if let Some(t) = authored.spawn {
            game.heading = authored::yaw_of(&t);
        }
        let mut placed = BTreeMap::new();
        for (id, a, x) in authored.list() {
            game.set_object(id, a, &x)?;
            placed.insert(id, (a, x));
        }
        let character_asset = if content.has_asset(AssetId::from_path(CHARACTER_ASSET)) { CHARACTER_ASSET } else { PLACEHOLDER_ASSET }.to_string();
        let mut s = Session {
            mode,
            editor: Some(Editor::new()),
            painter: if editor_ui { Some(Painter::new(dev).map_err(|e| format!("egui painter: {e}"))?) } else { None },
            painted: false,
            free: FreeCamera { speed: 12.0, fast_speed: 120.0, ..FreeCamera::new(Pose::from_camera(&crate::proving::street_camera())) },
            // Behind the character: the follow camera looks the way it faces.
            yaw: game.heading + std::f32::consts::PI,
            pitch: -0.25,
            fixed: FixedStep::default(),
            collision_debug: false,
            inset: false,
            editor_ui,
            layer_root,
            authored,
            placed,
            collision_gen: content.collision_gen,
            log: Vec::new(),
            pick: PickSlot::default(),
            stats: FrameStats::default(),
            camera: crate::proving::street_camera(),
            size: [1, 1],
            ppp: 1.0,
            views_ready: 1,
            t0: Instant::now(),
            ui_ms: Vec::new(),
            character_asset,
            last_frame: None,
            saved: Vec::new(),
            edits: 0,
            overlay_empty: false,
            game: Some(game),
        };
        s.place_all(r, dev, content)?;
        let n = s.game.as_ref().map_or(0, Game::static_body_count);
        s.log(LogLevel::Info, format!("character: {} ({n} static collision bodies)", s.character_asset));
        Ok(s)
    }

    fn log(&mut self, level: LogLevel, text: String) {
        self.log.push(LogLine { time: self.t0.elapsed().as_secs_f64(), level, text });
    }

    /// Hot-reload lines for the editor log.
    pub fn log_hot(&mut self, ev: &crate::hot::HotEvents) {
        for (err, l) in crate::hot::lines(ev) {
            self.log(if err { LogLevel::Error } else { LogLevel::Info }, l);
        }
    }

    /// (Re)spawns the authored placements and the character.
    fn place_all(&mut self, r: &mut Renderer, dev: &dyn Device, content: &mut Content) -> Result<(), String> {
        let mut list: Vec<(ObjectId, AssetId, Affine3A, String)> =
            self.authored.places.iter().map(|p| (p.id, p.asset, self.placed.get(&p.id).map_or_else(|| p.affine(), |x| x.1), p.prefab.clone())).collect();
        let ch = self.game.as_ref().map_or(Affine3A::IDENTITY, |g| g.character_affine(1.0));
        list.push((CHARACTER, AssetId::from_path(&self.character_asset), ch, self.character_asset.clone()));
        content.set_authored(r, dev, &list)?;
        Ok(())
    }

    /// Current node-frame affine of an authored object.
    pub fn placed(&self, id: ObjectId) -> Option<Affine3A> {
        self.placed.get(&id).map(|x| x.1)
    }

    /// Fixed steps, character, camera, editor UI, overlay. Call before `begin_frame`.
    pub fn update(&mut self, r: &mut Renderer, dev: &dyn Device, content: &mut Content, inp: FrameIn) -> Result<FrameOut, String> {
        let now = Instant::now();
        if let Some(l) = self.last_frame {
            self.stats.frame_ms = (now - l).as_secs_f32() * 1e3;
        }
        self.last_frame = Some(now);
        self.size = inp.size;
        self.ppp = inp.pixels_per_point.max(0.5);
        // Pack collision changed (hot reload): rebuild the statics, keep the mover and objects.
        if content.collision_gen != self.collision_gen {
            self.collision_gen = content.collision_gen;
            if let Some(g) = &mut self.game {
                let list: Vec<(ObjectId, AssetId, Affine3A)> = self.placed.iter().map(|(id, (a, x))| (*id, *a, *x)).collect();
                g.rebuild(&content.collision, &list)?;
                let n = g.static_body_count();
                self.log(LogLevel::Info, format!("collision reloaded: {n} static bodies"));
            }
        }

        // Look (not sim state: it only steers the input and the camera).
        if self.mode == Mode::Game {
            self.yaw -= inp.look.x * LOOK_RAD_PER_PX;
            self.pitch = (self.pitch - inp.look.y * LOOK_RAD_PER_PX).clamp(-1.2, 0.6);
        }
        // One time boundary for the character and the fly camera.
        let game_in = match self.mode {
            Mode::Game => GameInput { yaw: self.yaw, ..inp.game },
            Mode::Editor => GameInput { yaw: self.yaw, ..Default::default() },
        };
        let cam_in = match self.mode {
            Mode::Editor => inp.cam,
            Mode::Game => CameraInput::default(),
        };
        let fly = self.mode == Mode::Editor;
        let look = inp.look;
        let mut both = Both { game: self.game.as_mut(), cam: &mut self.free, fly };
        let mut steps = 0;
        let alpha = self.fixed.run(inp.dt, &mut both, |i| {
            steps += 1;
            (game_in, if i == 0 { CameraInput { look, ..cam_in } } else { cam_in })
        });

        // Character instance.
        if let Some(g) = &self.game {
            content.move_object(r, CHARACTER, &g.character_affine(alpha));
        }
        let camera = match (inp.camera, self.mode, &self.game) {
            (Some(c), _, _) => c,
            (None, Mode::Game, Some(g)) => g.follow_camera(alpha, self.yaw, self.pitch),
            _ => self.free.interpolated(alpha),
        };
        self.camera = camera;

        // Editor.
        let mut out = FrameOut { camera, wants_pointer: false, wants_keyboard: false, steps };
        let mut gizmo = ed_editor::DrawList::default();
        if self.mode == Mode::Editor
            && let Some(mut ed) = self.editor.take()
        {
            let t0 = Instant::now();
            let mut host = EwHost {
                r,
                content,
                game: self.game.as_mut(),
                camera,
                vp: ViewportSize { size_px: self.size, pixels_per_point: self.ppp },
                root: &self.layer_root,
                stats: &self.stats,
                pick: &mut self.pick,
                log: &mut self.log,
            };
            let mut eo = ed.frame(&mut host, &EditorInput { time: inp.time, events: inp.events });
            self.editor = Some(ed);
            for (id, t) in &eo.transform_changes {
                if let Some((a, _)) = self.placed.get(id).copied() {
                    self.placed.insert(*id, (a, authored::node_affine(t)));
                }
            }
            self.edits += eo.transform_changes.len() as u64;
            for p in &eo.saved {
                let l = format!("saved {}", p.display());
                self.log(LogLevel::Info, l);
            }
            self.saved.extend(eo.saved.iter().cloned());
            if let Some(mut job) = eo.paint.take() {
                match &mut self.painter {
                    Some(p) => {
                        p.prepare(dev, &mut job).map_err(|e| format!("egui prepare: {e}"))?;
                        self.painted = true;
                        job.discard();
                    }
                    None => job.discard(),
                }
            }
            gizmo = eo.gizmo;
            out.wants_pointer = eo.wants_pointer && self.editor_ui;
            out.wants_keyboard = eo.wants_keyboard && self.editor_ui;
            self.ui_ms.push(t0.elapsed().as_secs_f64() * 1e3);
        }

        // Overlay: gizmo (camera-relative already) + collision debug lines.
        let conv = |v: &ed_editor::DrawVertex| OverlayVertex { pos: v.pos, color: v.color };
        let mut lines: Vec<OverlayVertex> = gizmo.lines.iter().map(conv).collect();
        let tris: Vec<OverlayVertex> = gizmo.triangles.iter().map(conv).collect();
        if self.collision_debug
            && let Some(g) = &self.game
        {
            let room = (ed_render::MAX_OVERLAY_VERTICES as usize).saturating_sub(lines.len() + tris.len()) / 2;
            lines.extend(g.debug_lines(g.feet(alpha), DEBUG_RADIUS_M, camera.position, room));
        }
        // An empty batch is only sent once (no per-frame overlay refill when nothing is drawn).
        if !(lines.is_empty() && tris.is_empty() && self.overlay_empty) {
            r.set_overlay(camera.position, &lines, &tris);
            self.overlay_empty = lines.is_empty() && tris.is_empty();
        }

        if self.inset && self.views_ready < 2 {
            r.prepare_views(dev, 2).map_err(|e| format!("prepare_views: {e}"))?;
            self.views_ready = 2;
        }
        Ok(out)
    }

    /// Whether this frame paints the editor UI over the main view.
    pub fn paints_ui(&self) -> bool {
        self.mode == Mode::Editor && self.painted && self.painter.is_some()
    }

    /// The top-down inset camera (60 m above the character, looking straight down).
    pub fn inset_camera(&self) -> Camera {
        let feet = self.game.as_ref().map_or(DVec3::ZERO, |g| crate::collide::enu_to_yup(g.cur));
        Camera { position: ORIGIN + feet + DVec3::Y * 60.0, yaw: 0.0, pitch: -1.5, fov_y: 1.0, near: 0.5 }
    }

    /// Records the frame's views into `target`: main view, then the inset (slot 1, its own
    /// culling), then the editor UI. `readback` copies the final image (UI included).
    pub fn record(&mut self, r: &mut Renderer, dev: &dyn Device, enc: &mut dyn CommandEncoder, target: ViewTarget, mut params: FrameParams, readback: Option<BufferId>) -> Result<(), String> {
        let ui = self.paints_ui();
        let inset = self.inset && self.views_ready >= 2;
        let extra = ui || inset;
        if let (Some(px), None, None) = (self.pick.pixel, self.pick.sent, self.pick.answer) {
            params.pick = Some((px[0], px[1]));
            self.pick.sent = Some(enc.frame_index());
        }
        let (w, h) = (target.width, target.height);
        let main = View {
            camera: self.camera,
            viewport: Viewport { x: 0, y: 0, width: w, height: h },
            target: ViewTarget { final_state: if extra { Some(ResourceState::RenderTarget) } else { target.final_state }, ..target },
            flags: ViewFlags::default(),
        };
        // The renderer copies out unless the UI is painted after it.
        let rb_renderer = if ui { None } else { readback };
        r.render(dev, enc, &main, &FrameParams { readback: if inset { None } else { rb_renderer }, ..params }).map_err(|e| format!("render: {e}"))?;
        if inset {
            let side = (h / 3).min(w / 3).max(16);
            let v = View {
                camera: self.inset_camera(),
                viewport: Viewport { x: w.saturating_sub(side + 16), y: 16, width: side, height: side },
                target: ViewTarget { initial: ResourceState::RenderTarget, final_state: Some(ResourceState::RenderTarget), ..target },
                flags: ViewFlags { slot: 1, keep_output: true, overlay: false },
            };
            r.render(dev, enc, &v, &FrameParams { readback: rb_renderer, pick: None, pop_stats: false, vis_readback: None, ..params }).map_err(|e| format!("render inset: {e}"))?;
        }
        // With `extra`, every view above ended in RenderTarget.
        let mut state = ResourceState::RenderTarget;
        if ui && let Some(p) = &mut self.painter {
            p.paint(dev, enc, &PaintTarget { texture: target.texture, format: target.format, width: w, height: h, load: LoadOp::Load }).map_err(|e| format!("egui paint: {e}"))?;
            if let Some(rb) = readback {
                enc.transition(&[Transition::Texture { texture: target.texture, sub: None, from: state, to: ResourceState::CopySrc }]);
                enc.copy_texture_to_buffer(target.texture, Subresource { mip: 0, layer: 0 }, rb, 0, target.format.copy_row_pitch(w));
                state = ResourceState::CopySrc;
            }
        }
        if extra
            && let Some(f) = target.final_state
            && f != state
        {
            enc.transition(&[Transition::Texture { texture: target.texture, sub: None, from: state, to: f }]);
        }
        Ok(())
    }

    /// After a frame: answers the editor's pending pick, refreshes the counters.
    pub fn after_frame(&mut self, r: &Renderer, dev: &dyn Device) {
        if let (Some(px), Some(sent)) = (self.pick.pixel, self.pick.sent)
            && let Some(p) = r.pick_result_for(0)
            && p.frame >= sent
            && [p.x, p.y] == px
        {
            self.pick.answer = Some(match p.object {
                Some(o) => PickState::Hit(o),
                None => PickState::Miss,
            });
            self.pick.sent = None;
        }
        if let Some(c) = r.cull_stats_for(0) {
            self.stats.instances = c.total;
            self.stats.visible_instances = c.drawn();
        }
        self.stats.frame += 1;
        self.stats.gpu_memory_bytes = dev.allocated_bytes();
    }

    /// GPU pass timings for the stats panel (the caller owns `Renderer::timings`).
    pub fn set_timings(&mut self, t: &ed_render::graph::FrameTimings) {
        self.stats.gpu_ms = t.frame_ms;
        self.stats.passes = t.passes.iter().map(|p| PassTiming { label: p.name.to_string(), depth: 1, gpu_ms: p.ms }).collect();
    }

    /// CPU frame-build time for the stats panel.
    pub fn set_cpu_ms(&mut self, ms: f64) {
        self.stats.cpu_ms = ms as f32;
    }

    /// (mean, p99) of [`Session::ui_ms`].
    pub fn ui_stats(&self) -> (f64, f64) {
        if self.ui_ms.is_empty() {
            return (0.0, 0.0);
        }
        let mut v = self.ui_ms.clone();
        v.sort_by(f64::total_cmp);
        (v.iter().sum::<f64>() / v.len() as f64, v[((v.len() - 1) as f64 * 0.99).round() as usize])
    }

    pub fn destroy(&mut self, dev: &dyn Device) {
        if let Some(p) = self.painter.take() {
            p.destroy(dev);
        }
    }
}

/// The character and the fly camera on one fixed-step boundary.
struct Both<'a> {
    game: Option<&'a mut Game>,
    cam: &'a mut FreeCamera,
    fly: bool,
}

impl Simulation for Both<'_> {
    type Input = (GameInput, CameraInput);
    fn step(&mut self, input: &Self::Input, dt: f64) {
        if let Some(g) = &mut self.game {
            g.step(&input.0, dt);
        }
        if self.fly {
            self.cam.step(&input.1, dt);
        } else {
            self.cam.prev = self.cam.cur;
        }
    }
}

/// [`EditorHost`] over the running app: renderer instances, pack content, the sim.
struct EwHost<'a> {
    r: &'a mut Renderer,
    content: &'a mut Content,
    game: Option<&'a mut Game>,
    camera: Camera,
    vp: ViewportSize,
    root: &'a PathBuf,
    stats: &'a FrameStats,
    pick: &'a mut PickSlot,
    log: &'a mut Vec<LogLine>,
}

impl EditorHost for EwHost<'_> {
    fn pick(&mut self, pixel: [u32; 2]) -> PickState {
        if self.pick.pixel == Some(pixel)
            && let Some(a) = self.pick.answer.take()
        {
            self.pick.pixel = None;
            return a;
        }
        if self.pick.pixel != Some(pixel) {
            *self.pick = PickSlot { pixel: Some(pixel), sent: None, answer: None };
        }
        PickState::Pending
    }

    fn object_info(&self, id: ObjectId) -> Option<ObjectInfo> {
        let d = self.content.object(id)?;
        Some(ObjectInfo {
            object: id,
            node: Some(proving_node()),
            node_origin: Some(ORIGIN),
            prefab: if d.authored { d.asset_name.clone() } else { None },
            part: None,
            mesh: d.asset_name.clone().or_else(|| Some(format!("{:?}", d.asset))),
            material: d.material.map(|m| format!("{m:?}")),
            lod: None,
            transform: authored::editor_transform(&d.transform),
        })
    }

    fn set_object_transform(&mut self, id: ObjectId, transform: &Transform) {
        if id == CHARACTER {
            return;
        }
        let a = authored::node_affine(transform);
        if self.content.move_object(self.r, id, &a)
            && let (Some(g), Some(d)) = (self.game.as_deref_mut(), self.content.object(id))
            && let Err(e) = g.set_object(id, d.asset, &a)
        {
            self.log.push(LogLine { time: 0.0, level: LogLevel::Error, text: format!("collision for {id:?}: {e}") });
        }
    }

    fn camera(&self) -> Camera {
        self.camera
    }

    fn viewport(&self) -> ViewportSize {
        self.vp
    }

    fn enu_to_world(&self, _position: DVec3) -> DQuat {
        ed_editor::transform::flat_y_up_enu()
    }

    fn layer_root(&self) -> Option<PathBuf> {
        Some(self.root.clone())
    }

    fn bodies(&self) -> Vec<(String, BodyId)> {
        authored::bodies()
    }

    fn frame_stats(&self) -> FrameStats {
        self.stats.clone()
    }

    fn drain_log(&mut self, out: &mut Vec<LogLine>) {
        out.append(self.log);
    }
}
