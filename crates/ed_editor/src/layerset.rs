//! The override layers the editor has open: load, edit transforms of authored objects, track
//! dirty state, save atomically in canonical form (`docs/contracts/layers.md`).
//!
//! The editor edits `place` and `marker` operations (the operations with a position). Their
//! exact field values are kept as [`OpPose`] so undo restores the original bits and an
//! edit-then-undo leaves the file byte-identical. Dirty means "the canonical text the writer
//! would produce differs from what was loaded or last saved", so undoing back to the saved
//! state clears it and undoing past a save sets it again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ed_geo::{BodyId, NodeId, ObjectId};
use ed_pack::layers::{self, LayerFile, LayerIndex, OpBody, Quat, Scale, Vec3};
use glam::{DQuat, DVec3};

use crate::transform::{EnuFrame, Transform, enu_rotation, world_rotation};

#[derive(Debug, thiserror::Error)]
pub enum LayerSetError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {error}")]
    Index {
        path: PathBuf,
        error: layers::LayerError,
    },
    #[error(transparent)]
    Dir(#[from] layers::LayerDirError),
    #[error("body {0:?} has no name in the body table")]
    UnknownBody(BodyId),
}

/// The transform fields of a `place` or `marker` operation, exactly as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct OpPose {
    pub position: Vec3,
    pub rotation: Option<Quat>,
    /// Always `None` for markers.
    pub scale: Option<Scale>,
}

/// Which file of which layer an object lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectLoc {
    pub layer: usize,
    pub node: NodeId,
}

/// What kind of operation authored an object.
#[derive(Clone, Debug, PartialEq)]
pub enum AuthoredKind {
    Place { prefab: String },
    Marker { name: String, kind: String },
}

/// One layer directory.
#[derive(Debug)]
pub struct Layer {
    pub name: String,
    pub enabled: bool,
    pub dir: PathBuf,
    files: BTreeMap<NodeId, LayerFile>,
    /// Canonical text of each file as loaded or last saved.
    saved: BTreeMap<NodeId, String>,
    dirty: bool,
}

impl Layer {
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn files(&self) -> &BTreeMap<NodeId, LayerFile> {
        &self.files
    }
    pub fn op_count(&self) -> usize {
        self.files.values().map(|f| f.ops.len()).sum()
    }
    fn recompute_dirty(&mut self) {
        let differs =
            |n: &NodeId, f: &LayerFile| self.saved.get(n).is_none_or(|s| *s != f.to_toml());
        self.dirty = self.files.iter().any(|(n, f)| differs(n, f))
            || self.saved.keys().any(|n| !self.files.contains_key(n));
    }
}

/// All layers under one `layers/` root.
#[derive(Debug)]
pub struct LayerSet {
    pub root: PathBuf,
    layers: Vec<Layer>,
    objects: BTreeMap<ObjectId, ObjectLoc>,
    bodies: Vec<(String, BodyId)>,
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> LayerSetError + '_ {
    move |source| LayerSetError::Io {
        path: path.to_path_buf(),
        source,
    }
}

impl LayerSet {
    /// An empty set rooted at `root` (nothing loaded).
    pub fn empty(root: PathBuf, bodies: Vec<(String, BodyId)>) -> Self {
        LayerSet {
            root,
            layers: Vec::new(),
            objects: BTreeMap::new(),
            bodies,
        }
    }

