//! Trunk collision set management (G10): radius, hysteresis, overrides, determinism. Headless.

use ed_geo::ObjectId;
use ed_sim::*;
use ew::tree_collide::{DROP_M, RADIUS_M, TreeCollision};
use glam::DVec3;

fn desc(n: u128, x: f64, y: f64, r: f32) -> TrunkDesc {
    TrunkDesc { id: ObjectId(n), base: DVec3::new(x, y, 0.0), trunk_h: 6.0, tree_h: 15.0, radius: r }
}

fn world() -> World {
    let mut w = World::new(&SimConfig::default());
    demo::ground(&mut w, 1000.0);
    w
}

fn cands() -> Vec<TrunkDesc> {
    let mut v = vec![desc(5, 10.0, 0.0, 0.2), desc(2, 50.0, 0.0, 0.3), desc(9, 100.0, 0.0, 0.2), desc(3, 200.0, 0.0, 0.2), desc(7, 20.0, 5.0, 0.03)];
    v.sort_by_key(|d| d.id);
    v
}

#[test]
fn radius_hysteresis_and_thin_trees() {
    let mut w = world();
    let mut tc = TreeCollision::new();
    tc.apply(&mut w, DVec3::ZERO, &cands());
    // 5 and 2 are in range, 9 (100 m) is beyond RADIUS_M, 3 is far, 7 is a sapling.
    assert_eq!(tc.trunks.standing().collect::<Vec<_>>(), vec![ObjectId(2), ObjectId(5)]);
    assert!(100.0 > RADIUS_M && 100.0 < DROP_M);
    // Walk 60 m east: 9 comes in, 5 (50 m behind) stays, hysteresis keeps 2 until DROP_M.
    tc.apply(&mut w, DVec3::new(60.0, 0.0, 0.0), &cands());
    assert_eq!(tc.trunks.standing().collect::<Vec<_>>(), vec![ObjectId(2), ObjectId(5), ObjectId(9)]);
    tc.apply(&mut w, DVec3::new(180.0, 0.0, 0.0), &cands());
    assert_eq!(tc.trunks.standing().collect::<Vec<_>>(), vec![ObjectId(3), ObjectId(9)]);
}

#[test]
fn removed_trees_stay_removed_and_runs_agree() {
    let go = |order_rev: bool| {
        let mut w = world();
        let mut tc = TreeCollision::new();
        let mut c = cands();
        if order_rev {
            // The caller sorts; the result must not depend on how the stands were found.
            c.reverse();
            c.sort_by_key(|d| d.id);
        }
        tc.apply(&mut w, DVec3::ZERO, &c);
        tc.trunks.mark_removed(&mut w, ObjectId(5));
        tc.apply(&mut w, DVec3::new(300.0, 0.0, 0.0), &c);
        tc.apply(&mut w, DVec3::ZERO, &c);
        assert!(!tc.trunks.is_standing(ObjectId(5)), "a removed tree came back");
        assert!(tc.trunks.is_standing(ObjectId(2)));
        (w.state_hash(), tc.trunks.overrides().to_bytes(), w.body_count())
    };
    assert_eq!(go(false), go(true));
}

#[test]
fn due_throttles() {
    let mut tc = TreeCollision::new();
    assert!(tc.due(DVec3::ZERO, 3));
    assert!(!tc.due(DVec3::new(1.0, 0.0, 0.0), 3));
    assert!(tc.due(DVec3::new(20.0, 0.0, 0.0), 3));
    assert!(tc.due(DVec3::new(20.0, 0.0, 0.0), 4), "stands changed");
}
