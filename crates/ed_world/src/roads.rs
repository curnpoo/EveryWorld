//! Roads, paths and rail from the OpenMapTiles `transportation` layer, draped
//! on the terrain grid, plus region-specific markings and street furniture.

use crate::mesh::{MeshData, rgb};
use crate::project::TileProjector;
use crate::style::Region;
use crate::surface::SurfaceMap;
use crate::terrain::TerrainGrid;
use ed_data::mvt::{self, GeomType};
use glam::Vec3;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Right,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterLine {
    /// North America: double solid yellow on two-way roads.
    DoubleYellow,
    /// Europe & most of the world: dashed white.
    DashedWhite,
    /// Japan/UK-ish: solid white on majors, dashed on minors.
    SolidWhite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoleKind {
    None,
    Wood,
    Concrete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalLayout {
    Vertical,
    Horizontal,
}

/// Everything about streets that changes when you cross a border.
#[derive(Debug, Clone, Copy)]
pub struct StreetKit {
    pub drive_side: Side,
    pub center_line: CenterLine,
    pub utility_poles: PoleKind,
    pub signals: SignalLayout,
    pub signal_on_far_side: bool,
    pub asphalt: u32,
    pub sidewalk: u32,
    pub curb_height: f32,
    pub stop_sign_text: &'static str,
}

impl StreetKit {
    pub fn for_region(r: Region) -> StreetKit {
        use Region::*;
        let base = StreetKit {
            drive_side: Side::Right,
            center_line: CenterLine::DashedWhite,
            utility_poles: PoleKind::None,
            signals: SignalLayout::Vertical,
            signal_on_far_side: false,
            asphalt: 0x4b4e54,
            sidewalk: 0xc9c6bf,
            curb_height: 0.15,
            stop_sign_text: "STOP",
        };
        match r {
            NorthAmericaCity | NorthAmericaSuburb => StreetKit {
                center_line: CenterLine::DoubleYellow,
                utility_poles: if r == NorthAmericaSuburb { PoleKind::Wood } else { PoleKind::None },
                signal_on_far_side: true,
                ..base
            },
            LatinAmerica => StreetKit { center_line: CenterLine::DoubleYellow, utility_poles: PoleKind::Concrete, stop_sign_text: "ALTO", ..base },
            Japan => StreetKit {
                drive_side: Side::Left,
                center_line: CenterLine::SolidWhite,
                utility_poles: PoleKind::Concrete,
                signals: SignalLayout::Horizontal,
                stop_sign_text: "止まれ",
                ..base
            },
            British | Oceania => StreetKit { drive_side: Side::Left, ..base },
            SouthAsia => StreetKit { drive_side: Side::Left, utility_poles: PoleKind::Concrete, asphalt: 0x55575a, ..base },
            SubSaharan => StreetKit { utility_poles: PoleKind::Wood, asphalt: 0x56585b, sidewalk: 0xbfa98a, ..base },
            SoutheastAsia => StreetKit { utility_poles: PoleKind::Concrete, ..base },
            EastAsia => StreetKit { center_line: CenterLine::DoubleYellow, signals: SignalLayout::Horizontal, stop_sign_text: "停", ..base },
            NorthAfricaMiddleEast => StreetKit { asphalt: 0x5a5a58, sidewalk: 0xd6c8a8, stop_sign_text: "قف", ..base },
            _ => base,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropKind {
    StreetLight,
    UtilityPole(PoleKind),
    TrafficSignal(SignalLayout),
    StopSign,
}

#[derive(Debug, Clone, Copy)]
pub struct StreetProp {
    pub kind: PropKind,
    pub pos: Vec3,
    /// Direction the prop faces (toward the road / oncoming traffic).
    pub facing: Vec3,
}

#[derive(Debug, Default)]
pub struct RoadOutput {
    pub mesh: MeshData,
    /// Collision-only copy of decks, rails and piers (no markings or kerbs detail).
    pub collider: MeshData,
    pub props: Vec<StreetProp>,
    /// Road polylines in local space (for the occupancy mask / AI traffic).
    pub centerlines: Vec<(RoadClass, Vec<Vec3>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoadClass {
    Motorway,
    Trunk,
    Primary,
    Secondary,
    Tertiary,
    Minor,
    Service,
    Track,
    Path,
    Rail,
}

impl RoadClass {
    fn from_omt(class: &str) -> Option<Self> {
        use RoadClass::*;
        Some(match class {
            "motorway" => Motorway,
            "trunk" => Trunk,
            "primary" => Primary,
            "secondary" => Secondary,
            "tertiary" => Tertiary,
            "minor" | "residential" | "unclassified" | "living_street" | "raceway" => Minor,
            "service" | "busway" => Service,
            "track" => Track,
            "path" | "pedestrian" | "footway" | "cycleway" | "steps" | "bridleway" => Path,
            "rail" | "transit" => Rail,
            _ => return None,
        })
    }

    pub fn width(self) -> f32 {
        use RoadClass::*;
        match self {
            Motorway => 15.0,
            Trunk => 12.0,
            Primary => 10.0,
            Secondary => 8.5,
            Tertiary => 7.5,
            Minor => 6.5,
            Service => 4.0,
            Track => 2.6,
            Path => 1.3,
            Rail => 3.2,
        }
    }

    pub fn is_car_road(self) -> bool {
        !matches!(self, RoadClass::Path | RoadClass::Rail | RoadClass::Track)
    }

    fn rank(self) -> u8 {
        use RoadClass::*;
        match self {
            Motorway | Trunk => 5,
            Primary => 4,
            Secondary => 3,
            Tertiary => 2,
            Minor => 1,
            _ => 0,
        }
    }
}

const STEP_M: f32 = 3.0;

/// Deck simplification: a dense centerline sample is dropped when the road
/// stays within this many meters of the chord without it...
const DECK_TOL_M: f32 = 0.08;
/// ...but no span grows past this, so colour noise and the terrain-fit
/// of the curtains keep a sample every so often.
const DECK_MAX_SPAN_M: f32 = 12.0;
/// Ground-following ways (paths, tracks, rail) hug the terrain, so they
/// keep a tighter tolerance and span (about the terrain grid spacing).
const WAY_TOL_M: f32 = 0.05;
const WAY_MAX_SPAN_M: f32 = 6.0;

/// Simplified copy of a centerline (Douglas-Peucker in 3D with a max span).
/// Ends are always kept. The dense samples exist to carve the terrain and
/// follow its bumps; a graded deck is smooth by construction, so straight
/// runs collapse to a few vertices without changing the surface.
fn simplify_deck(pts: &[Vec3], tol: f32, max_span: f32) -> Vec<Vec3> {
    let n = pts.len();
    if n <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (pts[a], pts[b]);
        let ab = pb - pa;
        let len2 = ab.length_squared().max(1e-6);
        let (mut worst, mut wi) = (0.0f32, a + 1);
        for i in a + 1..b {
            let t = ((pts[i] - pa).dot(ab) / len2).clamp(0.0, 1.0);
            let d = pts[i].distance(pa + ab * t);
            if d > worst {
                worst = d;
                wi = i;
            }
        }
        let span = (pb - pa).length();
        if worst > tol {
            keep[wi] = true;
            stack.push((a, wi));
            stack.push((wi, b));
        } else if span > max_span {
            let mid = (a + b) / 2;
            keep[mid] = true;
            stack.push((a, mid));
            stack.push((mid, b));
        }
    }
    pts.iter().zip(keep).filter_map(|(p, k)| k.then_some(*p)).collect()
}

/// How a road meets the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bed {
    /// Paved road: a smoothed road bed; the terrain is cut/filled to meet it.
    Graded,
    /// Track, path, rail, driveway: follows the ground, pressed into it.
    Conform,
    /// Deck on a straight line between its ends; terrain untouched.
    Bridge,
}

#[derive(Debug, Clone)]
struct Item {
    class: RoadClass,
    kind: Bed,
    /// Tile uv of each sample and the bed height there.
    uv: Vec<(f64, f64)>,
    bed: Vec<f32>,
    /// Local-space centerline at bed height.
    pts: Vec<Vec3>,
    oneway: bool,
    paved: bool,
    urban: bool,
    /// Bridge deck that spans water (gets piers and clearance).
    over_water: bool,
}

/// Road layout for one tile: bed heights decided once, used both to carve the
/// terrain grid (`grade`) and to build the road geometry (`generate`).
#[derive(Debug, Default, Clone)]
pub struct RoadPlan {
    items: Vec<Item>,
    junctions: Vec<((i32, i32), (u8, u8))>,
    extent: f64,
}

impl RoadPlan {
    /// Any paved car road that grades the terrain.
    pub fn grades_terrain(&self) -> bool {
        self.items.iter().any(|i| i.kind != Bed::Bridge)
    }
}

pub fn plan(vt: &mvt::Tile, proj: &TileProjector, grid: &TerrainGrid, surface: &SurfaceMap) -> RoadPlan {
    let mut plan = RoadPlan::default();
    let Some(layer) = vt.layer("transportation") else { return plan };
    let ext = layer.extent as f64;
    plan.extent = ext;
    let wm = grid.tile.width_m();
    // Count how many car roads touch each vertex to find intersections.
    let mut junctions: HashMap<(i32, i32), (u8, u8)> = HashMap::new();

    // Lower classes first so bigger roads win where they overlap.
    let mut feats: Vec<(&mvt::Feature, RoadClass)> = layer
        .features
        .iter()
        .filter(|f| f.geom_type == GeomType::LineString)
        .filter(|f| f.str("brunnel") != Some("tunnel"))
        .filter_map(|f| RoadClass::from_omt(f.str("class").unwrap_or("")).map(|c| (f, c)))
        .collect();
    feats.sort_by_key(|(_, c)| c.rank());

    for (f, class) in feats {
        let oneway = f.num("oneway").unwrap_or(0.0) != 0.0;
        let paved = !matches!(f.str("surface"), Some("unpaved"));
        let bridge = f.str("brunnel") == Some("bridge");
        for line in &f.rings {
            if line.len() < 2 {
                continue;
            }
            if class.is_car_road() {
                for p in [line[0], line[line.len() - 1]] {
                    let e = junctions.entry((p[0] as i32, p[1] as i32)).or_default();
                    e.0 += 1;
                    e.1 = e.1.max(class.rank());
                }
            }
            let (uv, ground) = densify(line, ext, wm, grid);
            if uv.len() < 2 {
                continue;
            }
            let urban = surface.cover_at(line[0][0] as f64 / ext, line[0][1] as f64 / ext).is_urban();
            // Water under each sample: a road that crosses water without a mapped bridge gets
            // one anyway, anchored on the dry samples either side.
            let wf: Vec<f32> = uv.iter().map(|&(u, v)| surface.water_fraction(u, v)).collect();
            let mut spans: Vec<(usize, usize, bool)> = Vec::new();
            if bridge || class == RoadClass::Rail || !wf.iter().any(|&f| f >= 0.4) {
                spans.push((0, uv.len() - 1, bridge));
            } else {
                let n = uv.len();
                let mut i = 0;
                let mut start = 0;
                while i < n {
                    if wf[i] >= 0.4 {
                        let mut a = i;
                        while a > 0 && wf[a] >= 0.2 {
                            a -= 1;
                        }
                        a = a.saturating_sub(1);
                        let mut b = i;
                        while b + 1 < n && wf[b] >= 0.2 {
                            b += 1;
                        }
                        b = (b + 1).min(n - 1);
                        if a > start {
                            spans.push((start, a, false));
                        }
                        // Deck must start and end on land; if the road leaves the tile over water, still bridge it.
                        spans.push((a.max(start), b, true));
                        start = b;
                        i = b;
                        if b == n - 1 {
                            start = n;
                        }
                    }
                    i += 1;
                }
                if start + 1 < n {
                    spans.push((start, n - 1, false));
                }
            }
            for (s0, s1, is_bridge) in spans {
                if s1 <= s0 {
                    continue;
                }
                let (uv, ground, wf) = (uv[s0..=s1].to_vec(), ground[s0..=s1].to_vec(), &wf[s0..=s1]);
                let kind = if is_bridge {
                    Bed::Bridge
                } else if class.rank() >= 1 && paved {
                    Bed::Graded
                } else {
                    Bed::Conform
                };
                let wet = wf.iter().any(|&f| f >= 0.3);
                let bed = match kind {
                    Bed::Graded => smooth(&ground, 10, 3),
                    Bed::Bridge => bridge_bed(&ground, wet),
                    Bed::Conform => smooth(&ground, 5, 2),
                };
                let pts = uv.iter().zip(&bed).map(|(&(u, v), &h)| proj.local(u, v, h as f64)).collect();
                plan.items.push(Item { class, kind, uv, bed, pts, oneway, paved, urban, over_water: wet && kind == Bed::Bridge });
            }
        }
    }
    plan.junctions = junctions.into_iter().collect();
    plan
}

/// Bridge deck heights: a straight line between the (firmest) end heights; over water it
/// ramps up to clear the surface and arches slightly, so the deck never dips under waves.
fn bridge_bed(ground: &[f32], wet: bool) -> Vec<f32> {
    let n = ground.len() - 1;
    let k = 2.min(n);
    let a = if wet { ground[..=k].iter().cloned().fold(f32::MIN, f32::max) } else { ground[0] };
    let b = if wet { ground[n - k..].iter().cloned().fold(f32::MIN, f32::max) } else { ground[n] };
    let span = n as f32 * STEP_M;
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let mut h = a + (b - a) * t;
            if wet {
                let edge = (t.min(1.0 - t) * span / 12.0).clamp(0.0, 1.0);
                let edge = edge * edge * (3.0 - 2.0 * edge);
                h += 1.8 * edge + (std::f32::consts::PI * t).sin() * (span * 0.02).min(2.0);
            }
            h
        })
        .collect()
}

/// Polyline in tile px -> (tile uv, ground height) every ~STEP_M meters.
fn densify(line: &[[f32; 2]], ext: f64, wm: f64, grid: &TerrainGrid) -> (Vec<(f64, f64)>, Vec<f32>) {
    let mut uv: Vec<(f64, f64)> = Vec::new();
    let mut push = |u: f64, v: f64| {
        if uv.last().is_none_or(|q| ((q.0 - u).powi(2) + (q.1 - v).powi(2)).sqrt() * wm > 0.01) {
            uv.push((u, v));
        }
    };
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (((b[0] - a[0]) as f64).hypot((b[1] - a[1]) as f64)) / ext * wm;
        let n = (len / STEP_M as f64).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = k as f64 / n as f64;
            push((a[0] as f64 + (b[0] - a[0]) as f64 * t) / ext, (a[1] as f64 + (b[1] - a[1]) as f64 * t) / ext);
        }
    }
    let l = line[line.len() - 1];
    push(l[0] as f64 / ext, l[1] as f64 / ext);
    let h = uv.iter().map(|&(u, v)| grid.sample(u, v)).collect();
    (uv, h)
}

/// Box-filter the heights along the road: the bed ignores bumps shorter than
/// ~2*radius*STEP_M, like a real graded road.
fn smooth(h: &[f32], radius: usize, passes: usize) -> Vec<f32> {
    let mut a = h.to_vec();
    for _ in 0..passes {
        let b = a.clone();
        for i in 0..a.len() {
            let (lo, hi) = (i.saturating_sub(radius), (i + radius).min(a.len() - 1));
            a[i] = b[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
        }
    }
    a
}

/// Cut and fill the terrain so graded roads sit ON it: grid vertices near the
/// centerline take the bed height exactly (so the terrain collider is the road
/// surface too), and the ground eases back to natural over a shoulder whose
/// length grows with the cut/fill depth. Run before anything else samples the grid.
pub fn grade(grid: &mut TerrainGrid, plan: &RoadPlan) {
    let seg = grid.segments;
    let n = seg + 1;
    let wm = grid.tile.width_m();
    let cell = wm / seg as f64;
    for it in plan.items.iter()
        .filter(|i| i.kind != Bed::Bridge) {
        let hw = it.class.width() as f64 / 2.0;
        // Wide roads: every triangle under the deck lies on the flat bench. Trails
        // only get a narrow one (the level deck + sloped curtain hide the rest).
        let narrow = hw < 2.0;
        let r1 = hw + if narrow { 0.8 } else { 1.45 } * cell;
        let sk = if narrow { 0.6 } else { 1.0 };
        let reach = r1 + 14.0;
        let mut best: HashMap<usize, (f64, f32)> = HashMap::new();
        for k in 0..it.uv.len() - 1 {
            let (a, b) = ((it.uv[k].0 * wm, it.uv[k].1 * wm), (it.uv[k + 1].0 * wm, it.uv[k + 1].1 * wm));
            let (ha, hb) = (it.bed[k], it.bed[k + 1]);
            let i0 = (((a.0.min(b.0) - reach) / cell).floor().max(0.0)) as usize;
            let i1 = (((a.0.max(b.0) + reach) / cell).ceil().max(0.0) as usize).min(seg);
            let j0 = (((a.1.min(b.1) - reach) / cell).floor().max(0.0)) as usize;
            let j1 = (((a.1.max(b.1) + reach) / cell).ceil().max(0.0) as usize).min(seg);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let p = (i as f64 * cell, j as f64 * cell);
                    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
                    let d = (p.0 - (a.0 + dx * t)).hypot(p.1 - (a.1 + dy * t));
                    if d < reach {
                        let bh = ha + (hb - ha) * t as f32;
                        let e = best.entry(j * n + i).or_insert((d, bh));
                        if d < e.0 {
                            *e = (d, bh);
                        }
                    }
                }
            }
        }
        for (idx, (d, bh)) in best {
            let nat = grid.heights[idx];
            // Cut banks (ground above the bed) stand steeper than fills.
            let dh = (bh - nat) as f64;
            let s = if dh < 0.0 { (1.5 + 1.2 * -dh).min(8.0) * sk } else { (3.0 + 2.5 * dh).min(14.0) * sk };
            let w = if d <= r1 {
                1.0
            } else if d < r1 + s {
                let x = (d - r1) / s;
                1.0 - x * x * (3.0 - 2.0 * x)
            } else {
                0.0
            };
            grid.heights[idx] = nat + (bh - nat) * w as f32;
        }
    }
}

