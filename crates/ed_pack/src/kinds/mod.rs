//! Typed chunk bodies. Each has a schema-1 binary layout documented on its type; every layout
//! is little-endian with bulk streams 16-aligned relative to the payload start (which is
//! itself 16-aligned in the pack file), so vertex, index and texel bytes can be handed to the
//! GPU in place.
//!
//! Asset references are [`AssetId`]s (`pack.md` section 4: "texture references by
//! `AssetId`"), resolved to content hashes through the manifest, so a texture edit does not
//! change the hash of every material, prefab and node that uses it.

use crate::error::{Error, Result};
use crate::hash::AssetId;
use crate::wire::{In, Out};

macro_rules! wire_enum {
    ($(#[$m:meta])* pub enum $name:ident : $repr:ty { $($(#[$vm:meta])* $var:ident = $val:expr),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr($repr)]
        pub enum $name { $($(#[$vm])* $var = $val),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub fn from_wire(v: $repr) -> Option<$name> {
                $(if v == $val { return Some($name::$var); })+
                None
            }
            pub fn to_wire(self) -> $repr {
                self as $repr
            }
        }
    };
}

mod clusters;
mod collision;
mod instances;
mod layer;
mod material;
mod mesh;
mod opaque;
mod prefab;
mod skeleton;
mod texture;

pub use clusters::Clusters;
pub use collision::{Collision, CollisionShape, Shape};
pub use instances::{Instance, Instances};
pub use layer::LayerText;
pub use material::{AlphaMode, Material, MaterialModel, Param};
pub use mesh::{IndexFormat, Mesh, MeshLod, MeshSection, VertexAttribute, VertexFormat, VertexSemantic};
pub use opaque::Opaque;
pub use prefab::{Prefab, PrefabNode, PrefabPoint, PointKind};
pub use skeleton::{Joint, Skeleton, MAX_JOINTS};
pub use texture::{ColourRole, Texture, TextureFormat};

/// Translation, rotation (quaternion `x, y, z, w`) and non-uniform scale. 40 bytes on the wire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Transform { translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] }
    }
}

impl Transform {
    pub(crate) fn write(&self, o: &mut Out) {
        o.f32s(&self.translation);
        o.f32s(&self.rotation);
        o.f32s(&self.scale);
    }
    pub(crate) fn read(r: &mut In) -> Result<Transform> {
        let t = Transform { translation: r.f32x()?, rotation: r.f32x()?, scale: r.f32x()? };
        t.validate()?;
        Ok(t)
    }
    pub fn validate(&self) -> Result<()> {
        finite(&self.translation, "transform translation")?;
        finite(&self.rotation, "transform rotation")?;
        finite(&self.scale, "transform scale")
    }
}

/// Bounding sphere and axis-aligned box. 40 bytes on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Bounds {
    pub centre: [f32; 3],
    pub radius: f32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    pub(crate) fn write(&self, o: &mut Out) {
        o.f32s(&self.centre);
        o.f32(self.radius);
        o.f32s(&self.min);
        o.f32s(&self.max);
    }
    pub(crate) fn read(r: &mut In) -> Result<Bounds> {
        Ok(Bounds { centre: r.f32x()?, radius: r.f32()?, min: r.f32x()?, max: r.f32x()? })
    }
    pub fn validate(&self) -> Result<()> {
        finite(&self.centre, "bounds")?;
        finite(&self.min, "bounds")?;
        finite(&self.max, "bounds")?;
        if !(self.radius.is_finite() && self.radius >= 0.0) || (0..3).any(|i| self.min[i] > self.max[i]) {
            return Err(Error::Invalid("bounds: negative radius or min > max".into()));
        }
        Ok(())
    }
}

pub(crate) fn finite(v: &[f32], what: &str) -> Result<()> {
    if v.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(Error::Invalid(format!("{what}: non-finite value")))
    }
}

pub(crate) fn read_asset(r: &mut In) -> Result<AssetId> {
    Ok(AssetId(r.arr()?))
}

/// Optional asset reference: flag byte (0 or 1), then 32 bytes if present.
pub(crate) fn write_opt_asset(o: &mut Out, a: &Option<AssetId>) {
    match a {
        Some(a) => {
            o.u8(1);
            o.bytes(&a.0);
        }
        None => o.u8(0),
    }
}
pub(crate) fn read_opt_asset(r: &mut In) -> Result<Option<AssetId>> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some(read_asset(r)?)),
        f => Err(Error::Corrupt(format!("bad optional-asset flag {f}"))),
    }
}
