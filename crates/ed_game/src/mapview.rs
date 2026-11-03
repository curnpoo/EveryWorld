//! The world map used by the main menu and the pause menu.
//!
//! A slippy map drawn from the game's own data (see `ed_world::maprender`):
//! drag to pan, scroll to zoom around the cursor, click a point to pick it.
//! Place names (countries, states, cities, towns, villages) are UI text laid
//! over the tiles, decluttered by importance so they never pile up.
//!
//! The widget is any UI node with a `MapCanvas` component; this module fills
//! it with tiles, labels, the pick marker and the "drop here" popup.

use crate::config::Config;
use crate::spawn::{SpawnHeight, SpawnRequest};
use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::AsyncComputeTaskPool;
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::Geodetic;
use ed_geo::tile::TileId;
use ed_world::maprender::{self, Label, PlaceClass};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

pub struct MapViewPlugin;

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        app.insert_resource(MapState::new(tx, rx))
            .add_systems(Update, (map_input, request_tiles, receive_tiles, layout_map, popup_buttons).chain());
    }
}

/// Marks the UI node the map draws into.
#[derive(Component)]
pub struct MapCanvas;

#[derive(Component)]
struct TileNode(TileId);

#[derive(Component)]
struct LabelNode(usize);

#[derive(Component)]
struct Marker;

#[derive(Component)]
struct Popup;

#[derive(Component)]
struct PopupTitle;

#[derive(Component, Clone, Copy)]
enum PopupAction {
    Sky,
    Ground,
    Close,
}

const MAX_LABELS: usize = 140;
const MAX_IN_FLIGHT: usize = 8;

#[derive(Resource)]
pub struct MapState {
    /// Fractional zoom (0 = whole world in one 256 px tile).
    pub zoom: f64,
    /// Center in Web-Mercator world coordinates (0..1).
    pub center: (f64, f64),
    pub selected: Option<Geodetic>,
    tiles: HashMap<TileId, (Handle<Image>, Vec<Label>)>,
    pending: HashSet<TileId>,
    tx: Sender<maprender::MapTile>,
    rx: Receiver<maprender::MapTile>,
    template: Option<String>,
    template_tried: bool,
    drag: Option<Vec2>,
    drag_dist: f32,
}

impl MapState {
    fn new(tx: Sender<maprender::MapTile>, rx: Receiver<maprender::MapTile>) -> Self {
        Self {
            zoom: 2.0,
            center: (0.5, 0.42),
            selected: None,
            tiles: HashMap::new(),
            pending: HashSet::new(),
            tx,
            rx,
            template: None,
            template_tried: false,
            drag: None,
            drag_dist: 0.0,
        }
    }

    /// Center the map on a place at a given zoom.
    pub fn focus(&mut self, g: Geodetic, zoom: f64) {
        self.center = to_world(g);
        self.zoom = zoom;
    }
}

