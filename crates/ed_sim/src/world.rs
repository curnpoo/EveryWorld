//! The simulation world: entity table, Box3D world, movers, queries, hashing and snapshots.

use crate::codec::{Fnv, Reader, Writer};
use crate::ffi::{self, B3Quat, B3Vec3, EdBodyDesc, EdBodyState, EdHit, EdShapeMat, EdWorldDesc};
use crate::frame::{dquat_from_b, dquat_to_b, dvec_from_b, dvec_to_b, pos_from_b, pos_to_b, quat_to_b, vec_from_b, vec_to_b};
use crate::mover::{self, MoverRt};
use crate::types::*;
use crate::{DT, SUBSTEPS, SimError};
use glam::{DQuat, DVec3, Quat, Vec3};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, OnceLock};

type R<T> = Result<T, SimError>;

/// Box3D keeps its worlds in a global array and `b3CreateWorld` is not thread-safe, so creation
/// and destruction are serialised. Each world is otherwise independent.
fn backend_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Identifies the binary layout of everything a snapshot carries: our boundary structs and
/// Box3D's (its image also carries its own layout hash). Snapshots only restore on a build with
/// the same value.
fn build_id() -> u64 {
    static ID: OnceLock<u64> = OnceLock::new();
    *ID.get_or_init(|| {
        let mut h = Fnv::new();
        h.bytes(b"ed_sim snapshot v1");
        for v in ffi::c_layout() {
            h.u64(v);
        }
        h.0
    })
}

// ---------------------------------------------------------------------------------------------
// Public value types

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SimConfig {
    pub seed: u64,
    /// Pinned data version; part of the world's identity, carried in snapshots and replays.
    pub data_version: u64,
    /// Region origin, ECEF metres. The region frame is east, north, up there.
    pub origin_ecef: DVec3,
    /// Uniform gravity in the region frame, m/s^2. (Per-body radial gravity is not built yet.)
    pub gravity: DVec3,
    pub enable_sleep: bool,
    pub enable_continuous: bool,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            seed: 0,
            data_version: 0,
            origin_ecef: DVec3::ZERO,
            gravity: DVec3::new(0.0, 0.0, -9.80665),
            enable_sleep: true,
            enable_continuous: true,
        }
    }
}

