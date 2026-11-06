//! Turns a solved network into the tile's `RoadPlan`: items with solved bed heights, clipped to the tile
//! (plus a halo that is graded but not drawn, so the terrain agrees on both sides of a tile border).

use super::net::RoadNet;
use super::solve::{self, Solved};
use super::width;
use super::{Bed, Item, RoadPlan, smooth};
use crate::project::TileProjector;
use crate::surface::SurfaceMap;
use crate::terrain::TerrainGrid;
use std::collections::HashMap;

/// Geometry is emitted this far past the tile border (m) so neighbours overlap instead of leaving a crack.
pub const EMIT_MARGIN_M: f64 = 2.0;
/// Roads this far outside the tile still shape its terrain (the bench and slope reach ~25 m from a road).
pub const GRADE_REACH_M: f64 = 40.0;

#[derive(Clone, Copy, Debug)]
struct Pt {
    x: f64,
    y: f64,
    z: f32,
    k: u8,
    arc: f32,
    /// Smoothed natural ground under the point.
    g: f32,
    /// Embankment high enough, and level enough across, for retaining walls.
    w: bool,
    /// Cut deep enough, and level enough across, for a retaining wall.
    cut: bool,
}

#[derive(Clone, Copy)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    fn grown(wm: f64, m: f64) -> Rect {
        Rect {
            x0: -m,
            y0: -m,
            x1: wm + m,
            y1: wm + m,
        }
    }
    fn has(&self, x: f64, y: f64) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
}

fn lerp(a: &Pt, b: &Pt, t: f64) -> Pt {
    Pt {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        z: a.z + (b.z - a.z) * t as f32,
        k: a.k,
        arc: a.arc + (b.arc - a.arc) * t as f32,
        g: a.g + (b.g - a.g) * t as f32,
        w: a.w,
        cut: a.cut,
    }
}

/// Insert a vertex wherever a segment crosses the border of either rectangle.
fn split(pts: &[Pt], rects: [Rect; 2]) -> Vec<Pt> {
    let mut out = Vec::with_capacity(pts.len() + 8);
    for w in pts.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        out.push(*a);
        let mut ts: Vec<f64> = Vec::new();
        for r in rects {
            for (a0, d, lo, hi) in [(a.x, b.x - a.x, r.x0, r.x1), (a.y, b.y - a.y, r.y0, r.y1)] {
                if d.abs() < 1e-12 {
                    continue;
                }
                for edge in [lo, hi] {
                    let t = (edge - a0) / d;
                    if t > 1e-9 && t < 1.0 - 1e-9 {
                        ts.push(t);
                    }
                }
            }
        }
        ts.sort_by(|p, q| p.partial_cmp(q).unwrap());
        ts.dedup_by(|p, q| (*p - *q).abs() < 1e-9);
        for t in ts {
            out.push(lerp(a, b, t));
        }
    }
    out.extend(pts.last().copied());
    out
}

