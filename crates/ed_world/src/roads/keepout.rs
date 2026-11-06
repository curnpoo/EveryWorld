//! Where a building must not stand: on a carriageway, or on a bridge deck.
//!
//! Machine-learned footprints (and the occasional OSM mistake) put buildings on roads, in medians and on decks.
//! A footprint whose centre lies within 90 % of a car road's half width is dropped. Footpaths, tracks, service
//! lanes and tunnels do not count: houses do stand beside them, and a tunnel is underground.

use super::net::RoadNet;
use std::collections::BTreeMap;

const CELL: f64 = 32.0;

pub struct KeepOut {
    cells: BTreeMap<(i64, i64), Vec<([f64; 2], [f64; 2], f64)>>,
}

impl KeepOut {
    pub fn new(net: &RoadNet) -> Self {
        let mut cells: BTreeMap<(i64, i64), Vec<([f64; 2], [f64; 2], f64)>> = BTreeMap::new();
        for e in net
            .edges
            .iter()
            .filter(|e| e.class.is_car_road() && e.class.rank() >= 1 && !e.tunnel && e.paved)
        {
            let r = e.class.width() as f64 * 0.5 * 0.9;
            for w in e.pts.windows(2) {
                let (x0, x1) = (
                    ((w[0][0].min(w[1][0]) - r) / CELL).floor() as i64,
                    ((w[0][0].max(w[1][0]) + r) / CELL).floor() as i64,
                );
                let (y0, y1) = (
                    ((w[0][1].min(w[1][1]) - r) / CELL).floor() as i64,
                    ((w[0][1].max(w[1][1]) + r) / CELL).floor() as i64,
                );
                for gx in x0..=x1 {
                    for gy in y0..=y1 {
                        cells.entry((gx, gy)).or_default().push((w[0], w[1], r));
                    }
                }
            }
        }
        Self { cells }
    }

    /// Is the point (x, y) in metres inside a carriageway?
    pub fn blocks(&self, x: f64, y: f64) -> bool {
        let key = ((x / CELL).floor() as i64, (y / CELL).floor() as i64);
        self.cells.get(&key).is_some_and(|segs| {
            segs.iter().any(|&(a, b, r)| {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let t = (((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9))
                    .clamp(0.0, 1.0);
                (x - (a[0] + dx * t)).hypot(y - (a[1] + dy * t)) < r
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roads::RoadClass;
    use crate::roads::solve::tests::edge;

    #[test]
    fn building_on_the_road_is_dropped_but_beside_it_is_kept() {
        let net = RoadNet::from_edges(
            2400.0,
            vec![
                edge(
                    RoadClass::Primary,
                    false,
                    vec![[0.0, 100.0], [500.0, 100.0]],
                ),
                edge(RoadClass::Path, false, vec![[0.0, 200.0], [500.0, 200.0]]),
            ],
        );
        let k = KeepOut::new(&net);
        assert!(k.blocks(250.0, 101.0));
        assert!(!k.blocks(250.0, 112.0));
        assert!(
            !k.blocks(250.0, 200.0),
            "a footpath does not keep houses out"
        );
    }
}
