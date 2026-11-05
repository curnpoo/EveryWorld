//! HUD: compass strip (top), mode pill (bottom), debug line, and spaceflight
//! target rings that track the Moon and planets across the screen with name
//! and distance.

use crate::menu::{GameState, GraphicsSettings};
use crate::origin::{Focus, WorldOrigin};
use crate::player::{MoveMode, Player};
use crate::sky::{CelestialBody, Clock, CurrentWeather};
use crate::streaming::{DetailManager, TileManager, detail_stats};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::window::{PrimaryWindow, Window};
use ed_geo::tile::TileId;
use ed_space::BodyId;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(Update, (update_text, update_markers, update_toast, stats_visibility, hud_visibility));
    }
}

#[derive(Component)]
struct CompassText;
#[derive(Component)]
struct PillText;
#[derive(Component)]
struct StatsRoot;
#[derive(Component)]
struct StatsText;
#[derive(Component)]
struct ArrivalToast;
#[derive(Component)]
struct ArrivalText;

/// A spawn just landed; the HUD shows the place name for a few seconds.
#[derive(Resource)]
pub struct ArrivalNotice {
    pub name: String,
    pub until_s: f64,
}
#[derive(Component)]
struct Marker(BodyId);
#[derive(Component)]
struct MarkerLabel(BodyId);

const INK: Color = Color::srgb(0.12, 0.13, 0.16);
const PAPER: Color = Color::srgba(1.0, 1.0, 1.0, 0.82);

fn pill(bg: Color) -> (Node, BackgroundColor) {
    (
        Node { padding: UiRect::axes(Val::Px(16.0), Val::Px(7.0)), border_radius: BorderRadius::all(Val::Px(18.0)), ..default() },
        BackgroundColor(bg),
    )
}

#[derive(Component)]
struct HudRoot;

fn hud_visibility(cfg: Res<crate::config::Config>, state: Res<State<crate::menu::GameState>>, mut roots: Query<&mut Visibility, With<HudRoot>>) {
    let want = if *state.get() == crate::menu::GameState::Playing && !cfg.no_hud { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        v.set_if_neq(want);
    }
}