pub fn generate(plan: &RoadPlan, proj: &TileProjector, grid: &TerrainGrid, surface: &SurfaceMap, region: Region) -> RoadOutput {
    let mut out = RoadOutput::default();
    let kit = StreetKit::for_region(region);
    let up = proj.up();
    let uvm = proj.uv_map();
    let ground = |p: Vec3| {
        let [u, v] = uvm.uv(p);
        grid.sample(u as f64, v as f64)
    };

    for it in &plan.items {
        let class = it.class;
        // Re-snap the centerline to the graded terrain, so every offset below is
        // relative to the surface that actually renders. Bridge decks keep their line.
        let snapped: Vec<Vec3>;
        let pts: &Vec<Vec3> = if it.kind == Bed::Bridge {
            &it.pts
        } else {
            snapped = it.uv.iter().map(|&(u, v)| proj.local(u, v, grid.sample(u, v) as f64)).collect();
            &snapped
        };
        let w = class.width();
        let hw = w / 2.0;
        match it.kind {
            Bed::Bridge if !class.is_car_road() => {
                let prof = conform_profile(class, it.paved, false, &kit);
                let way = simplify_deck(pts, WAY_TOL_M, WAY_MAX_SPAN_M);
                sweep(&mut out.mesh, &way, up, &prof, None, false, 1.2, 0.0, 0.06);
                sweep(&mut out.collider, &way, up, &prof, None, false, 1.2, 0.0, 0.06);
            }
            Bed::Graded | Bed::Bridge => {
                let deck = simplify_deck(pts, DECK_TOL_M, DECK_MAX_SPAN_M);
                let prof = paved_profile(hw, it.urban && it.kind == Bed::Graded, &kit);
                let skirt = if it.kind == Bed::Bridge { 1.2 } else { 0.6 };
                sweep(&mut out.mesh, &deck, up, &prof, Some(&ground as &dyn Fn(Vec3) -> f32), true, skirt, 0.04, 0.05);
                sweep(&mut out.collider, &deck, up, &prof, Some(&ground as &dyn Fn(Vec3) -> f32), true, skirt, 0.04, 0.05);
                if it.paved && class.rank() >= 1 {
                    markings(&mut out.mesh, &deck, up, if it.kind == Bed::Bridge { None } else { Some(&ground as &dyn Fn(Vec3) -> f32) }, true, class, &kit, it.oneway);
                }
                if it.over_water {
                    piers(&mut out.mesh, pts, &it.bed, up, hw, &ground);
                    piers(&mut out.collider, pts, &it.bed, up, hw, &ground);
                }
                let rail_on_deck = it.kind == Bed::Bridge;
                if rail_on_deck || (matches!(class, RoadClass::Motorway | RoadClass::Trunk) && !it.urban) {
                    let off = if rail_on_deck { hw + 0.15 } else { hw + 1.5 };
                    for s in [1.0f32, -1.0] {
                        let g = offset_line(pts, up, s * off, |i, p| if rail_on_deck { 0.0 } else { ground(p) - it.bed[i] });
                        guardrail(&mut out.mesh, &g, up, s);
                        guardrail(&mut out.collider, &g, up, s);
                    }
                }
            }
            Bed::Conform => {
                let prof = conform_profile(class, it.paved, it.urban, &kit);
                let way = simplify_deck(pts, WAY_TOL_M, WAY_MAX_SPAN_M);
                sweep(&mut out.mesh, &way, up, &prof, Some(&ground as &dyn Fn(Vec3) -> f32), true, 0.5, 0.09, if it.paved { 0.07 } else { 0.2 });
                sweep(&mut out.collider, &way, up, &prof, Some(&ground as &dyn Fn(Vec3) -> f32), true, 0.5, 0.09, if it.paved { 0.07 } else { 0.2 });
            }
        }
        street_props(&mut out.props, pts, up, class, &kit, it.urban, it.kind != Bed::Conform);
        out.centerlines.push((class, pts.clone()));
    }

    // Signals at urban junctions between real roads; stop signs at minor ones.
    let ext = plan.extent;
    for &((x, y), (count, rank)) in &plan.junctions {
        if count < 3 || !(0..ext as i32).contains(&x) || !(0..ext as i32).contains(&y) {
            continue;
        }
        let (u, v) = (x as f64 / ext, y as f64 / ext);
        if !surface.cover_at(u, v).is_urban() {
            continue;
        }
        let p = proj.local(u, v, grid.sample(u, v) as f64);
        let off = Vec3::new(5.0, 0.0, 5.0).reject_from_normalized(up);
        let kind = if rank >= 2 { PropKind::TrafficSignal(kit.signals) } else { PropKind::StopSign };
        out.props.push(StreetProp { kind, pos: p + off + up * kit.curb_height, facing: -off.normalize_or(Vec3::X) });
    }
    out
}