fn to_world(g: Geodetic) -> (f64, f64) {
    let lat = g.lat_deg.clamp(-85.0511, 85.0511).to_radians();
    ((g.lon_deg + 180.0) / 360.0, (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0)
}

fn from_world(w: (f64, f64)) -> Geodetic {
    let lon = w.0 * 360.0 - 180.0;
    let lat = (PI * (1.0 - 2.0 * w.1)).sinh().atan().to_degrees();
    Geodetic::new(lat, ed_geo::wrap_lon(lon), 0.0)
}

/// Pixel size of one world unit at a fractional zoom.
fn world_px(zoom: f64) -> f64 {
    256.0 * 2f64.powf(zoom)
}

fn canvas_size(node: &ComputedNode) -> Vec2 {
    node.size() * node.inverse_scale_factor()
}

fn map_input(
    mut st: ResMut<MapState>,
    canvases: Query<(&RelativeCursorPosition, &ComputedNode, &InheritedVisibility), With<MapCanvas>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    popup: Query<&RelativeCursorPosition, With<Popup>>,
) {
    let Some((rel, node, vis)) = canvases.iter().find(|c| c.2.get()) else {
        st.drag = None;
        return;
    };
    let size = canvas_size(node);
    let over_popup = popup.iter().any(|p| p.cursor_over);
    let Some(norm) = rel.normalized.filter(|_| rel.cursor_over && !over_popup) else {
        if !buttons.pressed(MouseButton::Left) {
            st.drag = None;
        }
        return;
    };
    let _ = vis;
    // Cursor in canvas pixels, origin at the canvas center. (normalized is -0.5..0.5 in Bevy 0.20.)
    let cur = Vec2::new(norm.x * size.x, norm.y * size.y);
    let wp = world_px(st.zoom);
    let cur_world = (st.center.0 + cur.x as f64 / wp, st.center.1 + cur.y as f64 / wp);
    // Zoom around the cursor.
    if scroll.delta.y != 0.0 {
        let nz = (st.zoom + scroll.delta.y as f64 * 0.35).clamp(1.0, 17.0);
        let nwp = world_px(nz);
        st.center = (cur_world.0 - cur.x as f64 / nwp, cur_world.1 - cur.y as f64 / nwp);
        st.zoom = nz;
    }
    // Drag to pan; a press-release with little movement is a click.
    if buttons.just_pressed(MouseButton::Left) {
        st.drag = Some(cur);
        st.drag_dist = 0.0;
    }
    if buttons.pressed(MouseButton::Left) && st.drag.is_some() {
        let d = motion.delta;
        st.drag_dist += d.length();
        let wp = world_px(st.zoom);
        st.center.0 -= d.x as f64 / wp;
        st.center.1 -= d.y as f64 / wp;
    }
    if buttons.just_released(MouseButton::Left) && st.drag.take().is_some() && st.drag_dist < 5.0 {
        st.selected = Some(from_world((cur_world.0.rem_euclid(1.0), cur_world.1.clamp(0.0, 1.0))));
    }
    st.center.0 = st.center.0.rem_euclid(1.0);
    st.center.1 = st.center.1.clamp(0.0, 1.0);
}

/// The tiles covering the canvas at the current zoom.
fn visible(st: &MapState, size: Vec2) -> (u8, Vec<(TileId, f32, f32, f32)>) {
    let z = (st.zoom.floor() as u8).min(16);
    let n = 1u32 << z;
    let wp = world_px(st.zoom);
    let tile_px = (wp / n as f64) as f32;
    let (cx, cy) = (st.center.0 * n as f64, st.center.1 * n as f64);
    let half_w = size.x as f64 / 2.0 / tile_px as f64;
    let half_h = size.y as f64 / 2.0 / tile_px as f64;
    let mut out = Vec::new();
    for ty in (cy - half_h).floor() as i64..=(cy + half_h).floor() as i64 {
        if ty < 0 || ty >= n as i64 {
            continue;
        }
        for tx in (cx - half_w).floor() as i64..=(cx + half_w).floor() as i64 {
            let id = TileId::new(z, tx.rem_euclid(n as i64) as u32, ty as u32);
            let left = ((tx as f64 - cx) * tile_px as f64) as f32 + size.x / 2.0;
            let top = ((ty as f64 - cy) * tile_px as f64) as f32 + size.y / 2.0;
            out.push((id, left, top, tile_px));
        }
    }
    (z, out)
}

fn request_tiles(cfg: Res<Config>, mut st: ResMut<MapState>, canvases: Query<(&ComputedNode, &InheritedVisibility), With<MapCanvas>>) {
    let Some((node, _)) = canvases.iter().find(|c| c.1.get()) else { return };
    if !st.template_tried {
        st.template_tried = true;
        st.template = ed_data::vector::tile_template(&cfg.cache).ok();
    }
    let (_, tiles) = visible(&st, canvas_size(node));
    // Also want parents so something shows while zooming.
    for (id, ..) in tiles {
        for t in [id, id.parent().unwrap_or(id)] {
            if st.pending.len() >= MAX_IN_FLIGHT || st.tiles.contains_key(&t) || st.pending.contains(&t) {
                continue;
            }
            st.pending.insert(t);
            let (tx, cache, tpl) = (st.tx.clone(), cfg.cache.clone(), st.template.clone());
            AsyncComputeTaskPool::get()
                .spawn(async move {
                    let _ = tx.send(maprender::fetch_and_render(&cache, tpl.as_deref(), t));
                })
                .detach();
        }
    }
}

fn receive_tiles(mut st: ResMut<MapState>, mut images: ResMut<Assets<Image>>) {
    for _ in 0..4 {
        let Ok(t) = st.rx.try_recv() else { break };
        st.pending.remove(&t.id);
        let img = Image::new(
            Extent3d { width: maprender::SIZE as u32, height: maprender::SIZE as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            t.rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        st.tiles.insert(t.id, (images.add(img), t.labels));
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_map(
    mut commands: Commands,
    st: Res<MapState>,
    canvases: Query<(Entity, &ComputedNode, &InheritedVisibility, Option<&Children>), With<MapCanvas>>,
    mut tile_nodes: Query<(Entity, &TileNode, &mut Node, &mut ImageNode), (Without<LabelNode>, Without<Marker>, Without<Popup>)>,
    mut labels: Query<(&LabelNode, &mut Node, &mut Text, &mut TextFont, &mut Visibility), (Without<TileNode>, Without<Marker>, Without<Popup>)>,
    mut marker: Query<(&mut Node, &mut Visibility), (With<Marker>, Without<TileNode>, Without<LabelNode>, Without<Popup>)>,
    mut popup: Query<(&mut Node, &mut Visibility), (With<Popup>, Without<TileNode>, Without<LabelNode>, Without<Marker>)>,
    mut popup_title: Query<&mut Text, (With<PopupTitle>, Without<LabelNode>)>,
) {
    for (canvas, node, vis, children) in &canvases {
        if !vis.get() {
            continue;
        }
        // First use: create the label pool, marker and popup.
        if children.is_none_or(|c| c.is_empty()) {
            build_overlays(&mut commands, canvas);
            continue;
        }
        let size = canvas_size(node);
        let (z, vis_tiles) = visible(&st, size);
        let wanted: HashMap<TileId, (f32, f32, f32)> = vis_tiles.iter().map(|(id, l, t, s)| (*id, (*l, *t, *s))).collect();
        let mut have: HashSet<TileId> = HashSet::new();
        for (e, tn, mut n, mut img) in &mut tile_nodes {
            match wanted.get(&tn.0) {
                Some((l, t, s)) => {
                    have.insert(tn.0);
                    n.left = Val::Px(*l);
                    n.top = Val::Px(*t);
                    n.width = Val::Px(*s + 0.6);
                    n.height = Val::Px(*s + 0.6);
                    // Show the parent's quadrant until this tile arrives.
                    if let Some((h, _)) = st.tiles.get(&tn.0) {
                        if img.image != *h {
                            img.image = h.clone();
                            img.rect = None;
                        }
                    } else if let Some(p) = tn.0.parent()
                        && let Some((h, _)) = st.tiles.get(&p)
                    {
                        img.image = h.clone();
                        let (qx, qy) = ((tn.0.x % 2) as f32 * 128.0, (tn.0.y % 2) as f32 * 128.0);
                        img.rect = Some(Rect::new(qx, qy, qx + 128.0, qy + 128.0));
                    }
                }
                None => commands.entity(e).despawn(),
            }
        }
        for (id, (l, t, s)) in &wanted {
            if have.contains(id) {
                continue;
            }
            commands.spawn((
                TileNode(*id),
                Node { position_type: PositionType::Absolute, left: Val::Px(*l), top: Val::Px(*t), width: Val::Px(*s + 0.6), height: Val::Px(*s + 0.6), ..default() },
                ImageNode { color: Color::WHITE, ..default() },
                ZIndex(0),
                ChildOf(canvas),
            ));
        }

        // Labels: gather, sort by importance, place greedily without overlap.
        let mut cands: Vec<(&Label, Vec2)> = Vec::new();
        for (id, (l, t, s)) in &wanted {
            if let Some((_, ls)) = st.tiles.get(id) {
                for lab in ls.iter().filter(|lab| lab.class.min_zoom() <= z) {
                    cands.push((lab, Vec2::new(l + lab.u * s, t + lab.v * s)));
                }
            }
        }
        cands.sort_by_key(|(l, _)| (l.class, l.rank));
        let mut placed: Vec<Rect> = Vec::new();
        let mut used = 0;
        let mut pool: Vec<_> = labels.iter_mut().collect();
        pool.sort_by_key(|(ln, ..)| ln.0);
        for (lab, p) in cands {
            if used >= pool.len() {
                break;
            }
            let fs = match lab.class {
                PlaceClass::Continent | PlaceClass::Country => 16.0,
                PlaceClass::State => 12.0,
                PlaceClass::City => if lab.rank <= 4 { 15.0 } else { 13.0 },
                PlaceClass::Town => 12.0,
                PlaceClass::Village => 11.0,
            };
            let w = lab.name.chars().count() as f32 * fs * 0.56;
            let r = Rect::new(p.x - w * 0.5, p.y - fs * 0.6, p.x + w * 0.5, p.y + fs * 0.6);
            if r.max.x < 0.0 || r.max.y < 0.0 || r.min.x > size.x || r.min.y > size.y {
                continue;
            }
            let pad = Rect { min: r.min - Vec2::splat(6.0), max: r.max + Vec2::splat(6.0) };
            if placed.iter().any(|q| !q.intersect(pad).is_empty()) {
                continue;
            }
            placed.push(r);
            let (_, n, t, f, v) = &mut pool[used];
            n.left = Val::Px(r.min.x);
            n.top = Val::Px(r.min.y);
            if t.0 != lab.name {
                t.0 = lab.name.clone();
            }
            f.font_size = FontSize::Px(fs);
            **v = Visibility::Inherited;
            used += 1;
        }
        for (_, _, _, _, v) in pool.iter_mut().skip(used) {
            **v = Visibility::Hidden;
        }

        // Selection marker + popup.
        if let Some(sel) = st.selected {
            let w = to_world(sel);
            let wp = world_px(st.zoom);
            let mut dx = w.0 - st.center.0;
            dx -= dx.round(); // nearest wrap
            let p = Vec2::new((dx * wp) as f32 + size.x / 2.0, ((w.1 - st.center.1) * wp) as f32 + size.y / 2.0);
            if let Ok((mut n, mut v)) = marker.single_mut() {
                n.left = Val::Px(p.x - 9.0);
                n.top = Val::Px(p.y - 9.0);
                *v = Visibility::Inherited;
            }
            if let Ok((mut n, mut v)) = popup.single_mut() {
                n.left = Val::Px((p.x + 16.0).min(size.x - 270.0).max(8.0));
                n.top = Val::Px((p.y - 40.0).clamp(8.0, (size.y - 150.0).max(8.0)));
                *v = Visibility::Inherited;
            }
            if let Ok(mut t) = popup_title.single_mut() {
                let s = format!("{:.4}°, {:.4}°", sel.lat_deg, sel.lon_deg);
                if t.0 != s {
                    t.0 = s;
                }
            }
        } else {
            if let Ok((_, mut v)) = marker.single_mut() {
                *v = Visibility::Hidden;
            }
            if let Ok((_, mut v)) = popup.single_mut() {
                *v = Visibility::Hidden;
            }
        }
    }
}

fn build_overlays(commands: &mut Commands, canvas: Entity) {
    for i in 0..MAX_LABELS {
        commands.spawn((
            LabelNode(i),
            Text::new(""),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(Color::srgb(0.16, 0.17, 0.2)),
            TextShadow { offset: Vec2::new(0.0, 1.0), color: Color::srgba(1.0, 1.0, 1.0, 0.85) },
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
            ZIndex(2),
            ChildOf(canvas),
        ));
    }
    commands.spawn((
        Marker,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(18.0),
            height: Val::Px(18.0),
            border: UiRect::all(Val::Px(4.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgb(0.98, 0.42, 0.1)),
        BorderColor::all(Color::WHITE),
        Visibility::Hidden,
        ZIndex(3),
        ChildOf(canvas),
    ));
    commands
        .spawn((
            Popup,
            RelativeCursorPosition::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(260.0),
                padding: UiRect::all(Val::Px(14.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.08, 0.1, 0.92)),
            Visibility::Hidden,
            ZIndex(4),
            ChildOf(canvas),
        ))
        .with_children(|p| {
            p.spawn((Text::new("Drop in here"), TextFont { font_size: FontSize::Px(17.0), ..default() }, TextColor(Color::WHITE)));
            p.spawn((PopupTitle, Text::new(""), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6))));
            for (label, action, accent) in [("Drop from the sky", PopupAction::Sky, true), ("Start on the ground", PopupAction::Ground, false), ("Cancel", PopupAction::Close, false)] {
                crate::menu::button(p, label, action, accent);
            }
        });
}

#[allow(deprecated)]
fn popup_buttons(
    mut st: ResMut<MapState>,
    q: Query<(&Interaction, &PopupAction), Changed<Interaction>>,
    mut spawns: MessageWriter<SpawnRequest>,
) {
    for (i, a) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        let Some(sel) = st.selected else { continue };
        match a {
            PopupAction::Close => st.selected = None,
            PopupAction::Sky | PopupAction::Ground => {
                let height = if matches!(a, PopupAction::Sky) { SpawnHeight::Sky } else { SpawnHeight::Ground };
                spawns.write(SpawnRequest::At { pos: sel, name: format!("{:.4}, {:.4}", sel.lat_deg, sel.lon_deg), height });
                st.selected = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_roundtrip() {
        let g = Geodetic::new(46.02, 7.75, 0.0);
        let back = from_world(to_world(g));
        assert!((back.lat_deg - g.lat_deg).abs() < 1e-9 && (back.lon_deg - g.lon_deg).abs() < 1e-9);
    }
}
