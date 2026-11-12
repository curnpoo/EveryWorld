//! The editor loop: what the app must provide ([`EditorHost`]), what it gets back
//! ([`EditorOutput`]), and [`Editor::frame`] tying input, picking, the gizmo, undo, layers and
//! the panels together.

use std::collections::VecDeque;
use std::path::PathBuf;

use ed_geo::{BodyId, NodeId, ObjectId};
use ed_render::Camera;
use glam::{DQuat, DVec2, DVec3};

use crate::gizmo::{Delta, Drag, DrawList, Gizmo, Handle, Mode, Space, ViewMath};
use crate::layerset::{AuthoredKind, LayerSet, OpPose};
use crate::model::{Command, ObjectEdit, Selection, Snap, UndoStack};
use crate::painter::PaintJob;
use crate::panels;
use crate::transform::{EnuFrame, Transform, enu_rotation, world_rotation};

// ---------------------------------------------------------------------------------------------
// Host interface

/// Answer to [`EditorHost::pick`]. Picking reads back a GPU id target, so the answer is a
/// few frames late (`rhi.md`: readback).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickState {
    /// Asked; no answer yet. The editor asks again next frame with the same pixel.
    Pending,
    Hit(ObjectId),
    /// Sky or nothing pickable under the pixel.
    Miss,
}

/// What the host knows about an object (for the inspector and the gizmo).
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectInfo {
    pub object: ObjectId,
    /// The node the object's instance lives in.
    pub node: Option<NodeId>,
    /// That node's f64 origin; the inspector shows the position relative to it.
    pub node_origin: Option<DVec3>,
    pub prefab: Option<String>,
    /// Part index inside the prefab.
    pub part: Option<u32>,
    pub mesh: Option<String>,
    pub material: Option<String>,
    pub lod: Option<u32>,
    pub transform: Transform,
}

/// Size of the view the editor draws over, physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportSize {
    pub size_px: [u32; 2],
    pub pixels_per_point: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PassTiming {
    pub label: String,
    /// Nesting depth of the timer scope.
    pub depth: u32,
    pub gpu_ms: f32,
}

/// Per-frame numbers the host measures, fed to the stats panels as plain data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    pub frame: u64,
    /// Wall time between frames.
    pub frame_ms: f32,
    pub cpu_ms: f32,
    /// Sum of top-level GPU timer scopes (a few frames late).
    pub gpu_ms: f32,
    pub passes: Vec<PassTiming>,
    pub instances: u32,
    pub visible_instances: u32,
    pub gpu_memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// One line of the hot-reload / editor log.
#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    /// Seconds, on the host's clock.
    pub time: f64,
    pub level: LogLevel,
    pub text: String,
}

/// What the app (`ew`) provides to the editor.
pub trait EditorHost {
    /// The object under a physical pixel (origin top-left). Called once per frame with the
    /// same pixel until the answer is not `Pending`; the host issues its GPU pick request on
    /// the first call and answers when the readback for that pixel has arrived.
    fn pick(&mut self, pixel: [u32; 2]) -> PickState;
    /// What the host knows about an object, including its current transform.
    fn object_info(&self, id: ObjectId) -> Option<ObjectInfo>;
    /// Move an object's instance(s) now. Every change is also listed in
    /// [`EditorOutput::transform_changes`].
    fn set_object_transform(&mut self, id: ObjectId, transform: &Transform);
    /// The camera of the view the editor works in.
    fn camera(&self) -> Camera;
    fn viewport(&self) -> ViewportSize;
    /// Rotation mapping local east, north, up at `position` to world axes (layer rotations
    /// are ENU-relative; `transform::flat_y_up_enu` for the flat proving ground).
    fn enu_to_world(&self, position: DVec3) -> DQuat;
    /// The `layers/` directory (holding `layers.toml`); `None` runs without layers.
    fn layer_root(&self) -> Option<PathBuf>;
    /// Body catalog names for layer file paths (`earth`, ...).
    fn bodies(&self) -> Vec<(String, BodyId)>;
    fn frame_stats(&self) -> FrameStats;
    /// Hot-reload and other log lines since the last call.
    fn drain_log(&mut self, _out: &mut Vec<LogLine>) {}
}