fn side_dirs(pts: &[Vec3], up: Vec3) -> Vec<Vec3> {
    (0..pts.len())
        .map(|i| {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(pts.len() - 1)];
            // Right-hand side of travel direction.
            (b - a).cross(up).normalize_or_zero()
        })
        .collect()
}

/// One point of a road cross-section: meters right of center, meters above the
/// bed, and its color. `grad` blends the strip that starts here to the next
/// point's color (soft verges); otherwise the strip is one flat color.
#[derive(Debug, Clone, Copy)]
struct P {
    o: f32,
    h: f32,
    c: [f32; 4],
    grad: bool,
}

fn p(o: f32, h: f32, c: [f32; 4], grad: bool) -> P {
    P { o, h, c, grad }
}

const CROWN: f32 = 0.07;

/// Crown height of a paved road at offset `o`: a gentle camber, higher at the centerline.
fn crown(o: f32, hw: f32) -> f32 {
    CROWN - (CROWN - 0.02) * (o.abs() / hw).min(1.0)
}

/// Cross-section of a paved road, left to right. Urban: sidewalk, curb face,
/// gutter, cambered asphalt. Rural: gravel shoulder easing into the verge.
fn paved_profile(hw: f32, urban: bool, kit: &StreetKit) -> Vec<P> {
    let asphalt = rgb(kit.asphalt);
    let mut v = Vec::new();
    if urban {
        let (sw, ch) = (rgb(kit.sidewalk), kit.curb_height + 0.01);
        let curb = rgb(0xaaa8a0);
        let gutter = rgb(0x6c6d70);
        v.push(p(-hw - 2.4, ch, sw, false));
        v.push(p(-hw - 0.16, ch, curb, false));
        v.push(p(-hw - 0.16, 0.03, gutter, false));
        v.push(p(-hw, 0.02, asphalt, false));
        v.push(p(0.0, CROWN, asphalt, false));
        v.push(p(hw, 0.02, gutter, false));
        v.push(p(hw + 0.16, 0.03, curb, false));
        v.push(p(hw + 0.16, ch, sw, false));
        v.push(p(hw + 2.4, ch, sw, false));
    } else {
        let gravel = rgb(0x827d6e);
        let verge = rgb(0x6a6c48);
        let ditch = lerp_color(verge, rgb(0x5a4f3a), 0.5);
        v.push(p(-hw - 3.4, 0.03, verge, true));
        v.push(p(-hw - 2.2, 0.02, ditch, true));
        v.push(p(-hw - 1.2, 0.02, verge, true));
        v.push(p(-hw - 0.5, 0.0, gravel, false));
        v.push(p(-hw, 0.02, asphalt, false));
        v.push(p(0.0, CROWN, asphalt, false));
        v.push(p(hw, 0.02, gravel, false));
        v.push(p(hw + 0.5, 0.0, gravel, true));
        v.push(p(hw + 1.2, 0.02, verge, true));
        v.push(p(hw + 2.2, 0.02, ditch, true));
        v.push(p(hw + 3.4, 0.03, verge, false));
    }
    v
}

