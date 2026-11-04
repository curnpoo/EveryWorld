//! Procedural geometry for rocks (baked per chunk) and street furniture
//! (library assets placed per tile, see [`crate::library`]). Trees and shrubs live in [`crate::trees`] (see
//! [`crate::trees::bake_tile`]).

use crate::foliage::RockInstance;
use crate::library::{self, AssetDef, ColliderShape, Library, Placement, RenderPath};
use crate::mesh::{MeshData, rgb, scale_color};
use crate::roads::{PoleKind, PropKind, SignalLayout, StreetProp};
use glam::{Quat, Vec3};
use std::f32::consts::TAU;

fn cylinder(m: &mut MeshData, base: Vec3, r0: f32, r1: f32, h: f32, seg: usize, c: [f32; 4]) {
    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * TAU;
        let a1 = (i + 1) as f32 / seg as f32 * TAU;
        let d0 = Vec3::new(a0.cos(), 0.0, a0.sin());
        let d1 = Vec3::new(a1.cos(), 0.0, a1.sin());
        let p = [base + d0 * r0, base + d1 * r0, base + d1 * r1 + Vec3::Y * h, base + d0 * r1 + Vec3::Y * h];
        // Outward-facing: order so normal points along d.
        m.quad([p[1], p[0], p[3], p[2]], [[0.0; 2]; 4], c);
    }
    if r1 > 0.001 {
        // Cap the top.
        let top = base + Vec3::Y * h;
        for i in 0..seg {
            let a0 = i as f32 / seg as f32 * TAU;
            let a1 = (i + 1) as f32 / seg as f32 * TAU;
            m.triangle([top, top + Vec3::new(a1.cos(), 0.0, a1.sin()) * r1, top + Vec3::new(a0.cos(), 0.0, a0.sin()) * r1], [[0.0; 2]; 3], c);
        }
    }
}

/// Smooth-shaded low-poly ellipsoid (UV sphere, few segments).
fn blob(m: &mut MeshData, center: Vec3, radii: Vec3, c: [f32; 4]) {
    let (rings, seg) = (5, 8);
    let base = m.positions.len() as u32;
    for j in 0..=rings {
        let phi = j as f32 / rings as f32 * std::f32::consts::PI;
        for i in 0..=seg {
            let th = i as f32 / seg as f32 * TAU;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            // Darker underside = cheap ambient occlusion.
            let shade = 0.72 + 0.28 * (n.y * 0.5 + 0.5);
            m.vertex((center + n * radii).to_array(), (n / radii).normalize().to_array(), [0.0, 0.0], scale_color(c, shade));
        }
    }
    let w = seg as u32 + 1;
    for j in 0..rings as u32 {
        for i in 0..seg as u32 {
            let a = base + j * w + i;
            let b = a + w;
            m.tri(a, a + 1, b);
            m.tri(a + 1, b + 1, b);
        }
    }
}

fn rock_mesh(c: [f32; 4]) -> MeshData {
    // Squashed, faceted blob reads as "rock".
    let mut m = MeshData::default();
    let (rings, seg) = (3, 6);
    let base = m.positions.len() as u32;
    for j in 0..=rings {
        let phi = j as f32 / rings as f32 * std::f32::consts::PI;
        for i in 0..=seg {
            let th = i as f32 / seg as f32 * TAU;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            let jitter = 0.85 + 0.3 * crate::hash01(i as u64 % seg as u64, j as u64);
            m.vertex((n * jitter).to_array(), n.to_array(), [0.0; 2], c);
        }
    }
    let w = seg as u32 + 1;
    for j in 0..rings as u32 {
        for i in 0..seg as u32 {
            let a = base + j * w + i;
            let b = a + w;
            m.tri(a, a + 1, b);
            m.tri(a + 1, b + 1, b);
        }
    }
    m
}

