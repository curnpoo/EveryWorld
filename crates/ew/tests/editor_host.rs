//! M4b: the editor through the real `ew` host (Metal device, renderer, pack content, sim).
//! Pick the planter bench with the GPU pick (async), drag the gizmo's east arrow, Cmd+S into a
//! copy of the layers, restart from disk and find the bench (render instance, layer transform
//! and collision) where it was dropped. Generated pack instances stay read-only.
//! Writes target/m4/editor_before.png and editor_after.png (panels + gizmo on the bench).

mod common;
use ed_editor::editor::InputEvent;
use ed_editor::egui;
use ed_editor::gizmo::{Gizmo, Handle, Mode as GizmoMode, ViewMath};
use ed_editor::transform::flat_y_up_enu;
use ed_geo::ObjectId;
use ew::camera::Pose;
use ew::harness::Harness;
use ew::session::Mode;
use glam::DVec3;
use std::path::{Path, PathBuf};

const BENCH: ObjectId = ObjectId(0x8000_0000_0000_0000_0000_0000_0000_0001);
const W: u32 = 1600;
const H: u32 = 900;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &p);
        } else {
            std::fs::copy(e.path(), p).unwrap();
        }
    }
}

fn bench_position(h: &Harness) -> DVec3 {
    h.session.authored.places.iter().find(|p| p.id == BENCH).expect("bench in the layers").transform.position
}

/// Points the fly camera at `target` from 12 m south and 5 m up.
fn look_at(h: &mut Harness, target: DVec3) {
    let eye = target + DVec3::new(0.0, 5.0, 12.0);
    let pose = Pose { position: eye, yaw: 0.0, pitch: -(5.0f64 / 12.0).atan() as f32 };
    h.session.free.prev = pose;
    h.session.free.cur = pose;
}

fn view(h: &Harness) -> ViewMath {
    ViewMath::new(&h.session.camera, [W, H])
}

fn press(button: bool) -> InputEvent {
    InputEvent::PointerButton { button: egui::PointerButton::Primary, pressed: button }
}

fn cmd_s(h: &mut Harness) {
    let m = egui::Modifiers { command: true, mac_cmd: true, ..Default::default() };
    h.frame(vec![InputEvent::Modifiers(m), InputEvent::Key { key: egui::Key::S, pressed: true, repeat: false }]).unwrap();
    h.frame(vec![InputEvent::Key { key: egui::Key::S, pressed: false, repeat: false }, InputEvent::Modifiers(egui::Modifiers::default())]).unwrap();
}

#[test]
fn pick_drag_save_restart_through_the_ew_host() {
    let ws = common::workspace();
    let packs = common::packs();
    let out = ws.join("target/m4");
    let layers: PathBuf = std::env::temp_dir().join(format!("ew-editor-host-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&layers);
    copy_dir(&ws.join("assets-src/layers"), &layers);

    let mut h = Harness::new(&packs, layers.clone(), W, H, Mode::Editor, true, false).expect("harness");
    let start = bench_position(&h);
    look_at(&mut h, start);
    for _ in 0..4 {
        h.frame(vec![]).unwrap();
    }

    // Generated instances are read-only.
    let generated = h.content.objects().find(|o| !o.1).expect("a generated instance").0;
    let before = h.content.object(generated).unwrap().transform;
    assert!(!h.move_object(generated, &glam::Affine3A::from_translation(glam::Vec3::splat(5.0))), "generated instances are read-only");
    assert_eq!(h.content.object(generated).unwrap().transform, before);

    // Pick the bench: the GPU answer arrives a few frames later.
    let bench_px = view(&h).project(start).expect("bench on screen");
    let at = [bench_px.x as f32, bench_px.y as f32];
    h.frame(vec![InputEvent::PointerMoved(at)]).unwrap();
    h.frame(vec![press(true)]).unwrap();
    h.frame(vec![press(false)]).unwrap();
    let mut waited = 0;
    while h.session.editor.as_ref().unwrap().state.selection.primary().is_none() && waited < 10 {
        h.frame(vec![]).unwrap();
        waited += 1;
    }
    assert_eq!(h.session.editor.as_ref().unwrap().state.selection.primary(), Some(BENCH), "GPU pick selected the bench ({waited} extra frames)");
    println!("pick answered {waited} frames after the click");

    // Gizmo on the bench; capture "before".
    let v = view(&h);
    let g = Gizmo::new(GizmoMode::Translate, start, flat_y_up_enu(), &v);
    let grab = v.project(start + g.axes[0] * g.length * 0.75).unwrap();
    assert_eq!(g.hit(&v, grab), Some(Handle::Axis(0)));
    let o = h.frame(vec![InputEvent::PointerMoved([grab.x as f32, grab.y as f32])]).unwrap();
    assert!(!o.wants_pointer, "the bench is not under a panel");
    h.capture(&out.join("editor_before.png")).unwrap();

    // Drag the east arrow 2 m.
    let to = v.project(start + g.axes[0] * (g.length * 0.75 + 2.0)).unwrap();
    h.frame(vec![press(true)]).unwrap();
    assert!(h.session.editor.as_ref().unwrap().dragging(), "press on the arrow starts a drag");
    for k in 1..=6 {
        let p = grab + (to - grab) * (k as f64 / 6.0);
        h.frame(vec![InputEvent::PointerMoved([p.x as f32, p.y as f32])]).unwrap();
    }
    h.frame(vec![press(false)]).unwrap();
    assert!(h.session.edits >= 6, "the host applied the drag ({} edits)", h.session.edits);
    let after_drag = h.content.object(BENCH).unwrap().transform.translation;
    let moved = ew::authored::editor_transform(&h.content.object(BENCH).unwrap().transform).position;
    let dx = moved - start;
    println!("drag moved the bench by {dx:?}");
    assert!((dx.x - 2.0).abs() < 0.1 && dx.y.abs() < 1e-6 && dx.z.abs() < 1e-6, "moved 2 m east: {dx:?}");
    // Collision followed the render instance.
    let shapes = h.session.game.as_ref().unwrap().object_shapes(BENCH).expect("bench collision").to_vec();
    assert!(!shapes.is_empty(), "the bench has collision");
    let c = ed_assets::collision::centre(&shapes[0].shape);
    let node_east = (moved - ed_assets::proving::ORIGIN).x as f32;
    assert!((c.x - node_east).abs() < 1.5, "collision moved with the bench: shape x {} vs bench x {node_east}", c.x);
    h.capture(&out.join("editor_after.png")).unwrap();

    // Save.
    cmd_s(&mut h);
    assert!(!h.session.saved.is_empty(), "Cmd+S wrote the layer file");
    println!("saved {:?}", h.session.saved);
    h.destroy();

    // Restart from disk.
    let mut h2 = Harness::new(&packs, layers.clone(), W, H, Mode::Editor, false, false).expect("restart");
    let reloaded = bench_position(&h2);
    assert!((reloaded - moved).length() < 1e-6, "restart keeps the move: {reloaded:?} vs {moved:?}");
    let inst = h2.content.object(BENCH).unwrap().transform.translation;
    assert!((inst - after_drag).length() < 1e-3, "render instance at the saved place");
    let shapes2 = h2.session.game.as_ref().unwrap().object_shapes(BENCH).unwrap().to_vec();
    assert!((ed_assets::collision::centre(&shapes2[0].shape) - c).length() < 1e-3, "collision at the saved place");
    h2.frame(vec![]).unwrap();
    h2.destroy();
    let _ = std::fs::remove_dir_all(&layers);
}
