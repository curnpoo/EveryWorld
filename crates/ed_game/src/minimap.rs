//! In-game circular minimap, bottom-right of the 3D view.
//!
//! The map mosaic is composited **on the CPU** into one canvas texture whose
//! pixels already contain the circular alpha mask and the camera-heading
//! rotation: minimap tiles (from `ed_world::maprender`) are sampled through the
//! inverse rotation, so no Bevy UI clip and no rotated node group is involved.
//! A dot marks the camera position, a needle shows the way forward, an "N"
//! rides the rim, and remote players appear as blue dots.

use crate::config::Config;
use crate::menu::GameState;
use crate::net::RemotePlayers;
use crate::origin::WorldOrigin;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::AsyncComputeTaskPool;
use bevy::text::FontSize;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::tile::TileId;
use ed_world::maprender;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::sync::Arc;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        app.insert_resource(MinimapState::new(tx, rx))
            .insert_resource(MinimapRemoteDots::default())
            .add_systems(Startup, spawn_minimap)
            .add_systems(
                Update,
                (
                    update_minimap,
                    receive_tiles,
                    paint_minimap,
                    position_north,
                    minimap_visibility,
                    update_minimap_remote_dots,
                )
                    .chain(),
            );
    }
}

pub const SIZE_PX: f32 = 220.0;
/// Canvas pixels per CSS pixel.
const RES: u32 = 2;
/// Displayed size of one Web-Mercator tile. Matches `maprender::SIZE`, so the
/// minimap is at native texel density.
const TILE_PX: f32 = maprender::SIZE as f32;
/// Slippy zoom the minimap rests at (native tile density) and its limits.
const MAX_VIEW_ZOOM: f32 = 16.6;
const MIN_VIEW_ZOOM: f32 = 11.0;
const MIN_TILE_Z: u8 = 11;
const MAX_TILE_Z: u8 = 16;

/// Target view zoom for a ground speed (m/s): faster -> further out, slower -> closer in.
fn zoom_for_speed(speed: f32) -> f32 {
    (MAX_VIEW_ZOOM - 0.9 * (1.0 + speed.max(0.0) / 4.0).log2()).clamp(MIN_VIEW_ZOOM, MAX_VIEW_ZOOM)
}

/// Integer tile level used to draw a (continuous) view zoom.
fn tile_z(view_zoom: f32) -> u8 {
    (view_zoom.round() as i32).clamp(MIN_TILE_Z as i32, MAX_TILE_Z as i32) as u8
}

/// On-screen size of one tile at this view zoom.
fn tile_px(view_zoom: f32) -> f32 {
    TILE_PX * (view_zoom - tile_z(view_zoom) as f32).exp2()
}
const MAX_IN_FLIGHT: usize = 6;
const MAX_CACHED: usize = 96;

#[derive(Component)]
struct MinimapRoot;

#[derive(Component)]
struct MinimapCanvas;

#[derive(Component)]
struct MinimapNorth;

#[derive(Component)]
struct MinimapRemoteDot {
    id: u64,
}

#[derive(Resource, Default)]
struct MinimapRemoteDots {
    map: HashMap<u64, Entity>,
}

#[derive(Resource)]
struct MinimapState {
    /// Camera position in Web-Mercator world coordinates (0..1).
    center: (f64, f64),
    /// Compass heading of the camera, degrees clockwise from north.
    heading: f32,
    /// Continuous slippy zoom, eased toward `zoom_for_speed`.
    view_zoom: f32,
    speed: f32,
    prev: Option<ed_geo::Geodetic>,
    tiles: HashMap<TileId, Arc<Vec<u8>>>,
    pending: HashSet<TileId>,
    tx: Sender<maprender::MapTile>,
    rx: Receiver<maprender::MapTile>,
    template: Option<String>,
    template_tried: bool,
    paint: Option<PaintKey>,
}

