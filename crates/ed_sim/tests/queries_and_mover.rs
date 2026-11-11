//! Ray and shape casts against analytically known scenes; character mover cases; shapes;
//! debug draw.

use ed_sim::demo;
use ed_sim::*;
use glam::{DQuat, DVec3, Quat, Vec3};

fn step(w: &mut World, cmds: &[Command]) {
    let tick = w.tick();
    w.step(&SimInput { tick, commands: cmds, utc: tick as f64 * DT });
}

fn static_box(w: &mut World, center: DVec3, half: Vec3) -> Entity {
    w.create_body(BodyDesc::new(BodyKind::Static, center).with_shape(ShapeDesc::cuboid(half))).unwrap()
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn ray_casts_hit_known_points() {
    let mut w = World::new(&SimConfig::default());
    let ground = demo::ground(&mut w, 50.0);
    let cube = static_box(&mut w, DVec3::new(10.0, 0.0, 1.0), Vec3::splat(1.0));
    let f = QueryFilter::default();

    let h = w.cast_ray(DVec3::new(0.0, 0.0, 10.0), DVec3::new(0.0, 0.0, -20.0), f).unwrap();
    assert_eq!(h.entity, ground);
    assert!(near(h.point.z, 0.0, 1e-4) && near(h.fraction as f64, 0.5, 1e-5), "{h:?}");
    assert!((h.normal - Vec3::Z).length() < 1e-5);

    let h = w.cast_ray(DVec3::new(10.0, 0.0, 5.0), DVec3::new(0.0, 0.0, -10.0), f).unwrap();
    assert_eq!(h.entity, cube);
    assert!(near(h.point.z, 2.0, 1e-4), "{h:?}");

    // Horizontal, east into the cube's west face at x = 9.
    let h = w.cast_ray(DVec3::new(0.0, 0.0, 1.0), DVec3::new(20.0, 0.0, 0.0), f).unwrap();
    assert_eq!(h.entity, cube);
    assert!(near(h.point.x, 9.0, 1e-4) && near(h.fraction as f64, 0.45, 1e-5), "{h:?}");
    assert!((h.normal - Vec3::NEG_X).length() < 1e-5);

    // North-facing check: a box north of the origin is hit going +y (region north).
    let north = static_box(&mut w, DVec3::new(0.0, 8.0, 1.0), Vec3::splat(1.0));
    let h = w.cast_ray(DVec3::new(0.0, 0.0, 1.0), DVec3::new(0.0, 20.0, 0.0), f).unwrap();
    assert_eq!(h.entity, north);
    assert!(near(h.point.y, 7.0, 1e-4), "{h:?}");
    assert!((h.normal - Vec3::NEG_Y).length() < 1e-5);

    // Misses and filters.
    assert!(w.cast_ray(DVec3::new(0.0, 0.0, 10.0), DVec3::new(0.0, 0.0, 5.0), f).is_none());
    let none = QueryFilter { category: layers::DEFAULT, mask: layers::MOVER };
    assert!(w.cast_ray(DVec3::new(0.0, 0.0, 10.0), DVec3::new(0.0, 0.0, -20.0), none).is_none());

    // Far from the origin, f64 positions keep millimetre precision.
    let far = static_box(&mut w, DVec3::new(3.0e6, -2.0e6, 100.0), Vec3::splat(1.0));
    let h = w.cast_ray(DVec3::new(3.0e6, -2.0e6, 110.0), DVec3::new(0.0, 0.0, -20.0), f).unwrap();
    assert_eq!(h.entity, far);
    assert!(near(h.point.z, 101.0, 1e-3), "{h:?}");
}

#[test]
fn shape_casts_hit_known_fractions() {
    let mut w = World::new(&SimConfig::default());
    let ground = demo::ground(&mut w, 50.0);
    let f = QueryFilter::default();
    let tol = 0.02; // shape casts stop within the backend's linear slop

    let h = w.cast_shape(CastShape::Sphere { radius: 1.0 }, DVec3::new(5.0, 5.0, 10.0), DVec3::new(0.0, 0.0, -20.0), f).unwrap();
    assert_eq!(h.entity, ground);
    assert!(near(h.fraction as f64 * 20.0, 9.0, tol), "{h:?}");
    assert!(near(h.point.z, 0.0, tol));

    let h = w
        .cast_shape(CastShape::Capsule { half_height: 0.5, radius: 0.25 }, DVec3::new(20.0, 0.0, 5.0), DVec3::new(0.0, 0.0, -10.0), f)
        .unwrap();
    assert!(near(h.fraction as f64 * 10.0, 4.25, tol), "{h:?}");

    let cub = CastShape::Cuboid { half_extents: Vec3::splat(0.5), rotation: Quat::IDENTITY };
    let h = w.cast_shape(cub, DVec3::new(-10.0, 0.0, 5.0), DVec3::new(0.0, 0.0, -10.0), f).unwrap();
    assert!(near(h.fraction as f64 * 10.0, 4.5, tol), "{h:?}");

    // A cube tipped 45 degrees about x reaches lower by its half diagonal.
    let tipped = CastShape::Cuboid { half_extents: Vec3::splat(0.5), rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_4) };
    let h = w.cast_shape(tipped, DVec3::new(-15.0, 0.0, 5.0), DVec3::new(0.0, 0.0, -10.0), f).unwrap();
    assert!(near(h.fraction as f64 * 10.0, 5.0 - 0.5f64 * 2f64.sqrt(), tol), "{h:?}");

    // Sideways sweep against a wall at x = 30 (west face).
    let wall = static_box(&mut w, DVec3::new(31.0, 0.0, 5.0), Vec3::new(1.0, 10.0, 5.0));
    let h = w.cast_shape(CastShape::Sphere { radius: 0.5 }, DVec3::new(20.0, 0.0, 2.0), DVec3::new(20.0, 0.0, 0.0), f).unwrap();
    assert_eq!(h.entity, wall);
    assert!(near(20.0 + h.fraction as f64 * 20.0, 29.5, tol), "{h:?}");
}