fn spawn_hud(mut commands: Commands) {
    // Top: compass strip.
    commands
        .spawn((HudRoot, Node { position_type: PositionType::Absolute, top: Val::Px(14.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() }))
        .with_children(|c| {
            c.spawn(pill(PAPER)).with_children(|c| {
                c.spawn((CompassText, Text::new(""), TextFont { font_size: FontSize::Px(17.0), ..default() }, TextColor(INK)));
            });
        });
    // Bottom: mode pill.
    commands
        .spawn((HudRoot, Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        }))
        .with_children(|c| {
            c.spawn(pill(Color::srgba(0.1, 0.11, 0.14, 0.72))).with_children(|c| {
                c.spawn((PillText, Text::new(""), TextFont { font_size: FontSize::Px(18.0), ..default() }, TextColor(Color::WHITE)));
            });
        });
    // Top-left: extended stats, toggled in Settings > Debug stats.
    commands
        .spawn((HudRoot, Node { position_type: PositionType::Absolute, left: Val::Px(14.0), top: Val::Px(14.0), ..default() }))
        .with_children(|c| {
            c.spawn((
                StatsRoot,
                Node {
                    padding: UiRect::all(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    max_width: Val::Px(520.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.72)),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((StatsText, Text::new(""), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgba(1.0, 1.0, 1.0, 0.86))));
            });
        });
    // Centre-screen arrival toast.
    commands
        .spawn((
            ArrivalToast,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(18.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(6),
        ))
        .with_children(|p| {
            p.spawn(pill(Color::srgba(0.04, 0.05, 0.07, 0.82))).with_children(|p| {
                p.spawn((ArrivalText, Text::new(""), TextFont { font_size: FontSize::Px(20.0), ..default() }, TextColor(Color::WHITE)));
            });
        });
    // Body markers (ring + label), positioned every frame.
    for id in [BodyId::Moon, BodyId::Mars, BodyId::Venus, BodyId::Jupiter, BodyId::Saturn, BodyId::Mercury] {
        commands.spawn((
            Marker(id),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(Color::srgba(0.4, 0.9, 1.0, 0.9)),
            Visibility::Hidden,
        ));
        commands.spawn((
            MarkerLabel(id),
            Text::new(""),
            TextFont { font_size: FontSize::Px(13.0), ..default() },
            TextColor(Color::srgba(0.85, 0.97, 1.0, 0.95)),
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
        ));
    }
}

fn compass_strip(heading: f32) -> String {
    // 90° window, a tick every 5°, cardinal letters on the 45s.
    let mut s = String::new();
    let start = (heading - 45.0).round() as i32;
    for d in (start..=start + 90).filter(|d| d.rem_euclid(5) == 0) {
        let a = d.rem_euclid(360);
        let label = match a {
            0 => "N",
            45 => "NE",
            90 => "E",
            135 => "SE",
            180 => "S",
            225 => "SW",
            270 => "W",
            315 => "NW",
            _ if a % 15 == 0 => "|",
            _ => "·",
        };
        s.push_str(label);
        s.push(' ');
    }
    format!("{}   {:03.0}°", s.trim_end(), heading.rem_euclid(360.0))
}

fn fmt_dist(m: f64) -> String {
    if m < 1_000.0 {
        format!("{m:.0} m")
    } else if m < 1.0e6 {
        format!("{:.1} km", m / 1000.0)
    } else if m < 1.0e9 {
        format!("{:.0}k km", m / 1.0e6)
    } else {
        format!("{:.2} AU", m / ed_space::AU)
    }
}

#[allow(clippy::too_many_arguments)]
fn update_text(
    time: Res<Time>,
    clock: Res<Clock>,
    gfx: Res<GraphicsSettings>,
    origin: Res<WorldOrigin>,
    weather: Option<Res<CurrentWeather>>,
    mgr: Option<Res<TileManager>>,
    dm: Option<Res<DetailManager>>,
    sat_pool: Option<Res<crate::ground_material::SatPool>>,
    players: Query<(&Transform, &LinearVelocity, &Player)>,
    cams: Query<&Transform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut compass: Query<&mut Text, (With<CompassText>, Without<PillText>, Without<StatsText>)>,
    mut pill: Query<&mut Text, (With<PillText>, Without<CompassText>, Without<StatsText>)>,
    mut stats: Query<&mut Text, (With<StatsText>, Without<CompassText>, Without<PillText>)>,
) {
    let Ok(ct) = cams.single() else { return };
    let up = origin.up_at(ct.translation);
    let fwd = ct.forward().as_vec3().reject_from_normalized(up);
    let enu = ed_geo::render_to_enu(fwd.as_dvec3());
    let heading = (enu.x.atan2(enu.y).to_degrees() as f32).rem_euclid(360.0);
    if let Ok(mut t) = compass.single_mut() {
        t.0 = compass_strip(heading);
    }
    let (pos, vel, mode, agl, boost, wing_on) = match players.single() {
        Ok((t, v, p)) => (t.translation, v.0, p.mode, p.agl, p.boost_stage, p.wingsuit_on),
        Err(_) => (ct.translation, Vec3::ZERO, MoveMode::Fall, 0.0, 0, true),
    };
    let g = origin.to_geodetic(pos);
    let up_p = origin.up_at(pos);
    let mph = vel.length() * 2.236_94;
    let vs = vel.dot(up_p) * 2.236_94;
    let ft = g.alt_m * 3.280_84;
    if let Ok(mut t) = pill.single_mut() {
        let mut s = format!("{}   {:.0} mph   {:.0} ft", mode.label(), mph, ft);
        if vs.abs() > 5.0 {
            s.push_str(&format!("   {}{:.0} mph", if vs < 0.0 { "↓" } else { "↑" }, vs.abs()));
        }
        if mode == MoveMode::RocketShoes {
            s.push_str(&format!("   BOOST {}", ["I", "II", "III", "MAX"][boost]));
        }
        if !matches!(mode, MoveMode::Walk | MoveMode::Parachute) {
            s.push_str(if wing_on { "   WINGSUIT ON [F]" } else { "   WINGSUIT OFF [F]" });
        }
        let mach = vel.length() as f64 / ed_flight::atmosphere(g.alt_m).speed_of_sound;
        if mach > 0.8 {
            s.push_str(&format!("   MACH {mach:.2}"));
        }
        t.0 = s;
    }
    let Ok(mut t) = stats.single_mut() else { return };
    if !gfx.show_stats {
        if !t.0.is_empty() {
            t.0.clear();
        }
        return;
    }
    let local_h = ((clock.unix_s / 3600.0 + g.lon_deg / 15.0).rem_euclid(24.0)) as f32;
    let tile = TileId::containing(g.lat_deg, g.lon_deg, 14);
    let w = weather.and_then(|w| w.0.clone());
    let wtxt = w.map_or("weather n/a".to_string(), |w| format!("{} {:.0}°C wind {:.0} m/s", w.describe(), w.temperature_c, w.wind_speed_ms));
    let (tiles_loaded, tiles_pending, tiles_desired) = mgr.as_ref().map_or((0, 0, 0), |m| (m.loaded.len(), m.pending.len(), m.desired.len()));
    let (p_io, p_build, p_apply) = mgr.as_ref().map_or((0, 0, 0), |m| m.pipeline());
    let sat_free = sat_pool.as_ref().map_or([0; 3], |p| p.free_layers());
    let (buildings, trees, tris) = mgr.as_ref().map_or((0, 0, 0), |m| (m.totals.buildings, m.totals.trees, m.totals.triangles));
    let (detail_loaded, detail_flight) = dm.as_ref().map_or((0, 0), |d| detail_stats(d));
    let (win_w, win_h, win_scale) = windows.single().map_or((0, 0, 1.0), |w| (w.physical_width(), w.physical_height(), w.scale_factor()));
    let fps = 1.0 / time.delta_secs().max(1e-4);
    let ms = time.delta_secs_f64() * 1000.0;
    t.0 = format!(
        "EveryWorld — debug stats\n\
         pos {:.5}, {:.5}   alt {:.0} m   AGL {:.0} m\n\
         tile {}/{}/{}   hdg {:.0}°   solar {:02}:{:02}\n\
         {:.0} fps ({:.1} ms)   render {}%   window {}x{} @{:.1}x ({}x{} px)\n\
         tiles {} loaded / {} pending / {} desired   (io {} / build {} / apply {})\n\
         satellite layers free {} / {} / {}\n\
         buildings {}   trees {}   tris {:.1}M\n\
         detail chunks {} loaded / {} in flight\n\
         mode {}   speed {:.0} mph   vertical {:+.0} mph\n\
         {}",
        g.lat_deg,
        g.lon_deg,
        g.alt_m,
        agl,
        tile.z,
        tile.x,
        tile.y,
        heading,
        local_h as u32,
        (local_h.fract() * 60.0) as u32,
        fps,
        ms,
        gfx.render_pct,
        win_w,
        win_h,
        win_scale,
        (win_w as f32 / win_scale) as u32,
        (win_h as f32 / win_scale) as u32,
        tiles_loaded,
        tiles_pending,
        tiles_desired,
        p_io,
        p_build,
        p_apply,
        sat_free[0],
        sat_free[1],
        sat_free[2],
        buildings,
        trees,
        tris as f32 / 1e6,
        detail_loaded,
        detail_flight,
        mode.label(),
        mph,
        vs,
        wtxt,
    );
}


fn stats_visibility(gfx: Res<GraphicsSettings>, mut roots: Query<&mut Visibility, With<StatsRoot>>) {
    let want = if gfx.show_stats { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        v.set_if_neq(want);
    }
}

fn update_toast(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut commands: Commands,
    notice: Option<ResMut<ArrivalNotice>>,
    mut roots: Query<&mut Visibility, With<ArrivalToast>>,
    mut texts: Query<&mut Text, With<ArrivalText>>,
) {
    let Some(mut notice) = notice else {
        for mut v in &mut roots {
            v.set_if_neq(Visibility::Hidden);
        }
        return;
    };
    let playing = *state.get() == GameState::Playing;
    if notice.until_s == 0.0 {
        notice.until_s = time.elapsed_secs_f64() + 5.0;
        if let Ok(mut t) = texts.single_mut() {
            t.0 = format!("You've arrived at {}", notice.name);
        }
    }
    let show = playing && time.elapsed_secs_f64() < notice.until_s;
    for mut v in &mut roots {
        v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    if !show && time.elapsed_secs_f64() >= notice.until_s {
        commands.remove_resource::<ArrivalNotice>();
    }
}


#[allow(clippy::type_complexity)]
fn update_markers(
    cams: Query<(&Camera, &GlobalTransform), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    gfx: Res<crate::menu::GraphicsSettings>,
    focus: Query<&GlobalTransform, With<Focus>>,
    origin: Res<WorldOrigin>,
    bodies: Query<(&CelestialBody, &GlobalTransform)>,
    mut rings: Query<(&Marker, &mut Node, &mut Visibility), Without<MarkerLabel>>,
    mut labels: Query<(&MarkerLabel, &mut Node, &mut Text, &mut Visibility), Without<Marker>>,
) {
    let Ok((cam, cgt)) = cams.single() else { return };
    let observer = focus.iter().next().map(|f| f.translation()).unwrap_or(cgt.translation());
    let in_space = origin.to_geodetic(observer).alt_m >= crate::sky::SPACE_ALTITUDE_M;
    for (m, mut node, mut vis) in &mut rings {
        if !in_space {
            *vis = Visibility::Hidden;
            if let Some((_, _, _, mut lv)) = labels.iter_mut().find(|l| l.0.0 == m.0) {
                *lv = Visibility::Hidden;
            }
            continue;
        }
        let Some((_, bgt)) = bodies.iter().find(|(b, _)| b.0 == m.0) else { continue };
        let p = bgt.translation();
        let in_front = (p - cgt.translation()).dot(cgt.forward().as_vec3()) > 0.0;
        match cam.world_to_viewport(cgt, p) {
            Ok(s) if in_front => {
                let s = s * (100.0 / gfx.render_pct as f32);
                node.left = Val::Px(s.x - 13.0);
                node.top = Val::Px(s.y - 13.0);
                *vis = Visibility::Visible;
                if let Some((_, mut ln, mut lt, mut lv)) = labels.iter_mut().find(|l| l.0.0 == m.0) {
                    ln.left = Val::Px(s.x + 18.0);
                    ln.top = Val::Px(s.y - 9.0);
                    let d = (p - cgt.translation()).length() as f64;
                    lt.0 = format!("{}  {}", ed_space::info(m.0).name, fmt_dist(d));
                    *lv = Visibility::Visible;
                }
            }
            _ => {
                *vis = Visibility::Hidden;
                if let Some((_, _, _, mut lv)) = labels.iter_mut().find(|l| l.0.0 == m.0) {
                    *lv = Visibility::Hidden;
                }
            }
        }
    }
}
