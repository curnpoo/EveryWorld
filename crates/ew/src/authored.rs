//! Authored placements from layer files (M4b). The canonical source is the layer root
//! (`assets-src/layers`, `layers.md`): `place` ops become renderer placements and collision
//! bodies, the `spawn` marker places the character. Generated content (pack instances) stays
//! read-only; only these objects are editable.

use crate::collide::yup_to_enu;
use ed_assets::proving::ORIGIN;
use ed_editor::layerset::{AuthoredKind, LayerSet};
use ed_editor::transform::{ConstFrame, Transform, flat_y_up_enu};
use ed_geo::{BodyId, ObjectId};
use ed_pack::AssetId;
use glam::{Affine3A, DVec3, Vec3};
use std::path::{Path, PathBuf};

/// Layer files' body table.
pub fn bodies() -> Vec<(String, BodyId)> {
    vec![("earth".to_string(), BodyId(3))]
}

/// The proving ground's ENU frame (flat, y up).
pub fn frame() -> ConstFrame {
    ConstFrame(flat_y_up_enu())
}

/// `assets-src/layers` under the current directory or the workspace root.
pub fn default_layer_root() -> PathBuf {
    let local = PathBuf::from("assets-src/layers");
    if local.join("layers.toml").exists() {
        return local;
    }
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets-src/layers");
    if ws.join("layers.toml").exists() { ws } else { local }
}

/// An editor transform as a renderer affine relative to the proving node.
///
/// Editor rotations map an object's local axes (east, north, up for the identity layer
/// rotation) to the world; meshes are authored y-up, so the mesh-to-object step is
/// `enu_from_yup` (local y -> up) and per-axis scale swaps y and z.
pub fn node_affine(t: &Transform) -> Affine3A {
    let r = (t.rotation.as_quat() * ed_assets::proving::enu_from_yup()).normalize();
    let s = Vec3::new(t.scale.x as f32, t.scale.z as f32, t.scale.y as f32);
    Affine3A::from_scale_rotation_translation(s, r, (t.position - ORIGIN).as_vec3())
}

/// Inverse of [`node_affine`] (for transforms read back from the renderer).
pub fn editor_transform(a: &Affine3A) -> Transform {
    let (s, r, p) = a.to_scale_rotation_translation();
    Transform {
        position: ORIGIN + p.as_dvec3(),
        rotation: (r * ed_assets::proving::enu_from_yup().inverse()).normalize().as_dquat(),
        scale: glam::DVec3::new(s.x as f64, s.z as f64, s.y as f64),
    }
}

/// One authored `place`.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub id: ObjectId,
    pub prefab: String,
    pub asset: AssetId,
    pub transform: Transform,
}

impl Place {
    pub fn affine(&self) -> Affine3A {
        node_affine(&self.transform)
    }
}

/// What the layers author.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Authored {
    pub places: Vec<Place>,
    /// The `spawn` marker: world transform.
    pub spawn: Option<Transform>,
}

impl Authored {
    pub fn from_set(set: &LayerSet) -> Authored {
        let f = frame();
        let mut a = Authored::default();
        for (id, _) in set.objects() {
            let Some(t) = set.transform(id, &f) else { continue };
            match set.kind(id) {
                Some(AuthoredKind::Place { prefab }) => a.places.push(Place { id, asset: AssetId::from_path(&prefab), prefab, transform: t }),
                Some(AuthoredKind::Marker { kind, .. }) if kind == "spawn" && a.spawn.is_none() => a.spawn = Some(t),
                _ => {}
            }
        }
        a
    }

    pub fn load(root: &Path) -> Result<Authored, String> {
        let set = LayerSet::load(root, bodies()).map_err(|e| format!("layers {}: {e}", root.display()))?;
        Ok(Self::from_set(&set))
    }

    /// Character feet in the sim (node ENU), slightly above the marker so the first step
    /// settles onto the ground.
    pub fn spawn_feet(&self) -> DVec3 {
        let p = self.spawn.map(|t| t.position).unwrap_or(ORIGIN + DVec3::new(-1.5, 0.0, 244.0));
        yup_to_enu(p - ORIGIN) + DVec3::Z * 0.05
    }

    /// (id, asset, node affine) of every place.
    pub fn list(&self) -> Vec<(ObjectId, AssetId, Affine3A)> {
        self.places.iter().map(|p| (p.id, p.asset, p.affine())).collect()
    }
}

/// Facing of a transform about y (radians), for the character at the spawn marker.
pub fn yaw_of(t: &Transform) -> f32 {
    let f = node_affine(t).transform_vector3(Vec3::Z);
    f.x.atan2(f.z)
}
