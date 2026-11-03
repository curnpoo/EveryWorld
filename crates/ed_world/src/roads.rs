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
            Track => 3.0,
            Path => 1.8,
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

const STEP_M: f32 = 6.0;

pub fn generate(vt: &mvt::Tile, proj: &TileProjector, grid: &TerrainGrid, surface: &SurfaceMap, region: Region) -> RoadOutput {
    let mut out = RoadOutput::default();
    let Some(layer) = vt.layer("transportation") else { return out };
    let kit = StreetKit::for_region(region);
    let ext = layer.extent as f64;
    let up = proj.up();
    // Count how many car roads touch each vertex to find intersections.
    let mut junctions: HashMap<(i32, i32), (u8, u8)> = HashMap::new();

    // Draw lower classes first so bigger roads overlap them cleanly.
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
            let pts = densify(line, ext, proj, grid);
            if pts.len() < 2 {
                continue;
            }
            let lift = 0.12 + class.rank() as f32 * 0.01;
            let urban = surface.cover_at(line[0][0] as f64 / ext, line[0][1] as f64 / ext).is_urban();
            let base_color = match class {
                RoadClass::Path => rgb(if urban { 0xc9c3b8 } else { 0xcdb998 }),
                RoadClass::Track => rgb(0xa88e6a),
                RoadClass::Rail => rgb(0x6b5d52),
                _ if !paved => rgb(0xa88e6a),
                _ => rgb(kit.asphalt),
            };
            let w = class.width();
            ribbon(&mut out.mesh, &pts, up, -w / 2.0, w / 2.0, lift, base_color, None);
            if class.is_car_road() && urban && class.rank() >= 1 {
                let sw = rgb(kit.sidewalk);
                ribbon(&mut out.mesh, &pts, up, -w / 2.0 - 2.2, -w / 2.0, lift + kit.curb_height, sw, None);
                ribbon(&mut out.mesh, &pts, up, w / 2.0, w / 2.0 + 2.2, lift + kit.curb_height, sw, None);
            }
            if class.is_car_road() && paved && class.rank() >= 1 {
                markings(&mut out.mesh, &pts, up, class, &kit, oneway, lift + 0.02);
            }
            if matches!(class, RoadClass::Motorway | RoadClass::Trunk) && !urban {
                guardrail(&mut out.mesh, &pts, up, w / 2.0 + 0.6, lift);
                guardrail(&mut out.mesh, &pts, up, -w / 2.0 - 0.6, lift);
            }
            street_props(&mut out.props, &pts, up, class, &kit, urban);
            out.centerlines.push((class, pts));
        }
    }

    // Signals at urban junctions between real roads; stop signs at minor ones.
    for ((x, y), (count, rank)) in junctions {
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
        out.props.push(StreetProp { kind, pos: p + off, facing: -off.normalize_or(Vec3::X) });
    }
    out
}

/// Polyline in tile px -> draped local points every ~STEP_M meters.
fn densify(line: &[[f32; 2]], ext: f64, proj: &TileProjector, grid: &TerrainGrid) -> Vec<Vec3> {
    let mut out = Vec::new();
    let at = |x: f64, y: f64| {
        let (u, v) = (x / ext, y / ext);
        proj.local(u, v, grid.sample(u, v) as f64)
    };
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let pa = at(a[0] as f64, a[1] as f64);
        let pb = at(b[0] as f64, b[1] as f64);
        let n = ((pb - pa).length() / STEP_M).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = k as f64 / n as f64;
            let p = at(a[0] as f64 + (b[0] - a[0]) as f64 * t, a[1] as f64 + (b[1] - a[1]) as f64 * t);
            if out.last().is_none_or(|q: &Vec3| q.distance(p) > 0.01) {
                out.push(p);
            }
        }
    }
    let l = line[line.len() - 1];
    out.push(at(l[0] as f64, l[1] as f64));
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

