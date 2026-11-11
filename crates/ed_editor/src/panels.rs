//! Editor panels: stats, pass timings, inspector, layers, log. Plain functions over
//! [`EditorState`]; button presses become [`Action`]s the editor carries out after the pass.

use egui::{Color32, RichText};

use crate::editor::{Action, EditorState, FrameStats, Inspected, LogLevel};
use crate::gizmo::{Mode, Space};
use crate::layerset::AuthoredKind;
use crate::model::GRID_STEPS;

/// All panels.
pub fn ui(state: &mut EditorState, ctx: &egui::Context, stats: &FrameStats) {
    stats_panel(ctx, stats);
    passes_panel(ctx, stats);
    inspector_panel(state, ctx);
    layers_panel(state, ctx);
    log_panel(state, ctx);
}

/// Panels sit in the four corners (stats and passes stacked top-left), anchored so they do
/// not overlap at any window size; they can be collapsed.
fn window(title: &str, corner: egui::Align2, offset: [f32; 2], w: f32) -> egui::Window<'_> {
    egui::Window::new(title)
        .anchor(corner, offset)
        .default_width(w)
        .min_width(w)
        .resizable(false)
}

pub fn stats_panel(ctx: &egui::Context, s: &FrameStats) {
    window("Stats", egui::Align2::LEFT_TOP, [8.0, 8.0], 240.0).show(ctx, |ui| {
        egui::Grid::new("stats")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                let fps = if s.frame_ms > 0.0 {
                    1000.0 / s.frame_ms
                } else {
                    0.0
                };
                row(ui, "frame", format!("{:.2} ms ({fps:.0} fps)", s.frame_ms));
                row(ui, "cpu", format!("{:.2} ms", s.cpu_ms));
                row(ui, "gpu", format!("{:.2} ms", s.gpu_ms));
                row(
                    ui,
                    "instances",
                    format!("{} ({} visible)", s.instances, s.visible_instances),
                );
                row(
                    ui,
                    "gpu memory",
                    format!("{:.1} MB", s.gpu_memory_bytes as f64 / (1024.0 * 1024.0)),
                );
                row(ui, "frame #", s.frame.to_string());
            });
    });
}

pub fn passes_panel(ctx: &egui::Context, s: &FrameStats) {
    window("Passes", egui::Align2::LEFT_TOP, [8.0, 200.0], 240.0).show(ctx, |ui| {
        if s.passes.is_empty() {
            ui.weak("no GPU timings yet");
            return;
        }
        egui::Grid::new("passes")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for p in &s.passes {
                    ui.label(format!("{}{}", "  ".repeat(p.depth as usize), p.label));
                    ui.monospace(format!("{:6.3} ms", p.gpu_ms));
                    ui.end_row();
                }
            });
    });
}

pub fn inspector_panel(state: &mut EditorState, ctx: &egui::Context) {
    window("Inspector", egui::Align2::RIGHT_TOP, [-8.0, 8.0], 330.0).show(ctx, |ui| {
        if state.inspected.is_empty() {
            ui.weak("click an object to select it");
            return;
        }
        if state.inspected.len() > 1 {
            ui.label(format!(
                "{} selected (last is primary)",
                state.inspected.len()
            ));
        }
        let primary = state.inspected.last().cloned();
        if let Some(i) = primary {
            inspected(ui, &i);
        }
        if ui.button("Clear selection").clicked() {
            state.actions.push(Action::ClearSelection);
        }
    });
}

