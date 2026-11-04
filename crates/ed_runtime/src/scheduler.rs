//! Streaming decisions: build order and hole-free retirement.
//!
//! Two sets drive streaming:
//! - **wanted**: the quadtree LOD selection around the camera, in every
//!   direction (`ed_world::lod::select`).
//! - **in view**: the wanted tiles the camera can currently see.
//!
//! Only wanted tiles in view are *built* (visible ones first). Loaded tiles
//! that leave the view stay loaded while they are still wanted, so turning
//! around doesn't rebuild the street behind you; [`crate::residency::Lru`]
//! caps how many such tiles are kept. A tile that is no longer wanted retires
//! once every wanted tile covering its area is either loaded or out of view:
//! no holes where you can look, and no coarse tiles held forever for areas
//! nobody is looking at.

use ed_geo::tile::TileId;
use std::collections::HashSet;

/// Build priority; lower runs first.
///
/// `dist_over_width` is distance to the tile centre over the tile width (the
/// old sort key: fine tiles near you and coarse tiles at the horizon rank
/// alike). `ahead` is the cosine between the camera velocity and the
/// direction to the tile, weighted by speed (0 when still); tiles you are
/// moving towards are fetched earlier. Off-screen work ranks behind all
/// on-screen work.
pub fn build_priority(dist_over_width: f64, visible: bool, ahead: f32) -> f32 {
    let base = dist_over_width as f32 * (1.0 - 0.4 * ahead.clamp(0.0, 1.0));
    if visible { base } else { 1_000.0 + base }
}

/// How strongly to prefetch ahead at this speed: 0 when standing, 1 from
/// about 50 m/s (fast car, plane).
pub fn ahead_weight(speed_ms: f32) -> f32 {
    (speed_ms / 50.0).clamp(0.0, 1.0)
}

/// Loaded tiles to retire now.
///
/// A loaded tile that is still wanted is kept. Otherwise it retires unless a
/// wanted tile covering its area (an ancestor or a descendant) is *blocking*:
/// in view and not loaded yet. Linear in the number of tiles times zoom depth
/// (the old version compared every loaded tile with every wanted tile, every
/// frame).
pub fn retire(
    loaded: impl IntoIterator<Item = TileId>,
    wanted: &HashSet<TileId>,
    is_loaded: impl Fn(&TileId) -> bool,
    in_view: impl Fn(&TileId) -> bool,
) -> Vec<TileId> {
    let blocking = |t: &TileId| wanted.contains(t) && !is_loaded(t) && in_view(t);
    // Every strict ancestor of a blocking wanted tile must stay.
    let mut held: HashSet<TileId> = HashSet::new();
    for w in wanted.iter().filter(|w| blocking(w)) {
        for z in 0..w.z {
            held.insert(w.ancestor(z).0);
        }
    }
    loaded
        .into_iter()
        .filter(|t| !wanted.contains(t))
        .filter(|t| !held.contains(t))
        // ...and a blocking wanted ancestor keeps its loaded descendants.
        .filter(|t| (0..t.z).all(|z| !blocking(&t.ancestor(z).0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[TileId]) -> HashSet<TileId> {
        v.iter().copied().collect()
    }

    #[test]
    fn coarse_tile_waits_for_visible_children() {
        let parent = TileId::new(10, 500, 300);
        let kids = parent.children();
        let wanted = set(&kids);
        let mut loaded = set(&[parent, kids[0], kids[1]]);
        // Two visible children missing: keep the parent.
        let r = retire(loaded.iter().copied(), &wanted, |t| loaded.contains(t), |_| true);
        assert!(r.is_empty());
        // Missing children are out of view: the parent may go.
        let r = retire(loaded.iter().copied(), &wanted, |t| loaded.contains(t), |t| *t != kids[2] && *t != kids[3]);
        assert_eq!(r, vec![parent]);
        // All children in: the parent goes.
        loaded.extend([kids[2], kids[3]]);
        let r = retire(loaded.iter().copied(), &wanted, |t| loaded.contains(t), |_| true);
        assert_eq!(r, vec![parent]);
    }

    #[test]
    fn fine_tiles_wait_for_visible_coarse_replacement() {
        let parent = TileId::new(9, 100, 100);
        let kids = parent.children();
        let wanted = set(&[parent]);
        let loaded = set(&kids);
        let r = retire(loaded.iter().copied(), &wanted, |t| loaded.contains(t), |_| true);
        assert!(r.is_empty(), "children stay until the parent arrives");
        let mut l2 = loaded.clone();
        l2.insert(parent);
        let mut r = retire(l2.iter().copied(), &wanted, |t| l2.contains(t), |_| true);
        r.sort();
        let mut k = kids.to_vec();
        k.sort();
        assert_eq!(r, k);
    }

    #[test]
    fn wanted_tiles_are_never_retired_and_strangers_are() {
        let a = TileId::new(12, 1, 1);
        let far = TileId::new(12, 3000, 3000);
        let wanted = set(&[a]);
        let loaded = set(&[a, far]);
        let r = retire(loaded.iter().copied(), &wanted, |t| loaded.contains(t), |_| true);
        assert_eq!(r, vec![far]);
    }

    #[test]
    fn deep_descendant_holds_grandparent() {
        let g = TileId::new(8, 10, 10);
        let grandkid = g.children()[1].children()[2];
        let wanted = set(&[grandkid]);
        let loaded = set(&[g]);
        assert!(retire(loaded.iter().copied(), &wanted, |t| loaded.contains(t), |_| true).is_empty());
        assert_eq!(retire(loaded.iter().copied(), &wanted, |t| loaded.contains(t), |_| false), vec![g]);
    }

    #[test]
    fn priority_orders_visible_first_and_prefetches_ahead() {
        assert!(build_priority(5.0, true, 0.0) < build_priority(0.5, false, 0.0));
        assert!(build_priority(3.0, true, 1.0) < build_priority(3.0, true, 0.0));
        assert_eq!(build_priority(3.0, true, -1.0), build_priority(3.0, true, 0.0), "no penalty behind");
        assert_eq!(ahead_weight(0.0), 0.0);
        assert_eq!(ahead_weight(200.0), 1.0);
    }
}
