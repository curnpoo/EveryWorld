//! Loading screen for spawns and fast travel.
//!
//! Shown from the moment the player is placed until the wanted tile set is in and the native
//! upload queue has drained (or `EW_LOAD_TIMEOUT` seconds pass). While it is up the player is
//! held at the drop point, so the city is ready before the skydive starts instead of freezing
//! mid-fall.

use crate::menu::GameState;
use crate::origin::{Focus, WorldOrigin};
use crate::player::Player;
use crate::spawn::SpawnSerial;
use crate::streaming::TileManager;
use avian3d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;
use bevy::text::FontSize;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingScreen>()
            .add_systems(Startup, spawn_ui)
            .add_systems(Update, (begin_on_arrival, update_loading, update_ui).chain());
    }
}

#[derive(Resource, Default)]
pub struct LoadingScreen {
    pub active: bool,
    serial: u64,
    started: f32,
    released: Option<f32>,
    name: String,
    status: &'static str,
    progress: f32,
    anchor: Option<Vec3>,
}

/// Minimum time on screen so a fast load doesn't flash.
const MIN_SHOW_S: f32 = 0.6;
/// Fade-out after release.
const FADE_S: f32 = 0.35;

#[derive(Component)]
struct LoadingRoot;
#[derive(Component)]
struct LoadingName;
#[derive(Component)]
struct LoadingStatus;
#[derive(Component)]
struct LoadingBar;

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            LoadingRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.0)),
            Visibility::Hidden,
            ZIndex(100),
        ))
        .with_children(|c| {
            c.spawn((LoadingName, Text::new(""), TextFont { font_size: FontSize::Px(28.0), ..default() }, TextColor(Color::srgba(1.0, 1.0, 1.0, 0.94))));
            c.spawn((
                Node { width: Val::Px(260.0), height: Val::Px(3.0), ..default() },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
            ))
            .with_children(|b| {
                b.spawn((
                    LoadingBar,
                    Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(Color::srgb(0.85, 0.62, 0.32)),
                ));
            });
            c.spawn((LoadingStatus, Text::new(""), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(Color::srgba(0.78, 0.82, 0.88, 0.8))));
        });
}

fn begin_on_arrival(
    time: Res<Time<Real>>,
    serial: Option<Res<SpawnSerial>>,
    notice: Option<Res<crate::hud::ArrivalNotice>>,
    mut screen: ResMut<LoadingScreen>,
    players: Query<&Transform, (With<Player>, With<Focus>)>,
    cams: Query<&Transform, (With<Camera3d>, With<Focus>, Without<Player>)>,
) {
    let Some(s) = serial else { return };
    if s.0 == screen.serial {
        return;
    }
    screen.serial = s.0;
    screen.active = true;
    screen.released = None;
    screen.started = time.elapsed_secs();
    screen.progress = 0.0;
    screen.status = "Streaming the city";
    screen.name = notice.map(|n| n.name.clone()).unwrap_or_default();
    screen.anchor = players.iter().next().or_else(|| cams.iter().next()).map(|t| t.translation);
    info!("loading screen: {} (spawn {})", if screen.name.is_empty() { "spawn" } else { &screen.name }, s.0);
}

#[allow(clippy::too_many_arguments)]
fn update_loading(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    mut screen: ResMut<LoadingScreen>,
    mgr: Option<Res<TileManager>>,
    mut players: Query<(&mut Transform, Option<&mut LinearVelocity>, Option<&mut Position>), With<Player>>,
    mut cams: Query<&mut Transform, (With<Camera3d>, Without<Player>, Without<crate::globe::GlobeCamera>)>,
) {
    if !screen.active {
        return;
    }
    let now = time.elapsed_secs();
    let playing = *state.get() == GameState::Playing;
    let timeout: f32 = std::env::var("EW_LOAD_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(25.0);

    // Progress: wanted tiles in, then the geometry upload queue.
    let (tiles, got, want) = mgr.as_ref().map_or((0.0, 0, 0), |m| {
        let want = m.desired.len();
        let got = m.desired.iter().filter(|t| m.loaded.contains_key(*t)).count();
        (if want == 0 { 0.0 } else { got as f32 / want as f32 }, got, want)
    });
    let queued = crate::probes::readyq() + crate::probes::prepq();
    let ready = playing && want > 0 && got == want && queued == 0;
    let elapsed = now - screen.started;
    screen.progress = tiles;
    screen.status = if want == 0 || got < want {
        "Streaming the city"
    } else if queued > 0 {
        "Finishing geometry"
    } else {
        "Ready"
    };

    // Hold the drop point so the player does not fall into a half-built city.
    if let Some(anchor) = screen.anchor {
        if let Ok((mut t, vel, pos)) = players.single_mut() {
            t.translation = anchor;
            if let Some(mut v) = vel {
                v.0 = Vec3::ZERO;
            }
            if let Some(mut p) = pos {
                p.0 = anchor;
            }
        } else if let Ok(mut c) = cams.single_mut() {
            c.translation = anchor;
        }
    }

    if screen.released.is_none() && elapsed > MIN_SHOW_S && ((ready && playing) || elapsed > timeout || !playing) {
        screen.released = Some(now);
        screen.status = if ready { "Ready" } else { "Still streaming" };
        info!("loading screen released after {elapsed:.1}s ({:.0}% tiles, {queued} queued) - {}", tiles * 100.0, if ready { "ready" } else { "timeout" });
    }
    if let Some(r) = screen.released {
        if now - r > FADE_S {
            screen.active = false;
            screen.anchor = None;
        }
    }
}

fn update_ui(
    time: Res<Time<Real>>,
    cfg: Res<crate::config::Config>,
    state: Res<State<GameState>>,
    screen: Res<LoadingScreen>,
    mut roots: Query<(&mut BackgroundColor, &mut Visibility), With<LoadingRoot>>,
    mut names: Query<&mut Text, (With<LoadingName>, Without<LoadingStatus>)>,
    mut status: Query<&mut Text, (With<LoadingStatus>, Without<LoadingName>)>,
    mut bars: Query<&mut Node, (With<LoadingBar>, Without<LoadingRoot>)>,
) {
    let Ok((mut bg, mut vis)) = roots.single_mut() else { return };
    let show = screen.active
        && *state.get() == GameState::Playing
        && (cfg.screenshot.is_none() || std::env::var_os("EW_LOAD_SHOT").is_some());
    if !show {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Inherited;
    let now = time.elapsed_secs();
    let alpha = screen.released.map_or(1.0, |r| (1.0 - (now - r) / FADE_S).clamp(0.0, 1.0));
    bg.0 = Color::srgba(0.02, 0.03, 0.05, 0.92 * alpha);
    if let Ok(mut t) = names.single_mut() {
        if t.0 != screen.name {
            t.0 = screen.name.clone();
        }
    }
    if let Ok(mut t) = status.single_mut() {
        let pct = (screen.progress * 100.0).round() as i32;
        let line = if screen.progress < 0.999 {
            format!("{} {}%", screen.status, pct)
        } else {
            screen.status.to_string()
        };
        if t.0 != line {
            t.0 = line;
        }
    }
    if let Ok(mut n) = bars.single_mut() {
        n.width = Val::Percent((screen.progress * 100.0).clamp(0.0, 100.0));
    }
}
