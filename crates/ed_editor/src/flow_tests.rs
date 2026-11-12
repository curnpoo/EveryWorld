//! Full editor flow against a mock host: pick -> drag -> applied -> undo -> redo -> save ->
//! reload, plus the redraw limit. (A crate test rather than `tests/` so it can share the
//! canonical layer fixture in `layerset::tests`.)

use std::collections::BTreeMap;
use std::path::PathBuf;

use ed_geo::{BodyId, ObjectId};
use ed_render::Camera;
use glam::{DQuat, DVec2, DVec3};

use crate::editor::*;
use crate::gizmo::{Gizmo, Handle, Mode, ViewMath};
use crate::layerset::LayerSet;
use crate::layerset::tests::{POS, bodies, id, sample_root};
use crate::transform::{ConstFrame, Transform, flat_y_up_enu};

const SIZE: [u32; 2] = [1600, 900];

struct MockHost {
    root: PathBuf,
    camera: Camera,
    transforms: BTreeMap<ObjectId, Transform>,
    pick_calls: u32,
    picks: u32,
}

impl MockHost {
    fn new(root: PathBuf) -> Self {
        // 15 m south of and 5 m above the bench, looking at it.
        let target = DVec3::from_array(POS);
        let eye = target + DVec3::new(0.0, 5.0, 15.0);
        let pitch = -(5.0f64 / 15.0).atan() as f32;
        let camera = Camera {
            position: eye,
            yaw: 0.0,
            pitch,
            fov_y: 60f32.to_radians(),
            near: 0.1,
        };
        let mut transforms = BTreeMap::new();
        transforms.insert(id(1), Transform::from_position(target));
        MockHost {
            root,
            camera,
            transforms,
            pick_calls: 0,
            picks: 0,
        }
    }
    fn view(&self) -> ViewMath {
        ViewMath::new(&self.camera, SIZE)
    }
}

impl EditorHost for MockHost {
    fn pick(&mut self, pixel: [u32; 2]) -> PickState {
        // One frame of latency, like a GPU readback.
        self.pick_calls += 1;
        if self.pick_calls % 2 == 1 {
            return PickState::Pending;
        }
        self.picks += 1;
        let view = self.view();
        let p = DVec2::new(pixel[0] as f64, pixel[1] as f64);
        for (&id, t) in &self.transforms {
            if view
                .project(t.position)
                .is_some_and(|q| q.distance(p) < 20.0)
            {
                return PickState::Hit(id);
            }
        }
        PickState::Miss
    }
    fn object_info(&self, id: ObjectId) -> Option<ObjectInfo> {
        let t = *self.transforms.get(&id)?;
        Some(ObjectInfo {
            object: id,
            node: None,
            node_origin: None,
            prefab: Some("kit/bench".into()),
            part: Some(0),
            mesh: Some("bench".into()),
            material: Some("wood".into()),
            lod: Some(0),
            transform: t,
        })
    }
    fn set_object_transform(&mut self, id: ObjectId, transform: &Transform) {
        self.transforms.insert(id, *transform);
    }
    fn camera(&self) -> Camera {
        self.camera
    }
    fn viewport(&self) -> ViewportSize {
        ViewportSize {
            size_px: SIZE,
            pixels_per_point: 1.0,
        }
    }
    fn enu_to_world(&self, _position: DVec3) -> DQuat {
        flat_y_up_enu()
    }
    fn layer_root(&self) -> Option<PathBuf> {
        Some(self.root.clone())
    }
    fn bodies(&self) -> Vec<(String, BodyId)> {
        bodies()
    }
    fn frame_stats(&self) -> FrameStats {
        FrameStats {
            frame: 1,
            frame_ms: 16.6,
            cpu_ms: 4.0,
            gpu_ms: 9.0,
            ..Default::default()
        }
    }
}

struct Driver {
    ed: Editor,
    host: MockHost,
    time: f64,
}

