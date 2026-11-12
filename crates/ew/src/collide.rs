//! Pack collision -> `ed_sim` bodies (M4b). `ew` derives no collision of its own: the shapes
//! come from the packs (`collision` chunk of the proving node, `collision_asset` per mesh or
//! prefab, baked by `ew-bake`), and this module only converts them.
//!
//! Frames: the sim's region frame is east, north, up at the proving origin, which is also the
//! pack node frame, so node shapes go in unchanged. Authored placements (layer files) place
//! their asset's shapes with the placement's transform ([`ed_assets::collision::place`]).
//!
//! Node shapes are grouped into one static compound body per [`CELL_M`] square cell (sorted
//! cell keys, shapes in pack order), so ~100k shapes become a few hundred bodies and creation
//! order is deterministic.

use crate::content::PACKS;
use ed_assets::bake::proving_node;
use ed_pack::kinds::{Collision, CollisionShape, Shape};
use ed_pack::{AssetId, ChunkKey, ChunkKind, PackReader, Rung};
use ed_sim::{BodyDesc, BodyKind, Entity, Geometry, ShapeDesc, TriMesh, World};
use glam::{DVec3, Quat, Vec3};
use std::collections::BTreeMap;
use std::path::Path;

/// Side of the cells node shapes are grouped by, metres.
pub const CELL_M: f64 = 80.0;

/// The collision a set of packs carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackCollision {
    /// Proving node shapes, node frame (ENU at the proving origin).
    pub node: Vec<CollisionShape>,
    /// Per mesh or prefab asset, in the asset's own frame.
    pub assets: BTreeMap<AssetId, Vec<CollisionShape>>,
}

impl PackCollision {
    /// Reads the collision chunks of the packs in `dir` (mount order of [`PACKS`], first wins).
    /// Missing optional packs are skipped; a missing proving pack is an error.
    pub fn from_dir(dir: &Path) -> Result<PackCollision, String> {
        let mut readers = Vec::new();
        for s in &PACKS {
            let p = dir.join(format!("{}.ewpack", s.name));
            match PackReader::open_file(&p) {
                Ok(r) => readers.push(r),
                Err(e) if s.required => return Err(format!("{}: {e}\n  build it with: {}", p.display(), s.bake)),
                Err(_) => {}
            }
        }
        let mut keys: BTreeMap<ChunkKey, usize> = BTreeMap::new();
        for (i, r) in readers.iter().enumerate() {
            for e in r.entries() {
                keys.entry(e.key).or_insert(i);
            }
        }
        Self::from_keys(keys.keys().copied(), |k| {
            let c = readers[keys[k]].read(k).map_err(|e| format!("{k:?}: {e}"))?;
            c.decode::<Collision>().map_err(|e| format!("{k:?}: {e}"))
        })
    }

    /// Builds from a merged key list and a chunk decoder (shared with `Content`).
    pub fn from_keys(keys: impl IntoIterator<Item = ChunkKey>, mut read: impl FnMut(&ChunkKey) -> Result<Collision, String>) -> Result<PackCollision, String> {
        let mut out = PackCollision::default();
        let node_key = node_key();
        for k in keys {
            match k {
                ChunkKey::Asset { asset, kind: ChunkKind::CollisionAsset } => {
                    out.assets.insert(asset, read(&k)?.shapes);
                }
                _ if k == node_key => out.node = read(&k)?.shapes,
                _ => {}
            }
        }
        Ok(out)
    }

    pub fn is_collision_key(k: &ChunkKey) -> bool {
        matches!(k, ChunkKey::Asset { kind: ChunkKind::CollisionAsset, .. }) || *k == node_key()
    }
}

pub fn node_key() -> ChunkKey {
    ChunkKey::node(proving_node(), ChunkKind::Collision, Rung::R0)
}

/// Node-frame ENU (f64) <-> renderer y-up offset from the proving origin.
pub fn enu_to_yup(p: DVec3) -> DVec3 {
    DVec3::new(p.x, p.z, -p.y)
}
pub fn yup_to_enu(p: DVec3) -> DVec3 {
    DVec3::new(p.x, -p.z, p.y)
}

