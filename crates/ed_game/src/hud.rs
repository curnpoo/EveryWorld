//! HUD: compass strip (top), mode pill (bottom), debug line, and spaceflight
//! target rings that track the Moon and planets across the screen with name
//! and distance.

use crate::origin::{Focus, WorldOrigin};
use crate::player::{MoveMode, Player};
use crate::sky::{CelestialBody, Clock, CurrentWeather};
use crate::streaming::TileManager;
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy::text::FontSize;
use ed_space::BodyId;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(Update, (update_text, update_markers));
    }
}

#[derive(Component)]
struct CompassText;
#[derive(Component)]
struct PillText;
#[derive(Component)]
struct DebugText;
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

fn spawn_hud(mut commands: Commands) {
    // Top: compass strip.
    commands
        .spawn(Node { position_type: PositionType::Absolute, top: Val::Px(14.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() })
        .with_children(|c| {
            c.spawn(pill(PAPER)).with_children(|c| {
                c.spawn((CompassText, Text::new(""), TextFont { font_size: FontSize::Px(17.0), ..default() }, TextColor(INK)));
            });
        });
    // Bottom: mode pill + debug line.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(pill(Color::srgba(0.1, 0.11, 0.14, 0.72))).with_children(|c| {
                c.spawn((PillText, Text::new(""), TextFont { font_size: FontSize::Px(18.0), ..default() }, TextColor(Color::WHITE)));
            });
            c.spawn((DebugText, Text::new(""), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75))));
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
    origin: Res<WorldOrigin>,
    weather: Option<Res<CurrentWeather>>,
    mgr: Option<Res<TileManager>>,
    players: Query<(&Transform, &LinearVelocity, &Player)>,
    cams: Query<&Transform, With<Camera3d>>,
    mut compass: Query<&mut Text, (With<CompassText>, Without<PillText>, Without<DebugText>)>,
    mut pill: Query<&mut Text, (With<PillText>, Without<CompassText>, Without<DebugText>)>,
    mut dbg: Query<&mut Text, (With<DebugText>, Without<CompassText>, Without<PillText>)>,
) {
    let Ok(ct) = cams.single() else { return };
    let up = origin.up_at(ct.translation);
    let fwd = ct.forward().as_vec3().reject_from_normalized(up);
    let enu = ed_geo::render_to_enu(fwd.as_dvec3());
    let heading = (enu.x.atan2(enu.y).to_degrees() as f32).rem_euclid(360.0);
    if let Ok(mut t) = compass.single_mut() {
        t.0 = compass_strip(heading);
    }
    let (pos, vel, mode, agl, boost) = match players.single() {
        Ok((t, v, p)) => (t.translation, v.0, p.mode, p.agl, p.boost_stage),
        Err(_) => (ct.translation, Vec3::ZERO, MoveMode::Fall, 0.0, 0),
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
        let mach = vel.length() as f64 / ed_flight::atmosphere(g.alt_m).speed_of_sound;
        if mach > 0.8 {
            s.push_str(&format!("   MACH {mach:.2}"));
        }
        t.0 = s;
    }
    if let Ok(mut t) = dbg.single_mut() {
        let w = weather.and_then(|w| w.0.clone());
        let wtxt = w.map_or("weather n/a".to_string(), |w| format!("{} {:.0}°C wind {:.0} m/s", w.describe(), w.temperature_c, w.wind_speed_ms));
        let local_h = ((clock.unix_s / 3600.0 + g.lon_deg / 15.0).rem_euclid(24.0)) as f32;
        let tiles = mgr.map_or(String::new(), |m| {
            format!("tiles {} (+{})  bldgs {}  trees {}  tris {:.1}M", m.loaded.len(), m.pending.len(), m.totals.buildings, m.totals.trees, m.totals.triangles as f32 / 1e6)
        });
        t.0 = format!(
            "{:.5}, {:.5}  hdg {:.0}°  AGL {:.0} m  solar {:02}:{:02}  {}  {}  {:.0} fps",
            g.lat_deg,
            g.lon_deg,
            heading,
            agl,
            local_h as u32,
            ((local_h.fract()) * 60.0) as u32,
            wtxt,
            tiles,
            1.0 / time.delta_secs().max(1e-4)
        );
    }
}

#[allow(clippy::type_complexity)]
fn update_markers(
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
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