impl Driver {
    fn frame(&mut self, events: Vec<InputEvent>) -> EditorOutput {
        self.time += 1.0 / 60.0;
        let mut out = self.ed.frame(
            &mut self.host,
            &EditorInput {
                time: self.time,
                events,
            },
        );
        if let Some(job) = out.paint.take() {
            job.discard();
        }
        out
    }
    fn click(&mut self, p: DVec2) {
        let at = [p.x as f32, p.y as f32];
        self.frame(vec![InputEvent::PointerMoved(at)]);
        self.frame(vec![InputEvent::PointerButton {
            button: egui::PointerButton::Primary,
            pressed: true,
        }]);
        self.frame(vec![InputEvent::PointerButton {
            button: egui::PointerButton::Primary,
            pressed: false,
        }]);
    }
    fn cmd_key(&mut self, key: egui::Key, shift: bool) -> EditorOutput {
        let m = egui::Modifiers {
            command: true,
            mac_cmd: true,
            shift,
            ..Default::default()
        };
        let out = self.frame(vec![
            InputEvent::Modifiers(m),
            InputEvent::Key {
                key,
                pressed: true,
                repeat: false,
            },
        ]);
        self.frame(vec![
            InputEvent::Key {
                key,
                pressed: false,
                repeat: false,
            },
            InputEvent::Modifiers(egui::Modifiers::default()),
        ]);
        out
    }
    fn layer_pos(&self, obj: ObjectId) -> DVec3 {
        self.ed
            .state
            .layers
            .as_ref()
            .unwrap()
            .transform(obj, &ConstFrame(flat_y_up_enu()))
            .unwrap()
            .position
    }
}

#[test]
fn pick_drag_undo_redo_save_reload() {
    let (root, _) = sample_root("flow");
    let mut d = Driver {
        ed: Editor::new(),
        host: MockHost::new(root.clone()),
        time: 0.0,
    };
    d.frame(vec![]);
    assert_eq!(d.ed.state.layers.as_ref().unwrap().objects().count(), 2);
    let start = DVec3::from_array(POS);

    // Pick: the answer arrives a frame later.
    let view = d.host.view();
    let bench_px = view.project(start).unwrap();
    d.click(bench_px);
    for _ in 0..3 {
        d.frame(vec![]);
    }
    assert_eq!(d.ed.state.selection.primary(), Some(id(1)));
    assert!(!d.ed.picking());
    assert!(d.host.picks >= 1);
    assert!(d.ed.state.inspected[0].movable);

    // Drag the east (X) arrow 120 px to the right over several frames.
    assert_eq!(d.ed.state.mode, Mode::Translate);
    let g = Gizmo::new(Mode::Translate, start, flat_y_up_enu(), &view);
    let grab = view.project(start + g.axes[0] * g.length * 0.75).unwrap();
    assert_eq!(g.hit(&view, grab), Some(Handle::Axis(0)));
    let out = d.frame(vec![InputEvent::PointerMoved([
        grab.x as f32,
        grab.y as f32,
    ])]);
    assert!(
        !out.gizmo.is_empty(),
        "the selected authored object shows a gizmo"
    );
    d.frame(vec![InputEvent::PointerButton {
        button: egui::PointerButton::Primary,
        pressed: true,
    }]);
    assert!(
        d.ed.dragging(),
        "press on the arrow starts a drag, not a pick"
    );
    let mut changes = 0;
    for k in 1..=6 {
        let p = grab + DVec2::new(20.0 * k as f64, 0.0);
        changes += d
            .frame(vec![InputEvent::PointerMoved([p.x as f32, p.y as f32])])
            .transform_changes
            .len();
    }
    d.frame(vec![InputEvent::PointerButton {
        button: egui::PointerButton::Primary,
        pressed: false,
    }]);
    assert!(!d.ed.dragging());
    assert_eq!(changes, 6);
    assert_eq!(d.ed.state.undo.len(), (1, 0), "one drag is one command");

    let moved = d.layer_pos(id(1));
    let dx = moved - start;
    assert!(dx.x > 0.5, "moved east: {dx:?}");
    assert!(
        dx.y.abs() < 1e-9 && dx.z.abs() < 1e-9,
        "axis drag stays on the axis: {dx:?}"
    );
    assert!(
        (d.host.transforms[&id(1)].position - moved).length() < 1e-9,
        "host got the same transform"
    );
    assert!(d.ed.state.layers.as_ref().unwrap().any_dirty());

    // Undo restores the exact layer text (clean); redo re-applies.
    d.cmd_key(egui::Key::Z, false);
    assert_eq!(d.layer_pos(id(1)), start);
    assert_eq!(d.host.transforms[&id(1)].position, start);
    assert!(
        !d.ed.state.layers.as_ref().unwrap().any_dirty(),
        "undo to the saved state is clean"
    );
    d.cmd_key(egui::Key::Z, true);
    assert_eq!(d.layer_pos(id(1)), moved);
    assert!(d.ed.state.layers.as_ref().unwrap().any_dirty());

    // Save, then reload from disk in a fresh set.
    let out = d.cmd_key(egui::Key::S, false);
    assert_eq!(out.saved.len(), 1);
    assert!(!d.ed.state.layers.as_ref().unwrap().any_dirty());
    let fresh = LayerSet::load(&root, bodies()).unwrap();
    let reloaded = fresh
        .transform(id(1), &ConstFrame(flat_y_up_enu()))
        .unwrap()
        .position;
    assert!(
        (reloaded - moved).length() < 1e-9,
        "reload within 1e-9 m: {:e}",
        (reloaded - moved).length()
    );
    // The marker was not touched and keeps its exact pose.
    assert_eq!(
        fresh.pose(id(2)),
        d.ed.state.layers.as_ref().unwrap().pose(id(2))
    );

    // Undo after save makes the layer dirty again.
    d.cmd_key(egui::Key::Z, false);
    assert!(d.ed.state.layers.as_ref().unwrap().any_dirty());
    assert_eq!(d.layer_pos(id(1)), start);

    // Escape clears the selection and the gizmo.
    d.frame(vec![InputEvent::Key {
        key: egui::Key::Escape,
        pressed: true,
        repeat: false,
    }]);
    let out = d.frame(vec![]);
    assert!(d.ed.state.selection.is_empty());
    assert!(out.gizmo.is_empty());
}