#[test]
fn height_field_and_mesh_orientation() {
    let mut w = World::new(&SimConfig::default());
    let f = QueryFilter::default();
    // Rows: north (row 0) = 3 m, then 2 m, then 1 m. Columns east, 2 m apart. NW corner at (100, 0).
    let heights = vec![3.0, 3.0, 3.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0];
    let hf = w.add_geometry(Geometry::HeightField(HeightField { columns: 3, rows: 3, spacing: [2.0, 2.0], heights })).unwrap();
    let hfe = w.create_body(BodyDesc::new(BodyKind::Static, DVec3::new(100.0, 0.0, 0.0)).with_shape(ShapeDesc::new(Shape::HeightField(hf)))).unwrap();
    for (y, z) in [(-1.0, 2.5), (-3.0, 1.5), (-0.5, 2.75)] {
        let h = w.cast_ray(DVec3::new(101.0, y, 10.0), DVec3::new(0.0, 0.0, -20.0), f).unwrap();
        assert_eq!(h.entity, hfe);
        assert!(near(h.point.z, z, 0.01), "at y={y}: {h:?}");
        // Slope falls to the south: normal leans south (-y).
        assert!(h.normal.y < -0.1 && h.normal.z > 0.5, "{h:?}");
    }
    // Outside the field: nothing.
    assert!(w.cast_ray(DVec3::new(101.0, 1.0, 10.0), DVec3::new(0.0, 0.0, -20.0), f).is_none());

    // Mesh ramp z = x / 2 over x in [0, 4], at (200, 0, 0).
    let mesh = w
        .add_geometry(Geometry::Mesh(TriMesh {
            vertices: vec![Vec3::new(0.0, -2.0, 0.0), Vec3::new(4.0, -2.0, 2.0), Vec3::new(4.0, 2.0, 2.0), Vec3::new(0.0, 2.0, 0.0)],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }))
        .unwrap();
    let me = w.create_body(BodyDesc::new(BodyKind::Static, DVec3::new(200.0, 0.0, 0.0)).with_shape(ShapeDesc::new(Shape::Mesh(mesh)))).unwrap();
    let h = w.cast_ray(DVec3::new(202.0, 0.5, 10.0), DVec3::new(0.0, 0.0, -20.0), f).unwrap();
    assert_eq!(h.entity, me);
    assert!(near(h.point.z, 1.0, 1e-4), "{h:?}");
    let n = Vec3::new(-0.5, 0.0, 1.0).normalize();
    assert!((h.normal - n).length() < 1e-4, "{h:?}");

    // Meshes and height fields are refused on dynamic bodies.
    let bad = BodyDesc::new(BodyKind::Dynamic, DVec3::ZERO).with_shape(ShapeDesc::new(Shape::Mesh(mesh)));
    assert!(matches!(w.create_body(bad), Err(SimError::InvalidDesc(_))));
}