struct HostFrame<'a>(&'a dyn EditorHost);
impl EnuFrame for HostFrame<'_> {
    fn enu_to_world(&self, position: DVec3) -> DQuat {
        self.0.enu_to_world(position)
    }
}

// ---------------------------------------------------------------------------------------------
// Input and output

/// Input for one frame, in physical pixels. Keys and modifiers use egui's types
/// (`ed_editor::egui`), so the host maps its window events once.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    PointerMoved([f32; 2]),
    PointerButton {
        button: egui::PointerButton,
        pressed: bool,
    },
    PointerGone,
    /// Scroll in lines.
    Wheel([f32; 2]),
    Key {
        key: egui::Key,
        pressed: bool,
        repeat: bool,
    },
    Text(String),
    Modifiers(egui::Modifiers),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorInput {
    /// Seconds on any monotonic clock (UI only; never the sim).
    pub time: f64,
    pub events: Vec<InputEvent>,
}

/// What one editor frame produced.
#[derive(Debug, Default)]
pub struct EditorOutput {
    /// A new UI frame to paint. `None`: nothing changed, keep painting the previous one
    /// (`Painter::paint` with the last job, or `Painter::repaint`).
    pub paint: Option<PaintJob>,
    /// Gizmo lines and triangles, camera-relative f32, for the overlay pass (no depth test).
    pub gizmo: DrawList,
    /// Transforms applied this frame through [`EditorHost::set_object_transform`], in order.
    pub transform_changes: Vec<(ObjectId, Transform)>,
    /// The UI has the pointer or keyboard: the host should not fly the camera with them.
    pub wants_pointer: bool,
    pub wants_keyboard: bool,
    /// Files written or removed by a save this frame.
    pub saved: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------------------------
// State the panels read and write

/// Requests from the panels, carried out after the UI pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Save,
    Undo,
    Redo,
    ReloadLayers,
    ClearSelection,
}

/// The inspector's view of one selected object.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspected {
    pub object: ObjectId,
    pub info: Option<ObjectInfo>,
    /// Transform the editor uses (the layer's when the object is authored).
    pub transform: Option<Transform>,
    /// Rotation relative to local east, north, up.
    pub enu_rotation: Option<DQuat>,
    pub layer: Option<String>,
    pub layer_node: Option<NodeId>,
    pub authored: Option<AuthoredKind>,
    /// Only authored `place` and `marker` objects can be moved in stage E1.
    pub movable: bool,
}

impl Inspected {
    /// f64 position relative to the instance's node origin.
    pub fn node_relative(&self) -> Option<DVec3> {
        Some(self.transform?.position - self.info.as_ref()?.node_origin?)
    }
}

/// Everything the panels show or change.
#[derive(Debug, Default)]
pub struct EditorState {
    pub selection: Selection,
    pub mode: Mode,
    pub space: Space,
    pub snap: Snap,
    pub undo: UndoStack,
    pub layers: Option<LayerSet>,
    pub inspected: Vec<Inspected>,
    pub log: VecDeque<LogLine>,
    pub actions: Vec<Action>,
    /// Stats as last refreshed (the panels update at [`STATS_HZ`], not every frame).
    pub stats: FrameStats,
    pub time: f64,
    /// Host-set banner (current mode and key hints), drawn top-centre when non-empty.
    pub hint: String,
}

pub const LOG_LINES: usize = 200;
/// How often the stats panels refresh (and so how often an idle editor re-runs egui).
pub const STATS_HZ: f64 = 4.0;

impl EditorState {
    pub fn log(&mut self, level: LogLevel, text: impl Into<String>) {
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(LogLine {
            time: self.time,
            level,
            text: text.into(),
        });
    }
}

// ---------------------------------------------------------------------------------------------
// The editor

struct ActiveDrag {
    drag: Drag,
    id: u64,
    starts: Vec<(ObjectId, Transform, Option<OpPose>)>,
}