/// Inputs for one tick.
#[derive(Clone, Copy, Debug)]
pub struct SimInput<'a> {
    /// The tick being simulated; must equal [`World::tick`].
    pub tick: u64,
    /// Sorted by `(entity, sequence)`.
    pub commands: &'a [Command],
    /// Simulated UTC seconds (derived from the tick by the caller, never the wall clock).
    pub utc: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepOutput {
    /// The tick that was simulated.
    pub tick: u64,
    /// Entities created this tick: `(issuer, sequence, new entity)`.
    pub spawned: Vec<(Entity, u32, Entity)>,
    /// Commands that were dropped: `(entity, sequence, reason)`.
    pub rejected: Vec<(Entity, u32, SimError)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BodyState {
    pub position: DVec3,
    pub rotation: DQuat,
    pub linear_velocity: DVec3,
    pub angular_velocity: DVec3,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RayHit {
    pub entity: Entity,
    pub point: DVec3,
    pub normal: Vec3,
    /// Fraction of the translation travelled.
    pub fraction: f32,
}

pub type ShapeHit = RayHit;

/// A shape to sweep with [`World::cast_shape`], centred on the cast origin.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CastShape {
    Sphere { radius: f32 },
    /// Along region z.
    Capsule { half_height: f32, radius: f32 },
    Cuboid { half_extents: Vec3, rotation: Quat },
}

// ---------------------------------------------------------------------------------------------
// Geometry registry

struct GeomBackend(*mut c_void);

// SAFETY: Box3D hull, mesh and height-field data are immutable after creation and only read by
// the backend; the pointer is freed exactly once, in Drop.
unsafe impl Send for GeomBackend {}
// SAFETY: as above, shared reads of immutable data.
unsafe impl Sync for GeomBackend {}

pub(crate) struct GeomEntry {
    pub geom: Geometry,
    backend: GeomBackend,
    /// Hull edges in the hull's local region frame (for debug draw).
    pub hull_edges: Vec<[Vec3; 2]>,
}

impl Drop for GeomEntry {
    fn drop(&mut self) {
        let p = self.backend.0;
        // SAFETY: p came from the matching create function and is destroyed once. Shapes that
        // still use it belong to a World that holds this entry through an Arc, so it outlives them.
        unsafe {
            match self.geom {
                Geometry::Hull(_) => ffi::ed_hull_destroy(p),
                Geometry::Mesh(_) => ffi::ed_mesh_destroy(p),
                Geometry::HeightField(_) => ffi::ed_height_field_destroy(p),
            }
        }
    }
}

impl GeomEntry {
    fn new(geom: Geometry) -> R<Self> {
        match &geom {
            Geometry::Hull(points) => {
                if points.len() < 4 || points.iter().any(|p| !p.is_finite()) {
                    return Err(SimError::InvalidDesc("hull needs at least 4 finite points"));
                }
                let pts: Vec<B3Vec3> = points.iter().map(|p| vec_to_b(*p)).collect();
                // SAFETY: pts holds `len` points; the shim copies them.
                let h = unsafe { ffi::ed_hull_create(pts.as_ptr(), pts.len() as i32) };
                if h.is_null() {
                    return Err(SimError::Backend("hull construction failed (degenerate points?)"));
                }
                let mut buf = vec![0f32; 6 * 512];
                // SAFETY: buf holds 512 edges of 6 floats and that capacity is passed.
                let n = unsafe { ffi::ed_hull_edges(h, buf.as_mut_ptr(), 512) };
                let hull_edges = (0..n.max(0) as usize)
                    .map(|i| {
                        let e = &buf[6 * i..6 * i + 6];
                        [vec_from_b(B3Vec3 { x: e[0], y: e[1], z: e[2] }), vec_from_b(B3Vec3 { x: e[3], y: e[4], z: e[5] })]
                    })
                    .collect();
                Ok(GeomEntry { geom, backend: GeomBackend(h), hull_edges })
            }
            Geometry::Mesh(m) => {
                if m.triangles.is_empty() || m.vertices.iter().any(|p| !p.is_finite()) {
                    return Err(SimError::InvalidDesc("mesh needs triangles and finite vertices"));
                }
                let nv = m.vertices.len() as u32;
                if m.triangles.iter().flatten().any(|&i| i >= nv) {
                    return Err(SimError::InvalidDesc("mesh index out of range"));
                }
                let verts: Vec<B3Vec3> = m.vertices.iter().map(|p| vec_to_b(*p)).collect();
                let idx: Vec<i32> = m.triangles.iter().flatten().map(|&i| i as i32).collect();
                // SAFETY: verts/idx are sized as passed; Box3D copies them into its mesh.
                let h = unsafe { ffi::ed_mesh_create(verts.as_ptr(), verts.len() as i32, idx.as_ptr(), m.triangles.len() as i32) };
                if h.is_null() {
                    return Err(SimError::Backend("mesh construction failed"));
                }
                Ok(GeomEntry { geom, backend: GeomBackend(h), hull_edges: Vec::new() })
            }
            Geometry::HeightField(hf) => {
                if hf.columns < 2 || hf.rows < 2 || hf.heights.len() != (hf.columns as usize) * (hf.rows as usize) {
                    return Err(SimError::InvalidDesc("height field needs >= 2x2 points and rows*columns heights"));
                }
                if !(hf.spacing[0] > 0.0 && hf.spacing[1] > 0.0) || hf.heights.iter().any(|h| !h.is_finite()) {
                    return Err(SimError::InvalidDesc("height field spacing must be positive, heights finite"));
                }
                let mut lo = f32::MAX;
                let mut hi = f32::MIN;
                for &h in &hf.heights {
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
                if hi - lo < 1e-3 {
                    hi = lo + 1e-3;
                }
                // Row 0 = north edge, rows run south = Box3D +z; columns east = +x. Same order.
                let scale = B3Vec3 { x: hf.spacing[0], y: 1.0, z: hf.spacing[1] };
                // SAFETY: heights holds countX*countZ values; Box3D copies (quantises) them.
                let h = unsafe {
                    ffi::ed_height_field_create(hf.heights.as_ptr(), hf.columns as i32, hf.rows as i32, &scale, lo, hi)
                };
                if h.is_null() {
                    return Err(SimError::Backend("height field construction failed"));
                }
                Ok(GeomEntry { geom, backend: GeomBackend(h), hull_edges: Vec::new() })
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Entity table

#[derive(Clone)]
pub(crate) enum Payload {
    Body(Arc<BodyDesc>),
    Mover(Box<MoverRt>),
}

#[derive(Clone)]
pub(crate) struct Live {
    /// Backend body key (Box3D index and generation).
    pub key: u64,
    pub kind: BodyKind,
    pub payload: Payload,
}

#[derive(Clone)]
pub(crate) struct Slot {
    pub generation: u32,
    pub live: Option<Live>,
}

#[derive(Clone)]
struct Tables {
    tick: u64,
    rng: u64,
    slots: Vec<Slot>,
    free: Vec<u32>,
    geometry: Vec<Arc<GeomEntry>>,
}

/// A complete, self-contained world state: our tables plus Box3D's serialized world (bodies,
/// shapes, contacts with warm-start impulses, islands, sleep state, broad-phase).
#[derive(Clone)]
pub struct Snapshot {
    config: SimConfig,
    tables: Tables,
    backend: Arc<Vec<u8>>,
}

pub struct World {
    config: SimConfig,
    id: u32,
    t: Tables,
    /// Backend body key -> entity index. Derived from `t`.
    by_key: BTreeMap<u64, u32>,
    poisoned: bool,
    /// Box3D queries are not documented as safe to run concurrently on one world.
    _not_sync: PhantomData<std::cell::Cell<()>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _g = backend_lock();
        // SAFETY: id is this world's live Box3D world, destroyed once. Geometry it references is
        // dropped after this (fields drop after `drop`).
        unsafe { ffi::ed_world_destroy(self.id) };
    }
}

fn splitmix(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn kind_code(k: BodyKind) -> i32 {
    match k {
        BodyKind::Static => 0,
        BodyKind::Kinematic => 1,
        BodyKind::Dynamic => 2,
    }
}

fn shape_mat(s: &ShapeDesc) -> EdShapeMat {
    EdShapeMat {
        category_bits: s.filter.category,
        mask_bits: s.filter.mask,
        density: s.material.density,
        friction: s.material.friction,
        restitution: s.material.restitution,
        pad: 0.0,
    }
}

impl World {
    /// # Panics
    /// If the backend has no free world slot (Box3D allows 128 at once).
    pub fn new(config: &SimConfig) -> World {
        let desc = EdWorldDesc {
            gravity: dvec_to_b(config.gravity),
            enable_sleep: config.enable_sleep as u8,
            enable_continuous: config.enable_continuous as u8,
            pad: [0; 2],
        };
        let id = {
            let _g = backend_lock();
            // SAFETY: desc is a valid local; creation is serialised by the lock.
            unsafe { ffi::ed_world_create(&desc) }
        };
        assert!(id != 0, "Box3D world limit reached");
        World {
            config: *config,
            id,
            t: Tables { tick: 0, rng: splitmix(config.seed), slots: Vec::new(), free: Vec::new(), geometry: Vec::new() },
            by_key: BTreeMap::new(),
            poisoned: false,
            _not_sync: PhantomData,
        }
    }

    pub fn config(&self) -> &SimConfig {
        &self.config
    }

    /// The next tick to be simulated.
    pub fn tick(&self) -> u64 {
        self.t.tick
    }

    /// Registers collision geometry for hull, mesh and height-field shapes.
    pub fn add_geometry(&mut self, geom: Geometry) -> Result<GeomId, SimError> {
        let e = GeomEntry::new(geom)?;
        self.t.geometry.push(Arc::new(e));
        Ok(GeomId((self.t.geometry.len() - 1) as u32))
    }

    pub fn geometry(&self, id: GeomId) -> Option<&Geometry> {
        self.t.geometry.get(id.0 as usize).map(|e| &e.geom)
    }

    pub(crate) fn geom_entry(&self, id: GeomId) -> Option<&GeomEntry> {
        self.t.geometry.get(id.0 as usize).map(|e| &**e)
    }

    fn alloc(&mut self) -> Entity {
        if let Some(i) = self.t.free.pop() {
            Entity::new(i, self.t.slots[i as usize].generation)
        } else {
            self.t.slots.push(Slot { generation: 0, live: None });
            Entity::new((self.t.slots.len() - 1) as u32, 0)
        }
    }

    fn live(&self, e: Entity) -> Option<&Live> {
        let s = self.t.slots.get(e.index() as usize)?;
        if s.generation != e.generation() {
            return None;
        }
        s.live.as_ref()
    }

    fn live_mut(&mut self, e: Entity) -> Option<&mut Live> {
        let s = self.t.slots.get_mut(e.index() as usize)?;
        if s.generation != e.generation() {
            return None;
        }
        s.live.as_mut()
    }

    pub fn contains(&self, e: Entity) -> bool {
        self.live(e).is_some()
    }

    /// Live entities in index order.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.t.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.live.is_some())
            .map(|(i, s)| Entity::new(i as u32, s.generation))
    }

    pub fn body_count(&self) -> usize {
        self.t.slots.iter().filter(|s| s.live.is_some()).count()
    }

    pub fn awake_body_count(&self) -> usize {
        // SAFETY: plain query on our world.
        unsafe { ffi::ed_world_awake_body_count(self.id) }.max(0) as usize
    }

    fn validate(&self, d: &BodyDesc) -> R<()> {
        if !d.position.is_finite() || !d.rotation.is_finite() || d.rotation.length_squared() < 1e-12 {
            return Err(SimError::InvalidDesc("non-finite pose"));
        }
        for s in &d.shapes {
            let geom = |g: GeomId| self.geom_entry(g).ok_or(SimError::InvalidDesc("unknown geometry id"));
            match s.shape {
                Shape::Cuboid { half_extents: h } => {
                    if !(h.x > 0.0 && h.y > 0.0 && h.z > 0.0) {
                        return Err(SimError::InvalidDesc("box half extents must be positive"));
                    }
                }
                Shape::Sphere { radius } => {
                    if radius.is_nan() || radius <= 0.0 {
                        return Err(SimError::InvalidDesc("sphere radius must be positive"));
                    }
                }
                Shape::Capsule { half_height, radius } => {
                    if !(radius > 0.0 && half_height >= 0.0) {
                        return Err(SimError::InvalidDesc("capsule radius must be positive"));
                    }
                }
                Shape::Hull(g) => {
                    if !matches!(geom(g)?.geom, Geometry::Hull(_)) {
                        return Err(SimError::InvalidDesc("Shape::Hull needs Geometry::Hull"));
                    }
                }
                Shape::Mesh(g) | Shape::HeightField(g) => {
                    let ok = matches!(
                        (&s.shape, &geom(g)?.geom),
                        (Shape::Mesh(_), Geometry::Mesh(_)) | (Shape::HeightField(_), Geometry::HeightField(_))
                    );
                    if !ok {
                        return Err(SimError::InvalidDesc("shape and geometry kinds differ"));
                    }
                    if d.kind != BodyKind::Static {
                        return Err(SimError::InvalidDesc("meshes and height fields go on static bodies only"));
                    }
                    if s.offset != Vec3::ZERO || s.rotation != Quat::IDENTITY {
                        return Err(SimError::InvalidDesc("meshes and height fields take no local offset; move the body"));
                    }
                }
            }
        }
        Ok(())
    }

    /// Creates a body now. Use this for scene setup; during play spawn through commands so the
    /// replay sees it.
    pub fn create_body(&mut self, desc: BodyDesc) -> Result<Entity, SimError> {
        self.create_body_arc(Arc::new(desc))
    }

    fn create_body_arc(&mut self, desc: Arc<BodyDesc>) -> R<Entity> {
        self.validate(&desc)?;
        let d = &*desc;
        let bd = EdBodyDesc {
            position: pos_to_b(d.position),
            rotation: dquat_to_b(d.rotation),
            linear_velocity: dvec_to_b(d.linear_velocity),
            angular_velocity: dvec_to_b(d.angular_velocity),
            linear_damping: d.linear_damping,
            angular_damping: d.angular_damping,
            gravity_scale: d.gravity_scale,
            kind: kind_code(d.kind),
            is_bullet: d.bullet as u8,
            enable_sleep: d.can_sleep as u8,
            lock_rotation: d.lock_rotation as u8,
            pad: 0,
        };
        // SAFETY: bd is a valid local; our world is live.
        let key = unsafe { ffi::ed_body_create(self.id, &bd) };
        for s in &d.shapes {
            self.add_shape(key, s);
        }
        let e = self.alloc();
        self.t.slots[e.index() as usize].live = Some(Live { key, kind: d.kind, payload: Payload::Body(desc) });
        self.by_key.insert(key, e.index());
        Ok(e)
    }

    fn add_shape(&self, key: u64, s: &ShapeDesc) {
        let m = shape_mat(s);
        let p = vec_to_b(s.offset);
        let q = quat_to_b(s.rotation);
        // SAFETY (all arms): pointers are to live locals or to geometry kept alive by
        // `self.t.geometry`; the shape was validated, so the geometry kind matches.
        unsafe {
            match s.shape {
                Shape::Cuboid { half_extents: h } => {
                    // Half extents along local (east, north, up) = Box3D local (x, z, y).
                    let hb = B3Vec3 { x: h.x, y: h.z, z: h.y };
                    ffi::ed_shape_box(self.id, key, &m, &hb, &p, &q);
                }
                Shape::Sphere { radius } => {
                    ffi::ed_shape_sphere(self.id, key, &m, &p, radius);
                }
                Shape::Capsule { half_height, radius } => {
                    let axis = s.rotation * Vec3::new(0.0, 0.0, half_height);
                    let c = ffi::B3Capsule { center1: vec_to_b(s.offset - axis), center2: vec_to_b(s.offset + axis), radius };
                    ffi::ed_shape_capsule(self.id, key, &m, &c);
                }
                Shape::Hull(g) => {
                    let e = &self.t.geometry[g.0 as usize];
                    ffi::ed_shape_hull(self.id, key, &m, e.backend.0, &p, &q);
                }
                Shape::Mesh(g) => {
                    let e = &self.t.geometry[g.0 as usize];
                    ffi::ed_shape_mesh(self.id, key, &m, e.backend.0);
                }
                Shape::HeightField(g) => {
                    let e = &self.t.geometry[g.0 as usize];
                    ffi::ed_shape_height_field(self.id, key, &m, e.backend.0);
                }
            }
        }
    }

    /// Creates a character mover now (see [`World::create_body`] on when to prefer commands).
    pub fn create_mover(&mut self, desc: MoverDesc) -> Result<Entity, SimError> {
        let ok = desc.radius > 0.0
            && desc.height > 2.0 * desc.radius
            && desc.step_height >= 0.0
            && desc.step_height <= desc.height - 2.0 * desc.radius
            && desc.max_slope > 0.0
            && desc.max_slope < std::f32::consts::FRAC_PI_2
            && desc.snap_distance >= 0.0
            && desc.position.is_finite();
        if !ok {
            return Err(SimError::InvalidDesc(
                "mover: radius > 0, height > 2 radius, 0 <= step <= height - 2 radius, 0 < slope < 90 deg",
            ));
        }
        let bd = EdBodyDesc {
            position: pos_to_b(desc.position),
            rotation: B3Quat::default(),
            linear_velocity: B3Vec3::default(),
            angular_velocity: B3Vec3::default(),
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 0.0,
            kind: kind_code(BodyKind::Kinematic),
            is_bullet: 0,
            enable_sleep: 0,
            lock_rotation: 1,
            pad: 0,
        };
        let cap = mover::body_capsule(&desc);
        let m = EdShapeMat {
            category_bits: layers::MOVER,
            mask_bits: layers::ALL,
            density: 1000.0,
            friction: 0.0,
            restitution: 0.0,
            pad: 0.0,
        };
        // SAFETY: valid locals; our world is live.
        let key = unsafe {
            let key = ffi::ed_body_create(self.id, &bd);
            ffi::ed_shape_capsule(self.id, key, &m, &cap);
            key
        };
        let e = self.alloc();
        self.t.slots[e.index() as usize].live =
            Some(Live { key, kind: BodyKind::Kinematic, payload: Payload::Mover(Box::new(MoverRt::new(desc))) });
        self.by_key.insert(key, e.index());
        Ok(e)
    }

    fn destroy(&mut self, e: Entity) -> bool {
        let Some(live) = self.live(e).cloned() else { return false };
        // SAFETY: key is a live body of our world, destroyed once.
        unsafe { ffi::ed_body_destroy(self.id, live.key) };
        self.by_key.remove(&live.key);
        let s = &mut self.t.slots[e.index() as usize];
        s.live = None;
        s.generation = s.generation.wrapping_add(1);
        self.t.free.push(e.index());
        true
    }

    /// Removes an entity now. Returns false if it was not live.
    pub fn despawn(&mut self, e: Entity) -> bool {
        self.destroy(e)
    }

    /// Advances exactly one tick ([`DT`], [`SUBSTEPS`] substeps).
    ///
    /// # Panics
    /// If `input.tick` is not [`World::tick`], if commands are not sorted by
    /// `(entity, sequence)`, or if a failed restore left the world unusable.
    pub fn step(&mut self, input: &SimInput) -> StepOutput {
        assert!(!self.poisoned, "world unusable after a failed restore");
        assert_eq!(input.tick, self.t.tick, "SimInput.tick must be the world's next tick");
        assert!(
            input.commands.windows(2).all(|w| (w[0].entity, w[0].sequence) <= (w[1].entity, w[1].sequence)),
            "commands must be sorted by (entity, sequence)"
        );
        let mut out = StepOutput { tick: self.t.tick, ..Default::default() };
        for c in input.commands {
            if let Err(err) = self.apply(c, &mut out) {
                out.rejected.push((c.entity, c.sequence, err));
            }
        }

        // Movers, in entity order, against the world as it stands before the physics step.
        let dt = DT as f32;
        for i in 0..self.t.slots.len() {
            let Some(live) = self.t.slots[i].live.as_mut() else { continue };
            let Payload::Mover(m) = &mut live.payload else { continue };
            mover::update(self.id, m, dt);
            let p = pos_to_b(m.feet);
            let q = B3Quat::default();
            // SAFETY: valid locals; the key is a live kinematic body of our world.
            unsafe { ffi::ed_body_set_target(self.id, live.key, &p, &q, dt) };
        }

        // SAFETY: our world is live and not locked.
        unsafe { ffi::ed_world_step(self.id, dt, SUBSTEPS as i32) };
        self.t.tick += 1;
        out
    }

    fn apply(&mut self, c: &Command, out: &mut StepOutput) -> R<()> {
        const STALE: SimError = SimError::InvalidDesc("no such entity");
        match &c.kind {
            CommandKind::Spawn(d) => {
                let e = self.create_body_arc(d.clone())?;
                out.spawned.push((c.entity, c.sequence, e));
            }
            CommandKind::SpawnMover(d) => {
                let e = self.create_mover(*d)?;
                out.spawned.push((c.entity, c.sequence, e));
            }
            CommandKind::Despawn => {
                if !self.destroy(c.entity) {
                    return Err(STALE);
                }
            }
            CommandKind::MoverInput(i) => {
                let live = self.live_mut(c.entity).ok_or(STALE)?;
                let Payload::Mover(m) = &mut live.payload else {
                    return Err(SimError::InvalidDesc("not a mover"));
                };
                if !i.velocity.is_finite() || !i.jump_speed.is_finite() {
                    return Err(SimError::InvalidDesc("non-finite mover input"));
                }
                m.input = *i;
            }
            CommandKind::ApplyImpulse(j) => {
                let live = self.live(c.entity).ok_or(STALE)?;
                if live.kind != BodyKind::Dynamic {
                    return Err(SimError::InvalidDesc("impulse on a non-dynamic body"));
                }
                let jb = dvec_to_b(*j);
                // SAFETY: valid local; live key of our world.
                unsafe { ffi::ed_body_apply_impulse(self.id, live.key, &jb) };
            }
            CommandKind::SetVelocity { linear, angular } => {
                let live = self.live(c.entity).ok_or(STALE)?;
                if live.kind == BodyKind::Static || matches!(live.payload, Payload::Mover(_)) {
                    return Err(SimError::InvalidDesc("velocity on a static body or mover"));
                }
                let (v, w) = (dvec_to_b(*linear), dvec_to_b(*angular));
                // SAFETY: valid locals; live key of our world.
                unsafe { ffi::ed_body_set_velocity(self.id, live.key, &v, &w) };
            }
            CommandKind::SetKinematicTarget { position, rotation } => {
                let live = self.live(c.entity).ok_or(STALE)?;
                if live.kind != BodyKind::Kinematic || matches!(live.payload, Payload::Mover(_)) {
                    return Err(SimError::InvalidDesc("kinematic target on a non-kinematic body or mover"));
                }
                let (p, q) = (pos_to_b(*position), dquat_to_b(*rotation));
                // SAFETY: valid locals; live key of our world.
                unsafe { ffi::ed_body_set_target(self.id, live.key, &p, &q, DT as f32) };
            }
            CommandKind::Teleport { position, rotation } => {
                let id = self.id;
                let live = self.live_mut(c.entity).ok_or(STALE)?;
                let mut q = dquat_to_b(*rotation);
                if let Payload::Mover(m) = &mut live.payload {
                    m.feet = *position;
                    m.velocity = DVec3::ZERO;
                    m.grounded = false;
                    m.ground_normal = DVec3::Z;
                    q = B3Quat::default();
                }
                let p = pos_to_b(*position);
                // SAFETY: valid locals; live key of our world.
                unsafe { ffi::ed_body_set_transform(id, live.key, &p, &q) };
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------------------
    // State access

    fn read_states(&self, keys: &[u64]) -> Vec<EdBodyState> {
        let mut out = vec![EdBodyState::default(); keys.len()];
        // SAFETY: keys and out have the same length, passed as count; keys are live bodies.
        unsafe { ffi::ed_bodies_read(self.id, keys.as_ptr(), keys.len() as i32, out.as_mut_ptr()) };
        out
    }

    fn to_state(s: &EdBodyState) -> BodyState {
        BodyState {
            position: pos_from_b(s.p),
            rotation: dquat_from_b(s.q),
            linear_velocity: dvec_from_b(s.v),
            angular_velocity: dvec_from_b(s.w),
        }
    }

    pub fn body_state(&self, e: Entity) -> Option<BodyState> {
        let live = self.live(e)?;
        Some(Self::to_state(&self.read_states(&[live.key])[0]))
    }

    /// States of all live entities in index order.
    pub fn body_states(&self) -> Vec<(Entity, BodyState)> {
        let ents: Vec<Entity> = self.entities().collect();
        let keys: Vec<u64> = ents.iter().map(|e| self.live(*e).map(|l| l.key).unwrap_or(0)).collect();
        let st = self.read_states(&keys);
        ents.into_iter().zip(st.iter().map(Self::to_state)).collect()
    }

    pub fn body_kind(&self, e: Entity) -> Option<BodyKind> {
        self.live(e).map(|l| l.kind)
    }

    pub fn body_desc(&self, e: Entity) -> Option<&BodyDesc> {
        match &self.live(e)?.payload {
            Payload::Body(d) => Some(d),
            Payload::Mover(_) => None,
        }
    }

    pub fn is_awake(&self, e: Entity) -> Option<bool> {
        let live = self.live(e)?;
        // SAFETY: live key of our world.
        Some(unsafe { ffi::ed_body_is_awake(self.id, live.key) } != 0)
    }

    pub fn mover_state(&self, e: Entity) -> Option<MoverState> {
        match &self.live(e)?.payload {
            Payload::Mover(m) => Some(MoverState {
                position: m.feet,
                velocity: m.velocity,
                grounded: m.grounded,
                ground_normal: m.ground_normal,
            }),
            Payload::Body(_) => None,
        }
    }

    pub fn mover_desc(&self, e: Entity) -> Option<MoverDesc> {
        match &self.live(e)?.payload {
            Payload::Mover(m) => Some(m.desc),
            Payload::Body(_) => None,
        }
    }

    pub(crate) fn slots(&self) -> &[Slot] {
        &self.t.slots
    }

    pub(crate) fn backend_states(&self) -> Vec<(u32, EdBodyState)> {
        let mut idx = Vec::new();
        let mut keys = Vec::new();
        for (i, s) in self.t.slots.iter().enumerate() {
            if let Some(l) = &s.live {
                idx.push(i as u32);
                keys.push(l.key);
            }
        }
        idx.into_iter().zip(self.read_states(&keys)).collect()
    }

    // -----------------------------------------------------------------------------------------
    // Queries

    fn hit(&self, h: &EdHit) -> Option<RayHit> {
        let index = *self.by_key.get(&h.body_key)?;
        let entity = Entity::new(index, self.t.slots[index as usize].generation);
        Some(RayHit { entity, point: pos_from_b(h.point), normal: vec_from_b(h.normal), fraction: h.fraction })
    }

    /// Closest hit along `origin .. origin + translation`. Rays starting inside a shape ignore it.
    pub fn cast_ray(&self, origin: DVec3, translation: DVec3, filter: QueryFilter) -> Option<RayHit> {
        let o = pos_to_b(origin);
        let t = dvec_to_b(translation);
        let mut h = EdHit::default();
        // SAFETY: valid locals; our world is live.
        let r = unsafe { ffi::ed_cast_ray(self.id, &o, &t, filter.category, filter.mask, &mut h) };
        if r == 0 { None } else { self.hit(&h) }
    }

    /// Closest hit of `shape` swept from `origin` along `translation`.
    pub fn cast_shape(&self, shape: CastShape, origin: DVec3, translation: DVec3, filter: QueryFilter) -> Option<ShapeHit> {
        let mut pts = [B3Vec3::default(); 8];
        let (n, radius) = match shape {
            CastShape::Sphere { radius } => (1, radius),
            CastShape::Capsule { half_height, radius } => {
                pts[0] = vec_to_b(Vec3::new(0.0, 0.0, -half_height));
                pts[1] = vec_to_b(Vec3::new(0.0, 0.0, half_height));
                (2, radius)
            }
            CastShape::Cuboid { half_extents: h, rotation } => {
                for (i, p) in pts.iter_mut().enumerate() {
                    let sx = if i & 1 == 0 { -1.0 } else { 1.0 };
                    let sy = if i & 2 == 0 { -1.0 } else { 1.0 };
                    let sz = if i & 4 == 0 { -1.0 } else { 1.0 };
                    *p = vec_to_b(rotation * Vec3::new(sx * h.x, sy * h.y, sz * h.z));
                }
                (8, 0.0)
            }
        };
        let o = pos_to_b(origin);
        let t = dvec_to_b(translation);
        let mut h = EdHit::default();
        // SAFETY: pts holds at least n points; valid locals; our world is live.
        let r = unsafe {
            ffi::ed_cast_shape(self.id, &o, pts.as_ptr(), n, radius, &t, filter.category, filter.mask, &mut h)
        };
        if r == 0 { None } else { self.hit(&h) }
    }

    // -----------------------------------------------------------------------------------------
    // Hash

    /// FNV-1a 64 over, in entity index order, every slot's generation and, for live entities,
    /// the backend's exact position (f64), rotation, linear and angular velocity (f32) bits,
    /// plus mover state. Also the tick and the generator. Platform-independent byte order.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv::new();
        h.u64(self.t.tick);
        h.u64(self.t.rng);
        let states = self.backend_states();
        let mut it = states.iter().peekable();
        h.u32(self.t.slots.len() as u32);
        for (i, s) in self.t.slots.iter().enumerate() {
            h.u32(s.generation);
            let Some(l) = &s.live else {
                h.bytes(&[0]);
                continue;
            };
            h.bytes(&[1 + kind_code(l.kind) as u8]);
            let (_, st) = it.next().filter(|(j, _)| *j == i as u32).expect("state order");
            for f in [st.p.x, st.p.y, st.p.z] {
                h.f64(f);
            }
            for f in [st.q.x, st.q.y, st.q.z, st.q.s, st.v.x, st.v.y, st.v.z, st.w.x, st.w.y, st.w.z] {
                h.f32(f);
            }
            if let Payload::Mover(m) = &l.payload {
                for f in [m.feet, m.velocity, m.ground_normal, m.input.velocity] {
                    h.f64(f.x);
                    h.f64(f.y);
                    h.f64(f.z);
                }
                h.f32(m.input.jump_speed);
                h.bytes(&[m.grounded as u8]);
            }
        }
        h.0
    }

    // -----------------------------------------------------------------------------------------
    // Snapshot

    pub fn snapshot(&self) -> Snapshot {
        let mut size = 0i32;
        // SAFETY: our world is live; the shim returns a b3Alloc'd blob of `size` bytes or null.
        let blob = unsafe { ffi::ed_world_snapshot(self.id, &mut size) };
        assert!(!blob.is_null() && size > 0, "Box3D world serialization failed");
        // SAFETY: blob points to `size` initialised bytes until freed below.
        let bytes = unsafe { std::slice::from_raw_parts(blob, size as usize) }.to_vec();
        // SAFETY: frees the blob returned above, once, with its size.
        unsafe { ffi::ed_blob_free(blob, size) };
        Snapshot { config: self.config, tables: self.t.clone(), backend: Arc::new(bytes) }
    }

    /// Replaces the whole state with `s`. The world keeps its config except for what the snapshot
    /// carries (the snapshot's config wins).
    ///
    /// On `Err(Incompatible | Corrupt)` the world is unchanged. On `Err(Backend)` Box3D rejected the
    /// image after it began restoring and the world is unusable (stepping panics).
    pub fn restore(&mut self, s: &Snapshot) -> Result<(), SimError> {
        let len = i32::try_from(s.backend.len()).map_err(|_| SimError::Corrupt("snapshot too large"))?;
        // SAFETY: the blob is `len` readable bytes; the shim validates its framing before touching
        // the world and copies everything it keeps.
        let r = unsafe { ffi::ed_world_restore(self.id, s.backend.as_ptr(), len) };
        match r {
            1 => {}
            0 => return Err(SimError::Corrupt("backend snapshot framing")),
            _ => {
                self.poisoned = true;
                return Err(SimError::Backend("Box3D rejected the snapshot image"));
            }
        }
        self.config = s.config;
        self.t = s.tables.clone();
        self.by_key.clear();
        for (i, slot) in self.t.slots.iter().enumerate() {
            if let Some(l) = &slot.live {
                self.by_key.insert(l.key, i as u32);
            }
        }
        self.poisoned = false;
        Ok(())
    }

    /// A new world in the snapshot's state.
    pub fn from_snapshot(s: &Snapshot) -> Result<World, SimError> {
        let mut w = World::new(&s.config);
        w.restore(s)?;
        Ok(w)
    }
}

// ---------------------------------------------------------------------------------------------
// Snapshot encoding

const SNAP_MAGIC: &[u8; 8] = b"EDSIMSN1";

pub(crate) fn write_config(w: &mut Writer, c: &SimConfig) {
    w.u64(c.seed);
    w.u64(c.data_version);
    w.dvec3(c.origin_ecef);
    w.dvec3(c.gravity);
    w.bool(c.enable_sleep);
    w.bool(c.enable_continuous);
}

pub(crate) fn read_config(r: &mut Reader) -> R<SimConfig> {
    Ok(SimConfig {
        seed: r.u64()?,
        data_version: r.u64()?,
        origin_ecef: r.dvec3()?,
        gravity: r.dvec3()?,
        enable_sleep: r.bool()?,
        enable_continuous: r.bool()?,
    })
}

impl Snapshot {
    pub fn tick(&self) -> u64 {
        self.tables.tick
    }

    pub fn config(&self) -> &SimConfig {
        &self.config
    }

    /// Bytes for storage or transfer. Restores only on a build with the same layout (checked).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        self.write(&mut w);
        w.buf
    }

    pub(crate) fn write(&self, w: &mut Writer) {
        w.buf.extend_from_slice(SNAP_MAGIC);
        w.u64(build_id());
        write_config(w, &self.config);
        let t = &self.tables;
        w.u64(t.tick);
        w.u64(t.rng);
        w.len(t.geometry.len());
        for g in &t.geometry {
            write_geometry(w, &g.geom);
        }
        w.len(t.slots.len());
        for s in &t.slots {
            w.u32(s.generation);
            match &s.live {
                None => w.u8(0),
                Some(l) => {
                    w.u8(1);
                    w.u64(l.key);
                    match &l.payload {
                        Payload::Body(d) => {
                            w.u8(0);
                            write_body(w, d);
                        }
                        Payload::Mover(m) => {
                            w.u8(1);
                            write_mover_desc(w, &m.desc);
                            w.dvec3(m.input.velocity);
                            w.f32(m.input.jump_speed);
                            w.dvec3(m.feet);
                            w.dvec3(m.velocity);
                            w.bool(m.grounded);
                            w.dvec3(m.ground_normal);
                        }
                    }
                }
            }
        }
        w.len(t.free.len());
        for f in &t.free {
            w.u32(*f);
        }
        w.bytes(&self.backend);
    }

    pub fn from_bytes(data: &[u8]) -> Result<Snapshot, SimError> {
        let mut r = Reader::new(data);
        let s = Self::read(&mut r)?;
        if !r.done() {
            return Err(SimError::Corrupt("trailing bytes"));
        }
        Ok(s)
    }

    pub(crate) fn read(r: &mut Reader) -> R<Snapshot> {
        let mut magic = [0u8; 8];
        for m in &mut magic {
            *m = r.u8()?;
        }
        if &magic != SNAP_MAGIC {
            return Err(SimError::Corrupt("not a snapshot"));
        }
        if r.u64()? != build_id() {
            return Err(SimError::Incompatible("snapshot from a different build layout"));
        }
        let config = read_config(r)?;
        let tick = r.u64()?;
        let rng = r.u64()?;
        let ng = r.len(1)?;
        let mut geometry = Vec::with_capacity(ng);
        for _ in 0..ng {
            geometry.push(Arc::new(GeomEntry::new(read_geometry(r)?)?));
        }
        let ns = r.len(5)?;
        let mut slots = Vec::with_capacity(ns);
        for _ in 0..ns {
            let generation = r.u32()?;
            let live = match r.u8()? {
                0 => None,
                1 => {
                    let key = r.u64()?;
                    match r.u8()? {
                        0 => {
                            let d = read_body(r)?;
                            Some(Live { key, kind: d.kind, payload: Payload::Body(Arc::new(d)) })
                        }
                        1 => {
                            let desc = read_mover_desc(r)?;
                            let input = MoverInput { velocity: r.dvec3()?, jump_speed: r.f32()? };
                            let m = MoverRt {
                                desc,
                                input,
                                feet: r.dvec3()?,
                                velocity: r.dvec3()?,
                                grounded: r.bool()?,
                                ground_normal: r.dvec3()?,
                            };
                            Some(Live { key, kind: BodyKind::Kinematic, payload: Payload::Mover(Box::new(m)) })
                        }
                        _ => return Err(SimError::Corrupt("payload kind")),
                    }
                }
                _ => return Err(SimError::Corrupt("slot flag")),
            };
            slots.push(Slot { generation, live });
        }
        let nf = r.len(4)?;
        let mut free = Vec::with_capacity(nf);
        for _ in 0..nf {
            let f = r.u32()?;
            if f as usize >= slots.len() {
                return Err(SimError::Corrupt("free list index"));
            }
            free.push(f);
        }
        let backend = r.bytes()?.to_vec();
        Ok(Snapshot { config, tables: Tables { tick, rng, slots, free, geometry }, backend: Arc::new(backend) })
    }
}

#[cfg(test)]
#[test]
fn world_and_snapshot_are_send() {
    fn s<T: Send>() {}
    s::<World>();
    s::<Snapshot>();
}