impl MinimapState {
    fn new(tx: Sender<maprender::MapTile>, rx: Receiver<maprender::MapTile>) -> Self {
        Self {
            center: (0.5, 0.42),
            heading: 0.0,
            view_zoom: MAX_VIEW_ZOOM,
            speed: 0.0,
            prev: None,
            tiles: HashMap::new(),
            pending: HashSet::new(),
            tx,
            rx,
            template: None,
            template_tried: false,
            paint: None,
        }
    }
}

/// Parameters whose change requires a recomposite.
#[derive(Clone, Copy, PartialEq, Debug)]
struct PaintKey {
    heading: f32,
    center: (f64, f64),
    view_zoom: f32,
    n_tiles: u64,
}

fn paint_key_near(a: &PaintKey, b: &PaintKey) -> bool {
    let dh = (a.heading - b.heading).rem_euclid(360.0).abs();
    let scale = a.view_zoom.exp2().max(1.0) as f64 * 256.0;
    let dcx = (a.center.0 - b.center.0).abs() * scale / SIZE_PX as f64;
    let dcy = (a.center.1 - b.center.1).abs() * scale / SIZE_PX as f64;
    dh < 0.6
        && dcx < 1.0
        && dcy < 1.0
        && (a.view_zoom - b.view_zoom).abs() < 0.005
        && a.n_tiles == b.n_tiles
}

