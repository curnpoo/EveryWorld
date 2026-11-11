//! Minimal demo worlds: ground, a box pyramid, a mover probe; and the 1,000-box pile used by the
//! determinism tests and the timing measurement.

use crate::types::{BodyDesc, BodyKind, Entity, MoverDesc, ShapeDesc};
use crate::world::{SimConfig, SimInput, World};
use glam::{DVec3, Vec3};

pub struct Demo {
    pub world: World,
    pub ground: Entity,
    pub boxes: Vec<Entity>,
    pub mover: Entity,
}

/// A static ground slab whose top face is z = 0, `half` metres across each way.
pub fn ground(world: &mut World, half: f32) -> Entity {
    let desc = BodyDesc::new(BodyKind::Static, DVec3::new(0.0, 0.0, -1.0))
        .with_shape(ShapeDesc::cuboid(Vec3::new(half, half, 1.0)));
    world.create_body(desc).expect("ground")
}

/// A pyramid of `base` rows of 1 m boxes along x, centred at `at` (on the ground).
pub fn pyramid(world: &mut World, at: DVec3, base: u32) -> Vec<Entity> {
    let mut out = Vec::new();
    let h = 0.5f32;
    for row in 0..base {
        let n = base - row;
        for i in 0..n {
            let x = (i as f64 - (n - 1) as f64 * 0.5) * 1.0;
            let z = h as f64 + row as f64;
            let d = BodyDesc::new(BodyKind::Dynamic, at + DVec3::new(x, 0.0, z))
                .with_shape(ShapeDesc::cuboid(Vec3::splat(h * 0.99)));
            out.push(world.create_body(d).expect("box"));
        }
    }
    out
}

/// `nx * ny * nz` boxes in a loose grid above the ground, slightly offset per layer so they
/// tumble into a pile.
pub fn box_pile(world: &mut World, nx: u32, ny: u32, nz: u32) -> Vec<Entity> {
    let mut out = Vec::with_capacity((nx * ny * nz) as usize);
    let h = 0.4f32;
    let gap = 1.1f64;
    for k in 0..nz {
        let shift = if k % 2 == 0 { 0.0 } else { 0.35 };
        for j in 0..ny {
            for i in 0..nx {
                let p = DVec3::new(
                    (i as f64 - nx as f64 * 0.5) * gap + shift,
                    (j as f64 - ny as f64 * 0.5) * gap + shift,
                    1.0 + k as f64 * gap,
                );
                let d = BodyDesc::new(BodyKind::Dynamic, p).with_shape(ShapeDesc::cuboid(Vec3::splat(h)));
                out.push(world.create_body(d).expect("box"));
            }
        }
    }
    out
}

/// Ground, a 6-row box pyramid and a mover probe standing 4 m south of it.
pub fn demo_world(config: &SimConfig) -> Demo {
    let mut world = World::new(config);
    let ground = ground(&mut world, 50.0);
    let boxes = pyramid(&mut world, DVec3::new(0.0, 0.0, 0.0), 6);
    let mover = world
        .create_mover(MoverDesc { position: DVec3::new(0.0, -4.0, 0.0), ..MoverDesc::default() })
        .expect("mover");
    Demo { world, ground, boxes, mover }
}

/// Ground plus a 10 x 10 x 10 pile of dynamic boxes: the 1,000-body determinism scene.
pub fn thousand_boxes(config: &SimConfig) -> (World, Vec<Entity>) {
    let mut world = World::new(config);
    ground(&mut world, 60.0);
    let boxes = box_pile(&mut world, 10, 10, 10);
    (world, boxes)
}

/// Steps the 1,000-box scene with no commands and returns the state hash after every tick.
pub fn thousand_box_hashes(steps: u32) -> Vec<u64> {
    let (mut world, _) = thousand_boxes(&SimConfig::default());
    (0..steps)
        .map(|_| {
            let tick = world.tick();
            world.step(&SimInput { tick, commands: &[], utc: tick as f64 * crate::DT });
            world.state_hash()
        })
        .collect()
}

/// Order-sensitive digest of a hash sequence.
pub fn digest(hashes: &[u64]) -> u64 {
    let mut h = crate::codec::Fnv::new();
    for x in hashes {
        h.u64(*x);
    }
    h.0
}
