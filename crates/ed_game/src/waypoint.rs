//! Waypoint system: a single active waypoint the player can place on the menu
//! map. Surfaces: menu-map great-circle line, minimap direction arrow + label,
//! and a 3D HUD tag anchored to the ground point.

use crate::config::Config;

use crate::origin::WorldOrigin;
use bevy::prelude::*;
use bevy::mesh::PrimitiveTopology;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::window::PrimaryWindow;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::Geodetic;


pub struct WaypointPlugin;

impl Plugin for WaypointPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        app.insert_resource(Waypoint::default())
            .insert_resource(WaypointElev { tx, rx, pending: false })
            .add_systems(Startup, (spawn_waypoint_tag, spawn_waypoint_globe_line))
            .add_systems(
                Update,
                (
                    consume_pending_waypoint,
                    poll_waypoint_elevation,
                    update_waypoint_tag,
                    update_waypoint_globe_line,
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Resource, Clone, Copy, Default)]
pub struct Waypoint {
    pub pos: Option<Geodetic>,
}

impl Waypoint {
    pub fn set(&mut self, g: Geodetic) {
        self.pos = Some(g);
    }
    pub fn clear(&mut self) {
        self.pos = None;
    }
}

/// Async elevation fetch state for waypoint ground height.
#[derive(Resource)]
pub struct WaypointElev {
    tx: Sender<Result<Geodetic, String>>,
    rx: Receiver<Result<Geodetic, String>>,
    pending: bool,
}

impl WaypointElev {
    pub fn request(&mut self, cache: ed_data::cache::DiskCache, g: Geodetic) {
        if self.pending {
            return;
        }
        self.pending = true;
        let tx = self.tx.clone();
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let mut g = g;
                let t = ed_geo::tile::TileId::containing(g.lat_deg, g.lon_deg, 12);
                if let Ok(ht) = ed_data::elevation::fetch(&cache, t) {
                    let (u, v) = t.geodetic_to_uv(g.lat_deg, g.lon_deg);
                    g.alt_m = ht.sample(u, v).max(0.0) as f64;
                }
                let _ = tx.send(Ok(g));
            })
            .detach();
    }
}