/// Rotate a model so its +Y aligns with `up`, spin about it, scale, place.
fn place(dst: &mut MeshData, src: &MeshData, pos: Vec3, up: Vec3, yaw: f32, scale: Vec3, tint: Option<[f32; 4]>) {
    let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(yaw);
    let base = dst.positions.len() as u32;
    for i in 0..src.positions.len() {
        let p = rot * (Vec3::from(src.positions[i]) * scale) + pos;
        let n = (rot * (Vec3::from(src.normals[i]) / scale)).normalize_or_zero();
        let c = match tint {
            Some(t) => [src.colors[i][0] * t[0], src.colors[i][1] * t[1], src.colors[i][2] * t[2], 1.0],
            None => src.colors[i],
        };
        dst.vertex(p.to_array(), n.to_array(), src.uvs[i], c);
    }
    dst.indices.extend(src.indices.iter().map(|i| i + base));
}

pub fn bake_rocks(rocks: &[RockInstance], up: Vec3) -> MeshData {
    let model = rock_mesh([1.0; 4]);
    let mut out = MeshData::default();
    for r in rocks {
        place(&mut out, &model, r.pos, up, r.yaw, r.scale, Some(r.tint));
    }
    out
}

/// Rocks bucketed into a `grid` x `grid` set of chunk meshes (tile mesh space),
/// so the renderer can frustum-cull and distance-cull them per chunk.
pub fn bake_rocks_chunked(rocks: &[RockInstance], up: Vec3, width: f32, grid: usize) -> Vec<MeshData> {
    let grid = grid.max(1);
    let model = rock_mesh([1.0; 4]);
    let mut cells: Vec<MeshData> = (0..grid * grid).map(|_| MeshData::default()).collect();
    let cell = (width / grid as f32).max(1.0);
    let half = width * 0.5;
    for r in rocks {
        let i = (((r.pos.x + half) / cell).floor() as i64).clamp(0, grid as i64 - 1) as usize;
        let j = (((r.pos.z + half) / cell).floor() as i64).clamp(0, grid as i64 - 1) as usize;
        place(&mut cells[j * grid + i], &model, r.pos, up, r.yaw, r.scale, Some(r.tint));
    }
    cells.retain(|m| !m.is_empty());
    for m in &mut cells {
        m.palettize();
    }
    cells
}

fn street_prop_mesh(kind: PropKind) -> MeshData {
    let mut m = MeshData::default();
    let grey = rgb(0x5a5f66);
    match kind {
        PropKind::StreetLight => {
            cylinder(&mut m, Vec3::ZERO, 0.12, 0.08, 7.5, 6, grey);
            // Arm out toward the road (+Z in model space) and a lamp head.
            m.quad([Vec3::new(-0.05, 7.4, 0.0), Vec3::new(0.05, 7.4, 0.0), Vec3::new(0.05, 7.5, 2.0), Vec3::new(-0.05, 7.5, 2.0)], [[0.0; 2]; 4], grey);
            blob(&mut m, Vec3::new(0.0, 7.35, 2.0), Vec3::new(0.25, 0.12, 0.45), rgb(0xfff2c6));
        }
        PropKind::UtilityPole(kind) => {
            let c = if kind == PoleKind::Wood { rgb(0x6e5440) } else { rgb(0xb5b2aa) };
            cylinder(&mut m, Vec3::ZERO, 0.16, 0.12, 10.0, 6, c);
            m.quad([Vec3::new(-1.2, 9.2, -0.06), Vec3::new(1.2, 9.2, -0.06), Vec3::new(1.2, 9.4, -0.06), Vec3::new(-1.2, 9.4, -0.06)], [[0.0; 2]; 4], c);
            m.quad([Vec3::new(1.2, 9.2, 0.06), Vec3::new(-1.2, 9.2, 0.06), Vec3::new(-1.2, 9.4, 0.06), Vec3::new(1.2, 9.4, 0.06)], [[0.0; 2]; 4], c);
            if kind == PoleKind::Wood {
                cylinder(&mut m, Vec3::new(0.0, 7.8, 0.3), 0.3, 0.3, 0.9, 6, rgb(0x777b80)); // transformer can
            }
        }
        PropKind::TrafficSignal(layout) => {
            let pole = rgb(0x3d4147);
            cylinder(&mut m, Vec3::ZERO, 0.12, 0.1, 5.5, 6, pole);
            let housing = rgb(0x2a2c30);
            let lights = [rgb(0xff3b30), rgb(0xffcc00), rgb(0x34c759)];
            for (k, c) in lights.iter().enumerate() {
                let off = match layout {
                    SignalLayout::Vertical => Vec3::new(0.0, 5.2 - k as f32 * 0.4, 0.25),
                    SignalLayout::Horizontal => Vec3::new(-0.4 + k as f32 * 0.4, 5.0, 0.25),
                };
                blob(&mut m, off - Vec3::Z * 0.1, Vec3::splat(0.22), housing);
                blob(&mut m, off + Vec3::Z * 0.06, Vec3::splat(0.13), *c);
            }
        }
        PropKind::StopSign => {
            cylinder(&mut m, Vec3::ZERO, 0.04, 0.04, 2.2, 5, grey);
            let red = rgb(0xc8102e);
            let c = Vec3::new(0.0, 2.3, 0.05);
            for i in 0..8 {
                let a0 = (i as f32 + 0.5) / 8.0 * TAU;
                let a1 = (i as f32 + 1.5) / 8.0 * TAU;
                let p0 = c + Vec3::new(a0.cos(), a0.sin(), 0.0) * 0.38;
                let p1 = c + Vec3::new(a1.cos(), a1.sin(), 0.0) * 0.38;
                m.triangle([c, p0, p1], [[0.0; 2]; 3], red);
                m.triangle([c, p1, p0], [[0.0; 2]; 3], scale_color(red, 0.6));
            }
        }
    }
    m
}