fn inspected(ui: &mut egui::Ui, i: &Inspected) {
    egui::Grid::new("inspector")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            row(ui, "object", i.object.to_string());
            if let Some(info) = &i.info {
                if let Some(n) = info.node {
                    row(ui, "node", n.to_string());
                }
                if let Some(p) = &info.prefab {
                    row(ui, "prefab", p.clone());
                }
                if let Some(p) = info.part {
                    row(ui, "part", p.to_string());
                }
                if let Some(m) = &info.mesh {
                    row(ui, "mesh", m.clone());
                }
                if let Some(m) = &info.material {
                    row(ui, "material", m.clone());
                }
                if let Some(l) = info.lod {
                    row(ui, "lod", l.to_string());
                }
            }
            match &i.authored {
                Some(AuthoredKind::Place { prefab }) => {
                    row(ui, "layer op", format!("place {prefab}"))
                }
                Some(AuthoredKind::Marker { name, kind }) => {
                    row(ui, "layer op", format!("marker {name} ({kind})"))
                }
                None => row(ui, "layer op", "generated (read-only)".into()),
            }
            if let Some(l) = &i.layer {
                row(ui, "layer", l.clone());
            }
            if let Some(n) = i.layer_node {
                row(ui, "layer file node", n.to_string());
            }
            if let Some(t) = &i.transform {
                row(
                    ui,
                    "position",
                    format!(
                        "{:.3}, {:.3}, {:.3}",
                        t.position.x, t.position.y, t.position.z
                    ),
                );
                if let Some(r) = i.node_relative() {
                    row(
                        ui,
                        "rel. to node",
                        format!("{:.3}, {:.3}, {:.3}", r.x, r.y, r.z),
                    );
                }
                if let Some(q) = i.enu_rotation {
                    let (z, x, y) = q.to_euler(glam::EulerRot::ZXY);
                    row(
                        ui,
                        "rotation (ENU)",
                        format!(
                            "yaw {:.1}° pitch {:.1}° roll {:.1}°",
                            z.to_degrees(),
                            x.to_degrees(),
                            y.to_degrees()
                        ),
                    );
                }
                row(
                    ui,
                    "scale",
                    format!("{:.3}, {:.3}, {:.3}", t.scale.x, t.scale.y, t.scale.z),
                );
            }
        });
}

pub fn layers_panel(state: &mut EditorState, ctx: &egui::Context) {
    window("Layers", egui::Align2::RIGHT_BOTTOM, [-8.0, -8.0], 330.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.mode, Mode::Translate, "Move (W)");
            ui.selectable_value(&mut state.mode, Mode::Rotate, "Rotate (E)");
            ui.selectable_value(&mut state.mode, Mode::Scale, "Scale (R)");
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.space, Space::Enu, "ENU axes");
            ui.selectable_value(&mut state.space, Space::Object, "Object axes");
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.snap.enabled, "Snap (G)");
            for (k, s) in GRID_STEPS.iter().enumerate() {
                ui.selectable_value(&mut state.snap.grid, k, format!("{s} m"));
            }
            ui.label("15°");
        });
        ui.separator();
        match &state.layers {
            None => {
                ui.weak("no layer root");
            }
            Some(set) => {
                if set.layers().is_empty() {
                    ui.weak(format!("no layers in {}", set.root.display()));
                }
                egui::Grid::new("layers")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for l in set.layers() {
                            let name = if l.is_dirty() {
                                RichText::new(format!("{} *", l.name))
                                    .color(Color32::from_rgb(240, 190, 60))
                            } else {
                                RichText::new(&l.name)
                            };
                            ui.label(name);
                            ui.label(if l.enabled { "on" } else { "off" });
                            ui.label(format!("{} ops", l.op_count()));
                            ui.end_row();
                        }
                    });
            }
        }
        let dirty = state.layers.as_ref().is_some_and(|l| l.any_dirty());
        let (undo, redo) = state.undo.len();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(dirty, egui::Button::new("Save (Cmd+S)"))
                .clicked()
            {
                state.actions.push(Action::Save);
            }
            if ui
                .add_enabled(undo > 0, egui::Button::new(format!("Undo ({undo})")))
                .clicked()
            {
                state.actions.push(Action::Undo);
            }
            if ui
                .add_enabled(redo > 0, egui::Button::new(format!("Redo ({redo})")))
                .clicked()
            {
                state.actions.push(Action::Redo);
            }
            if ui.button("Reload").clicked() {
                state.actions.push(Action::ReloadLayers);
            }
        });
    });
}

pub fn log_panel(state: &mut EditorState, ctx: &egui::Context) {
    window("Log", egui::Align2::LEFT_BOTTOM, [8.0, -8.0], 360.0).show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .max_height(160.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if state.log.is_empty() {
                    ui.weak("hot reload and editor messages appear here");
                }
                for l in &state.log {
                    let c = match l.level {
                        LogLevel::Info => ui.visuals().text_color(),
                        LogLevel::Warn => Color32::from_rgb(240, 190, 60),
                        LogLevel::Error => Color32::from_rgb(240, 80, 70),
                    };
                    ui.label(
                        RichText::new(format!("{:8.2}  {}", l.time, l.text))
                            .color(c)
                            .monospace(),
                    );
                }
            });
    });
}

fn row(ui: &mut egui::Ui, k: &str, v: String) {
    ui.label(k);
    ui.monospace(v);
    ui.end_row();
}
