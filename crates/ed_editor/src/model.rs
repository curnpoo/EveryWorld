//! Pure editor state: selection, snapping settings, the undo stack of transform edits.

use ed_geo::ObjectId;

use crate::layerset::OpPose;
use crate::transform::Transform;

/// Selected objects in selection order; the last one is primary (it carries the gizmo).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    items: Vec<ObjectId>,
}

impl Selection {
    pub fn items(&self) -> &[ObjectId] {
        &self.items
    }
    pub fn primary(&self) -> Option<ObjectId> {
        self.items.last().copied()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn contains(&self, id: ObjectId) -> bool {
        self.items.contains(&id)
    }
    /// Replace the selection with one object.
    pub fn set(&mut self, id: ObjectId) {
        self.items.clear();
        self.items.push(id);
    }
    /// Shift-click: add (and make primary), or remove when already selected.
    pub fn toggle(&mut self, id: ObjectId) {
        if let Some(i) = self.items.iter().position(|&x| x == id) {
            self.items.remove(i);
        } else {
            self.items.push(id);
        }
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// Grid steps the translate snap offers, metres.
pub const GRID_STEPS: [f64; 3] = [0.1, 0.5, 2.0];
/// Rotation snap, radians (15 degrees).
pub const ANGLE_STEP: f64 = std::f64::consts::PI / 12.0;
/// Uniform scale snap.
pub const SCALE_STEP: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    pub enabled: bool,
    /// Index into [`GRID_STEPS`].
    pub grid: usize,
}

impl Default for Snap {
    fn default() -> Self {
        Snap {
            enabled: false,
            grid: 1,
        }
    }
}

impl Snap {
    pub fn grid_step(&self) -> f64 {
        GRID_STEPS[self.grid.min(GRID_STEPS.len() - 1)]
    }
    pub fn distance(&self, d: f64) -> f64 {
        if self.enabled {
            snap_to(d, self.grid_step())
        } else {
            d
        }
    }
    pub fn angle(&self, a: f64) -> f64 {
        if self.enabled {
            snap_to(a, ANGLE_STEP)
        } else {
            a
        }
    }
    pub fn scale(&self, s: f64) -> f64 {
        if self.enabled {
            snap_to(s, SCALE_STEP).max(SCALE_STEP)
        } else {
            s
        }
    }
}

pub fn snap_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// One object's part of a transform edit.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectEdit {
    pub object: ObjectId,
    pub before: Transform,
    pub after: Transform,
    /// Exact layer fields before and after, for objects that live in a layer.
    pub before_pose: Option<OpPose>,
    pub after_pose: Option<OpPose>,
}

/// A transform edit of one or more objects. A gizmo drag is one command however many frames
/// it lasts (`merge` key = the drag number).
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub label: &'static str,
    pub edits: Vec<ObjectEdit>,
    pub merge: Option<u64>,
}

/// Undo and redo stacks. Pushing clears redo.
#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    done: Vec<Command>,
    undone: Vec<Command>,
}

