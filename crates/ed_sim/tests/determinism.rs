//! Determinism gate: the 1,000-body scene over 60 simulated seconds, in-process and across
//! processes; snapshot/restore; record/replay.

use ed_sim::demo;
use ed_sim::*;
use glam::{DQuat, DVec3, Vec3};
use std::sync::Arc;

const STEPS: u32 = 3600;

fn step(w: &mut World, cmds: &[Command]) -> u64 {
    let tick = w.tick();
    w.step(&SimInput { tick, commands: cmds, utc: tick as f64 * DT });
    w.state_hash()
}

#[test]
fn thousand_bodies_same_hashes_twice_in_process() {
    let a = demo::thousand_box_hashes(STEPS);
    let b = demo::thousand_box_hashes(STEPS);
    assert_eq!(a.len(), STEPS as usize);
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x, y, "hash differs at tick {i}");
    }
    // The scene actually moved (hashes change early on).
    assert_ne!(a[0], a[60]);

    // And it settled into a pile rather than exploding.
    let (mut w, boxes) = demo::thousand_boxes(&SimConfig::default());
    for _ in 0..STEPS {
        step(&mut w, &[]);
    }
    assert_eq!(boxes.len(), 1000);
    for e in &boxes {
        let s = w.body_state(*e).unwrap();
        assert!(s.position.z > 0.0 && s.position.z < 15.0, "box at {:?}", s.position);
    }
    assert_eq!(w.state_hash(), *a.last().unwrap());
}

/// Child half of the cross-process test: only does work when the parent sets the variable.
#[test]
fn cross_process_child() {
    if std::env::var("ED_SIM_CHILD").is_err() {
        return;
    }
    let h = demo::thousand_box_hashes(STEPS);
    println!("ED_SIM_RESULT {:016x} {:016x}", h.last().unwrap(), demo::digest(&h));
}

#[test]
fn thousand_bodies_same_hashes_across_processes() {
    let exe = std::env::current_exe().unwrap();
    let run = || {
        let out = std::process::Command::new(&exe)
            .args(["--exact", "cross_process_child", "--nocapture", "--test-threads=1"])
            .env("ED_SIM_CHILD", "1")
            .output()
            .expect("spawn child");
        assert!(out.status.success(), "child failed: {}", String::from_utf8_lossy(&out.stderr));
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let at = text.find("ED_SIM_RESULT").unwrap_or_else(|| panic!("no result in {text}"));
        text[at..].lines().next().unwrap().trim().to_string()
    };
    let a = run();
    let b = run();
    assert_eq!(a, b);
    let h = demo::thousand_box_hashes(STEPS);
    assert_eq!(a, format!("ED_SIM_RESULT {:016x} {:016x}", h.last().unwrap(), demo::digest(&h)));
}

#[test]
fn snapshot_restore_continues_identically() {
    let (mut w, _) = demo::thousand_boxes(&SimConfig::default());
    let n = 90; // boxes still falling and colliding: contacts and warm starts are live
    let m = 300;
    for _ in 0..n {
        step(&mut w, &[]);
    }
    let snap = w.snapshot();
    let at_n = w.state_hash();
    let first: Vec<u64> = (n..m).map(|_| step(&mut w, &[])).collect();

    w.restore(&snap).unwrap();
    assert_eq!(w.tick(), n as u64);
    assert_eq!(w.state_hash(), at_n);
    let second: Vec<u64> = (n..m).map(|_| step(&mut w, &[])).collect();
    assert_eq!(first, second, "restore in place diverged");

    // Through bytes into a different world.
    let bytes = snap.to_bytes();
    let snap2 = Snapshot::from_bytes(&bytes).unwrap();
    let mut w2 = World::from_snapshot(&snap2).unwrap();
    assert_eq!(w2.state_hash(), at_n);
    let third: Vec<u64> = (n..m).map(|_| step(&mut w2, &[])).collect();
    assert_eq!(first, third, "restore from bytes into a new world diverged");

    // Corrupt bytes are refused without touching the world.
    let before = w2.state_hash();
    assert!(Snapshot::from_bytes(&bytes[..bytes.len() - 10]).is_err());
    let mut bad = bytes.clone();
    bad[0] ^= 0xff;
    assert!(Snapshot::from_bytes(&bad).is_err());
    assert_eq!(w2.state_hash(), before);
}

