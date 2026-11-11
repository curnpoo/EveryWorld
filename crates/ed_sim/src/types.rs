//! Public descriptions: entities, bodies, shapes, geometry, movers, commands. All in the region
//! frame (x east, y north, z up), metres, seconds, radians.

use crate::SimError;
use crate::codec::{Reader, Writer};
use glam::{DQuat, DVec3, Quat, Vec3};
use std::sync::Arc;

type R<T> = Result<T, SimError>;

/// Generational handle. Ordered by `(index, generation)`; commands are sorted by it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    /// "No entity": issuer of world-level commands (for example a spawn from the server).
    pub const NONE: Entity = Entity { index: u32::MAX, generation: 0 };

    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Entity { index, generation }
    }
    pub fn index(self) -> u32 {
        self.index
    }
    pub fn generation(self) -> u32 {
        self.generation
    }
    pub(crate) fn write(self, w: &mut Writer) {
        w.u32(self.index);
        w.u32(self.generation);
    }
    pub(crate) fn read(r: &mut Reader) -> R<Self> {
        Ok(Entity { index: r.u32()?, generation: r.u32()? })
    }
}

/// Handle to collision geometry registered with [`crate::World::add_geometry`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct GeomId(pub(crate) u32);

/// Collision category bits. Bit 0 is everything ordinary; bit 1 is reserved for movers.
pub mod layers {
    pub const DEFAULT: u64 = 1;
    pub const MOVER: u64 = 1 << 1;
    pub const ALL: u64 = u64::MAX;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Filter {
    /// What this shape is.
    pub category: u64,
    /// What it collides with.
    pub mask: u64,
}

impl Default for Filter {
    fn default() -> Self {
        Filter { category: layers::DEFAULT, mask: layers::ALL }
    }
}

/// Query filter: `mask` selects which shape categories are hit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QueryFilter {
    pub category: u64,
    pub mask: u64,
}