/// Cross-section of ground-following ways: pressed into the ground with soft,
/// ground-tinted edges (paths, tracks with wheel ruts, rail ballast, drives).
fn conform_profile(class: RoadClass, paved: bool, urban: bool, kit: &StreetKit) -> Vec<P> {
    let hw = class.width() / 2.0;
    let grass = rgb(0x6a6b44);
    let edge = |c: [f32; 4]| lerp_color(c, grass, 0.65);
    match class {
        RoadClass::Rail => {
            let b = rgb(0x6b5d52);
            vec![p(-hw - 0.9, 0.0, edge(b), true), p(-hw - 0.3, 0.22, b, false), p(hw + 0.3, 0.22, b, true), p(hw + 0.9, 0.0, edge(b), false)]
        }
        RoadClass::Track => {
            let base = rgb(0xa88e6a);
            let rut = rgb(0x7d6849);
            let ridge = lerp_color(base, grass, 0.35);
            vec![
                p(-hw - 0.9, 0.04, edge(base), true),
                p(-hw - 0.4, 0.1, ridge, true),
                p(-hw + 0.1, -0.03, base, true),
                p(-0.95, -0.1, rut, true),
                p(-0.5, -0.04, base, true),
                p(0.0, 0.01, ridge, true),
                p(0.5, -0.04, base, true),
                p(0.95, -0.1, rut, true),
                p(hw - 0.1, -0.03, base, true),
                p(hw + 0.4, 0.1, ridge, true),
                p(hw + 0.9, 0.04, edge(base), false),
            ]
        }
        RoadClass::Path if urban => {
            let c = rgb(0xc9c3b8);
            vec![p(-hw - 0.15, 0.0, edge(c), true), p(-hw, 0.03, c, false), p(hw, 0.03, c, true), p(hw + 0.15, 0.0, edge(c), false)]
        }
        RoadClass::Path => {
            let base = rgb(0xb59a73);
            vec![p(-hw - 0.5, 0.0, edge(base), true), p(-hw, -0.02, base, true), p(0.0, -0.05, base, true), p(hw, -0.02, base, true), p(hw + 0.5, 0.0, edge(base), false)]
        }
        _ => {
            // Service lanes and unpaved roads.
            let base = if paved { rgb(kit.asphalt) } else { rgb(0xa88e6a) };
            vec![p(-hw - 0.6, 0.0, edge(base), true), p(-hw, 0.0, base, true), p(0.0, 0.03, base, true), p(hw, 0.0, base, true), p(hw + 0.6, 0.0, edge(base), false)]
        }
    }
}

