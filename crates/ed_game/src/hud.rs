//! HUD: compass strip (top), mode pill (bottom), telemetry cards, debug stats,
//! and spaceflight target rings that track the Moon and planets across the sky
//! with name and distance.

use crate::menu::{GameState, GraphicsSettings};
use crate::origin::{Focus, WorldOrigin};
use crate::player::{MoveMode, Player};
use crate::sky::{CelestialBody, Clock, CurrentWeather};
use crate::streaming::{DetailManager, TileManager, detail_stats};
use avian3d::prelude::LinearVelocity;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::FontSize;
use bevy::window::{PrimaryWindow, Window};
use ed_geo::tile::TileId;
use ed_space::BodyId;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(
            Update,
            (
                update_text,
                paint_tape,
                update_markers,
                update_toast,
                stats_visibility,
                hud_visibility,
            ),
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
    CompassDeg,
    SpeedVal,
    SpeedSub,
    AltVal,
    AltSub,
    Mode,
    Stats,
}

#[derive(Component)]
struct HudTapeCanvas;

/// The compass strip is drawn into a canvas texture: ticks every 5°, mid and
/// major marks at 15°/45°, and the needle hanging from the top rim. Pixel
/// offsets track the heading, so the strip slides like a real tape.
#[derive(Resource)]
struct HudTape {
    handle: Handle<Image>,
    /// Last painted tape centre in canvas px; `None` forces a repaint.
    last_center: Option<f32>,
}

const TAPE_W_CSS: f32 = 560.0;
const TAPE_H_CSS: f32 = 30.0;
const TAPE_RES: u32 = 2;
/// 70° of heading across the strip.
const TAPE_PX_PER_DEG: f32 = TAPE_W_CSS / 70.0;

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