    /// Reads `root/layers.toml` and every layer directory it lists. A missing `layers.toml`
    /// is an empty set; a listed layer without a directory is an empty layer.
    pub fn load(root: &Path, bodies: Vec<(String, BodyId)>) -> Result<Self, LayerSetError> {
        let mut set = LayerSet::empty(root.to_path_buf(), bodies);
        let index_path = root.join("layers.toml");
        let index = match std::fs::read_to_string(&index_path) {
            Ok(text) => LayerIndex::parse(&text).map_err(|error| LayerSetError::Index {
                path: index_path.clone(),
                error,
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => LayerIndex::default(),
            Err(e) => return Err(io(&index_path)(e)),
        };
        let body_of = |name: &str| set.bodies.iter().find(|(n, _)| n == name).map(|(_, b)| *b);
        let mut layers = Vec::new();
        for entry in &index.layers {
            let dir = root.join(&entry.name);
            let files = if dir.is_dir() {
                layers::read_layer_dir(&dir, &body_of)?
            } else {
                BTreeMap::new()
            };
            let saved = files.iter().map(|(n, f)| (*n, f.to_toml())).collect();
            layers.push(Layer {
                name: entry.name.clone(),
                enabled: entry.enabled,
                dir,
                files,
                saved,
                dirty: false,
            });
        }
        set.layers = layers;
        set.reindex();
        Ok(set)
    }

    fn reindex(&mut self) {
        self.objects.clear();
        // Later layers apply later (layers.md section 4), so a duplicate id resolves to the last.
        for (li, l) in self.layers.iter().enumerate() {
            for (node, f) in &l.files {
                for op in &f.ops {
                    if matches!(op.body, OpBody::Place(_) | OpBody::Marker(_)) {
                        self.objects.insert(
                            op.id,
                            ObjectLoc {
                                layer: li,
                                node: *node,
                            },
                        );
                    }
                }
            }
        }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }
    pub fn any_dirty(&self) -> bool {
        self.layers.iter().any(|l| l.dirty)
    }
    /// Authored objects the editor can move, in id order.
    pub fn objects(&self) -> impl Iterator<Item = (ObjectId, ObjectLoc)> + '_ {
        self.objects.iter().map(|(k, v)| (*k, *v))
    }
    pub fn locate(&self, id: ObjectId) -> Option<ObjectLoc> {
        self.objects.get(&id).copied()
    }

    fn op_body(&self, id: ObjectId) -> Option<&OpBody> {
        let loc = self.locate(id)?;
        self.layers[loc.layer]
            .files
            .get(&loc.node)?
            .ops
            .iter()
            .find(|o| o.id == id)
            .map(|o| &o.body)
    }

    pub fn kind(&self, id: ObjectId) -> Option<AuthoredKind> {
        match self.op_body(id)? {
            OpBody::Place(p) => Some(AuthoredKind::Place {
                prefab: p.prefab.clone(),
            }),
            OpBody::Marker(m) => Some(AuthoredKind::Marker {
                name: m.name.clone(),
                kind: m.kind.clone(),
            }),
            _ => None,
        }
    }

    pub fn pose(&self, id: ObjectId) -> Option<OpPose> {
        match self.op_body(id)? {
            OpBody::Place(p) => Some(OpPose {
                position: p.position,
                rotation: p.rotation,
                scale: p.scale.clone(),
            }),
            OpBody::Marker(m) => Some(OpPose {
                position: m.position,
                rotation: m.rotation,
                scale: None,
            }),
            _ => None,
        }
    }

    /// The object's world transform.
    pub fn transform(&self, id: ObjectId, frame: &dyn EnuFrame) -> Option<Transform> {
        Some(pose_to_transform(&self.pose(id)?, frame))
    }

    /// The pose to store for `t`, keeping the current rotation and scale fields (and so their
    /// exact bits, or their absence) when `t` does not change them.
    pub fn pose_for(&self, id: ObjectId, t: &Transform, frame: &dyn EnuFrame) -> Option<OpPose> {
        let cur = self.pose(id)?;
        let marker = matches!(self.op_body(id)?, OpBody::Marker(_));
        let cur_t = pose_to_transform(&cur, frame);
        let p = t.position;
        let rotation = if quat_close(cur_t.rotation, t.rotation) && cur.position == p.to_array() {
            cur.rotation
        } else {
            let q = enu_rotation(frame, p, t.rotation);
            let unchanged = cur.rotation.map_or(DQuat::IDENTITY, DQuat::from_array);
            if quat_close(q, unchanged) {
                cur.rotation
            } else {
                Some(q.to_array())
            }
        };
        let scale = if marker || (t.scale - cur_t.scale).abs().max_element() < 1e-12 {
            cur.scale
        } else if t.scale.x == t.scale.y && t.scale.y == t.scale.z {
            Some(Scale::Uniform(t.scale.x))
        } else {
            Some(Scale::Axes(t.scale.to_array()))
        };
        Some(OpPose {
            position: p.to_array(),
            rotation,
            scale,
        })
    }

    /// Writes `pose` into the object's operation. Moves the operation to another node file
    /// when its anchor leaves the L10 node (layers.md section 2). Returns false for an unknown
    /// or non-movable object.
    pub fn set_pose(&mut self, id: ObjectId, pose: &OpPose) -> bool {
        let Some(loc) = self.locate(id) else {
            return false;
        };
        let layer = &mut self.layers[loc.layer];
        let Some(file) = layer.files.get_mut(&loc.node) else {
            return false;
        };
        let Some(i) = file.ops.iter().position(|o| o.id == id) else {
            return false;
        };
        match &mut file.ops[i].body {
            OpBody::Place(p) => {
                p.position = pose.position;
                p.rotation = pose.rotation;
                p.scale = pose.scale.clone();
            }
            OpBody::Marker(m) => {
                m.position = pose.position;
                m.rotation = pose.rotation;
            }
            _ => return false,
        }
        let node = NodeId::from_unit(
            loc.node.body(),
            DVec3::from_array(pose.position),
            layers::LAYER_NODE_LEVEL,
        );
        if node != loc.node {
            let op = file.ops.remove(i);
            if file.ops.is_empty() && file.header.is_empty() {
                layer.files.remove(&loc.node);
            }
            let dst = layer.files.entry(node).or_default();
            dst.ops.push(op);
            dst.ops.sort_by_key(|o| o.id);
            self.objects.insert(
                id,
                ObjectLoc {
                    layer: loc.layer,
                    node,
                },
            );
        }
        layer.recompute_dirty();
        true
    }

    /// Writes every changed file of every dirty layer: temp file in the same directory, then
    /// rename, so a crash leaves either the old or the new file. Files whose last operation
    /// moved away are deleted. Returns the paths written or removed.
    pub fn save(&mut self) -> Result<Vec<PathBuf>, LayerSetError> {
        let mut touched = Vec::new();
        for li in 0..self.layers.len() {
            if !self.layers[li].dirty {
                continue;
            }
            let layer = &self.layers[li];
            let mut writes = Vec::new();
            for (node, file) in &layer.files {
                let text = file.to_toml();
                if layer.saved.get(node) != Some(&text) {
                    writes.push((*node, text));
                }
            }
            let removed: Vec<NodeId> = layer
                .saved
                .keys()
                .filter(|n| !layer.files.contains_key(n))
                .copied()
                .collect();
            for (node, text) in &writes {
                let path = layer
                    .dir
                    .join(layers::node_file_path(self.body_name(node.body())?, *node));
                write_atomic(&path, text.as_bytes())?;
                touched.push(path);
            }
            for node in &removed {
                let path = layer
                    .dir
                    .join(layers::node_file_path(self.body_name(node.body())?, *node));
                match std::fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(io(&path)(e)),
                }
                touched.push(path);
            }
            let layer = &mut self.layers[li];
            for (node, text) in writes {
                layer.saved.insert(node, text);
            }
            for node in removed {
                layer.saved.remove(&node);
            }
            layer.dirty = false;
        }
        Ok(touched)
    }

