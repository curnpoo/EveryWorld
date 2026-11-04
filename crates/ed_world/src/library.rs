//! Asset library: the catalog of reusable things the world places (props now;
//! boulders, trunks, vehicles, animals later). See `docs/plans/assets-and-agents.md`.
//!
//! World generation emits [`Placement`]s (asset id + transform in tile space)
//! instead of baking copies into tile meshes. The game cooks each asset once
//! (meshlet + collider) and draws every placement as an instance of it.
//!
//! Content packs add assets with [`Library::register`] and [`install`] the
//! library before the first tile is generated; generators look assets up by
//! stable string key, so rules reference data, not code.

use crate::mesh::MeshData;
use glam::{Quat, Vec3};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Index of an asset in the installed [`Library`]. Only meaningful for that library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(pub u16);

/// Which renderer draws the asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderPath {
    /// Static, opaque, palette-coloured: virtual geometry (meshlets).
    Meshlet,
}

/// Physics shape in the asset's model space (+Y up, origin on the ground).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColliderShape {
    None,
    /// Upright cylinder standing on the origin.
    Cylinder { radius: f32, height: f32 },
}

pub type Generator = Arc<dyn Fn() -> MeshData + Send + Sync>;

#[derive(Clone)]
pub struct AssetDef {
    /// Stable name, e.g. `prop/street_light`. Registering the same key again replaces it.
    pub key: String,
    pub render: RenderPath,
    pub collider: ColliderShape,
    /// Builds the model (vertex colours; the game palettizes for meshlets).
    pub build: Generator,
}

/// One instance of an asset, in the tile's local mesh space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub asset: AssetId,
    pub pos: Vec3,
    pub rot: Quat,
    pub scale: f32,
}

#[derive(Default, Clone)]
pub struct Library {
    defs: Vec<AssetDef>,
    index: HashMap<String, AssetId>,
}

impl Library {
    /// Built-in assets every world has.
    pub fn standard() -> Self {
        let mut lib = Library::default();
        crate::props::register_street_props(&mut lib);
        lib
    }

    pub fn register(&mut self, def: AssetDef) -> AssetId {
        if let Some(&id) = self.index.get(&def.key) {
            self.defs[id.0 as usize] = def;
            return id;
        }
        let id = AssetId(u16::try_from(self.defs.len()).expect("asset library full"));
        self.index.insert(def.key.clone(), id);
        self.defs.push(def);
        id
    }

    pub fn id(&self, key: &str) -> Option<AssetId> {
        self.index.get(key).copied()
    }

    pub fn get(&self, id: AssetId) -> &AssetDef {
        &self.defs[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (AssetId, &AssetDef)> {
        self.defs.iter().enumerate().map(|(i, d)| (AssetId(i as u16), d))
    }
}

static INSTALLED: OnceLock<Library> = OnceLock::new();

/// Make `lib` the world's library. Must run before the first tile is built;
/// returns `false` (and keeps the old one) if a library was already in use.
pub fn install(lib: Library) -> bool {
    INSTALLED.set(lib).is_ok()
}

/// The installed library (the standard one if nothing was installed).
pub fn get() -> &'static Library {
    INSTALLED.get_or_init(Library::standard)
}

/// Placement standing on `pos`, model +Y along `up`, model +Z turned toward
/// `facing` (projected onto the ground plane).
pub fn upright(asset: AssetId, pos: Vec3, up: Vec3, facing: Vec3) -> Placement {
    let rot_up = Quat::from_rotation_arc(Vec3::Y, up);
    let f = rot_up.inverse() * facing.reject_from_normalized(up).normalize_or(rot_up * Vec3::Z);
    Placement { asset, pos, rot: rot_up * Quat::from_rotation_y(f.x.atan2(f.z)), scale: 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(key: &str) -> AssetDef {
        AssetDef { key: key.into(), render: RenderPath::Meshlet, collider: ColliderShape::None, build: Arc::new(MeshData::default) }
    }

    #[test]
    fn register_replaces_by_key() {
        let mut lib = Library::default();
        let a = lib.register(def("a"));
        let b = lib.register(def("b"));
        assert_ne!(a, b);
        assert_eq!(lib.register(def("a")), a);
        assert_eq!(lib.len(), 2);
        assert_eq!(lib.id("b"), Some(b));
        assert_eq!(lib.id("missing"), None);
    }

    #[test]
    fn standard_has_street_props_with_geometry() {
        let lib = Library::standard();
        let id = lib.id("prop/street_light").expect("street light registered");
        assert!((lib.get(id).build)().triangle_count() > 0);
    }

    #[test]
    fn upright_points_model_z_at_facing() {
        let up = Vec3::new(0.1, 1.0, 0.0).normalize();
        let p = upright(AssetId(0), Vec3::ZERO, up, Vec3::X);
        assert!((p.rot * Vec3::Y - up).length() < 1e-5);
        let fz = p.rot * Vec3::Z;
        assert!(fz.dot(Vec3::X.reject_from_normalized(up).normalize()) > 0.999);
    }
}