pub struct Editor {
    pub state: EditorState,
    ctx: egui::Context,
    pointer: Option<DVec2>,
    modifiers: egui::Modifiers,
    pending_pick: Option<([u32; 2], bool)>,
    drag: Option<ActiveDrag>,
    drags: u64,
    hover: Option<Handle>,
    next_repaint: f64,
    next_stats: f64,
    ui_dirty: bool,
    loaded: bool,
    /// egui passes run (for tests and the redraw-limit check).
    pub ui_passes: u64,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            state: EditorState::default(),
            ctx: egui::Context::default(),
            pointer: None,
            modifiers: egui::Modifiers::default(),
            pending_pick: None,
            drag: None,
            drags: 0,
            hover: None,
            next_repaint: 0.0,
            next_stats: 0.0,
            ui_dirty: true,
            loaded: false,
            ui_passes: 0,
        }
    }

    pub fn egui_ctx(&self) -> &egui::Context {
        &self.ctx
    }

    /// (Re)load layers from the host's layer root. Undo history is dropped: it refers to the
    /// old file contents.
    pub fn load_layers(&mut self, host: &dyn EditorHost) {
        self.loaded = true;
        self.state.undo = UndoStack::default();
        self.state.layers = None;
        let Some(root) = host.layer_root() else {
            return;
        };
        match LayerSet::load(&root, host.bodies()) {
            Ok(set) => {
                let n = set.objects().count();
                self.state.log(
                    LogLevel::Info,
                    format!(
                        "layers: {} layers, {n} authored objects from {}",
                        set.layers().len(),
                        root.display()
                    ),
                );
                self.state.layers = Some(set);
            }
            Err(e) => {
                self.state.log(LogLevel::Error, format!("layers: {e}"));
                self.state.layers = Some(LayerSet::empty(root, host.bodies()));
            }
        }
        self.ui_dirty = true;
    }

    /// One frame: input, picking, gizmo, UI. Call once per rendered frame.
    pub fn frame(&mut self, host: &mut dyn EditorHost, input: &EditorInput) -> EditorOutput {
        let mut out = EditorOutput::default();
        self.state.time = input.time;
        if !self.loaded {
            self.load_layers(host);
        }
        let vp = host.viewport();
        let view = ViewMath::new(&host.camera(), vp.size_px);

        // Host log and stats (rate-limited).
        let mut lines = Vec::new();
        host.drain_log(&mut lines);
        if !lines.is_empty() {
            for l in lines {
                if self.state.log.len() == LOG_LINES {
                    self.state.log.pop_front();
                }
                self.state.log.push_back(l);
            }
            self.ui_dirty = true;
        }
        if input.time >= self.next_stats {
            self.state.stats = host.frame_stats();
            self.next_stats = input.time + 1.0 / STATS_HZ;
            self.ui_dirty = true;
        }

        // Scene input, using the previous UI pass's idea of who owns the pointer.
        let ui_pointer = self.ctx.egui_wants_pointer_input() || self.ctx.egui_is_using_pointer();
        let ui_keyboard = self.ctx.egui_wants_keyboard_input();
        for ev in &input.events {
            self.scene_event(host, &view, ev, ui_pointer, ui_keyboard, &mut out);
        }

        // Pick answer.
        if let Some((px, additive)) = self.pending_pick {
            match host.pick(px) {
                PickState::Pending => {}
                r => {
                    self.pending_pick = None;
                    match (r, additive) {
                        (PickState::Hit(id), false) => self.state.selection.set(id),
                        (PickState::Hit(id), true) => self.state.selection.toggle(id),
                        (_, false) => self.state.selection.clear(),
                        _ => {}
                    }
                    self.ui_dirty = true;
                }
            }
        }

        self.refresh_inspected(host);

        // UI pass, only when something changed.
        let run_ui = self.ui_dirty || !input.events.is_empty() || input.time >= self.next_repaint;
        if run_ui {
            out.paint = Some(self.run_ui(input, vp));
            self.ui_dirty = false;
        }
        let actions = std::mem::take(&mut self.state.actions);
        for a in actions {
            self.action(host, a, &mut out);
            self.ui_dirty = true;
        }
        out.wants_pointer = self.ctx.egui_wants_pointer_input() || self.ctx.egui_is_using_pointer();
        out.wants_keyboard = self.ctx.egui_wants_keyboard_input();

        // Gizmo.
        if let Some(g) = self.gizmo(host, &view) {
            if self.drag.is_none() {
                let hover = if out.wants_pointer {
                    None
                } else {
                    self.pointer.and_then(|p| g.hit(&view, p))
                };
                if hover != self.hover {
                    self.hover = hover;
                }
            }
            let hl = self.drag.as_ref().map(|d| d.drag.handle).or(self.hover);
            g.draw(&view, hl, &mut out.gizmo);
        } else {
            self.hover = None;
        }
        out
    }

    fn run_ui(&mut self, input: &EditorInput, vp: ViewportSize) -> PaintJob {
        let ppp = vp.pixels_per_point.max(0.25);
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(vp.size_px[0] as f32 / ppp, vp.size_px[1] as f32 / ppp),
            )),
            time: Some(input.time),
            focused: true,
            ..Default::default()
        };
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);
        let mut mods = self.modifiers;
        // Pointer position as of each event (button events carry the position at that time).
        let mut ptr = self.ctx.input(|i| i.pointer.latest_pos());
        for ev in &input.events {
            let to_pos = |p: [f32; 2]| egui::pos2(p[0] / ppp, p[1] / ppp);
            match ev {
                InputEvent::PointerMoved(p) => {
                    ptr = Some(to_pos(*p));
                    raw.events.push(egui::Event::PointerMoved(to_pos(*p)));
                }
                InputEvent::PointerButton { button, pressed } => {
                    if let Some(pos) = ptr {
                        raw.events.push(egui::Event::PointerButton {
                            pos,
                            button: *button,
                            pressed: *pressed,
                            modifiers: mods,
                        });
                    }
                }
                InputEvent::PointerGone => {
                    ptr = None;
                    raw.events.push(egui::Event::PointerGone);
                }
                InputEvent::Wheel(d) => raw.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(d[0], d[1]),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: mods,
                }),
                InputEvent::Key {
                    key,
                    pressed,
                    repeat,
                } => raw.events.push(egui::Event::Key {
                    key: *key,
                    physical_key: None,
                    pressed: *pressed,
                    repeat: *repeat,
                    modifiers: mods,
                }),
                InputEvent::Text(t) => raw.events.push(egui::Event::Text(t.clone())),
                InputEvent::Modifiers(m) => mods = *m,
            }
        }
        let stats = self.state.stats.clone();
        let state = &mut self.state;
        let full = self.ctx.run_pass(raw, |ctx| panels::ui(state, ctx, &stats));
        self.ui_passes += 1;
        let delay = full
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.repaint_delay.as_secs_f64())
            .unwrap_or(f64::INFINITY);
        self.next_repaint = if delay.is_finite() {
            input.time + delay
        } else {
            f64::INFINITY
        };
        let primitives = self.ctx.tessellate(full.shapes, full.pixels_per_point);
        PaintJob {
            primitives,
            textures: full.textures_delta,
            pixels_per_point: full.pixels_per_point,
        }
    }

    fn scene_event(
        &mut self,
        host: &mut dyn EditorHost,
        view: &ViewMath,
        ev: &InputEvent,
        ui_pointer: bool,
        ui_keyboard: bool,
        out: &mut EditorOutput,
    ) {
        match ev {
            InputEvent::Modifiers(m) => self.modifiers = *m,
            InputEvent::PointerMoved(p) => {
                self.pointer = Some(DVec2::new(p[0] as f64, p[1] as f64));
                if self.drag.is_some() {
                    self.drag_update(host, view, out);
                }
            }
            InputEvent::PointerGone => self.pointer = None,
            InputEvent::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: true,
            } => {
                let Some(p) = self.pointer else { return };
                if ui_pointer {
                    return;
                }
                let handle = self
                    .gizmo(host, view)
                    .and_then(|g| g.hit(view, p).map(|h| (g, h)));
                if let Some((g, h)) = handle {
                    self.drag_begin(host, view, g, h, p);
                } else {
                    let px = [p.x.max(0.0) as u32, p.y.max(0.0) as u32];
                    self.pending_pick = Some((px, self.modifiers.shift));
                }
            }
            InputEvent::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: false,
            } => self.drag_end(),
            InputEvent::Key {
                key, pressed: true, ..
            } if !ui_keyboard => {
                let cmd = self.modifiers.command;
                match key {
                    egui::Key::Z if cmd && self.modifiers.shift => self.redo(host, out),
                    egui::Key::Z if cmd => self.undo(host, out),
                    egui::Key::Y if cmd => self.redo(host, out),
                    egui::Key::S if cmd => self.save(out),
                    egui::Key::W if !cmd => self.state.mode = Mode::Translate,
                    egui::Key::E if !cmd => self.state.mode = Mode::Rotate,
                    egui::Key::R if !cmd => self.state.mode = Mode::Scale,
                    egui::Key::G if !cmd => self.state.snap.enabled = !self.state.snap.enabled,
                    egui::Key::Escape => {
                        if self.drag.is_some() {
                            self.drag_cancel(host, out);
                        } else {
                            self.state.selection.clear();
                        }
                    }
                    _ => return,
                }
                self.ui_dirty = true;
            }
            _ => {}
        }
    }

    /// The transform the editor works with: the layer's for authored objects, else the host's.
    fn transform_of(&self, host: &dyn EditorHost, id: ObjectId) -> Option<Transform> {
        if let Some(t) = self
            .state
            .layers
            .as_ref()
            .and_then(|l| l.transform(id, &HostFrame(host)))
        {
            return Some(t);
        }
        host.object_info(id).map(|i| i.transform)
    }

    fn movable(&self, id: ObjectId) -> bool {
        self.state
            .layers
            .as_ref()
            .is_some_and(|l| l.locate(id).is_some())
    }

    fn gizmo(&self, host: &dyn EditorHost, view: &ViewMath) -> Option<Gizmo> {
        if let Some(d) = &self.drag {
            // Keep the grabbed frame for the whole drag; follow the object's position.
            let id = d.starts.last()?.0;
            let t = self.transform_of(host, id)?;
            return Some(Gizmo {
                origin: t.position,
                ..d.drag.gizmo
            });
        }
        let id = self.state.selection.primary()?;
        if !self.movable(id) {
            return None;
        }
        let t = self.transform_of(host, id)?;
        let frame = match self.state.space {
            Space::Enu => host.enu_to_world(t.position),
            Space::Object => t.rotation,
        };
        Some(Gizmo::new(self.state.mode, t.position, frame, view))
    }

    fn drag_begin(
        &mut self,
        host: &dyn EditorHost,
        view: &ViewMath,
        g: Gizmo,
        h: Handle,
        p: DVec2,
    ) {
        let Some(drag) = Drag::begin(g, h, view, &view.ray(p)) else {
            return;
        };
        // The primary object goes last so the gizmo follows it.
        let mut ids: Vec<ObjectId> = self
            .state
            .selection
            .items()
            .iter()
            .copied()
            .filter(|&id| self.movable(id))
            .collect();
        if let Some(pr) = self.state.selection.primary() {
            ids.retain(|&i| i != pr);
            ids.push(pr);
        }
        let layers = self.state.layers.as_ref();
        let starts = ids
            .into_iter()
            .filter_map(|id| {
                Some((
                    id,
                    self.transform_of(host, id)?,
                    layers.and_then(|l| l.pose(id)),
                ))
            })
            .collect::<Vec<_>>();
        if starts.is_empty() {
            return;
        }
        self.drags += 1;
        self.drag = Some(ActiveDrag {
            drag,
            id: self.drags,
            starts,
        });
    }

    fn drag_update(&mut self, host: &mut dyn EditorHost, view: &ViewMath, out: &mut EditorOutput) {
        let (Some(d), Some(p)) = (&self.drag, self.pointer) else {
            return;
        };
        let Some(delta) = d.drag.update(view, &view.ray(p), &self.state.snap) else {
            return;
        };
        let starts = d.starts.clone();
        let label = match delta {
            Delta::Translate(_) => "move",
            Delta::Rotate(..) => "rotate",
            Delta::Scale(_) => "scale",
        };
        let id = d.id;
        let edits = {
            let frame = HostFrame(&*host);
            let upright = |from: DVec3, to: DVec3, q: DQuat| {
                world_rotation(&frame, to, enu_rotation(&frame, from, q))
            };
            starts
                .iter()
                .map(|(obj, t0, pose0)| {
                    let t = delta.apply(t0, &upright);
                    let after_pose = self
                        .state
                        .layers
                        .as_ref()
                        .and_then(|l| l.pose_for(*obj, &t, &frame));
                    ObjectEdit {
                        object: *obj,
                        before: *t0,
                        after: t,
                        before_pose: pose0.clone(),
                        after_pose,
                    }
                })
                .collect::<Vec<_>>()
        };
        for e in &edits {
            self.apply_one(host, e.object, &e.after, e.after_pose.as_ref(), out);
        }
        self.state.undo.push(Command {
            label,
            edits,
            merge: Some(id),
        });
        self.ui_dirty = true;
    }

    fn drag_end(&mut self) {
        if let Some(d) = self.drag.take() {
            if self.state.undo.top_merge() == Some(d.id) {
                self.state.undo.drop_noop_top();
                self.state.undo.seal();
            }
            self.ui_dirty = true;
        }
    }

    fn drag_cancel(&mut self, host: &mut dyn EditorHost, out: &mut EditorOutput) {
        let Some(d) = self.drag.take() else { return };
        // The drag's command is on top only if the drag moved something.
        if self.state.undo.top_merge() == Some(d.id) {
            self.state.undo.discard_top();
        }
        for (id, t0, pose0) in d.starts {
            self.apply_one(host, id, &t0, pose0.as_ref(), out);
        }
    }

    fn apply_one(
        &mut self,
        host: &mut dyn EditorHost,
        id: ObjectId,
        t: &Transform,
        pose: Option<&OpPose>,
        out: &mut EditorOutput,
    ) {
        if let (Some(l), Some(p)) = (self.state.layers.as_mut(), pose) {
            l.set_pose(id, p);
        }
        host.set_object_transform(id, t);
        out.transform_changes.push((id, *t));
    }

    pub fn undo(&mut self, host: &mut dyn EditorHost, out: &mut EditorOutput) {
        self.drag = None;
        let Some(c) = self.state.undo.undo().cloned() else {
            return;
        };
        for e in c.edits.iter().rev() {
            self.apply_one(host, e.object, &e.before, e.before_pose.as_ref(), out);
        }
        self.state.log(LogLevel::Info, format!("undo {}", c.label));
    }

    pub fn redo(&mut self, host: &mut dyn EditorHost, out: &mut EditorOutput) {
        self.drag = None;
        let Some(c) = self.state.undo.redo().cloned() else {
            return;
        };
        for e in &c.edits {
            self.apply_one(host, e.object, &e.after, e.after_pose.as_ref(), out);
        }
        self.state.log(LogLevel::Info, format!("redo {}", c.label));
    }

    pub fn save(&mut self, out: &mut EditorOutput) {
        let Some(l) = self.state.layers.as_mut() else {
            return;
        };
        match l.save() {
            Ok(paths) => {
                let msg = format!("saved {} file(s)", paths.len());
                out.saved.extend(paths);
                self.state.log(LogLevel::Info, msg);
            }
            Err(e) => self.state.log(LogLevel::Error, format!("save failed: {e}")),
        }
    }

    fn action(&mut self, host: &mut dyn EditorHost, a: Action, out: &mut EditorOutput) {
        match a {
            Action::Save => self.save(out),
            Action::Undo => self.undo(host, out),
            Action::Redo => self.redo(host, out),
            Action::ReloadLayers => self.load_layers(host),
            Action::ClearSelection => self.state.selection.clear(),
        }
    }

    fn refresh_inspected(&mut self, host: &dyn EditorHost) {
        let frame = HostFrame(host);
        let layers = self.state.layers.as_ref();
        self.state.inspected = self
            .state
            .selection
            .items()
            .iter()
            .map(|&id| {
                let info = host.object_info(id);
                let loc = layers.and_then(|l| l.locate(id));
                let transform = layers
                    .and_then(|l| l.transform(id, &frame))
                    .or(info.as_ref().map(|i| i.transform));
                Inspected {
                    object: id,
                    enu_rotation: transform.map(|t| enu_rotation(&frame, t.position, t.rotation)),
                    transform,
                    info,
                    layer: loc.and_then(|l| layers.map(|s| s.layers()[l.layer].name.clone())),
                    layer_node: loc.map(|l| l.node),
                    authored: layers.and_then(|l| l.kind(id)),
                    movable: loc.is_some(),
                }
            })
            .collect();
    }

    /// True while a gizmo drag is in progress.
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    /// True while a pick request is waiting for its answer.
    pub fn picking(&self) -> bool {
        self.pending_pick.is_some()
    }
}
