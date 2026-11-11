//! `collision` (kind 6, node) and `collision_asset` (kind 23): collision shapes.
//!
//! Schema 1 payload: `shape_count u32`, then per shape `type u8, reserved [3] (0),
//! surface u32`, then the shape fields:
//! ```text
//! 0 box        centre f32x3, half_extents f32x3, rotation f32x4
//! 1 sphere     centre f32x3, radius f32
//! 2 capsule    a f32x3, b f32x3, radius f32
//! 3 hull       point_count u32, points f32x3 x n
//! 4 trimesh    vertex_count u32, vertices f32x3 x n, triangle_count u32, u32x3 x m
//! 5 heightfield origin f32x3, cell_size f32x2, columns u32, rows u32, heights f32 x (columns*rows), row-major
//! ```
//! Coordinates are in the node frame (kind 6) or the asset frame (kind 23).

use super::finite;
use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::key::ChunkKind;
use crate::wire::{In, Out};

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Box { centre: [f32; 3], half_extents: [f32; 3], rotation: [f32; 4] },
    Sphere { centre: [f32; 3], radius: f32 },
    Capsule { a: [f32; 3], b: [f32; 3], radius: f32 },
    ConvexHull { points: Vec<[f32; 3]> },
    TriMesh { vertices: Vec<[f32; 3]>, triangles: Vec<[u32; 3]> },
    HeightField { origin: [f32; 3], cell_size: [f32; 2], columns: u32, rows: u32, heights: Vec<f32> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionShape {
    pub shape: Shape,
    /// Surface class and flags, interpreted by the sim.
    pub surface: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collision {
    /// `true` encodes as `collision_asset` (23), `false` as node `collision` (6).
    pub for_asset: bool,
    pub shapes: Vec<CollisionShape>,
}

impl Shape {
    pub fn validate(&self) -> Result<()> {
        let bad = |s: &str| Err(Error::Invalid(format!("collision: {s}")));
        match self {
            Shape::Box { centre, half_extents, rotation } => {
                finite(centre, "box")?;
                finite(rotation, "box")?;
                finite(half_extents, "box")?;
                if half_extents.iter().any(|h| *h < 0.0) {
                    return bad("negative box extent");
                }
            }
            Shape::Sphere { centre, radius } | Shape::Capsule { a: centre, radius, .. } => {
                finite(centre, "shape")?;
                if !(radius.is_finite() && *radius >= 0.0) {
                    return bad("bad radius");
                }
                if let Shape::Capsule { b, .. } = self {
                    finite(b, "capsule")?;
                }
            }
            Shape::ConvexHull { points } => {
                if points.is_empty() {
                    return bad("empty hull");
                }
                finite(points.as_flattened(), "hull")?;
            }
            Shape::TriMesh { vertices, triangles } => {
                finite(vertices.as_flattened(), "trimesh")?;
                if triangles.as_flattened().iter().any(|i| *i as usize >= vertices.len()) {
                    return bad("triangle index out of range");
                }
            }
            Shape::HeightField { origin, cell_size, columns, rows, heights } => {
                finite(origin, "heightfield")?;
                finite(cell_size, "heightfield")?;
                finite(heights, "heightfield")?;
                if *columns < 2 || *rows < 2 || (*columns as u64 * *rows as u64) != heights.len() as u64 {
                    return bad("heightfield size mismatch");
                }
            }
        }
        Ok(())
    }
}

fn write_points(o: &mut Out, p: &[[f32; 3]]) -> Result<()> {
    o.count(p.len(), "points")?;
    o.f32s(p.as_flattened());
    Ok(())
}
fn read_points(r: &mut In) -> Result<Vec<[f32; 3]>> {
    let n = r.count(12)?;
    (0..n).map(|_| r.f32x()).collect()
}

impl<'a> ChunkBody<'a> for Collision {
    const NAME: &'static str = "Collision";
    fn kind(&self) -> ChunkKind {
        if self.for_asset { ChunkKind::CollisionAsset } else { ChunkKind::Collision }
    }
    fn accepts(kind: ChunkKind) -> bool {
        matches!(kind, ChunkKind::Collision | ChunkKind::CollisionAsset)
    }

    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        let mut o = Out::new(buf);
        o.count(self.shapes.len(), "shapes")?;
        for s in &self.shapes {
            s.shape.validate()?;
            let tag = match &s.shape {
                Shape::Box { .. } => 0,
                Shape::Sphere { .. } => 1,
                Shape::Capsule { .. } => 2,
                Shape::ConvexHull { .. } => 3,
                Shape::TriMesh { .. } => 4,
                Shape::HeightField { .. } => 5,
            };
            o.u8(tag);
            o.bytes(&[0; 3]);
            o.u32(s.surface);
            match &s.shape {
                Shape::Box { centre, half_extents, rotation } => {
                    o.f32s(centre);
                    o.f32s(half_extents);
                    o.f32s(rotation);
                }
                Shape::Sphere { centre, radius } => {
                    o.f32s(centre);
                    o.f32(*radius);
                }
                Shape::Capsule { a, b, radius } => {
                    o.f32s(a);
                    o.f32s(b);
                    o.f32(*radius);
                }
                Shape::ConvexHull { points } => write_points(&mut o, points)?,
                Shape::TriMesh { vertices, triangles } => {
                    write_points(&mut o, vertices)?;
                    o.count(triangles.len(), "triangles")?;
                    for t in triangles {
                        t.iter().for_each(|i| o.u32(*i));
                    }
                }
                Shape::HeightField { origin, cell_size, columns, rows, heights } => {
                    o.f32s(origin);
                    o.f32s(cell_size);
                    o.u32(*columns);
                    o.u32(*rows);
                    o.f32s(heights);
                }
            }
        }
        Ok(())
    }

    fn decode_payload(kind: ChunkKind, p: &'a [u8]) -> Result<Collision> {
        let mut r = In::new(p);
        let n = r.count(8)?;
        let mut shapes = Vec::with_capacity(n);
        for _ in 0..n {
            let tag = r.u8()?;
            r.zeros(3, "shape reserved")?;
            let surface = r.u32()?;
            let shape = match tag {
                0 => Shape::Box { centre: r.f32x()?, half_extents: r.f32x()?, rotation: r.f32x()? },
                1 => Shape::Sphere { centre: r.f32x()?, radius: r.f32()? },
                2 => Shape::Capsule { a: r.f32x()?, b: r.f32x()?, radius: r.f32()? },
                3 => Shape::ConvexHull { points: read_points(&mut r)? },
                4 => {
                    let vertices = read_points(&mut r)?;
                    let m = r.count(12)?;
                    let triangles = (0..m).map(|_| Ok([r.u32()?, r.u32()?, r.u32()?])).collect::<Result<_>>()?;
                    Shape::TriMesh { vertices, triangles }
                }
                5 => {
                    let origin = r.f32x()?;
                    let cell_size = r.f32x()?;
                    let columns = r.u32()?;
                    let rows = r.u32()?;
                    let count = (columns as u64).saturating_mul(rows as u64);
                    if count.saturating_mul(4) > r.remaining() as u64 {
                        return Err(Error::Corrupt("heightfield larger than the chunk".into()));
                    }
                    let heights = (0..count).map(|_| r.f32()).collect::<Result<_>>()?;
                    Shape::HeightField { origin, cell_size, columns, rows, heights }
                }
                t => return Err(Error::Corrupt(format!("collision: bad shape type {t}"))),
            };
            shape.validate().map_err(|e| Error::Corrupt(e.to_string()))?;
            shapes.push(CollisionShape { shape, surface });
        }
        r.finish()?;
        Ok(Collision { for_asset: kind == ChunkKind::CollisionAsset, shapes })
    }
}