#[test]
fn all_body_kinds_and_shapes_simulate() {
    let mut w = World::new(&SimConfig::default());
    demo::ground(&mut w, 50.0);
    let hull = w
        .add_geometry(Geometry::Hull(vec![
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(0.5, -0.5, -0.5),
            Vec3::new(0.5, 0.5, -0.5),
            Vec3::new(-0.5, 0.5, -0.5),
            Vec3::new(-0.5, -0.5, 0.5),
            Vec3::new(0.5, -0.5, 0.5),
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(-0.5, 0.5, 0.5),
        ]))
        .unwrap();
    let dynamic = |p: DVec3, s: ShapeDesc| BodyDesc::new(BodyKind::Dynamic, p).with_shape(s);
    let bx = w.create_body(dynamic(DVec3::new(0.0, 0.0, 3.0), ShapeDesc::cuboid(Vec3::new(1.0, 0.5, 0.25)))).unwrap();
    let sp = w.create_body(dynamic(DVec3::new(4.0, 0.0, 3.0), ShapeDesc::sphere(0.5))).unwrap();
    let ca = w
        .create_body(dynamic(
            DVec3::new(8.0, 0.0, 3.0),
            ShapeDesc::capsule(0.5, 0.3).at(Vec3::ZERO, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        ))
        .unwrap();
    let hu = w.create_body(dynamic(DVec3::new(12.0, 0.0, 3.0), ShapeDesc::new(Shape::Hull(hull)))).unwrap();
    // Compound: two boxes side by side make a 2 x 1 x 1 dumbbell.
    let co = w
        .create_body(
            BodyDesc::new(BodyKind::Dynamic, DVec3::new(16.0, 0.0, 3.0))
                .with_shape(ShapeDesc::cuboid(Vec3::splat(0.5)).at(Vec3::new(-0.5, 0.0, 0.0), Quat::IDENTITY))
                .with_shape(ShapeDesc::cuboid(Vec3::splat(0.5)).at(Vec3::new(0.5, 0.0, 0.0), Quat::IDENTITY)),
        )
        .unwrap();
    // Kinematic platform moving north at 1 m/s via targets.
    let kin = w.create_body(BodyDesc::new(BodyKind::Kinematic, DVec3::new(-10.0, 0.0, 1.0)).with_shape(ShapeDesc::cuboid(Vec3::splat(0.5)))).unwrap();
    for i in 0..180u32 {
        let p = DVec3::new(-10.0, (i + 1) as f64 * DT, 1.0);
        let cmd = [Command { entity: kin, sequence: 0, kind: CommandKind::SetKinematicTarget { position: p, rotation: DQuat::IDENTITY } }];
        step(&mut w, &cmd);
    }
    let z = |e| w.body_state(e).unwrap().position.z;
    assert!(near(z(bx), 0.25, 0.02), "box {}", z(bx));
    assert!(near(z(sp), 0.5, 0.02), "sphere {}", z(sp));
    assert!(near(z(ca), 0.3, 0.02), "capsule {}", z(ca));
    assert!(near(z(hu), 0.5, 0.02), "hull {}", z(hu));
    assert!(near(z(co), 0.5, 0.02), "compound {}", z(co));
    let k = w.body_state(kin).unwrap();
    assert!(near(k.position.y, 3.0, 0.02) && near(k.position.z, 1.0, 1e-6), "kinematic {:?}", k.position);
    // The lying capsule's axis (shape rotated 90 degrees in the body) stays horizontal.
    let cs = w.body_state(ca).unwrap();
    let axis = cs.rotation * (DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2) * DVec3::Z);
    assert!(axis.z.abs() < 0.05, "capsule axis {axis:?}");
    // Debug draw has something for every body.
    let dd = DebugDraw::from_world(&w);
    assert!(dd.lines.len() > 6 * 12 && dd.triangles.is_empty());
}

// --------------------------------------------------------------------------------------------
// Mover

fn mover_world() -> World {
    let mut w = World::new(&SimConfig::default());
    demo::ground(&mut w, 100.0);
    w
}

fn drive(w: &mut World, m: Entity, v: DVec3, ticks: u32) -> Vec<MoverState> {
    let cmd = [Command { entity: m, sequence: 0, kind: CommandKind::MoverInput(MoverInput { velocity: v, jump_speed: 0.0 }) }];
    let mut out = Vec::new();
    step(w, &cmd);
    out.push(w.mover_state(m).unwrap());
    for _ in 1..ticks {
        step(w, &[]);
        out.push(w.mover_state(m).unwrap());
    }
    out
}

