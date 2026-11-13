//! Tree trunks in the sim: static collision, server-owned override records, felling (G10).
//!
//! A standing trunk is a static capsule. There are no contact events in the backend, so an
//! impact is estimated from the momentum a watched dynamic body (a car) loses along the line
//! from the trunk axis to its centre while it is within reach; the estimates add up over the
//! contact and reset after [`CONTACT_RESET_TICKS`] ticks of no contact. Above
//! `FELL_IMPULSE_PER_R2 * r^2` the tree is felled: a [`TreeState::Felled`] record is written,
//! the static trunk goes away and a dynamic capsule (mass from volume x [`WOOD_DENSITY`]) takes
//! its place with the impact impulse applied at the top. All constants are tuning knobs for the
//! destruction phase. Everything is a pure function of the call order (sorted `ObjectId`
//! iteration, no clock, no randomness).
//!
//! The records are the authoritative state (`ObjectId` keys, so they survive node reloads and
//! re-bakes); generators and the renderer apply them (skip a felled or removed id as standing).

use crate::{BodyDesc, BodyKind, Entity, Material, ShapeDesc, World};
use ed_geo::ObjectId;
use glam::{DQuat, DVec3, Vec3};
use std::collections::BTreeMap;

/// Trunks thinner than this (radius, m) do not block (saplings, shrubs).
pub const MIN_BLOCK_RADIUS_M: f32 = 0.06;
/// Felling threshold: normal impulse (N s) needed per m^2 of trunk radius. A 0.17 m trunk
/// takes ~11.5 kN s (a 1.5 t car at ~8 m/s); a 0.07 m one ~2 kN s.
pub const FELL_IMPULSE_PER_R2: f64 = 4.0e5;
/// Green wood, kg/m^3.
pub const WOOD_DENSITY: f32 = 600.0;
/// Ticks without contact after which the accumulated impact resets.
pub const CONTACT_RESET_TICKS: u32 = 10;
/// Cap on the speed given to the felled tree by the impact (m/s).
pub const MAX_PUSH_SPEED: f64 = 6.0;
/// Extra reach beyond body radius + trunk radius that counts as contact (m).
pub const CONTACT_MARGIN_M: f64 = 0.6;
/// Fraction of the tree height a felled log spans.
pub const LOG_FRACTION: f32 = 0.9;

/// One tree's trunk, sim (region) frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrunkDesc {
    pub id: ObjectId,
    /// Trunk base on the ground.
    pub base: DVec3,
    /// Collidable trunk height (up to the crown base).
    pub trunk_h: f32,
    /// Whole tree height (the felled log's length is a fraction of it).
    pub tree_h: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TreeState {
    /// Knocked over at `tick`, toward the horizontal direction `dir` (east, north).
    Felled { dir: [f32; 2], tick: u64 },
    /// Gone (cut, burned).
    Removed,
}

/// Override records by tree id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeOverrides(BTreeMap<ObjectId, TreeState>);

impl TreeOverrides {
    pub fn get(&self, id: ObjectId) -> Option<TreeState> {
        self.0.get(&id).copied()
    }
    pub fn contains(&self, id: ObjectId) -> bool {
        self.0.contains_key(&id)
    }
    pub fn insert(&mut self, id: ObjectId, s: TreeState) {
        self.0.insert(id, s);
    }
    pub fn iter(&self) -> impl Iterator<Item = (ObjectId, TreeState)> + '_ {
        self.0.iter().map(|(k, v)| (*k, *v))
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Little-endian bytes: count, then `id(16) tag(1) [dir.x dir.y (f32) tick (u64)]`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut o = Vec::new();
        o.extend((self.0.len() as u32).to_le_bytes());
        for (id, s) in &self.0 {
            o.extend(id.0.to_le_bytes());
            match s {
                TreeState::Removed => o.push(0),
                TreeState::Felled { dir, tick } => {
                    o.push(1);
                    o.extend(dir[0].to_le_bytes());
                    o.extend(dir[1].to_le_bytes());
                    o.extend(tick.to_le_bytes());
                }
            }
        }
        o
    }

    pub fn from_bytes(b: &[u8]) -> Option<TreeOverrides> {
        let mut p = 0usize;
        let mut take = |n: usize| -> Option<&[u8]> {
            let s = b.get(p..p + n)?;
            p += n;
            Some(s)
        };
        let n = u32::from_le_bytes(take(4)?.try_into().ok()?);
        let mut m = BTreeMap::new();
        for _ in 0..n {
            let id = ObjectId(u128::from_le_bytes(take(16)?.try_into().ok()?));
            let s = match take(1)?[0] {
                0 => TreeState::Removed,
                1 => {
                    let x = f32::from_le_bytes(take(4)?.try_into().ok()?);
                    let y = f32::from_le_bytes(take(4)?.try_into().ok()?);
                    let tick = u64::from_le_bytes(take(8)?.try_into().ok()?);
                    TreeState::Felled { dir: [x, y], tick }
                }
                _ => return None,
            };
            m.insert(id, s);
        }
        Some(TreeOverrides(m))
    }
}