impl Default for QueryFilter {
    fn default() -> Self {
        QueryFilter { category: layers::DEFAULT, mask: layers::ALL }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Material {
    /// kg / m^3.
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
}

impl Default for Material {
    fn default() -> Self {
        Material { density: 1000.0, friction: 0.6, restitution: 0.0 }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    /// Box with half extents along the local axes.
    Cuboid { half_extents: Vec3 },
    Sphere { radius: f32 },
    /// Capsule along local z (up): segment `[-half_height, half_height]` plus `radius`.
    Capsule { half_height: f32, radius: f32 },
    /// Convex hull from [`Geometry::Hull`].
    Hull(GeomId),
    /// Static triangle mesh from [`Geometry::Mesh`]. Static bodies only.
    Mesh(GeomId),
    /// Height field from [`Geometry::HeightField`]. Static bodies only; no local offset.
    HeightField(GeomId),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ShapeDesc {
    pub shape: Shape,
    /// Local offset and rotation within the body (ignored for meshes and height fields).
    pub offset: Vec3,
    pub rotation: Quat,
    pub material: Material,
    pub filter: Filter,
}

impl ShapeDesc {
    pub fn new(shape: Shape) -> Self {
        ShapeDesc { shape, offset: Vec3::ZERO, rotation: Quat::IDENTITY, material: Material::default(), filter: Filter::default() }
    }
    pub fn cuboid(half_extents: Vec3) -> Self {
        Self::new(Shape::Cuboid { half_extents })
    }
    pub fn sphere(radius: f32) -> Self {
        Self::new(Shape::Sphere { radius })
    }
    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self::new(Shape::Capsule { half_height, radius })
    }
    pub fn at(mut self, offset: Vec3, rotation: Quat) -> Self {
        self.offset = offset;
        self.rotation = rotation;
        self
    }
    pub fn with_material(mut self, m: Material) -> Self {
        self.material = m;
        self
    }
    pub fn with_filter(mut self, f: Filter) -> Self {
        self.filter = f;
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyKind {
    Static,
    Kinematic,
    Dynamic,
}

/// A rigid body. Several shapes make a compound.
#[derive(Clone, PartialEq, Debug)]
pub struct BodyDesc {
    pub kind: BodyKind,
    pub position: DVec3,
    pub rotation: DQuat,
    pub linear_velocity: DVec3,
    pub angular_velocity: DVec3,
    pub shapes: Vec<ShapeDesc>,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    /// Continuous collision against dynamic bodies too (fast projectiles).
    pub bullet: bool,
    pub can_sleep: bool,
    pub lock_rotation: bool,
}

impl BodyDesc {
    pub fn new(kind: BodyKind, position: DVec3) -> Self {
        BodyDesc {
            kind,
            position,
            rotation: DQuat::IDENTITY,
            linear_velocity: DVec3::ZERO,
            angular_velocity: DVec3::ZERO,
            shapes: Vec::new(),
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 1.0,
            bullet: false,
            can_sleep: true,
            lock_rotation: false,
        }
    }
    pub fn with_shape(mut self, s: ShapeDesc) -> Self {
        self.shapes.push(s);
        self
    }
    pub fn with_rotation(mut self, q: DQuat) -> Self {
        self.rotation = q;
        self
    }
}

/// Triangle mesh, counter-clockwise front faces.
#[derive(Clone, PartialEq, Debug)]
pub struct TriMesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

/// Height field in raster order: row 0 is the **north** edge, rows run south, columns run east.
/// The owning body's origin is the north-west corner. Height is local z.
#[derive(Clone, PartialEq, Debug)]
pub struct HeightField {
    pub columns: u32,
    pub rows: u32,
    /// Cell size east (between columns) and south (between rows), metres.
    pub spacing: [f32; 2],
    /// `rows * columns` heights, row-major.
    pub heights: Vec<f32>,
}

impl HeightField {
    /// Height at a grid point.
    pub fn at(&self, column: u32, row: u32) -> f32 {
        self.heights[(row * self.columns + column) as usize]
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Geometry {
    /// Convex hull of these points (at most 128 hull vertices).
    Hull(Vec<Vec3>),
    Mesh(TriMesh),
    HeightField(HeightField),
}

/// Character mover: a kinematic capsule swept and slid along geometry.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MoverDesc {
    /// Feet position (bottom of the capsule).
    pub position: DVec3,
    pub radius: f32,
    /// Total height, feet to top.
    pub height: f32,
    /// Ledges up to this height are stepped onto.
    pub step_height: f32,
    /// Steepest walkable slope, radians from horizontal.
    pub max_slope: f32,
    /// While grounded, the feet follow the ground down by up to this much per tick.
    pub snap_distance: f32,
    /// Downward acceleration while airborne, m/s^2.
    pub gravity: f32,
    /// Which shape categories block the mover.
    pub mask: u64,
}

impl Default for MoverDesc {
    fn default() -> Self {
        MoverDesc {
            position: DVec3::ZERO,
            radius: 0.3,
            height: 1.8,
            step_height: 0.35,
            max_slope: 50f32.to_radians(),
            snap_distance: 0.3,
            gravity: 9.80665,
            mask: layers::ALL & !layers::MOVER,
        }
    }
}

/// Held mover input. Stays in effect until replaced.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct MoverInput {
    /// Desired horizontal velocity (z is ignored), m/s.
    pub velocity: DVec3,
    /// Upward speed applied once when grounded; 0 = no jump.
    pub jump_speed: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MoverState {
    pub position: DVec3,
    pub velocity: DVec3,
    pub grounded: bool,
    pub ground_normal: DVec3,
}

/// One input to the simulation, applied at the start of a tick.
#[derive(Clone, PartialEq, Debug)]
pub struct Command {
    /// Target entity (or the issuer for spawns; may be [`Entity::NONE`]).
    pub entity: Entity,
    pub sequence: u32,
    pub kind: CommandKind,
}

#[derive(Clone, PartialEq, Debug)]
pub enum CommandKind {
    Spawn(Arc<BodyDesc>),
    SpawnMover(MoverDesc),
    Despawn,
    MoverInput(MoverInput),
    ApplyImpulse(DVec3),
    SetVelocity { linear: DVec3, angular: DVec3 },
    /// Kinematic bodies: move to this pose over the coming tick.
    SetKinematicTarget { position: DVec3, rotation: DQuat },
    Teleport { position: DVec3, rotation: DQuat },
}

// ---------------------------------------------------------------------------------------------
// Encoding

pub(crate) fn write_shape(w: &mut Writer, s: &ShapeDesc) {
    match s.shape {
        Shape::Cuboid { half_extents } => {
            w.u8(0);
            w.vec3(half_extents);
        }
        Shape::Sphere { radius } => {
            w.u8(1);
            w.f32(radius);
        }
        Shape::Capsule { half_height, radius } => {
            w.u8(2);
            w.f32(half_height);
            w.f32(radius);
        }
        Shape::Hull(g) => {
            w.u8(3);
            w.u32(g.0);
        }
        Shape::Mesh(g) => {
            w.u8(4);
            w.u32(g.0);
        }
        Shape::HeightField(g) => {
            w.u8(5);
            w.u32(g.0);
        }
    }
    w.vec3(s.offset);
    w.quat(s.rotation);
    w.f32(s.material.density);
    w.f32(s.material.friction);
    w.f32(s.material.restitution);
    w.u64(s.filter.category);
    w.u64(s.filter.mask);
}

pub(crate) fn read_shape(r: &mut Reader) -> R<ShapeDesc> {
    let shape = match r.u8()? {
        0 => Shape::Cuboid { half_extents: r.vec3()? },
        1 => Shape::Sphere { radius: r.f32()? },
        2 => Shape::Capsule { half_height: r.f32()?, radius: r.f32()? },
        3 => Shape::Hull(GeomId(r.u32()?)),
        4 => Shape::Mesh(GeomId(r.u32()?)),
        5 => Shape::HeightField(GeomId(r.u32()?)),
        _ => return Err(SimError::Corrupt("shape kind")),
    };
    Ok(ShapeDesc {
        shape,
        offset: r.vec3()?,
        rotation: r.quat()?,
        material: Material { density: r.f32()?, friction: r.f32()?, restitution: r.f32()? },
        filter: Filter { category: r.u64()?, mask: r.u64()? },
    })
}

pub(crate) fn write_body(w: &mut Writer, b: &BodyDesc) {
    w.u8(match b.kind {
        BodyKind::Static => 0,
        BodyKind::Kinematic => 1,
        BodyKind::Dynamic => 2,
    });
    w.dvec3(b.position);
    w.dquat(b.rotation);
    w.dvec3(b.linear_velocity);
    w.dvec3(b.angular_velocity);
    w.len(b.shapes.len());
    for s in &b.shapes {
        write_shape(w, s);
    }
    w.f32(b.linear_damping);
    w.f32(b.angular_damping);
    w.f32(b.gravity_scale);
    w.bool(b.bullet);
    w.bool(b.can_sleep);
    w.bool(b.lock_rotation);
}

pub(crate) fn read_body(r: &mut Reader) -> R<BodyDesc> {
    let kind = match r.u8()? {
        0 => BodyKind::Static,
        1 => BodyKind::Kinematic,
        2 => BodyKind::Dynamic,
        _ => return Err(SimError::Corrupt("body kind")),
    };
    let position = r.dvec3()?;
    let rotation = r.dquat()?;
    let linear_velocity = r.dvec3()?;
    let angular_velocity = r.dvec3()?;
    let n = r.len(8)?;
    let mut shapes = Vec::with_capacity(n);
    for _ in 0..n {
        shapes.push(read_shape(r)?);
    }
    Ok(BodyDesc {
        kind,
        position,
        rotation,
        linear_velocity,
        angular_velocity,
        shapes,
        linear_damping: r.f32()?,
        angular_damping: r.f32()?,
        gravity_scale: r.f32()?,
        bullet: r.bool()?,
        can_sleep: r.bool()?,
        lock_rotation: r.bool()?,
    })
}

pub(crate) fn write_mover_desc(w: &mut Writer, m: &MoverDesc) {
    w.dvec3(m.position);
    w.f32(m.radius);
    w.f32(m.height);
    w.f32(m.step_height);
    w.f32(m.max_slope);
    w.f32(m.snap_distance);
    w.f32(m.gravity);
    w.u64(m.mask);
}

pub(crate) fn read_mover_desc(r: &mut Reader) -> R<MoverDesc> {
    Ok(MoverDesc {
        position: r.dvec3()?,
        radius: r.f32()?,
        height: r.f32()?,
        step_height: r.f32()?,
        max_slope: r.f32()?,
        snap_distance: r.f32()?,
        gravity: r.f32()?,
        mask: r.u64()?,
    })
}

pub(crate) fn write_geometry(w: &mut Writer, g: &Geometry) {
    match g {
        Geometry::Hull(p) => {
            w.u8(0);
            w.len(p.len());
            for v in p {
                w.vec3(*v);
            }
        }
        Geometry::Mesh(m) => {
            w.u8(1);
            w.len(m.vertices.len());
            for v in &m.vertices {
                w.vec3(*v);
            }
            w.len(m.triangles.len());
            for t in &m.triangles {
                w.u32(t[0]);
                w.u32(t[1]);
                w.u32(t[2]);
            }
        }
        Geometry::HeightField(h) => {
            w.u8(2);
            w.u32(h.columns);
            w.u32(h.rows);
            w.f32(h.spacing[0]);
            w.f32(h.spacing[1]);
            w.len(h.heights.len());
            for v in &h.heights {
                w.f32(*v);
            }
        }
    }
}

pub(crate) fn read_geometry(r: &mut Reader) -> R<Geometry> {
    Ok(match r.u8()? {
        0 => {
            let n = r.len(12)?;
            let mut p = Vec::with_capacity(n);
            for _ in 0..n {
                p.push(r.vec3()?);
            }
            Geometry::Hull(p)
        }
        1 => {
            let n = r.len(12)?;
            let mut vertices = Vec::with_capacity(n);
            for _ in 0..n {
                vertices.push(r.vec3()?);
            }
            let t = r.len(12)?;
            let mut triangles = Vec::with_capacity(t);
            for _ in 0..t {
                triangles.push([r.u32()?, r.u32()?, r.u32()?]);
            }
            Geometry::Mesh(TriMesh { vertices, triangles })
        }
        2 => {
            let columns = r.u32()?;
            let rows = r.u32()?;
            let spacing = [r.f32()?, r.f32()?];
            let n = r.len(4)?;
            let mut heights = Vec::with_capacity(n);
            for _ in 0..n {
                heights.push(r.f32()?);
            }
            Geometry::HeightField(HeightField { columns, rows, spacing, heights })
        }
        _ => return Err(SimError::Corrupt("geometry kind")),
    })
}

pub(crate) fn write_command(w: &mut Writer, c: &Command) {
    c.entity.write(w);
    w.u32(c.sequence);
    match &c.kind {
        CommandKind::Spawn(b) => {
            w.u8(0);
            write_body(w, b);
        }
        CommandKind::SpawnMover(m) => {
            w.u8(1);
            write_mover_desc(w, m);
        }
        CommandKind::Despawn => w.u8(2),
        CommandKind::MoverInput(i) => {
            w.u8(3);
            w.dvec3(i.velocity);
            w.f32(i.jump_speed);
        }
        CommandKind::ApplyImpulse(v) => {
            w.u8(4);
            w.dvec3(*v);
        }
        CommandKind::SetVelocity { linear, angular } => {
            w.u8(5);
            w.dvec3(*linear);
            w.dvec3(*angular);
        }
        CommandKind::SetKinematicTarget { position, rotation } => {
            w.u8(6);
            w.dvec3(*position);
            w.dquat(*rotation);
        }
        CommandKind::Teleport { position, rotation } => {
            w.u8(7);
            w.dvec3(*position);
            w.dquat(*rotation);
        }
    }
}

pub(crate) fn read_command(r: &mut Reader) -> R<Command> {
    let entity = Entity::read(r)?;
    let sequence = r.u32()?;
    let kind = match r.u8()? {
        0 => CommandKind::Spawn(Arc::new(read_body(r)?)),
        1 => CommandKind::SpawnMover(read_mover_desc(r)?),
        2 => CommandKind::Despawn,
        3 => CommandKind::MoverInput(MoverInput { velocity: r.dvec3()?, jump_speed: r.f32()? }),
        4 => CommandKind::ApplyImpulse(r.dvec3()?),
        5 => CommandKind::SetVelocity { linear: r.dvec3()?, angular: r.dvec3()? },
        6 => CommandKind::SetKinematicTarget { position: r.dvec3()?, rotation: r.dquat()? },
        7 => CommandKind::Teleport { position: r.dvec3()?, rotation: r.dquat()? },
        _ => return Err(SimError::Corrupt("command kind")),
    };
    Ok(Command { entity, sequence, kind })
}