impl UndoStack {
    /// Push a command, or fold it into the top one when both carry the same merge key: the
    /// top keeps its `before` and takes the new `after` for every object.
    pub fn push(&mut self, cmd: Command) {
        self.undone.clear();
        if let (Some(top), Some(k)) = (self.done.last_mut(), cmd.merge)
            && top.merge == Some(k)
        {
            for e in cmd.edits {
                match top.edits.iter_mut().find(|t| t.object == e.object) {
                    Some(t) => {
                        t.after = e.after;
                        t.after_pose = e.after_pose;
                    }
                    None => top.edits.push(e),
                }
            }
            return;
        }
        self.done.push(cmd);
    }
    /// Close a merge group: later commands with the same key start a new command.
    pub fn seal(&mut self) {
        if let Some(top) = self.done.last_mut() {
            top.merge = None;
        }
    }
    /// Remove the top command if every edit in it is a no-op (a click on a handle without a
    /// move). Returns true if one was removed.
    pub fn drop_noop_top(&mut self) -> bool {
        if self.done.last().is_some_and(|c| {
            c.edits
                .iter()
                .all(|e| e.before == e.after && e.before_pose == e.after_pose)
        }) {
            self.done.pop();
            return true;
        }
        false
    }
    /// Merge key of the top command (an open drag), if any.
    pub fn top_merge(&self) -> Option<u64> {
        self.done.last().and_then(|c| c.merge)
    }
    /// Remove the top command without moving it to redo (a cancelled drag).
    pub fn discard_top(&mut self) -> Option<Command> {
        self.done.pop()
    }
    /// The command to revert; moves it to the redo stack.
    pub fn undo(&mut self) -> Option<&Command> {
        let c = self.done.pop()?;
        self.undone.push(c);
        self.undone.last()
    }
    /// The command to re-apply; moves it back to the undo stack.
    pub fn redo(&mut self) -> Option<&Command> {
        let c = self.undone.pop()?;
        self.done.push(c);
        self.done.last()
    }
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }
    pub fn len(&self) -> (usize, usize) {
        (self.done.len(), self.undone.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn edit(id: u128, from: f64, to: f64) -> ObjectEdit {
        ObjectEdit {
            object: ObjectId::authored(id),
            before: Transform::from_position(DVec3::splat(from)),
            after: Transform::from_position(DVec3::splat(to)),
            before_pose: None,
            after_pose: None,
        }
    }

    #[test]
    fn selection_primary_and_toggle() {
        let (a, b) = (ObjectId::authored(1), ObjectId::authored(2));
        let mut s = Selection::default();
        s.set(a);
        s.toggle(b);
        assert_eq!(s.primary(), Some(b));
        s.toggle(b);
        assert_eq!(s.primary(), Some(a));
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn snapping() {
        let s = Snap {
            enabled: true,
            grid: 1,
        };
        assert_eq!(s.distance(1.26), 1.5);
        assert_eq!(s.distance(-0.2), -0.0);
        assert!((s.angle(0.30) - ANGLE_STEP).abs() < 1e-12);
        assert_eq!(
            Snap {
                enabled: true,
                grid: 0
            }
            .distance(0.26),
            0.30000000000000004
        );
        assert_eq!(
            Snap {
                enabled: true,
                grid: 2
            }
            .distance(2.9),
            2.0
        );
        assert_eq!(Snap::default().distance(1.26), 1.26);
    }

    #[test]
    fn drags_merge_and_undo_redo() {
        let mut u = UndoStack::default();
        u.push(Command {
            label: "move",
            edits: vec![edit(1, 0.0, 1.0)],
            merge: Some(7),
        });
        u.push(Command {
            label: "move",
            edits: vec![edit(1, 1.0, 2.0)],
            merge: Some(7),
        });
        u.push(Command {
            label: "move",
            edits: vec![edit(1, 2.0, 3.0)],
            merge: Some(7),
        });
        assert_eq!(u.len(), (1, 0));
        u.seal();
        u.push(Command {
            label: "move",
            edits: vec![edit(1, 3.0, 4.0)],
            merge: Some(7),
        });
        assert_eq!(u.len(), (2, 0));
        let c = u.undo().unwrap().clone();
        assert_eq!(c.edits[0].before.position.x, 3.0);
        let c = u.undo().unwrap().clone();
        assert_eq!(
            (c.edits[0].before.position.x, c.edits[0].after.position.x),
            (0.0, 3.0)
        );
        assert!(!u.can_undo());
        assert_eq!(u.redo().unwrap().edits[0].after.position.x, 3.0);
        u.push(Command {
            label: "move",
            edits: vec![edit(1, 3.0, 9.0)],
            merge: None,
        });
        assert!(!u.can_redo());
    }

    #[test]
    fn noop_drag_is_dropped() {
        let mut u = UndoStack::default();
        u.push(Command {
            label: "move",
            edits: vec![edit(1, 1.0, 1.0)],
            merge: Some(1),
        });
        assert!(u.drop_noop_top());
        assert!(!u.can_undo());
    }
}