#[test]
fn click_on_nothing_clears_selection_and_ui_owns_its_windows() {
    let (root, _) = sample_root("miss");
    let mut d = Driver {
        ed: Editor::new(),
        host: MockHost::new(root),
        time: 0.0,
    };
    d.frame(vec![]);
    let bench_px = d.host.view().project(DVec3::from_array(POS)).unwrap();
    d.click(bench_px);
    for _ in 0..3 {
        d.frame(vec![]);
    }
    assert_eq!(d.ed.state.selection.primary(), Some(id(1)));
    // A click on empty ground misses.
    d.click(DVec2::new(1000.0, 750.0));
    for _ in 0..3 {
        d.frame(vec![]);
    }
    assert!(d.ed.state.selection.is_empty());
    // A click inside the Stats window goes to egui, not to picking.
    let picks = d.host.pick_calls;
    let out = d.frame(vec![InputEvent::PointerMoved([60.0, 40.0])]);
    assert!(
        out.wants_pointer || d.frame(vec![]).wants_pointer,
        "pointer over a panel belongs to the UI"
    );
    d.frame(vec![InputEvent::PointerButton {
        button: egui::PointerButton::Primary,
        pressed: true,
    }]);
    d.frame(vec![InputEvent::PointerButton {
        button: egui::PointerButton::Primary,
        pressed: false,
    }]);
    assert!(!d.ed.picking());
    assert_eq!(d.host.pick_calls, picks);
}

#[test]
fn idle_editor_skips_ui_passes() {
    let (root, _) = sample_root("idle");
    let mut d = Driver {
        ed: Editor::new(),
        host: MockHost::new(root),
        time: 0.0,
    };
    // Let start-up animations settle.
    for _ in 0..120 {
        d.frame(vec![]);
    }
    // Align to just after a UI pass, then run 0.2 s of idle frames (12 at 60 Hz): less than
    // one stats period, so at most one pass.
    let mut n = d.ed.ui_passes;
    while d.ed.ui_passes == n {
        n = d.ed.ui_passes;
        d.frame(vec![]);
    }
    let before = d.ed.ui_passes;
    for _ in 0..12 {
        d.frame(vec![]);
    }
    let painted = d.ed.ui_passes - before;
    assert!(
        painted <= 1,
        "idle frames re-ran egui {painted} times in 0.2 s"
    );
    // Over 2 s the stats still refresh (about STATS_HZ times a second).
    let before = d.ed.ui_passes;
    for _ in 0..120 {
        d.frame(vec![]);
    }
    let n = d.ed.ui_passes - before;
    assert!((6..=12).contains(&n), "{n} UI passes in 2 s idle");
}

#[test]
fn mode_hint_banner_never_takes_the_pointer_and_left_click_still_picks() {
    let (root, _) = sample_root("hint");
    let mut d = Driver {
        ed: Editor::new(),
        host: MockHost::new(root),
        time: 0.0,
    };
    d.ed.state.hint = "EDITOR | Tab: play".to_string();
    d.frame(vec![]);
    d.frame(vec![InputEvent::PointerMoved([800.0, 22.0])]);
    let out = d.frame(vec![]);
    assert!(!out.wants_pointer, "the banner is not interactive");
    // Left click on the bench selects it (pick is on the primary button).
    let bench_px = d.host.view().project(DVec3::from_array(POS)).unwrap();
    d.click(bench_px);
    for _ in 0..3 {
        d.frame(vec![]);
    }
    assert_eq!(d.ed.state.selection.primary(), Some(id(1)));
}
