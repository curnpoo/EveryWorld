//! HUD: compass strip (top), mode pill (bottom), telemetry cards, debug stats,
//! and spaceflight target rings that track the Moon and planets across the sky
//! with name and distance.

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
        app.add_systems(Startup, spawn_hud).add_systems(
            Update,
            (update_text, update_markers, update_toast, stats_visibility, hud_visibility),
        );
    }
}

#[allow(dead_code)]
pub const GLASS_BG: Color = Color::srgba(0.04, 0.06, 0.09, 0.72);
#[allow(dead_code)]
pub const GLASS_BORDER: Color = Color::srgba(1.0, 1.0, 1.0, 0.14);
#[allow(dead_code)]
pub const ACCENT_CYAN: Color = Color::srgb(0.35, 0.82, 0.98);
#[allow(dead_code)]
pub const ACCENT_AMBER: Color = Color::srgb(0.98, 0.65, 0.15);
#[allow(dead_code)]
pub const TEXT_PRIMARY: Color = Color::WHITE;
#[allow(dead_code)]
pub const TEXT_MUTED: Color = Color::srgba(1.0, 1.0, 1.0, 0.48);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudField {
    CompassTape,
    CompassDeg,
    SpeedVal,
    SpeedSub,
    AltVal,
    AltSub,
    Mode,
    Stats,
}

#[derive(Component)]
struct StatsRoot;

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

#[derive(Component)]
struct HudRoot;

fn hud_visibility(
    cfg: Res<crate::config::Config>,
    state: Res<State<crate::menu::GameState>>,
    loading: Option<Res<crate::loading::LoadingScreen>>,
    mut roots: Query<&mut Visibility, With<HudRoot>>,
) {
    let loading = loading.is_some_and(|l| l.active);
    let want = if *state.get() == crate::menu::GameState::Playing && !cfg.no_hud && !loading {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        v.set_if_neq(want);
    }
}

fn spawn_hud(mut commands: Commands) {
    // Top: compass strip and digital heading readout.
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(14.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(7.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    column_gap: Val::Px(14.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(GLASS_BG),
                BorderColor::all(GLASS_BORDER),
            ))
            .with_children(|p| {
                p.spawn((
                    HudField::CompassTape,
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.72)),
                ));
                p.spawn((
                    Node { width: Val::Px(1.0), height: Val::Px(14.0), ..default() },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.16)),
                ));
                p.spawn((
                    HudField::CompassDeg,
                    Text::new("000° N"),
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(ACCENT_CYAN),
                ));
            });
        });

    // Bottom-left: speed telemetry card.
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                bottom: Val::Px(18.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                min_width: Val::Px(130.0),
                ..default()
            },
            BackgroundColor(GLASS_BG),
            BorderColor::all(GLASS_BORDER),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("AIRSPEED"),
                TextFont { font_size: FontSize::Px(10.0), ..default() },
                TextColor(TEXT_MUTED),
            ));
            p.spawn(Node {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Baseline,
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    HudField::SpeedVal,
                    Text::new("0"),
                    TextFont { font_size: FontSize::Px(26.0), ..default() },
                    TextColor(TEXT_PRIMARY),
                ));
                r.spawn((
                    Text::new("MPH"),
                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                    TextColor(TEXT_MUTED),
                ));
            });
            p.spawn((
                HudField::SpeedSub,
                Text::new("MACH 0.00"),
                TextFont { font_size: FontSize::Px(11.0), ..default() },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.65)),
            ));
        });

    // Bottom-right: altitude and terrain clearance card (positioned above circular minimap).
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(18.0),
                bottom: Val::Px(248.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                width: Val::Px(crate::minimap::SIZE_PX),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(GLASS_BG),
            BorderColor::all(GLASS_BORDER),
        ))
        .with_children(|p| {
            p.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|top_row| {
                top_row.spawn((
                    Text::new("ALTITUDE (MSL)"),
                    TextFont { font_size: FontSize::Px(10.0), ..default() },
                    TextColor(TEXT_MUTED),
                ));
                top_row.spawn((
                    HudField::AltSub,
                    Text::new("AGL 0 FT"),
                    TextFont { font_size: FontSize::Px(11.0), ..default() },
                    TextColor(ACCENT_CYAN),
                ));
            });
            p.spawn(Node {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Baseline,
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    HudField::AltVal,
                    Text::new("0"),
                    TextFont { font_size: FontSize::Px(26.0), ..default() },
                    TextColor(TEXT_PRIMARY),
                ));
                r.spawn((
                    Text::new("FT"),
                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                    TextColor(TEXT_MUTED),
                ));
            });
        });

    // Bottom-center: mode & contextual flight control pill.
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(18.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(18.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(GLASS_BG),
                BorderColor::all(GLASS_BORDER),
            ))
            .with_children(|p| {
                p.spawn((
                    HudField::Mode,
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.90)),
                ));
            });
        });

    // Top-left: extended stats, toggled from the pause menu's Debug tab.
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(14.0),
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                StatsRoot,
                Node {
                    padding: UiRect::all(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    max_width: Val::Px(520.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.85)),
                BorderColor::all(GLASS_BORDER),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((
                    HudField::Stats,
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.86)),
                ));
            });
        });

    // Centre-screen arrival toast.
    commands
        .spawn((
            ArrivalToast,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(16.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(6),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.88)),
                BorderColor::all(Color::srgba(0.35, 0.82, 0.98, 0.40)),
            ))
            .with_children(|c| {
                c.spawn((
                    ArrivalText,
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        });

    // Body markers (ring + label), positioned every frame.
    for id in [
        BodyId::Moon,
        BodyId::Mars,
        BodyId::Venus,
        BodyId::Jupiter,
        BodyId::Saturn,
        BodyId::Mercury,
    ] {
        commands.spawn((
            Marker(id),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(Color::srgba(0.35, 0.82, 0.98, 0.85)),
            Visibility::Hidden,
        ));
        commands.spawn((
            MarkerLabel(id),
            Text::new(""),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(Color::srgba(0.85, 0.95, 1.0, 0.95)),
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
        ));
    }
}

fn cardinal_label(heading: f32) -> &'static str {
    let h = heading.rem_euclid(360.0);
    match ((h + 22.5) / 45.0).floor() as u32 % 8 {
        0 => "N",
        1 => "NE",
        2 => "E",
        3 => "SE",
        4 => "S",
        5 => "SW",
        6 => "W",
        7 => "NW",
        _ => "N",
    }
}

