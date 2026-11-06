//! The road graph for AI, racing and traffic.
//!
//! Built from the same network and the same solved heights as the road meshes, so a car following an edge is
//! on the road it sees. Node ids come from the world position (quantised to 0.5 m in the global tile grid), so
//! the same junction has the same id in every tile and window. Engine-free; lanes and speeds come from the
//! class and the region's practice until real lane tags are available.

use super::net::RoadNet;
use super::solve::Solved;
use super::{RoadClass, ground::smooth_ground};
use crate::project::TileProjector;
use crate::style::Region;
use glam::Vec3;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct NavNode {
    pub id: u64,
    pub pos: Vec3,
    pub signal: bool,
    pub stop: bool,
    /// The node is only where the tile was cut; another tile owns the rest of the road.
    pub border: bool,
}

#[derive(Debug, Clone)]
pub struct NavEdge {
    pub from: u64,
    pub to: u64,
    pub class: RoadClass,
    pub level: i8,
    /// Lanes in the drawing direction and against it.
    pub lanes: (u8, u8),
    pub oneway: bool,
    pub speed_mps: f32,
    pub half_width: f32,
    /// Centreline in tile-local render space, from `from` to `to`.
    pub pts: Vec<Vec3>,
}

#[derive(Debug, Clone, Default)]
pub struct NavTile {
    pub nodes: Vec<NavNode>,
    pub edges: Vec<NavEdge>,
}

/// Lanes per direction by class.
pub fn lanes_for(c: RoadClass, oneway: bool) -> (u8, u8) {
    let per = match c {
        RoadClass::Motorway | RoadClass::Trunk => 2,
        _ => 1,
    };
    if oneway { (per, 0) } else { (per, per) }
}

/// Target speed in m/s by class and practice (mph countries use the posted-limit table; the rest km/h).
pub fn speed_mps(c: RoadClass, region: Region) -> f32 {
    use RoadClass::*;
    let mph = matches!(
        region,
        Region::NorthAmericaCity | Region::NorthAmericaSuburb
    );
    let (m, k) = match c {
        Motorway => (65.0 * 0.447, 120.0 / 3.6),
        Trunk => (55.0 * 0.447, 100.0 / 3.6),
        Primary => (45.0 * 0.447, 80.0 / 3.6),
        Secondary => (40.0 * 0.447, 70.0 / 3.6),
        Tertiary => (35.0 * 0.447, 50.0 / 3.6),
        Minor => (25.0 * 0.447, 40.0 / 3.6),
        Service => (15.0 * 0.447, 20.0 / 3.6),
        Track => (15.0 * 0.447, 25.0 / 3.6),
        Path | Rail => (0.0, 0.0),
    };
    if mph { m as f32 } else { k as f32 }
}