fn settle(w: &mut World, m: Entity) {
    for _ in 0..10 {
        step(w, &[]);
    }
    let s = w.mover_state(m).unwrap();
    assert!(s.grounded && near(s.position.z, 0.0, 0.01), "did not settle: {s:?}");
}

#[test]
fn mover_steps_up_a_low_ledge() {
    let mut w = mover_world();
    static_box(&mut w, DVec3::new(4.0, 0.0, 0.125), Vec3::new(2.0, 2.0, 0.125)); // 0.25 m < 0.35 m
    let m = w.create_mover(MoverDesc::default()).unwrap();
    settle(&mut w, m);
    let path = drive(&mut w, m, DVec3::new(3.0, 0.0, 0.0), 60);
    let s = path.last().unwrap();
    assert!(s.position.x > 2.5, "stopped at {:?}", s.position);
    assert!(near(s.position.z, 0.25, 0.01) && s.grounded, "{s:?}");
}

#[test]
fn mover_is_blocked_by_a_tall_ledge() {
    let mut w = mover_world();
    static_box(&mut w, DVec3::new(4.0, 0.0, 0.3), Vec3::new(2.0, 2.0, 0.3)); // 0.6 m > 0.35 m
    let m = w.create_mover(MoverDesc::default()).unwrap();
    settle(&mut w, m);
    let path = drive(&mut w, m, DVec3::new(3.0, 0.0, 0.0), 90);
    let s = path.last().unwrap();
    assert!(near(s.position.x, 2.0 - 0.3, 0.03), "{s:?}");
    assert!(near(s.position.z, 0.0, 0.01) && s.grounded, "{s:?}");
}

#[test]
fn mover_slides_along_a_wall() {
    let mut w = mover_world();
    // Wall face at y = 1 (north of the mover), long in x.
    static_box(&mut w, DVec3::new(0.0, 1.5, 2.0), Vec3::new(30.0, 0.5, 2.0));
    let m = w.create_mover(MoverDesc::default()).unwrap();
    settle(&mut w, m);
    let path = drive(&mut w, m, DVec3::new(3.0, 3.0, 0.0), 120);
    let s = path.last().unwrap();
    assert!(near(s.position.y, 1.0 - 0.3, 0.03), "{s:?}");
    // Two seconds at 3 m/s east: the into-wall part is removed, the rest is kept.
    assert!(s.position.x > 5.5, "{s:?}");
    assert!(s.velocity.y.abs() < 0.1 && near(s.velocity.x, 3.0, 0.2), "{s:?}");
}

#[test]
fn mover_does_not_tunnel_through_thin_geometry() {
    let mut w = mover_world();
    // A 4 cm box wall at x = 5 and a single-sided mesh wall facing west at x = 5, y in [10, 20].
    static_box(&mut w, DVec3::new(5.02, 0.0, 2.0), Vec3::new(0.02, 4.0, 2.0));
    let mesh = w
        .add_geometry(Geometry::Mesh(TriMesh {
            vertices: vec![Vec3::new(5.0, 10.0, 0.0), Vec3::new(5.0, 10.0, 4.0), Vec3::new(5.0, 20.0, 4.0), Vec3::new(5.0, 20.0, 0.0)],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }))
        .unwrap();
    w.create_body(BodyDesc::new(BodyKind::Static, DVec3::ZERO).with_shape(ShapeDesc::new(Shape::Mesh(mesh)))).unwrap();

    for (y, label) in [(0.0, "box"), (15.0, "mesh")] {
        let m = w.create_mover(MoverDesc { position: DVec3::new(0.0, y, 0.0), ..Default::default() }).unwrap();
        settle(&mut w, m);
        // 300 m/s = 5 m per tick: more than the whole gap in one tick.
        let path = drive(&mut w, m, DVec3::new(300.0, 0.0, 0.0), 30);
        for s in &path {
            assert!(s.position.x < 5.0 - 0.3 + 0.02, "{label}: tunnelled to {:?}", s.position);
        }
        assert!(path.last().unwrap().position.x > 4.5, "{label}: never reached the wall");
        let teleport = [Command {
            entity: m,
            sequence: 0,
            kind: CommandKind::Teleport { position: DVec3::new(-50.0, y, 0.0), rotation: DQuat::IDENTITY },
        }];
        step(&mut w, &teleport);
    }
}