fn heading_tape(heading: f32) -> String {
    // 70° window, a tick every 5°, cardinal letters on the 45s.
    let mut s = String::new();
    let start = (heading - 35.0).round() as i32;
    for d in (start..=start + 70).filter(|d| d.rem_euclid(5) == 0) {
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
    s.trim_end().to_string()
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
    mut fields: Query<(&HudField, &mut Text)>,
) {
    let Ok(ct) = cams.single() else { return };
    let up = origin.up_at(ct.translation);
    let fwd = ct.forward().as_vec3().reject_from_normalized(up);
    let enu = ed_geo::render_to_enu(fwd.as_dvec3());
    let heading = (enu.x.atan2(enu.y).to_degrees() as f32).rem_euclid(360.0);

    let (pos, vel, mode, agl, boost, wing_on) = match players.single() {
        Ok((t, v, p)) => (t.translation, v.0, p.mode, p.agl, p.boost_stage, p.wingsuit_on),
        Err(_) => (ct.translation, Vec3::ZERO, MoveMode::Fall, 0.0, 0, true),
    };
    let g = origin.to_geodetic(pos);
    let up_p = origin.up_at(pos);
    let mph = vel.length() * 2.236_94;
    let vs = vel.dot(up_p) * 2.236_94;
    let ft = g.alt_m * 3.280_84;
    let mach = vel.length() as f64 / ed_flight::atmosphere(g.alt_m).speed_of_sound;

    let tape_str = heading_tape(heading);
    let deg_str = format!("{:03.0}° {}", heading.rem_euclid(360.0), cardinal_label(heading));
    let speed_val_str = format!("{:.0}", mph);
    let speed_sub_str = if mach >= 0.8 {
        format!("MACH {mach:.2}")
    } else {
        format!("{:.0} m/s", vel.length())
    };
    let alt_val_str = format!("{:.0}", ft);
    let alt_sub_str = {
        let ft_agl = agl * 3.280_84;
        let mut parts = Vec::new();
        if vs.abs() > 5.0 {
            parts.push(format!("{}{:.0} mph", if vs < 0.0 { "↓" } else { "↑" }, vs.abs()));
        }
        if ft_agl < 10_000.0 {
            parts.push(format!("AGL {:.0} ft", ft_agl));
        }
        if parts.is_empty() {
            "LEVEL".to_string()
        } else {
            parts.join("  •  ")
        }
    };
    let mode_str = {
        let mut s = mode.label().to_uppercase();
        if mode == MoveMode::RocketShoes {
            s.push_str(&format!("  •  BOOST {} [SPACE]", ["I", "II", "III", "MAX"][boost]));
        }
        if !matches!(mode, MoveMode::Walk | MoveMode::Parachute) {
            s.push_str(if wing_on { "  •  WINGSUIT READY [F]" } else { "  •  WINGSUIT OFF [F]" });
        }
        s
    };

    let mut stats_str = String::new();
    if gfx.show_stats {
        let local_h = ed_geo::sun::solar_hour(clock.unix_s, g.lon_deg) as f32;
        let tile = TileId::containing(g.lat_deg, g.lon_deg, 14);
        let w = weather.and_then(|w| w.0.clone());
        let wtxt = w.map_or("weather n/a".to_string(), |w| {
            format!("{} {:.0}°C wind {:.0} m/s", w.describe(), w.temperature_c, w.wind_speed_ms)
        });
        let (tiles_loaded, tiles_pending, tiles_desired) =
            mgr.as_ref().map_or((0, 0, 0), |m| (m.loaded.len(), m.pending.len(), m.desired.len()));
        let (p_io, p_build, p_apply) = mgr.as_ref().map_or((0, 0, 0), |m| m.pipeline());
        let sat_free = sat_pool.as_ref().map_or([0; 3], |p| p.free_layers());
        let (buildings, trees, tris) =
            mgr.as_ref().map_or((0, 0, 0), |m| (m.totals.buildings, m.totals.trees, m.totals.triangles));
        let (detail_loaded, detail_flight) = dm.as_ref().map_or((0, 0), |d| detail_stats(d));
        let (win_w, win_h, win_scale) =
            windows.single().map_or((0, 0, 1.0), |w| (w.physical_width(), w.physical_height(), w.scale_factor()));
        let fps = 1.0 / time.delta_secs().max(1e-4);
        let ms = time.delta_secs_f64() * 1000.0;
        stats_str = format!(
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

    for (field, mut t) in &mut fields {
        match field {
            HudField::CompassTape => {
                if t.0 != tape_str {
                    t.0 = tape_str.clone();
                }
            }
            HudField::CompassDeg => {
                if t.0 != deg_str {
                    t.0 = deg_str.clone();
                }
            }
            HudField::SpeedVal => {
                if t.0 != speed_val_str {
                    t.0 = speed_val_str.clone();
                }
            }
            HudField::SpeedSub => {
                if t.0 != speed_sub_str {
                    t.0 = speed_sub_str.clone();
                }
            }
            HudField::AltVal => {
                if t.0 != alt_val_str {
                    t.0 = alt_val_str.clone();
                }
            }
            HudField::AltSub => {
                if t.0 != alt_sub_str {
                    t.0 = alt_sub_str.clone();
                }
            }
            HudField::Mode => {
                if t.0 != mode_str {
                    t.0 = mode_str.clone();
                }
            }
            HudField::Stats => {
                if t.0 != stats_str {
                    t.0 = stats_str.clone();
                }
            }
        }
    }
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
    loading: Option<Res<crate::loading::LoadingScreen>>,
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
            t.0 = format!("Arrived at {}", notice.name);
        }
    }
    let show = playing && !loading.is_some_and(|l| l.active) && time.elapsed_secs_f64() < notice.until_s;
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
                node.left = Val::Px(s.x - 12.0);
                node.top = Val::Px(s.y - 12.0);
                *vis = Visibility::Visible;
                if let Some((_, mut ln, mut lt, mut lv)) = labels.iter_mut().find(|l| l.0.0 == m.0) {
                    ln.left = Val::Px(s.x + 18.0);
                    ln.top = Val::Px(s.y - 8.0);
                    let d = (p - cgt.translation()).length() as f64;
                    lt.0 = format!("{}  •  {}", ed_space::info(m.0).name.to_uppercase(), fmt_dist(d));
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