fn to_world(g: ed_geo::Geodetic) -> (f64, f64) {
    let lat = g.lat_deg.clamp(-85.0511, 85.0511).to_radians();
    (
        (g.lon_deg + 180.0) / 360.0,
        (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0,
    )
}

fn spawn_minimap(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let n = SIZE_PX as u32 * RES;
    let handle = images.add(Image::new_fill(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    commands
        .spawn((
            MinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(18.0),
                bottom: Val::Px(18.0),
                width: Val::Px(SIZE_PX),
                height: Val::Px(SIZE_PX),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(Color::srgba(0.88, 0.92, 0.96, 0.55)),
            Visibility::Hidden,
            ZIndex(5),
        ))
        .with_children(|p| {
            // The composited, circular, heading-rotated map. Its own pixels
            // carry the alpha mask, so nothing has to clip.
            p.spawn((
                MinimapCanvas,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    image: handle,
                    ..default()
                },
                ZIndex(0),
            ));
            // Player dot, always at the centre.
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                UiTransform::from_xy(Val::Px(-6.0), Val::Px(-6.0)),
                BackgroundColor(Color::srgb(0.98, 0.42, 0.1)),
                BorderColor::all(Color::WHITE),
                ZIndex(3),
            ));
            // Forward needle: the canvas is rotated, so "up" is always forward.
            p.spawn((
                Text::new("▲"),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::new(0.0, 1.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    ..default()
                },
                UiTransform::from_xy(Val::Px(-6.0), Val::Px(-24.0)),
                ZIndex(4),
            ));
            // A north marker on the rim, positioned by the heading.
            p.spawn((
                MinimapNorth,
                Text::new("N"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
                TextShadow {
                    offset: Vec2::new(0.0, 1.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                },
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ZIndex(4),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
fn update_minimap(
    cfg: Res<Config>,
    origin: Res<WorldOrigin>,
    time: Res<Time>,
    mut st: ResMut<MinimapState>,
    cams: Query<&Transform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    roots: Query<(&ComputedNode, &InheritedVisibility), With<MinimapRoot>>,
) {
    let Ok((node, vis)) = roots.single() else {
        return;
    };
    if !vis.get() {
        return;
    }
    let Ok(cam) = cams.single() else { return };
    let g = origin.to_geodetic(cam.translation);
    st.center = to_world(g);
    let dt = time.delta_secs().max(1e-3);
    if let Some(prev) = st.prev {
        let inst = (g.distance_to(prev) as f32 / dt).min(2000.0);
        // Teleports (spawn, map jump) are not speed.
        let inst = if inst > 1500.0 { 0.0 } else { inst };
        let k = 1.0 - (-dt * 1.5).exp();
        st.speed += (inst - st.speed) * k;
    }
    st.prev = Some(g);
    let goal = zoom_for_speed(st.speed);
    let k = 1.0 - (-dt * 2.2).exp();
    st.view_zoom += (goal - st.view_zoom) * k;
    let up = origin.up_at(cam.translation);
    let fwd = cam.forward().as_vec3().reject_from_normalized(up);
    if fwd.length_squared() > 1e-6 {
        let enu = ed_geo::render_to_enu(fwd.normalize().as_dvec3());
        st.heading = (enu.x.atan2(enu.y).to_degrees() as f32).rem_euclid(360.0);
    }

    if !st.template_tried {
        st.template_tried = true;
        st.template = ed_data::vector::tile_template(&cfg.cache).ok();
    }

    let size = canvas_size(node);
    let want = visible(&st, size);
    if st.pending.len() >= MAX_IN_FLIGHT {
        return;
    }
    for (id, ..) in want {
        if st.pending.len() >= MAX_IN_FLIGHT {
            break;
        }
        if st.tiles.contains_key(&id) || st.pending.contains(&id) {
            continue;
        }
        st.pending.insert(id);
        let (tx, cache, tpl) = (st.tx.clone(), cfg.cache.clone(), st.template.clone());
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let _ = tx.send(maprender::fetch_and_render(&cache, tpl.as_deref(), id));
            })
            .detach();
    }
}

fn receive_tiles(mut st: ResMut<MinimapState>) {
    let mut changed = false;
    for _ in 0..8 {
        let Ok(t) = st.rx.try_recv() else { break };
        st.pending.remove(&t.id);
        st.tiles.insert(t.id, Arc::new(t.rgba));
        changed = true;
    }
    if changed {
        st.paint = None;
    }
    prune_cache(&mut st);
}

fn prune_cache(st: &mut MinimapState) {
    if st.tiles.len() <= MAX_CACHED {
        return;
    }
    let n = 1u32 << tile_z(st.view_zoom);
    let (cx, cy) = (st.center.0 * n as f64, st.center.1 * n as f64);
    let mut keys: Vec<TileId> = st.tiles.keys().copied().collect();
    keys.sort_by(|a, b| {
        let d = |id: &TileId| {
            let dx = (id.x as f64 + 0.5 - cx).abs();
            let dy = (id.y as f64 + 0.5 - cy).abs();
            dx * dx + dy * dy
        };
        d(b).total_cmp(&d(a))
    });
    for k in keys.into_iter().take(st.tiles.len() - MAX_CACHED) {
        st.tiles.remove(&k);
    }
    st.paint = None;
}

fn canvas_size(node: &ComputedNode) -> Vec2 {
    node.size() * node.inverse_scale_factor()
}

/// Tile ids covering the canvas, with their top-left CSS pixels relative to
/// the canvas centre, in the unrotated north-up frame.
fn visible(st: &MinimapState, size: Vec2) -> Vec<(TileId, f32, f32)> {
    let tz = tile_z(st.view_zoom);
    let tpx = tile_px(st.view_zoom);
    let n = 1u32 << tz;
    let cx = st.center.0 * n as f64;
    let cy = st.center.1 * n as f64;
    let half = (size.length() as f64 * 0.5 / tpx as f64).ceil() as i64 + 1;
    let mut out = Vec::new();
    for ty in (cy.floor() as i64 - half)..=(cy.floor() as i64 + half) {
        if ty < 0 || ty >= n as i64 {
            continue;
        }
        for tx in (cx.floor() as i64 - half)..=(cx.floor() as i64 + half) {
            let id = TileId::new(tz, tx.rem_euclid(n as i64) as u32, ty as u32);
            let left = ((tx as f64 - cx) * tpx as f64) as f32 + size.x / 2.0;
            let top = ((ty as f64 - cy) * tpx as f64) as f32 + size.y / 2.0;
            out.push((id, left, top));
        }
    }
    out
}

/// Nearest tile texture for `id`: exact, else the closest available ancestor.
/// Returns the texture and how many levels it was promoted, so the sampler can
/// locate the sub-rect the child tile occupies within the coarser image.
fn best_tex(st: &HashMap<TileId, Arc<Vec<u8>>>, id: TileId) -> Option<(Arc<Vec<u8>>, u32)> {
    if let Some(t) = st.get(&id) {
        return Some((t.clone(), 0));
    }
    let mut cur = id;
    for k in 1..=6u32 {
        let p = cur.parent()?;
        if let Some(t) = st.get(&p) {
            return Some((t.clone(), k));
        }
        cur = p;
    }
    None
}

/// Bilinear RGBA sample from a `maprender::SIZE` square texture, coordinates in
/// texture pixels, clamped at the border.
fn sample_tex(tex: &[u8], mut x: f32, mut y: f32) -> [f32; 4] {
    let s = maprender::SIZE as f32 - 1.0;
    x = x.clamp(0.0, s);
    y = y.clamp(0.0, s);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let (x1, y1) = (
        usize::min(x0 + 1, maprender::SIZE - 1),
        usize::min(y0 + 1, maprender::SIZE - 1),
    );
    let px = |xx: usize, yy: usize| -> [f32; 4] {
        let at = (yy * maprender::SIZE + xx) * 4;
        [
            tex[at] as f32,
            tex[at + 1] as f32,
            tex[at + 2] as f32,
            tex[at + 3] as f32,
        ]
    };
    let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
    let w = |t: f32| 1.0 - t;
    std::array::from_fn(|i| {
        (a[i] * w(fx) * w(fy) + b[i] * fx * w(fy) + c[i] * w(fx) * fy + d[i] * fx * fy)
            .clamp(0.0, 255.0)
    })
}

/// One composite sample from continuous Web-Mercator tile-space coordinates
/// (units: tiles at zoom `tz`; `+x` east, `+y` south). Transparent outside
/// the pyramid.
fn sample_map(tiles: &HashMap<TileId, Arc<Vec<u8>>>, tz: u8, fx: f64, fy: f64) -> [f32; 4] {
    let n = 1u32 << tz;
    if fy < 0.0 || fy >= n as f64 {
        return [0.0; 4];
    }
    let ix = fx.floor() as i64;
    let iy = fy.floor() as i64;
    let id = TileId::new(tz, ix.rem_euclid(n as i64) as u32, iy as u32);
    let (u, v) = ((fx - ix as f64) as f32, (fy - iy as f64) as f32);
    let Some((tex, k)) = best_tex(tiles, id) else {
        return [0.0; 4];
    };
    let span = maprender::SIZE as f32 / (1u32 << k) as f32;
    let m = (1u32 << k) - 1;
    let qx = (id.x & m) as f32 * span;
    let qy = (id.y & m) as f32 * span;
    sample_tex(&tex, qx + u * span, qy + v * span)
}

fn paint_minimap(
    mut st: ResMut<MinimapState>,
    roots: Query<&InheritedVisibility, With<MinimapRoot>>,
    mut canvases: Query<&mut ImageNode, With<MinimapCanvas>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(vis) = roots.single() else { return };
    if !vis.get() {
        st.paint = None;
        return;
    }
    let Ok(mut cn) = canvases.single_mut() else {
        return;
    };
    let key = PaintKey {
        heading: st.heading,
        center: st.center,
        view_zoom: st.view_zoom,
        n_tiles: st.tiles.len() as u64,
    };
    if let Some(prev) = st.paint {
        if paint_key_near(&prev, &key) {
            return;
        }
    }
    st.paint = Some(key);
    let Some(mut img) = images.get_mut(&cn.image) else {
        return;
    };
    let Some(data) = img.data.as_mut() else {
        return;
    };
    paint_bytes(&st.tiles, st.center, st.heading, st.view_zoom, data);
}

/// The core composite: fills `out` (RGBA8 sRGB, straight alpha) with the
/// circular, heading-rotated map. `center` is the camera's Web-Mercator
/// position; `view_zoom` the continuous slippy zoom; `heading` compass degrees
/// clockwise from north.
///
/// Picking rules (asserted in tests): a texel whose unrotated, north-up offset
/// from the centre is `o` (CSS px, x east, y down/south) appears at screen
/// offset `s = Rot(-h)·o`, so each canvas pixel samples
/// `o = Rot(h)·s` with `Rot(θ)·v = (cosθ·x − sinθ·y, sinθ·x + cosθ·y)`.
/// Facing east therefore puts the texel east of the camera at the top of the
/// disk and the texel north of it at the left edge.
fn paint_bytes(
    tiles: &HashMap<TileId, Arc<Vec<u8>>>,
    center: (f64, f64),
    heading: f32,
    view_zoom: f32,
    out: &mut Vec<u8>,
) {
    let n = SIZE_PX as u32 * RES;
    if out.len() != (n * n * 4) as usize {
        *out = vec![0; (n * n * 4) as usize];
    }
    let tz = tile_z(view_zoom);
    let px = tile_px(view_zoom);
    let cxu = center.0 * (1u32 << tz) as f64;
    let cyu = center.1 * (1u32 << tz) as f64;
    let hw = n as f32 * 0.5;
    let r_css = SIZE_PX * 0.5;
    let h = heading.rem_euclid(360.0).to_radians() as f64;
    let (ch, sh) = (h.cos() as f32, h.sin() as f32);
    let rows = n as usize;
    for py in 0..rows {
        let sy = (py as f32 + 0.5 - hw) / RES as f32;
        let dy2 = sy * sy;
        for cx in 0..rows {
            let sx = (cx as f32 + 0.5 - hw) / RES as f32;
            let edge = ((r_css - 0.75) - (sx * sx + dy2).sqrt()).clamp(0.0, 1.0);
            if edge <= 0.0 {
                continue;
            }
            // Offset in the unrotated, north-up CSS frame.
            let ox = ch * sx - sh * sy;
            let oy = sh * sx + ch * sy;
            let f_x = cxu + ox as f64 / px as f64;
            let f_y = cyu + oy as f64 / px as f64;
            let s = sample_map(tiles, tz, f_x, f_y);
            let a = s[3] / 255.0 * edge;
            let at = (py * rows + cx) * 4;
            out[at] = s[0] as u8;
            out[at + 1] = s[1] as u8;
            out[at + 2] = s[2] as u8;
            out[at + 3] = (a * 255.0).min(255.0) as u8;
        }
    }
}

/// The "N" rides the rim: heading 0 puts it at the top, 90 at the left edge
/// (north on your left when facing east), etc.
fn position_north(
    st: Res<MinimapState>,
    mut north: Query<&mut Node, (With<MinimapNorth>, Without<MinimapRoot>)>,
) {
    let Ok(mut n) = north.single_mut() else { return };
    let r = SIZE_PX * 0.5 - 14.0;
    let h = st.heading.to_radians();
    n.left = Val::Px(SIZE_PX * 0.5 - h.sin() * r - 5.0);
    n.top = Val::Px(SIZE_PX * 0.5 - h.cos() * r - 6.0);
}

fn minimap_visibility(
    cfg: Res<crate::config::Config>,
    state: Res<State<GameState>>,
    loading: Option<Res<crate::loading::LoadingScreen>>,
    mut roots: Query<&mut Visibility, With<MinimapRoot>>,
) {
    let loading = loading.is_some_and(|l| l.active);
    let want = if *state.get() == GameState::Playing && !cfg.no_hud && !loading {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        v.set_if_neq(want);
    }
}

fn update_minimap_remote_dots(
    mut commands: Commands,
    st: Res<MinimapState>,
    remote: Res<RemotePlayers>,
    roots: Query<Entity, With<MinimapRoot>>,
    mut dots: ResMut<MinimapRemoteDots>,
) {
    let Ok(root) = roots.single() else { return };
    let to_remove: Vec<u64> = dots
        .map
        .keys()
        .copied()
        .filter(|id| !remote.players.contains_key(id))
        .collect();
    for id in to_remove {
        if let Some(e) = dots.map.remove(&id) {
            commands.entity(e).despawn();
        }
    }
    let scale = TILE_PX * 2.0f32.powf(st.view_zoom);
    let half = SIZE_PX * 0.5;
    // Display offset = Rot(-heading)·unrotated offset (same convention as the
    // canvas rotation; see `paint_bytes`).
    let (ch, sh) = (st.heading.to_radians().cos(), st.heading.to_radians().sin());
    for (id, rp) in remote.players.iter() {
        let geo = ed_geo::Geodetic::from_ecef(glam::DVec3::new(rp.pos[0], rp.pos[1], rp.pos[2]));
        let (wx, wy) = to_world(geo);
        let ox = (wx - st.center.0) * scale as f64;
        let oy = (wy - st.center.1) * scale as f64;
        let dx = ch * ox as f32 + sh * oy as f32;
        let dy = -sh * ox as f32 + ch * oy as f32;
        if let Some(&e) = dots.map.get(id) {
            commands.entity(e).insert(Node {
                position_type: PositionType::Absolute,
                left: Val::Px((half + dx as f32 - 4.0).clamp(-16.0, SIZE_PX + 16.0)),
                top: Val::Px((half + dy as f32 - 4.0).clamp(-16.0, SIZE_PX + 16.0)),
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::MAX,
                ..default()
            });
        } else {
            let e = commands
                .spawn((
                    MinimapRemoteDot { id: *id },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(half + dx as f32 - 4.0),
                        top: Val::Px(half + dy as f32 - 4.0),
                        width: Val::Px(8.0),
                        height: Val::Px(8.0),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.6, 1.0)),
                    BorderColor::all(Color::WHITE),
                    ChildOf(root),
                    ZIndex(2),
                ))
                .id();
            dots.map.insert(*id, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_tile(z: u8, center: (f64, f64), rgba: [u8; 4]) -> HashMap<TileId, Arc<Vec<u8>>> {
        let n = 1u32 << z;
        let idx = (center.0 * n as f64) as u32;
        let idy = (center.1 * n as f64) as u32;
        let id = TileId::new(z, idx, idy);
        let mut flat = Vec::with_capacity(maprender::SIZE * maprender::SIZE * 4);
        for _ in 0..maprender::SIZE * maprender::SIZE {
            flat.extend_from_slice(&rgba);
        }
        HashMap::from([(id, Arc::new(flat))])
    }

    fn solid(rgba: [u8; 4]) -> Arc<Vec<u8>> {
        let mut flat = Vec::with_capacity(maprender::SIZE * maprender::SIZE * 4);
        for _ in 0..maprender::SIZE * maprender::SIZE {
            flat.extend_from_slice(&rgba);
        }
        Arc::new(flat)
    }

    fn px(out: &[u8], cx: u32, cy: u32) -> [u8; 4] {
        let n = SIZE_PX as u32 * RES;
        let at = (cy as usize * n as usize + cx as usize) * 4;
        [out[at], out[at + 1], out[at + 2], out[at + 3]]
    }

    fn paint(
        center: (f64, f64),
        heading: f32,
        zoom: f32,
        tiles: &HashMap<TileId, Arc<Vec<u8>>>,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        paint_bytes(tiles, center, heading, zoom, &mut out);
        out
    }

    #[test]
    fn disk_mask_is_baked_in() {
        let center = (0.5, 0.5);
        let tiles = plain_tile(14, center, [200, 200, 200, 255]);
        let out = paint(center, 0.0, 14.0, &tiles);
        let n = SIZE_PX as u32 * RES;
        assert_eq!(px(&out, n / 2, n / 2), [200, 200, 200, 255]);
        assert_eq!(px(&out, n / 2 + 10, n / 2), [200, 200, 200, 255]);
        assert_eq!(px(&out, n / 2, n / 2 + 10), [200, 200, 200, 255]);
        // Corners: far outside the disk.
        assert_eq!(px(&out, 4, 4)[3], 0);
        assert_eq!(px(&out, n - 5, 8)[3], 0);
    }

    #[test]
    fn facing_east_shows_north_at_the_left_edge() {
        // h = 90° (due east). The texel due north of the camera must appear on
        // the screen's LEFT midline; the texel due east must appear ABOVE it.
        let z = 14u8;
        let center = (0.55, 0.5);
        let n = 1u32 << z;
        // Red tile to the camera's north, empty world elsewhere.
        let red_id = TileId::new(
            z,
            (center.0 * n as f64) as u32,
            (center.1 * n as f64) as u32 - 1,
        );
        let tiles = HashMap::from([(red_id, solid([255, 0, 0, 255]))]);
        let out = paint(center, 90.0, z as f32, &tiles);
        let np = SIZE_PX as u32 * RES;
        // Left edge midline (inside radius): the north tile shows through.
        assert_eq!(px(&out, 18, np / 2)[0], 255);
        // Right edge midline: south neighbour (n/a -> blank).
        assert_eq!(px(&out, np - 19, np / 2)[3], 0);
        // Top of the disk: the east neighbour (n/a -> blank).
        assert_eq!(px(&out, np / 2, 20)[3], 0);
    }

    #[test]
    fn heading_zero_keeps_north_up() {
        let z = 14u8;
        let n = 1u32 << z;
        let center = (0.55, 0.5);
        let red_id = TileId::new(
            z,
            (center.0 * n as f64) as u32,
            (center.1 * n as f64) as u32 - 1,
        );
        let tiles = HashMap::from([(red_id, solid([255, 0, 0, 255]))]);
        let out = paint(center, 0.0, z as f32, &tiles);
        let np = SIZE_PX as u32 * RES;
        // North tile shows above centre; below centre is blank.
        assert_eq!(px(&out, np / 2, 20)[0], 255);
        assert_eq!(px(&out, np / 2, np - 21)[3], 0);
    }

    #[test]
    fn heading_180_shows_north_at_the_bottom_midline() {
        // Facing south (h = 180°), the north texel must sit exactly on the
        // bottom midline. Guarded with an inner offset so the whole sampled
        // ray keeps its bearing (caught a degrees-as-radians rotation bug
        // that the tile granular edge tests slipped through).
        let z = 14u8;
        let n = 1u32 << z;
        let center = (0.55, 0.5);
        let red_id = TileId::new(
            z,
            (center.0 * n as f64) as u32,
            (center.1 * n as f64) as u32 - 1,
        );
        let tiles = HashMap::from([(red_id, solid([255, 0, 0, 255]))]);
        let out = paint(center, 180.0, z as f32, &tiles);
        let np = SIZE_PX as u32 * RES;
        for dy in [21, 60, 100] {
            let (cxff, cy) = (np / 2, np - dy);
            assert_eq!(px(&out, cxff, cy)[0], 255, "row -{dy}");
        }
        // Above centre it is blank (south neighbour).
        assert_eq!(px(&out, np / 2, 21)[3], 0);
    }

    #[test]
    fn world_roundtrip_is_stable() {
        let g = ed_geo::Geodetic::new(46.02, 7.75, 0.0);
        let w = to_world(g);
        assert!(w.0 > 0.0 && w.0 < 1.0);
        assert!(w.1 > 0.0 && w.1 < 1.0);
    }

    #[test]
    fn zoom_follows_speed() {
        assert!(zoom_for_speed(0.0) > zoom_for_speed(1.4));
        assert!(zoom_for_speed(30.0) > zoom_for_speed(300.0));
        assert_eq!(zoom_for_speed(1.0e6), MIN_VIEW_ZOOM);
    }
}