/// Every street prop variant with its library key. Keys are the stable names
/// rules and content packs use; the order here is irrelevant.
const STREET_PROPS: [(PropKind, &str, f32, f32); 7] = [
    (PropKind::StreetLight, "prop/street_light", 0.12, 7.5),
    (PropKind::UtilityPole(PoleKind::Wood), "prop/utility_pole_wood", 0.16, 10.0),
    (PropKind::UtilityPole(PoleKind::Concrete), "prop/utility_pole_concrete", 0.16, 10.0),
    (PropKind::UtilityPole(PoleKind::None), "prop/utility_pole", 0.16, 10.0),
    (PropKind::TrafficSignal(SignalLayout::Vertical), "prop/traffic_signal_vertical", 0.12, 5.5),
    (PropKind::TrafficSignal(SignalLayout::Horizontal), "prop/traffic_signal_horizontal", 0.12, 5.5),
    (PropKind::StopSign, "prop/stop_sign", 0.05, 2.2),
];

pub fn street_prop_key(kind: PropKind) -> &'static str {
    STREET_PROPS.iter().find(|(k, ..)| *k == kind).map_or("prop/street_light", |(_, key, ..)| key)
}

pub(crate) fn register_street_props(lib: &mut Library) {
    for (kind, key, radius, height) in STREET_PROPS {
        lib.register(AssetDef {
            key: key.into(),
            render: RenderPath::Meshlet,
            collider: ColliderShape::Cylinder { radius, height },
            build: std::sync::Arc::new(move || street_prop_mesh(kind)),
        });
    }
}

/// Street props as library placements (one instance of a shared model each).
pub fn place_street_props(lib: &Library, props: &[StreetProp], up: Vec3) -> Vec<Placement> {
    props
        .iter()
        .filter_map(|p| lib.id(street_prop_key(p.kind)).map(|id| library::upright(id, p.pos, up, p.facing)))
        .collect()
}

pub fn bake_street_props(props: &[StreetProp], up: Vec3) -> MeshData {
    use std::collections::HashMap;
    let mut cache: HashMap<PropKind, MeshData> = HashMap::new();
    let mut out = MeshData::default();
    for p in props {
        let model = cache.entry(p.kind).or_insert_with(|| street_prop_mesh(p.kind));
        // Model faces +Z; rotate so +Z points along `facing` (projected flat).
        let f = p.facing.reject_from_normalized(up).normalize_or(Vec3::Z);
        let rot_up = Quat::from_rotation_arc(Vec3::Y, up);
        let local_f = rot_up.inverse() * f;
        let yaw = local_f.x.atan2(local_f.z);
        place(&mut out, model, p.pos, up, yaw, Vec3::ONE, None);
    }
    out
}