#[test]
fn snapshot_with_mesh_heightfield_hull_and_mover() {
    // Geometry that Box3D references by pointer must survive a restore into another world.
    let mut w = World::new(&SimConfig::default());
    let hf = w
        .add_geometry(Geometry::HeightField(HeightField {
            columns: 9,
            rows: 9,
            spacing: [5.0, 5.0],
            heights: (0..81).map(|i| ((i % 9) as f32 * 0.3).sin() + (i / 9) as f32 * 0.1).collect(),
        }))
        .unwrap();
    w.create_body(BodyDesc::new(BodyKind::Static, DVec3::new(-20.0, 20.0, 0.0)).with_shape(ShapeDesc::new(Shape::HeightField(hf))))
        .unwrap();
    let mesh = w
        .add_geometry(Geometry::Mesh(TriMesh {
            vertices: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 1.0), Vec3::new(4.0, 4.0, 1.0), Vec3::new(0.0, 4.0, 0.0)],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }))
        .unwrap();
    w.create_body(BodyDesc::new(BodyKind::Static, DVec3::new(30.0, 0.0, 2.0)).with_shape(ShapeDesc::new(Shape::Mesh(mesh))))
        .unwrap();
    let hull = w
        .add_geometry(Geometry::Hull(vec![
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(0.5, -0.5, -0.5),
            Vec3::new(0.0, 0.5, -0.5),
            Vec3::new(0.0, 0.0, 0.6),
        ]))
        .unwrap();
    let mut rocks = Vec::new();
    for i in 0..20 {
        let p = DVec3::new(-10.0 + i as f64, 5.0 - (i % 5) as f64 * 2.0, 6.0 + (i % 3) as f64);
        rocks.push(w.create_body(BodyDesc::new(BodyKind::Dynamic, p).with_shape(ShapeDesc::new(Shape::Hull(hull)))).unwrap());
    }
    let mv = w.create_mover(MoverDesc { position: DVec3::new(0.0, 0.0, 3.0), ..Default::default() }).unwrap();
    let input = [Command {
        entity: mv,
        sequence: 0,
        kind: CommandKind::MoverInput(MoverInput { velocity: DVec3::new(1.5, 0.5, 0.0), jump_speed: 0.0 }),
    }];
    step(&mut w, &input);
    for _ in 0..60 {
        step(&mut w, &[]);
    }
    let snap = Snapshot::from_bytes(&w.snapshot().to_bytes()).unwrap();
    let a: Vec<u64> = (0..240).map(|_| step(&mut w, &[])).collect();
    drop(w); // the original geometry is gone; the copy must stand alone
    let mut w2 = World::from_snapshot(&snap).unwrap();
    let b: Vec<u64> = (0..240).map(|_| step(&mut w2, &[])).collect();
    assert_eq!(a, b);
    // Rocks landed on the height field rather than falling through.
    for r in &rocks {
        assert!(w2.body_state(*r).unwrap().position.z > -3.0);
    }
}

fn scripted_commands(tick: u64, mover: Entity, boxes: &[Entity]) -> Vec<Command> {
    let mut c = Vec::new();
    let mi = |v: DVec3, jump: f32| CommandKind::MoverInput(MoverInput { velocity: v, jump_speed: jump });
    match tick {
        5 => c.push(Command { entity: mover, sequence: 0, kind: mi(DVec3::new(2.0, 1.0, 0.0), 0.0) }),
        60 => c.push(Command { entity: mover, sequence: 1, kind: mi(DVec3::new(0.0, 3.0, 0.0), 5.0) }),
        90 => {
            let d = BodyDesc::new(BodyKind::Dynamic, DVec3::new(3.0, -2.0, 4.0))
                .with_shape(ShapeDesc::sphere(0.4))
                .with_rotation(DQuat::from_rotation_z(0.3));
            c.push(Command { entity: Entity::NONE, sequence: 0, kind: CommandKind::Spawn(Arc::new(d)) });
        }
        120 => {
            c.push(Command { entity: boxes[0], sequence: 0, kind: CommandKind::ApplyImpulse(DVec3::new(0.0, 4000.0, 2000.0)) });
            c.push(Command { entity: mover, sequence: 2, kind: mi(DVec3::new(-3.0, -1.0, 0.0), 0.0) });
        }
        200 => c.push(Command { entity: boxes[3], sequence: 0, kind: CommandKind::Despawn }),
        240 => c.push(Command { entity: mover, sequence: 3, kind: mi(DVec3::ZERO, 4.0) }),
        _ => {}
    }
    c.sort_by_key(|c| (c.entity, c.sequence));
    c
}

#[test]
fn record_and_replay_scripted_mover_run() {
    let mut demo = demo::demo_world(&SimConfig { seed: 7, ..Default::default() });
    let mut rec = Recorder::start(&demo.world);
    let mut hashes = Vec::new();
    let mut spawned = 0;
    for _ in 0..300 {
        let tick = demo.world.tick();
        let cmds = scripted_commands(tick, demo.mover, &demo.boxes);
        let input = SimInput { tick, commands: &cmds, utc: tick as f64 * DT };
        let out = demo.world.step(&input);
        assert!(out.rejected.is_empty(), "{:?}", out.rejected);
        spawned += out.spawned.len();
        let h = demo.world.state_hash();
        rec.record(&input, h);
        hashes.push(h);
    }
    assert_eq!(spawned, 1);
    let moved = demo.world.mover_state(demo.mover).unwrap();
    assert!(moved.position.distance(DVec3::new(0.0, -4.0, 0.0)) > 1.0, "mover did not move: {moved:?}");
    assert!(!demo.world.contains(demo.boxes[3]));

    let bytes = rec.finish();
    let replay = Replay::from_bytes(&bytes).unwrap();
    assert_eq!(replay.len(), 300);
    assert_eq!(replay.hashes(), hashes);
    let replayed = replay.run().unwrap().expect("replay diverged");
    assert_eq!(replayed, hashes);

    // A tampered recording is reported at the tick it no longer matches.
    let mut bad = bytes.clone();
    let n = bad.len();
    bad[n - 1] ^= 1;
    let div = Replay::from_bytes(&bad).unwrap().run().unwrap().unwrap_err();
    assert_eq!(div.tick, 299);
}

#[test]
#[should_panic(expected = "sorted")]
fn unsorted_commands_are_refused() {
    let mut w = World::new(&SimConfig::default());
    let a = w.create_mover(MoverDesc::default()).unwrap();
    let b = w.create_mover(MoverDesc { position: DVec3::new(5.0, 0.0, 0.0), ..Default::default() }).unwrap();
    let k = CommandKind::MoverInput(MoverInput::default());
    let cmds = [Command { entity: b, sequence: 0, kind: k.clone() }, Command { entity: a, sequence: 0, kind: k }];
    w.step(&SimInput { tick: 0, commands: &cmds, utc: 0.0 });
}