fn poll_waypoint_elevation(
    mut wp: ResMut<Waypoint>,
    mut elev: ResMut<WaypointElev>,
) {
    if !elev.pending {
        return;
    }
    if let Ok(res) = elev.rx.try_recv() {
        elev.pending = false;
        if let Ok(g) = res {
            if let Some(cur) = wp.pos {
                // Only apply if lat/lon still match (user didn't clear in between).
                if (cur.lat_deg - g.lat_deg).abs() < 1e-9
                    && (cur.lon_deg - g.lon_deg).abs() < 1e-9
                {
                    wp.pos = Some(g);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Consume pending waypoint from globe click
// ---------------------------------------------------------------------------

fn consume_pending_waypoint(
    mut map: ResMut<crate::mapview::MapState>,
    mut wp: ResMut<Waypoint>,
    cfg: Res<Config>,
    mut elev: ResMut<WaypointElev>,
) {
    if let Some(g) = map.pending_waypoint.take() {
        wp.set(g);
        elev.request(cfg.cache.clone(), g);
    }
}

// ---------------------------------------------------------------------------
// 3D HUD tag
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct WaypointTag;

#[derive(Component)]
pub struct WaypointTagDist;

#[derive(Component)]
pub struct WaypointTagIcon;

#[derive(Component)]
pub struct WaypointGlobeLine;

fn spawn_waypoint_tag(mut commands: Commands) {
    commands
        .spawn((
            WaypointTag,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-2000.0),
                top: Val::Px(-2000.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Visibility::Hidden,
            ZIndex(15),
        ))
        .with_children(|root| {
            // Distance text (shown only when <= 10 m)
            root.spawn((
                WaypointTagDist,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.85, 0.3)),
                Visibility::Hidden,
            ));
            // Icon: small diamond
            root.spawn((
                WaypointTagIcon,
                Node {
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::degrees(45.0)),
                BackgroundColor(Color::srgb(0.2, 0.85, 0.3)),
                BorderColor::all(Color::WHITE),
            ));
            // Connector line (shown when far)
            root.spawn((
                Node {
                    width: Val::Px(1.5),
                    height: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.85, 0.3, 0.6)),
                Visibility::Hidden,
            ));
        });
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn update_waypoint_tag(
    state: Res<State<crate::menu::GameState>>,
    mut wp: ResMut<Waypoint>,
    origin: Res<WorldOrigin>,
    camera: Query<(&Camera, &GlobalTransform), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut anchors: Query<(&mut Node, &ComputedNode, &mut Visibility, &mut ZIndex), (With<WaypointTag>, Without<WaypointTagDist>)>,
    mut dist_texts: Query<(&mut Text, &mut Visibility), (With<WaypointTagDist>, Without<WaypointTag>)>,
) {
    if *state.get() != crate::menu::GameState::Playing {
        for (_, _, mut vis, _) in anchors.iter_mut() { *vis = Visibility::Hidden; }
        for (_, mut dv) in dist_texts.iter_mut() { *dv = Visibility::Hidden; }
        return;
    }
    let Ok((cam, cam_gt)) = camera.single() else {
        return;
    };

    // No waypoint → hide.
    let Some(g) = wp.pos else {
        for (_, _, mut vis, _) in anchors.iter_mut() {
            *vis = Visibility::Hidden;
        }
        for (_, mut dv) in dist_texts.iter_mut() {
            *dv = Visibility::Hidden;
        }
        return;
    };

    let world_pos = origin.to_render(g);
    let dist = (world_pos - cam_gt.translation()).length() as f64;

    if dist > 1_000_000.0 {
        for (_, _, mut vis, _) in anchors.iter_mut() {
            *vis = Visibility::Hidden;
        }
        for (_, mut dv) in dist_texts.iter_mut() {
            *dv = Visibility::Hidden;
        }
        return;
    }

    // Auto-clear when visited (within 100 m).
    if dist <= 100.0 {
        wp.clear();
        for (_, _, mut vis, _) in anchors.iter_mut() {
            *vis = Visibility::Hidden;
        }
        for (_, mut dv) in dist_texts.iter_mut() {
            *dv = Visibility::Hidden;
        }
        return;
    }

    let win_logical = windows.single().ok().map(|w| w.resolution.size()).unwrap_or(Vec2::ZERO);

    for (mut node, computed, mut vis, mut z_index) in anchors.iter_mut() {
        let in_front = (world_pos - cam_gt.translation()).dot(cam_gt.forward().as_vec3()) > 0.0;

        let z = ((1_000_000.0 - dist) * 0.1).clamp(1.0, 9999.0) as i32;
        *z_index = ZIndex(z);

        match cam.world_to_viewport(cam_gt, world_pos) {
            Ok(screen) if in_front => {
                let scale = cam
                    .logical_viewport_size()
                    .filter(|s| s.x > 0.0 && win_logical.x > 0.0)
                    .map(|s| win_logical / s)
                    .unwrap_or(Vec2::ONE);
                let sx = screen.x * scale.x;
                let sy = screen.y * scale.y;

                let size = computed.size();
                let w = if size.x > 0.0 { size.x } else { 60.0 };
                let h = if size.y > 0.0 { size.y } else { 30.0 };

                node.left = Val::Px(sx - w * 0.5);
                node.top = Val::Px(sy - h - 4.0);
                *vis = Visibility::Inherited;
            }
            _ => {
                *vis = Visibility::Hidden;
            }
        }
    }

    // Distance label: km when >= 1 km, otherwise metres.
    for (mut txt, mut dv) in dist_texts.iter_mut() {
        let s = if dist >= 1000.0 {
            format!("{:.1} km", dist / 1000.0)
        } else {
            format!("{:.0} m", dist)
        };
        if txt.0 != s {
            txt.0 = s;
        }
        *dv = Visibility::Inherited;
    }
}

// ---------------------------------------------------------------------------
// Globe line (great circle)
// ---------------------------------------------------------------------------

fn spawn_waypoint_globe_line(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mat = mats.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.85, 0.3),
        unlit: true,
        ..default()
    });
    commands.spawn((
        WaypointGlobeLine,
        Mesh3d(meshes.add(Mesh::new(
            PrimitiveTopology::LineList,
            RenderAssetUsages::default(),
        ))),
        MeshMaterial3d(mat),
        RenderLayers::layer(crate::globe::GLOBE_LAYER),
        Visibility::Hidden,
    ));
}

fn update_waypoint_globe_line(
    wp: Res<Waypoint>,
    origin: Res<WorldOrigin>,
    mut q: Query<(&mut Mesh3d, &mut Visibility), With<WaypointGlobeLine>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok((mesh_handle, mut vis)) = q.single_mut() else {
        return;
    };

    let Some(g) = wp.pos else {
        *vis = Visibility::Hidden;
        return;
    };

    // Get player geodetic.
    // We don't have direct access to player position here, but we can use
    // the origin frame centre (which is near the player).
    let player_geo = origin.frame.origin;

    // Great-circle interpolation.
    let a = ed_world::globe::unit_from_latlon(player_geo.lat_deg, player_geo.lon_deg);
    let b = ed_world::globe::unit_from_latlon(g.lat_deg, g.lon_deg);

    let n = 64usize;
    let mut positions = Vec::with_capacity(n * 3);
    let mut indices = Vec::with_capacity((n - 1) * 2);

    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let p = a.lerp(b, t).normalize_or(a);
        let pp = p * 1.002; // slightly above surface
        positions.push([pp.x, pp.y, pp.z]);
        if i > 0 {
            indices.push((i - 1) as u32);
            indices.push(i as u32);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    // LineList needs indices.
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));

    if let Some(mut m) = meshes.get_mut(&mesh_handle.0) {
        *m = mesh;
    }
    *vis = Visibility::Visible;
}