/// Sweep a cross-section along `pts`. With `conform`, each profile point drops
/// to the real ground under it (so ways hug slopes); `skirt` hangs a curtain off
/// both outer edges so a lower ground never opens a crack beside the road.
fn sweep(m: &mut MeshData, pts: &[Vec3], up: Vec3, prof: &[P], conform: Option<&dyn Fn(Vec3) -> f32>, flat: bool, skirt: f32, lift: f32, var: f32) {
    if pts.len() < 2 || prof.len() < 2 {
        return;
    }
    let side = side_dirs(pts, up);
    // Subdivide wide strips (<= ~2 m) so each piece follows the terrain triangles
    // under it instead of spanning a crease between them.
    let mut dense: Vec<P> = Vec::new();
    for (k, &a) in prof.iter().enumerate() {
        dense.push(a);
        if let Some(&b) = prof.get(k + 1) {
            let n = (((b.o - a.o).abs() / 2.0).ceil() as usize).max(1);
            for j in 1..n {
                let t = j as f32 / n as f32;
                let c = if a.grad { lerp_color(a.c, b.c, t) } else { a.c };
                dense.push(P { o: a.o + (b.o - a.o) * t, h: a.h + (b.h - a.h) * t, c, grad: a.grad });
            }
        }
    }
    let prof = &dense[..];
    // Curtains: extend the profile down at both ends.
    let mut pr = Vec::with_capacity(prof.len() + 2);
    let first = prof[0];
    pr.push(P { h: first.h - skirt, o: first.o - skirt * 1.5, grad: false, ..first });
    pr.extend_from_slice(prof);
    let last = prof[prof.len() - 1];
    pr.push(P { h: last.h - skirt, o: last.o + skirt * 1.5, ..last });
    // Distance along the road, for UVs.
    let mut dist = Vec::with_capacity(pts.len());
    let mut d = 0.0f32;
    for i in 0..pts.len() {
        if i > 0 {
            d += pts[i].distance(pts[i - 1]);
        }
        dist.push(d);
    }
    // World position of every (point, profile index).
    let pos = |i: usize, k: usize| -> Vec3 {
        let q = &pr[k];
        let mut h = q.h;
        if let (Some(g), true) = (conform, flat) {
            // Level cross-section at the bed height; the outer curtain slopes down
            // to wherever the terrain is (embankment / cliff), never tilting the deck.
            if k == 0 || k == pr.len() - 1 {
                let edge = pr[if k == 0 { 1 } else { pr.len() - 2 }].h;
                let gh = g(pts[i] + side[i] * q.o) - g(pts[i]);
                h = (edge - 0.3).min(gh - 0.3).max(edge - 6.0);
            } else {
                h += lift;
            }
        } else if let Some(g) = conform {
            // Hug the ground under this profile point (curtains share their neighbour's).
            let at = pts[i] + side[i] * pr[k.clamp(1, pr.len() - 2)].o;
            // Drape on the real terrain triangles under this point, a hair above them.
            // Highest terrain within ~1.5 m, so a crease between samples can't poke through.
            let fw = side[i].cross(up);
            let top = [at, at + side[i] * 1.5, at - side[i] * 1.5, at + fw * 1.5, at - fw * 1.5].into_iter().map(g).fold(f32::MIN, f32::max);
            h += top - g(pts[i]) + lift;
        }
        pts[i] + side[i] * q.o + up * h
    };
    let mut prev: Option<Vec<(u32, u32)>> = None;
    for i in 0..pts.len() {
        let mut cur = Vec::with_capacity(pr.len() - 1);
        for k in 0..pr.len() - 1 {
            let (a, b) = (&pr[k], &pr[k + 1]);
            let (d_o, d_h) = (b.o - a.o, b.h - a.h);
            let n = (side[i] * -d_h + up * d_o).normalize_or_zero();
            let tint = |c: [f32; 4], o: f32| {
                let n = 0.6 * vnoise(dist[i] / 2.5, o / 0.7) + 0.4 * vnoise(dist[i] / 14.0 + 40.0, o / 3.0);
                let f = 1.0 + var * n;
                [c[0] * f, c[1] * f, c[2] * f, c[3]]
            };
            let ca = tint(a.c, a.o);
            let cb = tint(if a.grad { b.c } else { a.c }, b.o);
            let l = m.vertex(pos(i, k).to_array(), n.to_array(), [a.o, dist[i]], ca);
            let r = m.vertex(pos(i, k + 1).to_array(), n.to_array(), [b.o, dist[i]], cb);
            cur.push((l, r));
        }
        if let Some(pv) = &prev {
            for k in 0..cur.len() {
                let ((pl, pr_), (l, r)) = (pv[k], cur[k]);
                if (pr[k + 1].o - pr[k].o).abs() < 1e-4 && (pr[k + 1].h - pr[k].h).abs() < 1e-4 {
                    continue;
                }
                tri_out(m, pl, pr_, r);
                tri_out(m, pl, r, l);
            }
        }
        prev = Some(cur);
    }
}

