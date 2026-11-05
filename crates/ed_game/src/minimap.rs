//! In-game minimap, bottom-right of the 3D view.
//!
//! Drawn from the same vector/elevation data as the menu map
//! (`ed_world::maprender`): land cover, water, parks, roads and buildings.
//! The map is centered on the camera and rotated so the look direction points
//! up; a dot marks the camera position and a needle shows the way forward.

use crate::config::Config;
use crate::menu::GameState;
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

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        app.insert_resource(MinimapState::new(tx, rx))
            .add_systems(Startup, spawn_minimap)
            .add_systems(Update, (update_minimap, receive_tiles, layout_tiles, minimap_visibility).chain());
    }
}

const SIZE_PX: f32 = 220.0;
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
struct MinimapRotator;

#[derive(Component)]
struct MinimapTile(TileId);

#[derive(Component)]
struct MinimapNorth;

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
    tiles: HashMap<TileId, Handle<Image>>,
    pending: HashSet<TileId>,
    tx: Sender<maprender::MapTile>,
    rx: Receiver<maprender::MapTile>,
    template: Option<String>,
    template_tried: bool,
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
        }
    }
}

fn to_world(g: ed_geo::Geodetic) -> (f64, f64) {
    let lat = g.lat_deg.clamp(-85.0511, 85.0511).to_radians();
    ((g.lon_deg + 180.0) / 360.0, (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0)
}

fn canvas_size(node: &ComputedNode) -> Vec2 {
    node.size() * node.inverse_scale_factor()
}

fn spawn_minimap(mut commands: Commands) {
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
                border_radius: BorderRadius::all(Val::Px(18.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.61, 0.79, 0.88)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.55)),
            Visibility::Hidden,
            ZIndex(5),
        ))
        .with_children(|p| {
            // Everything under this node rotates with the camera heading.
            p.spawn((MinimapRotator, Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, UiTransform::IDENTITY, ZIndex(0)));
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
            // Forward needle: the map is rotated, so "up" is always forward.
            p.spawn((
                Text::new("▲"),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(Color::WHITE),
                Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(50.0), ..default() },
                UiTransform::from_xy(Val::Px(-6.0), Val::Px(-24.0)),
                ZIndex(4),
            ));
            // A north marker on the rim, positioned by the map rotation.
            p.spawn((
                MinimapNorth,
                Text::new("N"),
                TextFont { font_size: FontSize::Px(11.0), ..default() },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
                TextShadow { offset: Vec2::new(0.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.6) },
                Node { position_type: PositionType::Absolute, ..default() },
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
    let Ok((node, vis)) = roots.single() else { return };
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

fn receive_tiles(mut st: ResMut<MinimapState>, mut images: ResMut<Assets<Image>>) {
    for _ in 0..8 {
        let Ok(t) = st.rx.try_recv() else { break };
        st.pending.remove(&t.id);
        let img = Image::new(
            Extent3d { width: maprender::SIZE as u32, height: maprender::SIZE as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            t.rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        st.tiles.insert(t.id, images.add(img));
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
}

/// Tile ids covering the (rotated) canvas, with their top-left CSS pixels
/// relative to the canvas center, in the unrotated north-up frame.
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

fn best_image(st: &MinimapState, id: TileId) -> Option<(Handle<Image>, Option<Rect>)> {
    if let Some(h) = st.tiles.get(&id) {
        return Some((h.clone(), None));
    }
    let mut cur = id;
    for k in 1..=6u32 {
        let p = cur.parent()?;
        if let Some(h) = st.tiles.get(&p) {
            let span = maprender::SIZE as f32 / (1u32 << k) as f32;
            let m = (1u32 << k) - 1;
            let (qx, qy) = ((id.x & m) as f32 * span, (id.y & m) as f32 * span);
            return Some((h.clone(), Some(Rect::new(qx, qy, qx + span, qy + span))));
        }
        cur = p;
    }
    None
}

#[allow(clippy::type_complexity)]
fn layout_tiles(
    st: Res<MinimapState>,
    roots: Query<(&ComputedNode, &InheritedVisibility), With<MinimapRoot>>,
    mut rotators: Query<(Entity, &mut UiTransform), With<MinimapRotator>>,
    mut tiles: Query<(Entity, &MinimapTile, &ChildOf, &mut Node, &mut ImageNode, &mut Visibility), (Without<MinimapRoot>, Without<MinimapRotator>, Without<MinimapNorth>)>,
    mut north: Query<&mut Node, (With<MinimapNorth>, Without<MinimapTile>, Without<MinimapRotator>)>,
    mut commands: Commands,
) {
    let Ok((node, vis)) = roots.single() else { return };
    if !vis.get() {
        return;
    }
    let Ok((rotator, mut tf)) = rotators.single_mut() else { return };
    let size = canvas_size(node);
    tf.rotation = Rot2::radians(-st.heading.to_radians());

    let tpx = tile_px(st.view_zoom);
    let want = visible(&st, size);
    let wanted: HashMap<TileId, (f32, f32)> = want.iter().map(|(id, l, t)| (*id, (*l, *t))).collect();
    let mut have: HashSet<TileId> = HashSet::new();
    for (e, tile, parent, mut n, mut img, mut v) in &mut tiles {
        if parent.parent() != rotator {
            continue;
        }
        match wanted.get(&tile.0) {
            Some((left, top)) => {
                have.insert(tile.0);
                n.left = Val::Px(*left);
                n.top = Val::Px(*top);
                n.width = Val::Px(tpx);
                n.height = Val::Px(tpx);
                if let Some((h, rect)) = best_image(&st, tile.0) {
                    if img.image != h {
                        img.image = h;
                    }
                    if img.rect != rect {
                        img.rect = rect;
                    }
                    *v = Visibility::Inherited;
                } else {
                    *v = Visibility::Hidden;
                }
            }
            None => {
                commands.entity(e).despawn();
            }
        }
    }
    for (id, (left, top)) in &wanted {
        if have.contains(id) {
            continue;
        }
        let Some((h, rect)) = best_image(&st, *id) else { continue };
        commands.spawn((
            MinimapTile(*id),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(*left),
                top: Val::Px(*top),
                width: Val::Px(tpx),
                height: Val::Px(tpx),
                ..default()
            },
            ImageNode { image: h, rect, color: Color::WHITE, ..default() },
            ZIndex(0),
            ChildOf(rotator),
        ));
    }

    // North indicator on the rim: rotate the north vector by the map angle.
    if let Ok(mut n) = north.single_mut() {
        let r = size.x * 0.5 - 14.0;
        let h = st.heading.to_radians();
        let x = size.x * 0.5 - h.sin() * r - 5.0;
        let y = size.y * 0.5 - h.cos() * r - 6.0;
        n.left = Val::Px(x);
        n.top = Val::Px(y);
    }
}

fn minimap_visibility(cfg: Res<crate::config::Config>, state: Res<State<GameState>>, mut roots: Query<&mut Visibility, With<MinimapRoot>>) {
    let want = if *state.get() == GameState::Playing && !cfg.no_hud { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        v.set_if_neq(want);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_follows_speed() {
        assert!(zoom_for_speed(0.0) > zoom_for_speed(1.4));
        assert!(zoom_for_speed(1.4) > zoom_for_speed(30.0));
        assert!(zoom_for_speed(30.0) > zoom_for_speed(300.0));
        assert_eq!(zoom_for_speed(1.0e6), MIN_VIEW_ZOOM);
        let z = tile_z(16.6);
        assert!(tile_px(16.6) > TILE_PX && z == 16);
    }

    #[test]
    fn world_roundtrip_is_stable() {
        let g = ed_geo::Geodetic::new(46.02, 7.75, 0.0);
        let w = to_world(g);
        assert!(w.0 > 0.0 && w.0 < 1.0);
        assert!(w.1 > 0.0 && w.1 < 1.0);
    }
}