/// A strip from offset `o0` to `o1` (meters right of center), `lift` above
/// ground. `dash` = Some((on, off)) for dashed lines.
#[allow(clippy::too_many_arguments)]
fn ribbon(m: &mut MeshData, pts: &[Vec3], up: Vec3, o0: f32, o1: f32, lift: f32, color: [f32; 4], dash: Option<(f32, f32)>) {
    let side = side_dirs(pts, up);
    let mut dist = 0.0f32;
    let mut prev: Option<(u32, u32)> = None;
    for i in 0..pts.len() {
        if i > 0 {
            dist += pts[i].distance(pts[i - 1]);
        }
        let visible = match dash {
            Some((on, off)) => (dist % (on + off)) < on,
            None => true,
        };
        if !visible {
            prev = None;
            continue;
        }
        let base = pts[i] + up * lift;
        let l = m.vertex((base + side[i] * o0).to_array(), up.to_array(), [o0, dist], color);
        let r = m.vertex((base + side[i] * o1).to_array(), up.to_array(), [o1, dist], color);
        if let Some((pl, pr)) = prev {
            // CCW seen from above: (pl, l, r), (pl, r, pr).
            m.tri(pl, pr, r);
            m.tri(pl, r, l);
        }
        prev = Some((l, r));
    }
}

fn markings(m: &mut MeshData, pts: &[Vec3], up: Vec3, class: RoadClass, kit: &StreetKit, oneway: bool, lift: f32) {
    let white = rgb(0xf2f2ee);
    let yellow = rgb(0xf2c230);
    let w = class.width();
    if !oneway {
        match kit.center_line {
            CenterLine::DoubleYellow => {
                ribbon(m, pts, up, -0.22, -0.08, lift, yellow, None);
                ribbon(m, pts, up, 0.08, 0.22, lift, yellow, None);
            }
            CenterLine::SolidWhite if class.rank() >= 3 => ribbon(m, pts, up, -0.08, 0.08, lift, white, None),
            _ => ribbon(m, pts, up, -0.07, 0.07, lift, white, Some((3.0, 6.0))),
        }
    } else if class.rank() >= 3 {
        ribbon(m, pts, up, -0.07, 0.07, lift, white, Some((3.0, 9.0)));
    }
    if class.rank() >= 3 {
        ribbon(m, pts, up, w / 2.0 - 0.45, w / 2.0 - 0.3, lift, white, None);
        ribbon(m, pts, up, -w / 2.0 + 0.3, -w / 2.0 + 0.45, lift, white, None);
    }
}

fn guardrail(m: &mut MeshData, pts: &[Vec3], up: Vec3, offset: f32, lift: f32) {
    let steel = rgb(0xb8bcc2);
    let side = side_dirs(pts, up);
    // Rail: vertical strip from 0.45 to 0.8 m, two-sided.
    for (i, w) in pts.windows(2).enumerate() {
        let a = w[0] + side[i] * offset + up * lift;
        let b = w[1] + side[i + 1] * offset + up * lift;
        let q = [a + up * 0.45, b + up * 0.45, b + up * 0.8, a + up * 0.8];
        m.quad(q, [[0.0; 2]; 4], steel);
        m.quad([q[1], q[0], q[3], q[2]], [[0.0; 2]; 4], steel);
    }
}

fn street_props(props: &mut Vec<StreetProp>, pts: &[Vec3], up: Vec3, class: RoadClass, kit: &StreetKit, urban: bool) {
    if !class.is_car_road() {
        return;
    }
    let side = side_dirs(pts, up);
    let w = class.width();
    let mut dist = 0.0;
    let mut next_light = 15.0;
    let mut next_pole = 20.0;
    let mut flip = false;
    for i in 1..pts.len() {
        dist += pts[i].distance(pts[i - 1]);
        if urban && class.rank() >= 1 && dist >= next_light {
            next_light += 32.0;
            flip = !flip;
            let s = if flip { 1.0 } else { -1.0 };
            props.push(StreetProp { kind: PropKind::StreetLight, pos: pts[i] + side[i] * s * (w / 2.0 + 1.0), facing: -side[i] * s });
        }
        if kit.utility_poles != PoleKind::None && class.rank() <= 2 && dist >= next_pole {
            next_pole += 42.0;
            let s = if kit.drive_side == Side::Right { 1.0 } else { -1.0 };
            props.push(StreetProp {
                kind: PropKind::UtilityPole(kit.utility_poles),
                pos: pts[i] + side[i] * s * (w / 2.0 + 2.6),
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
        let out = generate(&vt, &proj, &grid, &surface, Region::NorthAmericaCity);
        assert!(!out.mesh.is_empty());
        let up = proj.up();
        for t in out.mesh.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(out.mesh.positions[i as usize]));
            let n = (b - a).cross(c - a);
            if n.length() > 1e-4 {
                assert!(n.normalize().dot(up) > 0.9, "road tri faces {n:?}");
            }
        }
    }
}