/// The sim shapes of `shapes` for a body at `body` (node frame). Heightfields are skipped (none
/// are baked yet); hulls and meshes register geometry in `world`.
pub fn sim_shapes(world: &mut World, shapes: &[CollisionShape], body: DVec3) -> Vec<ShapeDesc> {
    let rel = |p: [f32; 3]| (Vec3::from(p).as_dvec3() - body).as_vec3();
    let mut out = Vec::with_capacity(shapes.len());
    for s in shapes {
        let d = match &s.shape {
            Shape::Box { centre, half_extents, rotation } => ShapeDesc::cuboid(Vec3::from(*half_extents)).at(rel(*centre), Quat::from_array(*rotation).normalize()),
            Shape::Sphere { centre, radius } => ShapeDesc::sphere(*radius).at(rel(*centre), Quat::IDENTITY),
            Shape::Capsule { a, b, radius } => {
                let (a, b) = (rel(*a), rel(*b));
                let axis = b - a;
                let q = Quat::from_rotation_arc(Vec3::Z, axis.try_normalize().unwrap_or(Vec3::Z));
                ShapeDesc::capsule(axis.length() * 0.5, *radius).at((a + b) * 0.5, q)
            }
            Shape::ConvexHull { points } => match world.add_geometry(Geometry::Hull(points.iter().map(|p| rel(*p)).collect())) {
                Ok(g) => ShapeDesc::new(ed_sim::Shape::Hull(g)),
                Err(_) => continue,
            },
            Shape::TriMesh { vertices, triangles } => {
                match world.add_geometry(Geometry::Mesh(TriMesh { vertices: vertices.iter().map(|p| rel(*p)).collect(), triangles: triangles.clone() })) {
                    Ok(g) => ShapeDesc::new(ed_sim::Shape::Mesh(g)),
                    Err(_) => continue,
                }
            }
            Shape::HeightField { .. } => continue,
        };
        out.push(d);
    }
    out
}

/// One static compound body per [`CELL_M`] cell of node shapes. Returns the bodies in creation
/// order (sorted cell keys).
pub fn build_statics(world: &mut World, node: &[CollisionShape]) -> Result<Vec<Entity>, String> {
    let mut cells: BTreeMap<(i64, i64), Vec<CollisionShape>> = BTreeMap::new();
    for s in node {
        let c = ed_assets::collision::centre(&s.shape).as_dvec3();
        cells.entry(((c.x / CELL_M).floor() as i64, (c.y / CELL_M).floor() as i64)).or_default().push(s.clone());
    }
    let mut out = Vec::with_capacity(cells.len());
    for ((cx, cy), shapes) in cells {
        let body = DVec3::new((cx as f64 + 0.5) * CELL_M, (cy as f64 + 0.5) * CELL_M, 0.0);
        let mut desc = BodyDesc::new(BodyKind::Static, body);
        desc.shapes = sim_shapes(world, &shapes, body);
        if desc.shapes.is_empty() {
            continue;
        }
        out.push(world.create_body(desc).map_err(|e| format!("static cell ({cx}, {cy}): {e}"))?);
    }
    Ok(out)
}

/// One static body for already placed shapes (an authored object), at their first centre.
pub fn object_body(world: &mut World, placed: &[CollisionShape]) -> Result<Option<Entity>, String> {
    let Some(first) = placed.first() else { return Ok(None) };
    let body = ed_assets::collision::centre(&first.shape).as_dvec3();
    let mut desc = BodyDesc::new(BodyKind::Static, body);
    desc.shapes = sim_shapes(world, placed, body);
    if desc.shapes.is_empty() {
        return Ok(None);
    }
    world.create_body(desc).map(Some).map_err(|e| format!("object body: {e}"))
}

/// Highest point of a shape (node frame z), for boxes, spheres and capsules.
pub fn top_z(s: &Shape) -> f32 {
    match s {
        Shape::Box { centre, half_extents, rotation } => {
            let q = Quat::from_array(*rotation).normalize();
            let h = Vec3::from(*half_extents);
            centre[2] + (0..3).map(|i| (q * Vec3::AXES[i]).z.abs() * h[i]).sum::<f32>()
        }
        Shape::Sphere { centre, radius } => centre[2] + radius,
        Shape::Capsule { a, b, radius } => a[2].max(b[2]) + radius,
        _ => ed_assets::collision::centre(s).z,
    }
}