    fn body_name(&self, b: BodyId) -> Result<&str, LayerSetError> {
        self.bodies
            .iter()
            .find(|(_, id)| *id == b)
            .map(|(n, _)| n.as_str())
            .ok_or(LayerSetError::UnknownBody(b))
    }
}

pub fn pose_to_transform(p: &OpPose, frame: &dyn EnuFrame) -> Transform {
    let position = DVec3::from_array(p.position);
    let rel = p.rotation.map_or(DQuat::IDENTITY, DQuat::from_array);
    let scale = match &p.scale {
        None => DVec3::ONE,
        Some(Scale::Uniform(s)) => DVec3::splat(*s),
        Some(Scale::Axes(a)) => DVec3::from_array(*a),
    };
    Transform {
        position,
        rotation: world_rotation(frame, position, rel),
        scale,
    }
}

fn quat_close(a: DQuat, b: DQuat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-14
}

/// Write `bytes` to `path` through a temp file and a rename, creating parent directories.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), LayerSetError> {
    use std::io::Write as _;
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(io(parent))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("layer");
    let tmp = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(io(path)(e));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::transform::{ConstFrame, flat_y_up_enu};

    pub const EARTH: BodyId = BodyId(3);
    pub fn bodies() -> Vec<(String, BodyId)> {
        vec![("earth".into(), EARTH)]
    }
    pub const POS: [f64; 3] = [6_378_137.0, 0.0, -1_250_000.0];

    pub fn id(n: u128) -> ObjectId {
        ObjectId::authored(n)
    }

    /// A temp directory under target/ unique to this test.
    pub fn temp_dir(name: &str) -> PathBuf {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/editor/test-tmp")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A layer root with one layer `props` holding a place op and a marker in one node file.
    pub fn sample_root(name: &str) -> (PathBuf, NodeId) {
        let root = temp_dir(name);
        std::fs::write(
            root.join("layers.toml"),
            "[[layer]]\nname = \"props\"\nenabled = true\n",
        )
        .unwrap();
        let node = NodeId::from_unit(EARTH, DVec3::from_array(POS), 10);
        let text = format!(
            "[[op]]\nid = \"{}\"\nop = \"place\"\nprefab = \"kit/bench\"\nposition = [{}, {}, {}]\n\n[[op]]\nid = \"{}\"\nop = \"marker\"\nname = \"spawn\"\nkind = \"spawn\"\nposition = [{}, {}, {}]\nrotation = [0.0, 0.0, 0.3826834323650898, 0.9238795325112867]\n",
            id(1),
            layers::fmt_f64(POS[0]),
            layers::fmt_f64(POS[1]),
            layers::fmt_f64(POS[2]),
            id(2),
            layers::fmt_f64(POS[0] + 3.0),
            layers::fmt_f64(POS[1]),
            layers::fmt_f64(POS[2])
        );
        let path = root
            .join("props")
            .join(layers::node_file_path("earth", node));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
        // The fixture is canonical, so an unchanged save must leave it byte-identical.
        assert_eq!(LayerFile::parse(&text).unwrap().to_toml(), text);
        (root, node)
    }

    fn frame() -> ConstFrame {
        ConstFrame(flat_y_up_enu())
    }

    #[test]
    fn load_and_unchanged_save_writes_nothing() {
        let (root, node) = sample_root("unchanged");
        let path = root
            .join("props")
            .join(layers::node_file_path("earth", node));
        let before = std::fs::read(&path).unwrap();
        let mut set = LayerSet::load(&root, bodies()).unwrap();
        assert_eq!(set.layers().len(), 1);
        assert_eq!(set.objects().count(), 2);
        assert!(!set.any_dirty());
        assert!(set.save().unwrap().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn move_save_reload_and_undo_dirty() {
        let (root, node) = sample_root("move");
        let f = frame();
        let mut set = LayerSet::load(&root, bodies()).unwrap();
        let orig = set.pose(id(1)).unwrap();
        let mut t = set.transform(id(1), &f).unwrap();
        t.position += DVec3::new(1.25, 0.0, -0.5);
        let p = set.pose_for(id(1), &t, &f).unwrap();
        assert_eq!(p.rotation, None, "a pure move keeps the absent rotation");
        assert!(set.set_pose(id(1), &p));
        assert!(set.any_dirty());
        let written = set.save().unwrap();
        assert_eq!(written.len(), 1);
        assert!(!set.any_dirty());

        let again = LayerSet::load(&root, bodies()).unwrap();
        let t2 = again.transform(id(1), &f).unwrap();
        assert!((t2.position - t.position).abs().max_element() < 1e-9);
        assert_eq!(again.locate(id(1)).unwrap().node, node);

        // Undo after save: restoring the original pose makes the layer dirty again.
        assert!(set.set_pose(id(1), &orig));
        assert!(set.any_dirty());
        // ...and saving that restores the original bytes.
        set.save().unwrap();
        let (fresh, _) = sample_root("move-ref");
        let a = std::fs::read(
            root.join("props")
                .join(layers::node_file_path("earth", node)),
        )
        .unwrap();
        let b = std::fs::read(
            fresh
                .join("props")
                .join(layers::node_file_path("earth", node)),
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn edit_then_restore_is_clean() {
        let (root, _) = sample_root("restore");
        let f = frame();
        let mut set = LayerSet::load(&root, bodies()).unwrap();
        let orig = set.pose(id(2)).unwrap();
        let mut t = set.transform(id(2), &f).unwrap();
        t.rotation = DQuat::from_rotation_y(0.4) * t.rotation;
        let p = set.pose_for(id(2), &t, &f).unwrap();
        assert_ne!(p.rotation, orig.rotation);
        set.set_pose(id(2), &p);
        assert!(set.any_dirty());
        set.set_pose(id(2), &orig);
        assert!(!set.any_dirty());
    }

    #[test]
    fn moving_across_nodes_rebuckets() {
        let (root, node) = sample_root("rebucket");
        let f = frame();
        let mut set = LayerSet::load(&root, bodies()).unwrap();
        let mut t = set.transform(id(1), &f).unwrap();
        // ~40 km east: a different L10 node (L10 cells are ~10 km).
        t.position.z += 40_000.0;
        let p = set.pose_for(id(1), &t, &f).unwrap();
        set.set_pose(id(1), &p);
        let to = set.locate(id(1)).unwrap().node;
        assert_ne!(to, node);
        let written = set.save().unwrap();
        assert_eq!(written.len(), 2);
        let again = LayerSet::load(&root, bodies()).unwrap();
        assert_eq!(again.locate(id(1)).unwrap().node, to);
        assert_eq!(again.locate(id(2)).unwrap().node, node);
    }

    #[test]
    fn atomic_write_leaves_no_temp() {
        let d = temp_dir("atomic");
        let p = d.join("a/b.toml");
        write_atomic(&p, b"x = 1\n").unwrap();
        write_atomic(&p, b"x = 2\n").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"x = 2\n");
        assert_eq!(std::fs::read_dir(d.join("a")).unwrap().count(), 1);
    }
}