#[test]
fn mover_slope_limit_and_ground_snap() {
    let mut w = mover_world();
    let ramp = |w: &mut World, x0: f32, y0: f32, slope_deg: f32| {
        let run = 6.0f32;
        let rise = run * slope_deg.to_radians().tan();
        let mesh = w
            .add_geometry(Geometry::Mesh(TriMesh {
                vertices: vec![
                    Vec3::new(0.0, -2.0, 0.0),
                    Vec3::new(run, -2.0, rise),
                    Vec3::new(run, 2.0, rise),
                    Vec3::new(0.0, 2.0, 0.0),
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            }))
            .unwrap();
        w.create_body(BodyDesc::new(BodyKind::Static, DVec3::new(x0 as f64, y0 as f64, 0.0)).with_shape(ShapeDesc::new(Shape::Mesh(mesh))))
            .unwrap();
    };
    ramp(&mut w, 1.0, 0.0, 25.0);
    ramp(&mut w, 1.0, 20.0, 65.0);

    // Gentle ramp: climbs, grounded on every tick on the way up and on the way back down.
    let m = w.create_mover(MoverDesc::default()).unwrap();
    settle(&mut w, m);
    let up = drive(&mut w, m, DVec3::new(2.0, 0.0, 0.0), 120);
    let top = up.last().unwrap();
    // Two seconds at 2 m/s from x = 0; the ramp starts at x = 1.
    let expect = (top.position.x - 1.0) * 25f64.to_radians().tan();
    assert!(top.position.x > 3.9 && near(top.position.z, expect, 0.05), "did not climb: {top:?}");
    assert!(up.iter().all(|s| s.grounded), "lost ground climbing");
    let down = drive(&mut w, m, DVec3::new(-2.0, 0.0, 0.0), 90);
    assert!(down.iter().all(|s| s.grounded), "lost ground descending (snap)");
    assert!(down.last().unwrap().position.z < top.position.z - 1.0);

    // Steep ramp: it does not climb beyond a step.
    let m2 = w.create_mover(MoverDesc { position: DVec3::new(0.0, 20.0, 0.0), ..Default::default() }).unwrap();
    settle(&mut w, m2);
    let path = drive(&mut w, m2, DVec3::new(2.0, 0.0, 0.0), 120);
    let max_z = path.iter().map(|s| s.position.z).fold(f64::MIN, f64::max);
    assert!(max_z < 0.35 + 0.05, "climbed a 65 degree slope to {max_z}");
    let last = path.last().unwrap();
    assert!(last.grounded && last.position.x < 1.2, "steep: {last:?}");
}

#[test]
fn mover_jumps_and_lands() {
    let mut w = mover_world();
    let m = w.create_mover(MoverDesc::default()).unwrap();
    settle(&mut w, m);
    let cmd = [Command { entity: m, sequence: 0, kind: CommandKind::MoverInput(MoverInput { velocity: DVec3::ZERO, jump_speed: 5.0 }) }];
    step(&mut w, &cmd);
    let mut peak: f64 = 0.0;
    let mut airborne = 0;
    for _ in 0..120 {
        step(&mut w, &[]);
        let s = w.mover_state(m).unwrap();
        peak = peak.max(s.position.z);
        if !s.grounded {
            airborne += 1;
        }
    }
    // v^2 / 2g = 1.27 m.
    assert!(near(peak, 25.0 / (2.0 * 9.80665), 0.1), "peak {peak}");
    assert!(airborne > 50);
    let s = w.mover_state(m).unwrap();
    assert!(s.grounded && near(s.position.z, 0.0, 0.01), "{s:?}");
}

#[test]
fn mover_pushes_dynamic_boxes_and_movers_ignore_each_other() {
    let mut w = mover_world();
    let b = w.create_body(BodyDesc::new(BodyKind::Dynamic, DVec3::new(3.0, 0.0, 0.5)).with_shape(ShapeDesc::cuboid(Vec3::splat(0.5)))).unwrap();
    let m = w.create_mover(MoverDesc::default()).unwrap();
    let other = w.create_mover(MoverDesc { position: DVec3::new(0.0, 0.0, 0.0), ..Default::default() }).unwrap();
    settle(&mut w, m);
    drive(&mut w, m, DVec3::new(2.0, 0.0, 0.0), 60);
    // The box is a mover obstacle (it blocks): the mover stops at it.
    let s = w.mover_state(m).unwrap();
    assert!(s.position.x < 2.5 + 0.05, "{s:?}");
    // Overlapping movers did not push each other apart.
    assert!(w.mover_state(other).unwrap().position.x.abs() < 1e-6);
    let _ = b;
}
