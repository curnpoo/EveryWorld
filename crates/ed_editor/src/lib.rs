//! Editor mode (stage E1): selection, gizmos, snapping, undo/redo, override-layer editing and
//! saving, egui panels and an egui painter on `ed_gpu`.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_pack`, `ed_planet`, `ed_render`, `ed_world`,
//! `ed_sim`, plus `ed_gpu` for the painter (P1-D14). The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Everything except [`painter`] is engine-free and unit-tested. The application implements
//! [`EditorHost`] and calls [`Editor::frame`] once per frame; the returned
//! [`EditorOutput`] carries the egui paint job (for [`painter::Painter`]), the gizmo draw
//! list in camera-relative f32, and the transforms that changed.

pub mod editor;
#[cfg(test)]
mod flow_tests;
pub mod gizmo;
pub mod layerset;
pub mod model;
pub mod painter;
pub mod panels;
pub mod transform;

pub use editor::{
    Action, Editor, EditorHost, EditorInput, EditorOutput, EditorState, FrameStats, InputEvent,
    Inspected, LogLevel, LogLine, ObjectInfo, PassTiming, PickState, ViewportSize,
};
pub use egui;
pub use gizmo::{DrawList, DrawVertex, Gizmo, Handle, Mode, Space, ViewMath};
pub use layerset::{LayerSet, LayerSetError};
pub use model::{Selection, Snap, UndoStack};
pub use painter::{PaintJob, PaintTarget, Painter};
pub use transform::{EnuFrame, Transform};