/// Distance from `p` to a shape (0 inside), node frame. `None` for shapes not checked here
/// (hulls, meshes and heightfields: none are baked yet).
pub fn distance(p: Vec3, s: &Shape) -> Option<f32> {
    match s {
        Shape::Box { centre, half_extents, rotation } => {
            let l = Quat::from_array(*rotation).normalize().inverse() * (p - Vec3::from(*centre));
            let q = l.abs() - Vec3::from(*half_extents);
            Some(q.max(Vec3::ZERO).length())
        }
        Shape::Sphere { centre, radius } => Some(((p - Vec3::from(*centre)).length() - radius).max(0.0)),
        Shape::Capsule { a, b, radius } => {
            let (a, b) = (Vec3::from(*a), Vec3::from(*b));
            let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-12)).clamp(0.0, 1.0);
            Some(((p - (a + (b - a) * t)).length() - radius).max(0.0))
        }
        _ => None,
    }
}

/// Deepest penetration of an upright capsule (feet at `feet`, node frame) into `shapes`: the
/// largest `radius - distance(axis point, shape)` over 33 points along the axis (0 = clear).
/// Also returns the index of the worst shape.
pub fn capsule_penetration(feet: DVec3, radius: f32, height: f32, shapes: &[CollisionShape]) -> (f32, Option<usize>) {
    let (lo, hi) = (feet.as_vec3() + Vec3::Z * radius, feet.as_vec3() + Vec3::Z * (height - radius));
    let mut worst = (0.0f32, None);
    for (i, s) in shapes.iter().enumerate() {
        // Cheap reject on a bounding sphere.
        let reach = match &s.shape {
            Shape::Box { half_extents, .. } => Vec3::from(*half_extents).length(),
            Shape::Sphere { radius, .. } => *radius,
            Shape::Capsule { a, b, radius } => (Vec3::from(*a) - Vec3::from(*b)).length() * 0.5 + radius,
            _ => continue,
        };
        if (ed_assets::collision::centre(&s.shape) - (lo + hi) * 0.5).length() > reach + height {
            continue;
        }
        for k in 0..=32 {
            let p = lo.lerp(hi, k as f32 / 32.0);
            if let Some(d) = distance(p, &s.shape) {
                let pen = radius - d;
                if pen > worst.0 {
                    worst = (pen, Some(i));
                }
            }
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_assets::collision::{aabb_box, surface};

    #[test]
    fn box_distance_and_capsule_penetration() {
        let b = aabb_box(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 3.0), surface::BUILDING);
        assert_eq!(distance(Vec3::new(3.0, 0.0, 1.0), &b.shape), Some(2.0));
        assert_eq!(distance(Vec3::new(0.0, 0.0, 1.0), &b.shape), Some(0.0));
        let clear = capsule_penetration(DVec3::new(1.3, 0.0, 0.0), 0.3, 1.8, std::slice::from_ref(&b));
        assert!(clear.0 <= 1e-5, "{clear:?}");
        let into = capsule_penetration(DVec3::new(1.2, 0.0, 0.0), 0.3, 1.8, std::slice::from_ref(&b));
        assert!((into.0 - 0.1).abs() < 1e-4, "{into:?}");
    }

    #[test]
    fn statics_group_by_cell_in_a_fixed_order() {
        let shapes: Vec<_> = (0..10).map(|i| aabb_box(Vec3::new(i as f32 * 30.0, 0.0, 0.0), Vec3::new(i as f32 * 30.0 + 1.0, 1.0, 1.0), surface::PROP)).collect();
        let mut w = World::new(&Default::default());
        let bodies = build_statics(&mut w, &shapes).unwrap();
        // x centres 0.5 .. 270.5 -> cells 0..3 (4 bodies).
        assert_eq!(bodies.len(), 4);
        let mut w2 = World::new(&Default::default());
        build_statics(&mut w2, &shapes).unwrap();
        assert_eq!(w.state_hash(), w2.state_hash());
    }
}