impl NavTile {
    pub fn build(
        net: &RoadNet,
        solved: &Solved,
        ground: &dyn Fn(f64, f64) -> f32,
        proj: &TileProjector,
        region: Region,
    ) -> NavTile {
        let wm = net.wm;
        let t = proj.tile;
        let gid = |p: [f64; 2]| -> u64 {
            let gx = ((t.x as f64 * wm + p[0]) * 2.0).round() as i64;
            let gy = ((t.y as f64 * wm + p[1]) * 2.0).round() as i64;
            ((gx as u64) & 0xFFFF_FFFF) << 32 | ((gy as u64) & 0xFFFF_FFFF)
        };
        let inside = |p: [f64; 2]| p[0] >= 0.0 && p[0] <= wm && p[1] >= 0.0 && p[1] <= wm;
        let mut nodes: BTreeMap<u64, NavNode> = BTreeMap::new();
        let mut incident: BTreeMap<u64, (u8, u8, u8)> = BTreeMap::new(); // count, max rank, min rank
        let mut edges = Vec::new();
        for (ni, e) in net.edges.iter().enumerate() {
            if !e.class.is_car_road() && !matches!(e.class, RoadClass::Service)
                || matches!(
                    e.class,
                    RoadClass::Path | RoadClass::Rail | RoadClass::Track
                )
            {
                continue;
            }
            let sol = solved.of_net[ni].map(|si| &solved.edges[si]);
            let (xy, zs): (Vec<[f64; 2]>, Vec<f32>) = match sol {
                Some(s) => (s.pts.clone(), s.z.clone()),
                None => {
                    let d = super::solve::densify(&e.pts);
                    let z = d
                        .iter()
                        .map(|p| smooth_ground(ground, p[0], p[1]))
                        .collect();
                    (d, z)
                }
            };
            // Keep the part within the tile and a margin, so neighbouring tiles' graphs overlap on a shared node.
            let keep: Vec<usize> = (0..xy.len())
                .filter(|&k| {
                    xy[k][0] >= -30.0
                        && xy[k][0] <= wm + 30.0
                        && xy[k][1] >= -30.0
                        && xy[k][1] <= wm + 30.0
                })
                .collect();
            if keep.len() < 2 || !keep.iter().any(|&k| inside(xy[k])) {
                continue;
            }
            // Contiguous run only (a road that leaves and re-enters becomes two edges).
            let mut runs: Vec<Vec<usize>> = vec![vec![keep[0]]];
            for w in keep.windows(2) {
                if w[1] == w[0] + 1 {
                    runs.last_mut().unwrap().push(w[1]);
                } else {
                    runs.push(vec![w[1]]);
                }
            }
            for run in runs.into_iter().filter(|r| r.len() >= 2) {
                let (a, b) = (xy[run[0]], xy[*run.last().unwrap()]);
                let (ia, ib) = (gid(a), gid(b));
                let full_a = run[0] == 0;
                let full_b = *run.last().unwrap() == xy.len() - 1;
                for (id, p, z, full) in [
                    (ia, a, zs[run[0]], full_a),
                    (ib, b, zs[*run.last().unwrap()], full_b),
                ] {
                    nodes.entry(id).or_insert_with(|| NavNode {
                        id,
                        pos: proj.local(p[0] / wm, p[1] / wm, z as f64),
                        signal: false,
                        stop: false,
                        border: !full,
                    });
                    let c = incident.entry(id).or_insert((0, 0, 255));
                    c.0 += 1;
                    c.1 = c.1.max(e.class.rank());
                    c.2 = c.2.min(e.class.rank());
                }
                let pts = run
                    .iter()
                    .map(|&k| proj.local(xy[k][0] / wm, xy[k][1] / wm, zs[k] as f64))
                    .collect();
                edges.push(NavEdge {
                    from: ia,
                    to: ib,
                    class: e.class,
                    level: e.level(),
                    lanes: lanes_for(e.class, e.oneway),
                    oneway: e.oneway,
                    speed_mps: speed_mps(e.class, region),
                    half_width: e.class.width() / 2.0,
                    pts,
                });
            }
        }
        for (id, (count, max_rank, min_rank)) in incident {
            if let Some(n) = nodes.get_mut(&id) {
                if !n.border && count >= 3 {
                    n.signal = max_rank >= 3 && min_rank >= 1;
                    n.stop = !n.signal && max_rank <= 2;
                }
            }
        }
        NavTile {
            nodes: nodes.into_values().collect(),
            edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roads::solve::{self, tests::edge};
    use ed_geo::{EnuFrame, Geodetic, tile::TileId};

    #[test]
    fn junction_gets_a_signal_and_shared_ids() {
        let tile = TileId::new(14, 8544, 5827);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(46.0, 7.7, 0.0)));
        let wm = tile.width_m();
        let c = wm / 2.0;
        let net = RoadNet::from_edges(
            wm,
            vec![
                edge(RoadClass::Primary, false, vec![[c - 300.0, c], [c, c]]),
                edge(RoadClass::Primary, false, vec![[c, c], [c + 300.0, c]]),
                edge(RoadClass::Secondary, false, vec![[c, c], [c, c + 300.0]]),
            ],
        );
        let g = |_: f64, _: f64| 0.0f32;
        let solved = solve::solve(&net, &g);
        let nav = NavTile::build(&net, &solved, &g, &proj, Region::WesternEurope);
        assert_eq!(nav.edges.len(), 3);
        let hub = nav
            .nodes
            .iter()
            .find(|n| n.signal)
            .expect("a signalled hub");
        assert_eq!(
            nav.edges
                .iter()
                .filter(|e| e.from == hub.id || e.to == hub.id)
                .count(),
            3
        );
        assert!(
            nav.edges
                .iter()
                .all(|e| e.speed_mps > 5.0 && e.lanes.0 >= 1)
        );
    }

    #[test]
    fn motorway_oneway_has_all_lanes_forward() {
        assert_eq!(lanes_for(RoadClass::Motorway, true), (2, 0));
        assert_eq!(lanes_for(RoadClass::Minor, false), (1, 1));
        assert!(
            speed_mps(RoadClass::Motorway, Region::WesternEurope)
                > speed_mps(RoadClass::Minor, Region::WesternEurope)
        );
    }
}