fn spawn_hud(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Top: compass tick-strip (a canvas texture) and the digital heading readout.
    let tape = images.add(Image::new_fill(
        Extent3d {
            width: TAPE_W_CSS as u32 * TAPE_RES,
            height: TAPE_H_CSS as u32 * TAPE_RES,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    commands.insert_resource(HudTape {
        handle: tape.clone(),
        last_center: None,
    });
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(14.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    width: Val::Px(TAPE_W_CSS),
                    height: Val::Px(TAPE_H_CSS + 12.0),
                    padding: UiRect::vertical(Val::Px(6.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(GLASS_BG),
                BorderColor::all(GLASS_BORDER),
            ))
            .with_children(|p| {
                p.spawn((
                    HudTapeCanvas,
                    Node {
                        width: Val::Px(TAPE_W_CSS),
                        height: Val::Px(TAPE_H_CSS),
                        ..default()
                    },
                    ImageNode {
                        image: tape,
                        ..default()
                    },
                ));
            });
            c.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(6.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(GLASS_BG),
                BorderColor::all(GLASS_BORDER),
            ))
            .with_children(|p| {
                p.spawn((
                    HudField::CompassDeg,
                    Text::new("000° N"),
                    TextFont {
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextColor(TEXT_PRIMARY),
                ));
            });
        });

    // Bottom-left: speed card with fixed-width columns so the layout never
    // reflows as the values change.
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                bottom: Val::Px(18.0),
                padding: UiRect::all(Val::Px(14.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                column_gap: Val::Px(14.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(GLASS_BG),
            BorderColor::all(GLASS_BORDER),
        ))
        .with_children(|p| {
            p.spawn((Node {
                width: Val::Px(104.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(0.0),
                ..default()
            },))
                .with_children(|col| {
                    col.spawn(Node {
                        column_gap: Val::Px(5.0),
                        align_items: AlignItems::Baseline,
                        ..default()
                    })
                    .with_children(|r| {
                        r.spawn((
                            HudField::SpeedVal,
                            Text::new("0"),
                            TextFont {
                                font_size: FontSize::Px(30.0),
                                ..default()
                            },
                            TextColor(TEXT_PRIMARY),
                        ));
                        r.spawn((
                            Text::new("MPH"),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(TEXT_MUTED),
                        ));
                    });
                    col.spawn((
                        Text::new("SPEED"),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(TEXT_MUTED),
                    ));
                });
            p.spawn((
                Node {
                    width: Val::Px(1.0),
                    height: Val::Px(46.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
            ));
            p.spawn((Node {
                width: Val::Px(84.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(0.0),
                ..default()
            },))
                .with_children(|col| {
                    col.spawn((
                        Text::new("MACH"),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(TEXT_MUTED),
                    ));
                    col.spawn((
                        HudField::SpeedSub,
                        Text::new("0.00"),
                        TextFont {
                            font_size: FontSize::Px(26.0),
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                    ));
                });
        });

    // Bottom-right, beside the circular minimap: altitude with the vertical
    // speed readout beneath it. Fixed width, no header.
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(crate::minimap::SIZE_PX + 30.0),
                bottom: Val::Px(18.0),
                padding: UiRect::all(Val::Px(14.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                width: Val::Px(236.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(GLASS_BG),
            BorderColor::all(GLASS_BORDER),
        ))
        .with_children(|p| {
            p.spawn(Node {
                column_gap: Val::Px(5.0),
                align_items: AlignItems::Baseline,
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    HudField::AltVal,
                    Text::new("0"),
                    TextFont {
                        font_size: FontSize::Px(30.0),
                        ..default()
                    },
                    TextColor(TEXT_PRIMARY),
                ));
                r.spawn((
                    Text::new("FT"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(TEXT_MUTED),
                ));
            });
            p.spawn(Node {
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    HudField::AltSub,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
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
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
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
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
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
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
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
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(Color::srgba(0.85, 0.95, 1.0, 0.95)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
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

fn thousands(v: i64) -> String {
    let s = v.abs().to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    for (i, &c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c as char);
    }
    if v < 0 { format!("-{out}") } else { out }
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
        Ok((t, v, p)) => (
            t.translation,
            v.0,
            p.mode,
            p.agl,
            p.boost_stage,
            p.wingsuit_on,
        ),
        Err(_) => (ct.translation, Vec3::ZERO, MoveMode::Fall, 0.0, 0, true),
    };
    let g = origin.to_geodetic(pos);
    let up_p = origin.up_at(pos);
    let mph = vel.length() * 2.236_94;
    let vs = vel.dot(up_p) * 2.236_94;
    let ft = g.alt_m * 3.280_84;
    let mach = vel.length() as f64 / ed_flight::atmosphere(g.alt_m).speed_of_sound;

    let deg_str = format!(
        "{:03.0}° {}",
        heading.rem_euclid(360.0),
        cardinal_label(heading)
    );
    let speed_val_str = format!("{:.0}", mph);
    let speed_sub_str = format!("{mach:.2}");
    let alt_val_str = thousands(ft as i64);
    let alt_sub_str = if vs.abs() > 2.5 {
        format!("{} {:.0} MPH", if vs < 0.0 { "↓" } else { "↑" }, vs.abs())
    } else {
        String::new()
    };
    let mode_str = {
        let mut s = mode.label().to_uppercase();
        if mode == MoveMode::RocketShoes {
            s.push_str(&format!(
                "  •  BOOST {} [SPACE]",
                ["I", "II", "III", "MAX"][boost]
            ));
        }
        if !matches!(mode, MoveMode::Walk | MoveMode::Parachute) {
            s.push_str(if wing_on {
                "  •  WINGSUIT READY [F]"
            } else {
                "  •  WINGSUIT OFF [F]"
            });
        }
        s
    };

    let mut stats_str = String::new();
    if gfx.show_stats {
        let local_h = ed_geo::sun::solar_hour(clock.unix_s, g.lon_deg) as f32;
        let tile = TileId::containing(g.lat_deg, g.lon_deg, 14);
        let w = weather.and_then(|w| w.0.clone());
        let wtxt = w.map_or("weather n/a".to_string(), |w| {
            format!(
                "{} {:.0}°C wind {:.0} m/s",
                w.describe(),
                w.temperature_c,
                w.wind_speed_ms
            )
        });
        let (tiles_loaded, tiles_pending, tiles_desired) = mgr.as_ref().map_or((0, 0, 0), |m| {
            (m.loaded.len(), m.pending.len(), m.desired.len())
        });
        let (p_io, p_build, p_apply) = mgr.as_ref().map_or((0, 0, 0), |m| m.pipeline());
        let sat_free = sat_pool.as_ref().map_or([0; 3], |p| p.free_layers());
        let (buildings, trees, tris) = mgr.as_ref().map_or((0, 0, 0), |m| {
            (m.totals.buildings, m.totals.trees, m.totals.triangles)
        });
        let (detail_loaded, detail_flight) = dm.as_ref().map_or((0, 0), |d| detail_stats(d));
        let (win_w, win_h, win_scale) = windows.single().map_or((0, 0, 1.0), |w| {
            (w.physical_width(), w.physical_height(), w.scale_factor())
        });
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

fn paint_tape(
    mut tape: ResMut<HudTape>,
    mut images: ResMut<Assets<Image>>,
    cams: Query<&Transform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    origin: Res<WorldOrigin>,
    vis: Query<&InheritedVisibility, With<HudTapeCanvas>>,
) {
    let Ok(v) = vis.single() else { return };
    if !v.get() {
        tape.last_center = None;
        return;
    }
    let Ok(cam) = cams.single() else { return };
    let up = origin.up_at(cam.translation);
    let fwd = cam.forward().as_vec3().reject_from_normalized(up);
    let enu = ed_geo::render_to_enu(fwd.as_dvec3());
    let heading = (enu.x.atan2(enu.y).to_degrees() as f32).rem_euclid(360.0);
    // Tape centre in canvas px; redraw once it moved half a CSS pixel.
    let center_px = heading * TAPE_PX_PER_DEG * TAPE_RES as f32;
    if let Some(prev) = tape.last_center {
        if (center_px - prev).abs() < 0.5 {
            return;
        }
    }
    tape.last_center = Some(center_px);
    let Some(mut img) = images.get_mut(&tape.handle) else {
        return;
    };
    let Some(data) = img.data.as_mut() else {
        return;
    };
    let w = (TAPE_W_CSS as u32 * TAPE_RES) as usize;
    let h = (TAPE_H_CSS as u32 * TAPE_RES) as usize;
    data.fill(0);
    let ppd = TAPE_PX_PER_DEG * TAPE_RES as f32;
    let (warn, mid, low) = (200u8, 150u8, 110u8);
    let d0 = (heading - 36.0).floor() as i32;
    for d in d0..=(heading + 36.0) as i32 {
        let (hh, alpha) = if d.rem_euclid(45) == 0 {
            (h as i32 - 8, warn)
        } else if d.rem_euclid(15) == 0 {
            (h as i32 - 26, mid)
        } else if d.rem_euclid(5) == 0 {
            (h as i32 - 44, low)
        } else {
            continue;
        };
        // Bottom-anchored tick at its heading offset from the tape centre.
        let x = (((d as f32 - heading) * ppd) + w as f32 * 0.5).round() as i32;
        for dx in 0..3 {
            let xx = x + dx;
            if xx < 0 || xx >= w as i32 {
                continue;
            }
            for yy in 0..hh {
                let y = h as i32 - hh + yy;
                if y >= 0 && y < h as i32 {
                    let at = (y as usize * w + xx as usize) * 4;
                    data[at] = 255;
                    data[at + 1] = 255;
                    data[at + 2] = 255;
                    data[at + 3] = alpha;
                }
            }
        }
    }
    // Needle: white wedge hanging from the top rim over the tape centre.
    for i in 0..14_i32 {
        let half = ((13 - i) / 2 + 1) as i32;
        for dx in -half..=half {
            let xx = w as i32 / 2 + dx;
            if (0..w as i32).contains(&xx) && i < h as i32 {
                let at = (i as usize * w + xx as usize) * 4;
                data[at] = 255;
                data[at + 1] = 255;
                data[at + 2] = 255;
                data[at + 3] = 235;
            }
        }
    }
}

fn stats_visibility(
    gfx: Res<GraphicsSettings>,
    mut roots: Query<&mut Visibility, With<StatsRoot>>,
) {
    let want = if gfx.show_stats {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
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
    let show =
        playing && !loading.is_some_and(|l| l.active) && time.elapsed_secs_f64() < notice.until_s;
    for mut v in &mut roots {
        v.set_if_neq(if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
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
    let Ok((cam, cgt)) = cams.single() else {
        return;
    };
    let observer = focus
        .iter()
        .next()
        .map(|f| f.translation())
        .unwrap_or(cgt.translation());
    let in_space = origin.to_geodetic(observer).alt_m >= crate::sky::SPACE_ALTITUDE_M;
    for (m, mut node, mut vis) in &mut rings {
        if !in_space {
            *vis = Visibility::Hidden;
            if let Some((_, _, _, mut lv)) = labels.iter_mut().find(|l| l.0.0 == m.0) {
                *lv = Visibility::Hidden;
            }
            continue;
        }
        let Some((_, bgt)) = bodies.iter().find(|(b, _)| b.0 == m.0) else {
            continue;
        };
        let p = bgt.translation();
        let in_front = (p - cgt.translation()).dot(cgt.forward().as_vec3()) > 0.0;
        match cam.world_to_viewport(cgt, p) {
            Ok(s) if in_front => {
                let s = s * (100.0 / gfx.render_pct as f32);
                node.left = Val::Px(s.x - 12.0);
                node.top = Val::Px(s.y - 12.0);
                *vis = Visibility::Visible;
                if let Some((_, mut ln, mut lt, mut lv)) = labels.iter_mut().find(|l| l.0.0 == m.0)
                {
                    ln.left = Val::Px(s.x + 18.0);
                    ln.top = Val::Px(s.y - 8.0);
                    let d = (p - cgt.translation()).length() as f64;
                    lt.0 = format!(
                        "{}  •  {}",
                        ed_space::info(m.0).name.to_uppercase(),
                        fmt_dist(d)
                    );
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
