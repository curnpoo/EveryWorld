//! Trunk collision, override records and felling (G10), headless.

use ed_geo::ObjectId;
use ed_sim::*;
use glam::DVec3;

fn tree(n: u128, y: f64, r: f32, h: f32) -> TrunkDesc {
    TrunkDesc { id: ObjectId(n), base: DVec3::new(0.0, y, 0.0), trunk_h: h * 0.5, tree_h: h, radius: r }
}

struct Scene {
    w: World,
    car: Vehicle,
    t: Trunks,
}

impl Scene {
    fn new(trees: &[TrunkDesc]) -> Scene {
        let mut w = World::new(&SimConfig::default());
        demo::ground(&mut w, 500.0);
        let d = VehicleDesc::default();
        let h = d.ride_height();
        let car = Vehicle::spawn(&mut w, d, DVec3::new(0.0, 0.0, h + 0.1), 0.0).unwrap();
        let mut t = Trunks::new();
        for d in trees {
            t.add(&mut w, *d);
        }
        Scene { w, car, t }
    }

    fn step(&mut self, input: VehicleInput) -> Vec<ObjectId> {
        let watched = [Watched { entity: self.car.body, radius: self.car.desc.body_half.y as f64 }];
        self.car.update(&mut self.w, &input);
        self.t.begin_step(&self.w, &watched);
        let tick = self.w.tick();
        let cmds: [Command; 0] = [];
        self.w.step(&SimInput { tick, commands: &cmds, utc: 0.0 });
        self.t.end_step(&mut self.w, tick)
    }

    /// Settles, accelerates to `speed` m/s then coasts into the trees for `secs`.
    fn run(&mut self, speed: f64, secs: f64) -> Vec<ObjectId> {
        let mut felled = Vec::new();
        for _ in 0..60 {
            self.step(VehicleInput::default());
        }
        for _ in 0..(secs * 60.0) as usize {
            let drive = if self.car.forward_speed(&self.w) < speed { 1.0 } else { 0.0 };
            felled.extend(self.step(VehicleInput { drive, ..Default::default() }));
        }
        felled
    }

    fn y(&self) -> f64 {
        self.w.body_state(self.car.body).unwrap().position.y
    }
}

#[test]
fn car_stops_at_a_trunk() {
    // A 0.35 m trunk (a mature tree), car at ~3 m/s, trunk 25 m ahead.
    let mut s = Scene::new(&[tree(1, 25.0, 0.35, 20.0)]);
    let felled = s.run(3.0, 14.0);
    assert!(felled.is_empty(), "a slow car felled a big tree");
    assert!(s.t.is_standing(ObjectId(1)));
    assert!(s.y() < 25.0, "car passed through the trunk: y = {}", s.y());
    assert!(s.car.forward_speed(&s.w) < 1.0, "still moving: {}", s.car.forward_speed(&s.w));
}

#[test]
fn fast_car_fells_a_small_tree_and_it_comes_to_rest() {
    let mut s = Scene::new(&[tree(2, 60.0, 0.09, 9.0)]);
    let felled = s.run(18.0, 8.0);
    assert_eq!(felled, vec![ObjectId(2)], "tree not felled");
    assert!(!s.t.is_standing(ObjectId(2)));
    match s.t.overrides().get(ObjectId(2)) {
        Some(TreeState::Felled { dir, .. }) => assert!(dir[1] > 0.5, "falls away from the car: {dir:?}"),
        o => panic!("record: {o:?}"),
    }
    let log = *s.t.fallen().next().expect("fallen body");
    // Let it come to rest (car parked off to the side by braking).
    for _ in 0..600 {
        s.step(VehicleInput { drive: -1.0, ..Default::default() });
    }
    let st = s.w.body_state(log.entity).unwrap();
    assert!(st.linear_velocity.length() < 0.2 && st.angular_velocity.length() < 0.2, "not at rest: {st:?}");
    // Lying: the log's axis is near horizontal.
    let axis = st.rotation * DVec3::Z;
    assert!(axis.z.abs() < 0.5, "log still upright: {axis:?}");
    assert!(st.position.z < 1.5, "log height {}", st.position.z);
    let (m, _) = s.w.mass(log.entity).unwrap();
    assert!((3.0..300.0).contains(&m), "mass {m}");
}

#[test]
fn thin_trunks_do_not_block_and_overrides_skip() {
    let mut w = World::new(&SimConfig::default());
    let mut t = Trunks::new();
    assert!(!t.add(&mut w, tree(3, 10.0, 0.03, 4.0)), "sapling blocks");
    assert!(t.add(&mut w, tree(4, 10.0, 0.2, 12.0)));
    assert!(!t.add(&mut w, tree(4, 10.0, 0.2, 12.0)), "added twice");
    t.mark_removed(&mut w, ObjectId(4));
    assert!(!t.is_standing(ObjectId(4)));
    assert!(!t.add(&mut w, tree(4, 10.0, 0.2, 12.0)), "removed tree came back");
}

#[test]
fn records_round_trip() {
    let mut o = TreeOverrides::default();
    o.insert(ObjectId(9), TreeState::Removed);
    o.insert(ObjectId(5), TreeState::Felled { dir: [0.0, 1.0], tick: 77 });
    let back = TreeOverrides::from_bytes(&o.to_bytes()).unwrap();
    assert_eq!(o, back);
    assert!(TreeOverrides::from_bytes(&o.to_bytes()[..10]).is_none());
}

#[test]
fn felling_is_deterministic_across_runs() {
    let go = || {
        let mut s = Scene::new(&[tree(2, 60.0, 0.09, 9.0)]);
        let f = s.run(18.0, 8.0);
        for _ in 0..300 {
            s.step(VehicleInput::default());
        }
        let log = s.t.fallen().next().map(|l| s.w.body_state(l.entity).unwrap().position);
        (f, s.t.overrides().to_bytes(), s.w.state_hash(), log)
    };
    assert_eq!(go(), go());
}