/// Push a triangle, flipped if needed so its front agrees with its vertex normals.
fn hash2(a: i32, b: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 32767.5 - 1.0
}

/// Smooth value noise in [-1, 1].
fn vnoise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as i32, yi as i32);
    let a = hash2(xi, yi) + (hash2(xi + 1, yi) - hash2(xi, yi)) * sx;
    let b = hash2(xi, yi + 1) + (hash2(xi + 1, yi + 1) - hash2(xi, yi + 1)) * sx;
    a + (b - a) * sy
}

fn tri_out(m: &mut MeshData, a: u32, b: u32, c: u32) {
    let v = |i: u32| Vec3::from(m.positions[i as usize]);
    let n = (v(b) - v(a)).cross(v(c) - v(a));
    let want = Vec3::from(m.normals[a as usize]) + Vec3::from(m.normals[b as usize]) + Vec3::from(m.normals[c as usize]);
    if n.dot(want) >= 0.0 {
        m.tri(a, b, c);
    } else {
        m.tri(a, c, b);
    }
}

/// A flat painted strip `o0..o1` along the road, following the cambered surface.
fn paint(m: &mut MeshData, pts: &[Vec3], up: Vec3, ground: Option<&dyn Fn(Vec3) -> f32>, flat: bool, hw: f32, o0: f32, o1: f32, color: [f32; 4], dash: Option<(f32, f32)>) {
    let lift = 0.03;
    let prof = [p(o0, crown(o0, hw) + lift, color, false), p(o1, crown(o1, hw) + lift, color, false)];
    match dash {
        None => sweep(m, pts, up, &prof, ground, flat, 0.0, 0.04, 0.0),
        Some((on, off)) => {
            for run in dash_runs(pts, on, off) {
                sweep(m, &run, up, &prof, ground, flat, 0.0, 0.04, 0.0);
            }
        }
    }
}

/// Cut the polyline into `on`-long dashes every `on + off` meters. Cuts fall
/// exactly on the dash boundaries (not on the 6 m samples), so short dashes survive.
fn dash_runs(pts: &[Vec3], on: f32, off: f32) -> Vec<Vec<Vec3>> {
    let mut runs = Vec::new();
    let mut cur: Vec<Vec3> = Vec::new();
    let period = on + off;
    let mut s = 0.0f32;
    for w in pts.windows(2) {
        let len = w[0].distance(w[1]);
        if len < 1e-4 {
            continue;
        }
        let mut t = 0.0f32;
        while t < len {
            let phase = (s + t) % period;
            let (inside, next) = if phase < on { (true, on - phase) } else { (false, period - phase) };
            let step = next.min(len - t);
            if inside {
                if cur.is_empty() {
                    cur.push(w[0].lerp(w[1], t / len));
                }
                cur.push(w[0].lerp(w[1], (t + step) / len));
                if step >= next - 1e-4 {
                    runs.push(std::mem::take(&mut cur));
                }
            }
            t += step.max(1e-4);
        }
        s += len;
    }
    if cur.len() >= 2 {
        runs.push(cur);
    }
    runs.retain(|r| r.len() >= 2);
    runs
}

fn markings(m: &mut MeshData, pts: &[Vec3], up: Vec3, ground: Option<&dyn Fn(Vec3) -> f32>, flat: bool, class: RoadClass, kit: &StreetKit, oneway: bool) {
    let white = rgb(0xf2f2ee);
    let yellow = rgb(0xf2c230);
    let hw = class.width() / 2.0;
    if !oneway {
        match kit.center_line {
            CenterLine::DoubleYellow => {
                paint(m, pts, up, ground, flat, hw, -0.26, -0.11, yellow, None);
                paint(m, pts, up, ground, flat, hw, 0.11, 0.26, yellow, None);
            }
            CenterLine::SolidWhite if class.rank() >= 3 => paint(m, pts, up, ground, flat, hw, -0.08, 0.08, white, None),
            _ => paint(m, pts, up, ground, flat, hw, -0.08, 0.08, white, Some((3.0, 6.0))),
        }
    } else if class.rank() >= 3 {
        paint(m, pts, up, ground, flat, hw, -0.08, 0.08, white, Some((3.0, 9.0)));
    }
    if class.rank() >= 2 {
        paint(m, pts, up, ground, flat, hw, hw - 0.5, hw - 0.35, white, None);
        paint(m, pts, up, ground, flat, hw, -hw + 0.35, -hw + 0.5, white, None);
    }
}

/// A polyline parallel to `pts`, `off` meters to the right, with a per-point height correction.
fn offset_line(pts: &[Vec3], up: Vec3, off: f32, dh: impl Fn(usize, Vec3) -> f32) -> Vec<Vec3> {
    let side = side_dirs(pts, up);
    (0..pts.len())
        .map(|i| {
            let q = pts[i] + side[i] * off;
            q + up * dh(i, q)
        })
        .collect()
}