pub fn plan_v2(
    net: &RoadNet,
    solved: &Solved,
    proj: &TileProjector,
    grid: &TerrainGrid,
    surface: &SurfaceMap,
) -> RoadPlan {
    let wm = net.wm;
    let ext = 4096.0f64;
    let emit_r = Rect::grown(wm, EMIT_MARGIN_M);
    let grade_r = Rect::grown(wm, GRADE_REACH_M);
    let mut plan = RoadPlan {
        extent: ext,
        stats: solved.stats.clone(),
        v2: true,
        wm,
        ..Default::default()
    };
    let row = width::right_of_way(net);

    // Bigger roads last so they win where they overlap.
    let mut order: Vec<usize> = (0..net.edges.len()).collect();
    order.sort_by_key(|&i| (net.edges[i].class.rank(), i));

    for ni in order {
        let e = &net.edges[ni];
        if e.pts.len() < 2 {
            continue;
        }
        let sol = solved.of_net[ni].map(|si| &solved.edges[si]);
        let pts: Vec<Pt> = match sol {
            Some(s) => (0..s.pts.len())
                .map(|k| Pt {
                    x: s.pts[k][0],
                    y: s.pts[k][1],
                    z: s.z[k],
                    k: s.kind[k],
                    arc: s.s[k],
                    g: s.ground[k],
                    w: false,
                    cut: false,
                })
                .collect(),
            None => {
                let d = solve::densify(&e.pts);
                let mut arc = 0.0f32;
                d.iter()
                    .enumerate()
                    .map(|(i, p)| {
                        if i > 0 {
                            arc += super::net::dist(d[i - 1], *p) as f32;
                        }
                        Pt {
                            x: p[0],
                            y: p[1],
                            z: 0.0,
                            k: 0,
                            arc,
                            g: 0.0,
                            w: false,
                            cut: false,
                        }
                    })
                    .collect()
            }
        };
        if pts.len() < 2 {
            continue;
        }
        let mut pts = pts;
        if sol.is_some() && !e.bridge && !e.tunnel {
            mark_walled(&mut pts, width::edge_width(e, row[ni]) as f64 / 2.0, wm, grid);
            mark_cut(&mut pts, width::edge_width(e, row[ni]) as f64 / 2.0, wm, grid);
        }
        let pts = split(&pts, [emit_r, grade_r]);
        let seg_k = |i: usize| -> u8 {
            if e.tunnel {
                pts[i].k.max(pts[i + 1].k)
            } else {
                (pts[i].w && pts[i + 1].w) as u8
            }
        };
        let seg_c = |i: usize| -> u8 {
            let (mx, my) = (
                (pts[i].x + pts[i + 1].x) * 0.5,
                (pts[i].y + pts[i + 1].y) * 0.5,
            );
            if emit_r.has(mx, my) {
                0
            } else if grade_r.has(mx, my) {
                1
            } else {
                2
            }
        };
        let nseg = pts.len() - 1;
        let mut i = 0;
        while i < nseg {
            let (c, k) = (seg_c(i), seg_k(i));
            let mut j = i;
            while j + 1 < nseg && seg_c(j + 1) == c && seg_k(j + 1) == k {
                j += 1;
            }
            let (a, b) = (i, j);
            i = j + 1;
            if c == 2 {
                continue;
            }
            let kind = if e.tunnel {
                if k == 0 { Bed::Graded } else { Bed::Tunnel }
            } else if e.bridge {
                Bed::Bridge
            } else if sol.is_some() {
                if pts[i].cut && pts[i + 1].cut {
                    Bed::Cut
                } else if k == 1 {
                    Bed::Fill
                } else {
                    Bed::Graded
                }
            } else {
                Bed::Conform
            };
            // Halo pieces only matter where they reshape the terrain.
            if c == 1 && !matches!(kind, Bed::Graded | Bed::Conform | Bed::Fill | Bed::Cut) {
                continue;
            }
            if c == 1 && matches!(kind, Bed::Conform | Bed::Fill) {
                continue;
            }
            let run = &pts[a..=b + 1];
            let uv: Vec<(f64, f64)> = run.iter().map(|p| (p.x / wm, p.y / wm)).collect();
            let bed: Vec<f32> = if sol.is_some() {
                run.iter().map(|p| p.z).collect()
            } else {
                let ground: Vec<f32> = uv
                    .iter()
                    .map(|&(u, v)| grid.sample(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
                    .collect();
                smooth(&ground, 5, 2)
            };
            let lpts = uv
                .iter()
                .zip(&bed)
                .map(|(&(u, v), &h)| proj.local(u, v, h as f64))
                .collect();
            let urban = surface
                .cover_at(uv[0].0.clamp(0.0, 1.0), uv[0].1.clamp(0.0, 1.0))
                .is_urban();
            let width = if matches!(kind, Bed::Graded | Bed::Bridge | Bed::Tunnel | Bed::Fill) {
                width::edge_width(e, row[ni])
            } else {
                e.class.width()
            };
            // Another road passing under a deck: keep piers away from that stretch.
            let mut clear_arcs = Vec::new();
            if kind == Bed::Bridge {
                if let Some(si) = solved.of_net[ni] {
                    for cr in solved.crossings.iter().filter(|c| c.upper == si) {
                        let lo = &net.edges[solved.edges[cr.lower].net_idx];
                        let half =
                            (lo.class.width() / (2.0 * cr.sin) + e.class.width() / 2.0 + 12.0)
                                .min(80.0);
                        clear_arcs.push((cr.su - half - run[0].arc, cr.su + half - run[0].arc));
                    }
                }
            }
            let prev_open = a > 0 && seg_c(a - 1) == c && seg_k(a - 1) == 0;
            let next_open = b + 1 < nseg && seg_c(b + 1) == c && seg_k(b + 1) == 0;
            plan.items.push(Item {
                class: e.class,
                kind,
                uv,
                bed,
                pts: lpts,
                oneway: e.oneway,
                paved: e.paved,
                urban,
                width,
                emit: c == 0,
                cycleway: e.cycleway,
                section: if kind == Bed::Tunnel { k } else { 0 },
                portal: (
                    kind == Bed::Tunnel && prev_open,
                    kind == Bed::Tunnel && next_open,
                ),
                clear_arcs,
                sidewalk: e.tags.sidewalk,
                cycle: e.tags.cycle,
                lanes: e.tags.lane_split(e.oneway),
                bridge_kind: super::BridgeKind::Beam,
                depth: if kind == Bed::Fill {
                    (run.iter().map(|p| p.z - p.g).fold(0.0f32, f32::max) + 0.8).clamp(1.5, 14.0)
                } else {
                    0.0
                },
            });
        }
    }

    // Junction nodes: where three or more car roads end at the same point.
    let mut j: HashMap<(i32, i32), (u8, u8)> = HashMap::new();
    for e in net
        .edges
        .iter()
        .filter(|e| e.class.is_car_road() && e.pts.len() >= 2)
    {
        for p in [e.pts[0], e.pts[e.pts.len() - 1]] {
            let (x, y) = (p[0] / wm * ext, p[1] / wm * ext);
            if (0.0..ext).contains(&x) && (0.0..ext).contains(&y) {
                let c = j.entry((x as i32, y as i32)).or_default();
                c.0 += 1;
                c.1 = c.1.max(e.class.rank());
            }
        }
    }
    let mut jv: Vec<_> = j.into_iter().collect();
    jv.sort();
    plan.junctions = jv;

    // Junction geometry: ground-level car roads ending at a shared node, with their directions and widths.
    let mut nodes: std::collections::BTreeMap<(i64, i64), (super::junction::JunctionSpec, usize)> =
        Default::default();
    for (ni, e) in net.edges.iter().enumerate() {
        if !e.class.is_car_road()
            || e.class.rank() < 1
            || e.bridge
            || e.tunnel
            || e.pts.len() < 2
            || !e.paved
        {
            continue;
        }
        let hw = width::edge_width(e, row[ni]) as f64 / 2.0;
        let n = e.pts.len();
        for (p, q) in [(e.pts[0], e.pts[1]), (e.pts[n - 1], e.pts[n - 2])] {
            if !(0.0..wm).contains(&p[0]) || !(0.0..wm).contains(&p[1]) {
                continue;
            }
            // Direction over the first ~12 m so a wiggly first segment does not skew the corner.
            let mut far = q;
            let mut acc = 0.0;
            let it: Box<dyn Iterator<Item = [f64; 2]>> = if p == e.pts[0] {
                Box::new(e.pts.iter().copied())
            } else {
                Box::new(e.pts.iter().rev().copied())
            };
            let mut prev = p;
            for c in it.skip(1) {
                acc += super::net::dist(prev, c);
                far = c;
                prev = c;
                if acc >= 12.0 {
                    break;
                }
            }
            let l = super::net::dist(p, far).max(1e-6);
            let dir = [(far[0] - p[0]) / l, (far[1] - p[1]) / l];
            let key = ((p[0] * 2.0).round() as i64, (p[1] * 2.0).round() as i64);
            let spec = nodes.entry(key).or_insert_with(|| {
                let urban = surface.cover_at(p[0] / wm, p[1] / wm).is_urban();
                (
                    super::junction::JunctionSpec {
                        pos: p,
                        urban,
                        branches: Vec::new(),
                    },
                    0,
                )
            });
            spec.0.branches.push(super::junction::Branch {
                dir,
                hw,
                class: e.class,
            });
            spec.1 += 1;
        }
    }
    // Median barriers, clipped to the emit rectangle (points outside end a run).
    for run in super::median::median_runs(net, solved) {
        let mut cur: Vec<[f64; 3]> = Vec::new();
        for p in run {
            if emit_r.has(p[0], p[1]) {
                cur.push(p);
            } else if !cur.is_empty() {
                if cur.len() >= 2 {
                    plan.medians.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
        if cur.len() >= 2 {
            plan.medians.push(cur);
        }
    }
    plan.jspecs = nodes
        .into_values()
        .filter(|(s, _)| s.branches.len() >= 3)
        .map(|(s, _)| s)
        .collect();
    plan
}

/// Samples of a ground road that sit on a real embankment: at least `FILL_MIN_M` above the natural ground, with no
/// hillside rising to the road level on either side (a sidehill cut must stay a graded cut). Runs shorter than
/// `FILL_RUN_M` are dropped.
pub const FILL_MIN_M: f32 = 2.5;
pub const FILL_RUN_M: f32 = 24.0;

fn mark_walled(pts: &mut [Pt], hw: f64, wm: f64, grid: &TerrainGrid) {
    let n = pts.len();
    let off = hw + 3.5;
    for k in 0..n {
        let (a, b) = (pts[k.saturating_sub(1)], pts[(k + 1).min(n - 1)]);
        let l = ((b.x - a.x).hypot(b.y - a.y)).max(1e-9);
        let (nx, ny) = (-(b.y - a.y) / l, (b.x - a.x) / l);
        let side = |s: f64| {
            let (u, v) = (((pts[k].x + nx * off * s) / wm).clamp(0.0, 1.0), ((pts[k].y + ny * off * s) / wm).clamp(0.0, 1.0));
            grid.sample(u, v)
        };
        pts[k].w = pts[k].z - pts[k].g >= FILL_MIN_M && side(1.0).max(side(-1.0)) <= pts[k].z + 0.3;
    }
    let mut i = 0;
    while i < n {
        if !pts[i].w {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < n && pts[j + 1].w {
            j += 1;
        }
        if pts[j].arc - pts[i].arc < FILL_RUN_M {
            for p in &mut pts[i..=j] {
                p.w = false;
            }
        }
        i = j + 1;
    }
}

fn mark_cut(pts: &mut [Pt], hw: f64, wm: f64, grid: &TerrainGrid) {
    let n = pts.len();
    let off = hw + 3.5;
    for k in 0..n {
        let (a, b) = (pts[k.saturating_sub(1)], pts[(k + 1).min(n - 1)]);
        let l = ((b.x - a.x).hypot(b.y - a.y)).max(1e-9);
        let (nx, ny) = (-(b.y - a.y) / l, (b.x - a.x) / l);
        let side = |s: f64| {
            let (u, v) = (((pts[k].x + nx * off * s) / wm).clamp(0.0, 1.0), ((pts[k].y + ny * off * s) / wm).clamp(0.0, 1.0));
            grid.sample(u, v)
        };
        pts[k].cut = pts[k].g - pts[k].z >= FILL_MIN_M && side(1.0).min(side(-1.0)) >= pts[k].z - 0.3;
    }
    let mut i = 0;
    while i < n {
        if !pts[i].cut {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < n && pts[j + 1].cut {
            j += 1;
        }
        if pts[j].arc - pts[i].arc < FILL_RUN_M {
            for p in &mut pts[i..=j] {
                p.cut = false;
            }
        }
        i = j + 1;
    }
}

/// Mark the terrain collider's quads over low-arch tunnel sections as holes, so a car can drive through the
/// shallow cover (the render mesh is untouched; the tube mesh is what you see and collide with inside).
pub fn carve_holes(grid: &mut TerrainGrid, plan: &RoadPlan) {
    let seg = grid.segments;
    let wm = grid.tile.width_m();
    let cell = wm / seg as f64;
    let mut holes = vec![false; seg * seg];
    let mut any = false;
    for it in plan
        .items
        .iter()
        .filter(|i| i.kind == Bed::Tunnel && i.section == 1)
    {
        let reach = it.width as f64 / 2.0 + 3.5;
        for k in 0..it.uv.len() - 1 {
            let (a, b) = (
                (it.uv[k].0 * wm, it.uv[k].1 * wm),
                (it.uv[k + 1].0 * wm, it.uv[k + 1].1 * wm),
            );
            let (i0, i1) = (
                ((a.0.min(b.0) - reach) / cell).floor().max(0.0) as usize,
                (((a.0.max(b.0) + reach) / cell).ceil().max(0.0) as usize).min(seg - 1),
            );
            let (j0, j1) = (
                ((a.1.min(b.1) - reach) / cell).floor().max(0.0) as usize,
                (((a.1.max(b.1) + reach) / cell).ceil().max(0.0) as usize).min(seg - 1),
            );
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            for jj in j0..=j1 {
                for ii in i0..=i1 {
                    let c = ((ii as f64 + 0.5) * cell, (jj as f64 + 0.5) * cell);
                    let t = (((c.0 - a.0) * dx + (c.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
                    if (c.0 - (a.0 + dx * t)).hypot(c.1 - (a.1 + dy * t)) < reach {
                        holes[jj * seg + ii] = true;
                        any = true;
                    }
                }
            }
        }
    }
    if any {
        grid.holes = holes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Pt {
        Pt {
            x,
            y,
            z: x as f32,
            k: 0,
            arc: x as f32,
            g: 0.0,
            w: false,
            cut: false,
        }
    }

    fn road(len: usize, z: f32, g: f32) -> Vec<Pt> {
        (0..len).map(|i| Pt { x: 100.0 + i as f64 * 4.0, y: 600.0, z, k: 0, arc: i as f32 * 4.0, g, w: false, cut: false }).collect()
    }

    #[test]
    fn embankment_is_walled_but_a_sidehill_cut_is_not() {
        let tile = ed_geo::tile::TileId::new(14, 8544, 5827);
        let wm = tile.width_m();
        let flat = TerrainGrid::flat(tile, 64, 0.0);
        let mut p = road(20, 5.0, 0.0);
        mark_walled(&mut p, 5.0, wm, &flat);
        assert!(p.iter().all(|q| q.w), "5 m on flat ground over 76 m is a walled fill");
        // Short run: dropped.
        let mut short = road(4, 5.0, 0.0);
        mark_walled(&mut short, 5.0, wm, &flat);
        assert!(short.iter().all(|q| !q.w));
        // Low fill: graded, not walled.
        let mut low = road(20, 1.5, 0.0);
        mark_walled(&mut low, 5.0, wm, &flat);
        assert!(low.iter().all(|q| !q.w));
        // Hillside on one side rising above the road: stays a graded cut.
        let mut hill = TerrainGrid::flat(tile, 64, 0.0);
        let n = 65;
        for j in 0..n {
            for i in 0..n {
                if (j as f64) / 64.0 * wm < 600.0 {
                    hill.heights[j * n + i] = 9.0;
                }
            }
        }
        let mut side = road(20, 5.0, 0.0);
        mark_walled(&mut side, 5.0, wm, &hill);
        assert!(side.iter().all(|q| !q.w));
    }

    #[test]
    fn split_inserts_vertices_at_rect_borders() {
        let r1 = Rect::grown(100.0, 2.0);
        let r2 = Rect::grown(100.0, 40.0);
        let out = split(&[pt(-60.0, 50.0), pt(160.0, 50.0)], [r1, r2]);
        let xs: Vec<f64> = out.iter().map(|p| p.x).collect();
        for want in [-40.0, -2.0, 102.0, 140.0] {
            assert!(
                xs.iter().any(|x| (x - want).abs() < 1e-6),
                "{want} in {xs:?}"
            );
        }
        // Heights are interpolated at the inserted vertices.
        let p = out.iter().find(|p| (p.x + 2.0).abs() < 1e-6).unwrap();
        assert!((p.z + 2.0).abs() < 1e-4);
    }
}