/// A body whose impacts on trunks are watched (a car).
#[derive(Clone, Copy, Debug)]
pub struct Watched {
    pub entity: Entity,
    /// Horizontal reach of the body from its centre (m).
    pub radius: f64,
}

/// A felled tree's dynamic body.
#[derive(Clone, Copy, Debug)]
pub struct Fallen {
    pub entity: Entity,
    pub desc: TrunkDesc,
}

#[derive(Default)]
pub struct Trunks {
    overrides: TreeOverrides,
    standing: BTreeMap<ObjectId, (Entity, TrunkDesc)>,
    fallen: BTreeMap<ObjectId, Fallen>,
    /// Accumulated impact and ticks since last contact per trunk.
    contact: BTreeMap<ObjectId, (f64, u32)>,
    before: Vec<(Watched, DVec3)>,
}

fn capsule_dims(len: f32, r: f32) -> (f32, f32) {
    (((len - 2.0 * r) * 0.5).max(0.01), r)
}

impl Trunks {
    pub fn new() -> Trunks {
        Trunks::default()
    }
    pub fn with_overrides(overrides: TreeOverrides) -> Trunks {
        Trunks { overrides, ..Default::default() }
    }
    pub fn overrides(&self) -> &TreeOverrides {
        &self.overrides
    }
    /// Ids with a live static trunk, sorted.
    pub fn standing(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.standing.keys().copied()
    }
    pub fn standing_count(&self) -> usize {
        self.standing.len()
    }
    /// Felled trees with a live body, sorted by id.
    pub fn fallen(&self) -> impl Iterator<Item = &Fallen> + '_ {
        self.fallen.values()
    }
    /// The description of a standing trunk.
    pub fn standing_desc(&self, id: ObjectId) -> Option<TrunkDesc> {
        self.standing.get(&id).map(|s| s.1)
    }
    pub fn is_standing(&self, id: ObjectId) -> bool {
        self.standing.contains_key(&id)
    }

    /// Whether a tree of this radius blocks at all.
    pub fn blocks(radius: f32) -> bool {
        radius >= MIN_BLOCK_RADIUS_M
    }
    pub fn fell_threshold(radius: f32) -> f64 {
        FELL_IMPULSE_PER_R2 * (radius as f64) * (radius as f64)
    }

    /// Adds the static trunk of `d` unless it is too thin, already present or overridden.
    /// A tree recorded as felled but without a body (records loaded from bytes) gets its
    /// log spawned lying toward the recorded direction. `true` if a static trunk was added.
    pub fn add(&mut self, world: &mut World, d: TrunkDesc) -> bool {
        match self.overrides.get(d.id) {
            Some(TreeState::Removed) => return false,
            Some(TreeState::Felled { dir, .. }) => {
                if !self.fallen.contains_key(&d.id) {
                    self.spawn_log(world, d, [dir[0] as f64, dir[1] as f64], None);
                }
                return false;
            }
            None => {}
        }
        if !Self::blocks(d.radius) || self.standing.contains_key(&d.id) {
            return false;
        }
        let (hh, r) = capsule_dims(d.trunk_h, d.radius);
        let mut bd = BodyDesc::new(BodyKind::Static, d.base + DVec3::Z * (d.trunk_h as f64 * 0.5));
        bd.shapes = vec![ShapeDesc::capsule(hh, r)];
        match world.create_body(bd) {
            Ok(e) => {
                self.standing.insert(d.id, (e, d));
                true
            }
            Err(_) => false,
        }
    }

    /// Drops the static trunk of `id` (out of range); no record.
    pub fn remove(&mut self, world: &mut World, id: ObjectId) -> bool {
        match self.standing.remove(&id) {
            Some((e, _)) => {
                self.contact.remove(&id);
                world.despawn(e)
            }
            None => false,
        }
    }

    /// Writes a record, removes the static trunk and despawns nothing else (cut, burned).
    pub fn mark_removed(&mut self, world: &mut World, id: ObjectId) {
        self.remove(world, id);
        self.overrides.insert(id, TreeState::Removed);
    }

    fn spawn_log(&mut self, world: &mut World, d: TrunkDesc, dir: [f64; 2], push: Option<f64>) -> Option<Entity> {
        let len = (d.tree_h * LOG_FRACTION).max(d.trunk_h);
        let (hh, r) = capsule_dims(len, d.radius);
        let h2 = DVec3::new(dir[0], dir[1], 0.0).normalize_or(DVec3::X);
        let mat = Material { density: WOOD_DENSITY, friction: 0.9, restitution: 0.0 };
        let (pos, rot) = match push {
            // Falling: standing, then pushed over.
            Some(_) => (d.base + DVec3::Z * (len as f64 * 0.5 + 0.05), DQuat::IDENTITY),
            // Reloaded record: lying toward `dir`, on the ground.
            None => (d.base + h2 * (len as f64 * 0.5) + DVec3::Z * d.radius as f64, DQuat::from_rotation_arc(DVec3::Z, h2)),
        };
        let mut bd = BodyDesc::new(BodyKind::Dynamic, pos).with_rotation(rot);
        bd.shapes = vec![ShapeDesc::capsule(hh, r).with_material(mat)];
        bd.angular_damping = 0.3;
        let e = world.create_body(bd).ok()?;
        if let Some(j) = push {
            let (m, _) = world.mass(e).unwrap_or((1.0, Vec3::ZERO));
            let j = j.min(m as f64 * MAX_PUSH_SPEED);
            let top = pos + DVec3::Z * (len as f64 * 0.5);
            world.apply_impulse_at(e, h2 * j, top);
        }
        self.fallen.insert(d.id, Fallen { entity: e, desc: d });
        Some(e)
    }

    /// Call before [`World::step`]: remembers the watched bodies' velocities.
    pub fn begin_step(&mut self, world: &World, watched: &[Watched]) {
        self.before.clear();
        for w in watched {
            if let Some(s) = world.body_state(w.entity) {
                self.before.push((*w, s.linear_velocity));
            }
        }
    }

    /// Call after [`World::step`] (the tick just simulated is `tick`): accumulates impacts and
    /// fells trunks over their threshold. Returns the ids felled this tick, sorted.
    pub fn end_step(&mut self, world: &mut World, tick: u64) -> Vec<ObjectId> {
        let mut hit: BTreeMap<ObjectId, (f64, DVec3)> = BTreeMap::new();
        for &(w, v0) in &self.before {
            let Some(s) = world.body_state(w.entity) else { continue };
            let Some((mass, _)) = world.mass(w.entity) else { continue };
            // Nearest trunk within reach (ties by id: BTreeMap order + strict less).
            let mut best: Option<(f64, ObjectId, DVec3)> = None;
            for (id, (_, d)) in &self.standing {
                let rel = DVec3::new(s.position.x - d.base.x, s.position.y - d.base.y, 0.0);
                let dist = rel.length();
                let reach = w.radius + d.radius as f64 + CONTACT_MARGIN_M;
                let dz = s.position.z - d.base.z;
                if dist < reach && dz > -1.0 && dz < d.trunk_h as f64 + 1.0 && best.map_or(true, |b| dist < b.0) {
                    best = Some((dist, *id, rel / dist.max(1e-6)));
                }
            }
            if let Some((_, id, n)) = best {
                // Momentum gained away from the trunk = what the trunk took out of the body.
                let dn = (s.linear_velocity - v0).dot(n);
                if dn > 0.0 {
                    let e = hit.entry(id).or_insert((0.0, DVec3::ZERO));
                    e.0 += mass as f64 * dn;
                    e.1 += n;
                }
            }
        }
        let mut felled = Vec::new();
        let ids: Vec<ObjectId> = self.contact.keys().chain(hit.keys()).copied().collect();
        for id in ids {
            let c = self.contact.entry(id).or_insert((0.0, 0));
            match hit.get(&id) {
                Some((j, _)) => {
                    c.0 += j;
                    c.1 = 0;
                }
                None => c.1 += 1,
            }
            if c.1 > CONTACT_RESET_TICKS {
                self.contact.remove(&id);
            }
        }
        for (id, (_, n)) in hit {
            let acc = self.contact.get(&id).map_or(0.0, |c| c.0);
            let Some(&(e, d)) = self.standing.get(&id) else { continue };
            if acc <= Self::fell_threshold(d.radius) {
                continue;
            }
            // Pushed away from the car: opposite to the trunk-to-body normal.
            let dir = -n.normalize_or(DVec3::X);
            self.standing.remove(&id);
            self.contact.remove(&id);
            world.despawn(e);
            self.overrides.insert(id, TreeState::Felled { dir: [dir.x as f32, dir.y as f32], tick });
            self.spawn_log(world, d, [dir.x, dir.y], Some(acc));
            felled.push(id);
        }
        felled.sort();
        felled
    }
}