/// W-beam guardrail on posts. `g` runs along the rail base; `s` is +1 on the
/// right of the road, -1 on the left (the rail faces the traffic).
fn guardrail(m: &mut MeshData, g: &[Vec3], up: Vec3, s: f32) {
    if g.len() < 2 {
        return;
    }
    let steel = rgb(0xb4b9bf);
    let post = rgb(0x7b6a55);
    let side = side_dirs(g, up);
    // W profile in (offset, height); bulges toward the road (negative offset for the right rail).
    let f = -s;
    let w: [(f32, f32); 7] = [(0.05, 0.45), (0.0, 0.51), (0.05, 0.57), (0.0, 0.63), (0.05, 0.69), (0.1, 0.69), (0.1, 0.45)];
    let loop_: Vec<(f32, f32)> = w.iter().map(|&(o, h)| (o * f, h)).collect();
    let n = loop_.len();
    let cen = (loop_.iter().map(|q| q.0).sum::<f32>() / n as f32, loop_.iter().map(|q| q.1).sum::<f32>() / n as f32);
    for i in 0..g.len() - 1 {
        for k in 0..n {
            let (a, b) = (loop_[k], loop_[(k + 1) % n]);
            let (d_o, d_h) = (b.0 - a.0, b.1 - a.1);
            // Edge normal in (offset, height), flipped to point away from the loop's middle.
            let mut no = (d_h, -d_o);
            if no.0 * ((a.0 + b.0) * 0.5 - cen.0) + no.1 * ((a.1 + b.1) * 0.5 - cen.1) < 0.0 {
                no = (-no.0, -no.1);
            }
            let nrm = |sd: Vec3| (sd * no.0 + up * no.1).normalize_or_zero();
            let (n0, n1) = (nrm(side[i]), nrm(side[i + 1]));
            let at = |j: usize, o: f32, h: f32| g[j] + side[j] * o + up * h;
            let v = [
                m.vertex(at(i, a.0, a.1).to_array(), n0.to_array(), [0.0, 0.0], steel),
                m.vertex(at(i, b.0, b.1).to_array(), n0.to_array(), [0.0, 0.0], steel),
                m.vertex(at(i + 1, b.0, b.1).to_array(), n1.to_array(), [0.0, 0.0], steel),
                m.vertex(at(i + 1, a.0, a.1).to_array(), n1.to_array(), [0.0, 0.0], steel),
            ];
            tri_out(m, v[0], v[1], v[2]);
            tri_out(m, v[0], v[2], v[3]);
        }
    }
    // Posts every ~3.8 m, set behind the rail, driven into the ground.
    let mut dist = 0.0f32;
    let mut next = 0.0f32;
    for i in 0..g.len() - 1 {
        let seg = g[i + 1] - g[i];
        let len = seg.length();
        while next <= dist + len {
            let t = ((next - dist) / len.max(1e-4)).clamp(0.0, 1.0);
            let c = g[i].lerp(g[i + 1], t);
            let sd = side[i].lerp(side[i + 1], t).normalize_or(side[i]);
            let fw = seg.normalize_or_zero();
            post_box(m, c - sd * (f * 0.12), up, sd, fw, post);
            next += 3.8;
        }
        dist += len;
    }
}

/// Concrete piers under a water-spanning deck every ~16 m, down to the (carved) bed.
fn piers(m: &mut MeshData, pts: &[Vec3], bed: &[f32], up: Vec3, hw: f32, ground: &dyn Fn(Vec3) -> f32) {
    let concrete = rgb(0x9a9893);
    let side = side_dirs(pts, up);
    let mut dist = 0.0f32;
    let mut next = 8.0f32;
    for i in 0..pts.len() - 1 {
        let seg = pts[i + 1] - pts[i];
        let len = seg.length();
        while next <= dist + len {
            let t = ((next - dist) / len.max(1e-4)).clamp(0.0, 1.0);
            let c = pts[i].lerp(pts[i + 1], t);
            let sd = side[i].lerp(side[i + 1], t).normalize_or(side[i]);
            let fw = seg.normalize_or_zero();
            let deck_h = bed[i] + (bed[i + 1] - bed[i]) * t;
            let depth = ground(c) - 3.0 - deck_h;
            box_post(m, c, up, sd, fw, concrete, (hw * 0.8).max(1.0), 0.6, depth.min(-1.5), -0.35);
            next += 16.0;
        }
        dist += len;
    }
}

/// Square post 0.1 m on a side, from 0.25 m underground to 0.7 m.
fn post_box(m: &mut MeshData, base: Vec3, up: Vec3, sd: Vec3, fw: Vec3, color: [f32; 4]) {
    box_post(m, base, up, sd, fw, color, 0.05, 0.05, -0.25, 0.7);
}

/// Axis box with half-sizes `ha` (across) and `hb` (along), from `lo` to `hi` above `base`.
#[allow(clippy::too_many_arguments)]
fn box_post(m: &mut MeshData, base: Vec3, up: Vec3, sd: Vec3, fw: Vec3, color: [f32; 4], ha: f32, hb: f32, lo: f32, hi: f32) {
    let (a, b) = (sd * ha, fw * hb);
    let c = |sa: f32, sb: f32, h: f32| base + a * sa + b * sb + up * h;
    let top = [c(-1.0, -1.0, hi), c(1.0, -1.0, hi), c(1.0, 1.0, hi), c(-1.0, 1.0, hi)];
    let bot = [c(-1.0, -1.0, lo), c(1.0, -1.0, lo), c(1.0, 1.0, lo), c(-1.0, 1.0, lo)];
    m.quad([top[3], top[2], top[1], top[0]], [[0.0; 2]; 4], color);
    for k in 0..4 {
        let k2 = (k + 1) % 4;
        let q = [bot[k], bot[k2], top[k2], top[k]];
        // Face the corner pair outward from the post center.
        let mid = (q[0] + q[1]) * 0.5;
        let flat = (q[1] - q[0]).cross(q[3] - q[0]);
        if flat.dot(mid - (base + up * ((lo + hi) * 0.5))) >= 0.0 {
            m.quad(q, [[0.0; 2]; 4], color);
        } else {
            m.quad([q[1], q[0], q[3], q[2]], [[0.0; 2]; 4], color);
        }
    }
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// Street lights and utility poles beside car roads. On curbed (urban, graded)
/// roads they stand on the sidewalk, one curb height up.
fn street_props(props: &mut Vec<StreetProp>, pts: &[Vec3], up: Vec3, class: RoadClass, kit: &StreetKit, urban: bool, curbed: bool) {
    if !class.is_car_road() {
        return;
    }
    let side = side_dirs(pts, up);
    let w = class.width();
    let lift = if urban && curbed { up * (kit.curb_height + 0.01) } else { Vec3::ZERO };
    let mut dist = 0.0;
    let mut next_light = 12.0;
    let mut next_pole = 20.0;
    let mut flip = false;
    for i in 1..pts.len() {
        dist += pts[i].distance(pts[i - 1]);
        if urban && class.rank() >= 1 && dist >= next_light {
            next_light += 28.0;
            flip = !flip;
            let s = if flip { 1.0 } else { -1.0 };
            props.push(StreetProp { kind: PropKind::StreetLight, pos: pts[i] + side[i] * s * (w / 2.0 + 1.0) + lift, facing: -side[i] * s });
        }
        if kit.utility_poles != PoleKind::None && class.rank() <= 2 && dist >= next_pole {
            next_pole += 42.0;
            let s = if kit.drive_side == Side::Right { 1.0 } else { -1.0 };
            props.push(StreetProp {
                kind: PropKind::UtilityPole(kit.utility_poles),
                pos: pts[i] + side[i] * s * (w / 2.0 + 2.6) + lift,
                facing: -side[i] * s,
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::{EnuFrame, Geodetic, tile::TileId};

    #[test]
    fn kits_differ_by_country() {
        assert_eq!(StreetKit::for_region(Region::NorthAmericaCity).center_line, CenterLine::DoubleYellow);
        assert_eq!(StreetKit::for_region(Region::WesternEurope).center_line, CenterLine::DashedWhite);
        assert_eq!(StreetKit::for_region(Region::Japan).drive_side, Side::Left);
        assert_eq!(StreetKit::for_region(Region::Japan).signals, SignalLayout::Horizontal);
    }

    #[test]
    fn road_ribbon_faces_up() {
        let tile = TileId::containing(40.75, -73.98, 14);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(40.75, -73.98, 0.0)));
        let grid = TerrainGrid::flat(tile, 8, 10.0);
        let mut props = std::collections::HashMap::new();
        props.insert("class".to_string(), mvt::Value::String("primary".into()));
        let vt = mvt::Tile {
            layers: vec![mvt::Layer {
                name: "transportation".into(),
                extent: 4096,
                features: vec![mvt::Feature { id: None, geom_type: GeomType::LineString, props, rings: vec![vec![[100.0, 2000.0], [4000.0, 2100.0]]] }],
            }],
        };
        let surface = SurfaceMap::uniform(tile, crate::biome::Biome::TemperateForest);
        let pl = plan(&vt, &proj, &grid, &surface);
        let out = generate(&pl, &proj, &grid, &surface, Region::NorthAmericaCity);
        assert!(!out.mesh.is_empty());
        let up = proj.up();
        let (mut total, mut upward) = (0, 0);
        for t in out.mesh.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(out.mesh.positions[i as usize]));
            let n = (b - a).cross(c - a);
            if n.length() > 1e-6 {
                total += 1;
                if n.normalize().dot(up) > 0.5 {
                    upward += 1;
                }
                // The face must agree with its vertex normals.
                let vn = t.iter().map(|&i| Vec3::from(out.mesh.normals[i as usize])).sum::<Vec3>();
                assert!(n.dot(vn) > 0.0, "tri winding disagrees with normals");
            }
        }
        assert!(upward * 10 > total * 6, "{upward}/{total} upward");
    }

    #[test]
    fn grading_puts_bed_under_centerline() {
        let tile = TileId::containing(40.75, -73.98, 14);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(40.75, -73.98, 0.0)));
        let mut grid = TerrainGrid::flat(tile, 32, 10.0);
        // A bumpy hillside across the tile.
        let n = grid.segments + 1;
        for j in 0..n {
            for i in 0..n {
                grid.heights[j * n + i] = 10.0 + i as f32 * 0.5 + ((i * 7 + j * 3) % 5) as f32 * 0.4;
            }
        }
        let mut props = std::collections::HashMap::new();
        props.insert("class".to_string(), mvt::Value::String("primary".into()));
        let vt = mvt::Tile {
            layers: vec![mvt::Layer {
                name: "transportation".into(),
                extent: 4096,
                features: vec![mvt::Feature { id: None, geom_type: GeomType::LineString, props, rings: vec![vec![[200.0, 2000.0], [3900.0, 2200.0]]] }],
            }],
        };
        let surface = SurfaceMap::uniform(tile, crate::biome::Biome::TemperateForest);
        let pl = plan(&vt, &proj, &grid, &surface);
        grade(&mut grid, &pl);
        let it = &pl.items[0];
        // Mid-line samples sit within a few cm of the bed (grid cells are linear between vertices).
        for k in 3..it.uv.len() - 3 {
            let g = grid.sample(it.uv[k].0, it.uv[k].1);
            assert!((g - it.bed[k]).abs() < 0.35, "ground {g} vs bed {} at {k}", it.bed[k]);
        }
        // The road surface must not sink into (or hover over) the terrain that
        // renders under it, even across the slope.
        let out = generate(&pl, &proj, &grid, &surface, Region::NorthAmericaCity);
        let up = proj.up();
        let uvm = proj.uv_map();
        let (mut worst_low, mut worst_high) = (0.0f32, 0.0f32);
        for t in out.mesh.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(out.mesh.positions[i as usize]));
            if (b - a).cross(c - a).normalize_or_zero().dot(up) < 0.99 {
                continue;
            }
            let cen = (a + b + c) / 3.0;
            let [u, v] = uvm.uv(cen);
            let rel = cen.dot(up) - proj.local(u as f64, v as f64, grid.sample(u as f64, v as f64) as f64).dot(up);
            worst_low = worst_low.min(rel);
            worst_high = worst_high.max(rel);
        }
        assert!(worst_low > -0.1, "road sinks {worst_low} m into terrain");
        assert!(worst_high < 0.5, "road hovers {worst_high} m over terrain");
    }
}

#[cfg(test)]
mod deck_tests {
    use super::*;

    #[test]
    fn straight_flat_run_collapses_but_keeps_span_cap() {
        let pts: Vec<Vec3> = (0..=100).map(|i| Vec3::new(i as f32 * 3.0, 0.0, 0.0)).collect();
        let s = simplify_deck(&pts, DECK_TOL_M, DECK_MAX_SPAN_M);
        assert!(s.len() < 40 && s.len() >= 300 / 12, "len {}", s.len());
        assert_eq!(s[0], pts[0]);
        assert_eq!(*s.last().unwrap(), pts[100]);
        for w in s.windows(2) {
            assert!(w[0].distance(w[1]) <= DECK_MAX_SPAN_M + 3.0);
        }
    }

    #[test]
    fn bend_and_slope_change_are_kept() {
        let mut pts: Vec<Vec3> = (0..=10).map(|i| Vec3::new(i as f32 * 3.0, 0.0, 0.0)).collect();
        pts.extend((1..=10).map(|i| Vec3::new(30.0, 0.0, i as f32 * 3.0)));
        let s = simplify_deck(&pts, DECK_TOL_M, DECK_MAX_SPAN_M);
        assert!(s.contains(&Vec3::new(30.0, 0.0, 0.0)));
        let mut hill: Vec<Vec3> = (0..=10).map(|i| Vec3::new(i as f32 * 3.0, 0.0, 0.0)).collect();
        hill.extend((1..=10).map(|i| Vec3::new(30.0 + i as f32 * 3.0, i as f32 * 0.6, 0.0)));
        assert!(simplify_deck(&hill, DECK_TOL_M, DECK_MAX_SPAN_M).contains(&Vec3::new(30.0, 0.0, 0.0)));
    }
}
