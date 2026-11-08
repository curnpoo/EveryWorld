//! Loading screen for spawns and fast travel.
//!
//! Shown from the moment the player is placed until the wanted tile set is in and the native
//! upload queue has drained (or `EW_LOAD_TIMEOUT` seconds pass). While it is up the player is
//! held at the drop point, so the city is ready before the skydive starts instead of freezing
//! mid-fall.

use crate::menu::GameState;
use crate::origin::Focus;
use crate::player::Player;
use crate::spawn::SpawnSerial;
use crate::streaming::TileManager;
use avian3d::prelude::{AngularVelocity, LinearVelocity, Position, RigidBodyDisabled};
use bevy::prelude::*;
use bevy::text::{FontSize, Justify};

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingScreen>()
            .add_systems(Startup, spawn_ui)
            .add_systems(Update, (begin_on_arrival, update_loading, freeze_player, update_ui).chain());
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
/// Fade-in from black at the start.
const FADE_IN_S: f32 = 0.25;
/// Fade-out after release.
const FADE_S: f32 = 0.35;

#[derive(Component)]
struct LoadingRoot;
#[derive(Component)]
struct LoadingWelcomeRow;
#[derive(Component)]
struct LoadingWelcome;
#[derive(Component)]
struct LoadingName;
#[derive(Component)]
struct LoadingMessage;
#[derive(Component)]
struct LoadingStatus;
#[derive(Component)]
struct LoadingBar;
#[derive(Component)]
struct LoadingBarTrack;

/// Marker on the local player while the loading screen holds it still.
#[derive(Component)]
struct PlayerFrozen;

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
            GlobalZIndex(100),
        ))
        .with_children(|c| {
            c.spawn((
                LoadingWelcomeRow,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    max_width: Val::Percent(86.0),
                    ..default()
                },
            ))
            .with_children(|head| {
                head.spawn((
                    LoadingWelcome,
                    Text::new("Welcome to"),
                    TextFont { font_size: FontSize::Px(32.0), ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.92)),
                    TextLayout::justify(Justify::Center),
                ));
                head.spawn((
                    LoadingName,
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(48.0), ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 1.0)),
                    TextLayout::justify(Justify::Center),
                ));
            });
            c.spawn((
                LoadingMessage,
                Text::new("Loading the world"),
                TextFont { font_size: FontSize::Px(18.0), ..default() },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.78)),
                TextLayout::justify(Justify::Center),
                Node { margin: UiRect::top(Val::Px(14.0)), ..default() },
            ));
            c.spawn((
                LoadingBarTrack,
                Node { width: Val::Px(320.0), height: Val::Px(3.0), ..default() },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
            ))
            .with_children(|b| {
                b.spawn((
                    LoadingBar,
                    Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(Color::srgb(0.85, 0.62, 0.32)),
                ));
            });
            c.spawn((
                LoadingStatus,
                Text::new(""),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgba(0.78, 0.82, 0.88, 0.62)),
                TextLayout::justify(Justify::Center),
            ));
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
    screen.status = "Streaming the world";
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
    notice: Option<Res<crate::hud::ArrivalNotice>>,
    players: Query<&Transform, With<Player>>,
    mut cams: Query<&mut Transform, (With<Camera3d>, Without<Player>, Without<crate::globe::GlobeCamera>)>,
) {
    if !screen.active {
        return;
    }
    // `ArrivalNotice` is inserted in the same command flush as the spawn serial, so it can
    // land a frame after the screen opens. Pick the name up as soon as it exists.
    if screen.name.is_empty() {
        if let Some(n) = notice.as_ref() {
            screen.name = n.name.clone();
        }
    }
    let now = time.elapsed_secs();
    let playing = *state.get() == GameState::Playing;
    let timeout: f32 = std::env::var("EW_LOAD_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(60.0);

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
        "Streaming the world"
    } else if queued > 0 {
        "Finishing geometry"
    } else {
        "Ready"
    };

    // Hold the fly camera at the drop point. The player body is held by
    // `freeze_player` so only the local avatar is frozen, not the world.
    let drift = players.single().map_or(0.0, |t| screen.anchor.map_or(0.0, |a| a.distance(t.translation)));
    if players.single().is_err() {
        if let (Some(anchor), Ok(mut c)) = (screen.anchor, cams.single_mut()) {
            c.translation = anchor;
        }
    }

    if screen.released.is_none() && elapsed > MIN_SHOW_S && ((ready && playing) || elapsed > timeout || !playing) {
        screen.released = Some(now);
        screen.status = if ready { "Ready" } else { "Still streaming" };
        info!("loading screen released after {elapsed:.1}s ({:.0}% tiles, {queued} queued, player drift {drift:.2} m) - {}", tiles * 100.0, if ready { "ready" } else { "timeout" });
    }
    if let Some(r) = screen.released {
        if now - r > FADE_S {
            screen.active = false;
            screen.anchor = None;
        }
    }
}

/// Freeze only the local player while the screen is up: pin the drop point, zero
/// its velocity, and take its rigid body out of the simulation. Other bodies
/// (including remote players in multiplayer) keep simulating normally.
fn freeze_player(
    mut commands: Commands,
    screen: Res<LoadingScreen>,
    mut q: Query<
        (
            Entity,
            &mut Transform,
            &mut Player,
            &mut LinearVelocity,
            &mut Position,
            Option<&mut AngularVelocity>,
            Option<&PlayerFrozen>,
        ),
        With<Player>,
    >,
) {
    let Ok((e, mut t, mut p, mut vel, mut pos, ang, frozen)) = q.single_mut() else { return };
    if screen.active {
        if let Some(anchor) = screen.anchor {
            t.translation = anchor;
            pos.0 = anchor;
        }
        vel.0 = Vec3::ZERO;
        if let Some(mut a) = ang {
            a.0 = Vec3::ZERO;
        }
        p.prev_vel = Vec3::ZERO;
        p.accel_smooth = Vec3::ZERO;
        if frozen.is_none() {
            commands.entity(e).insert((RigidBodyDisabled, PlayerFrozen));
        }
    } else if frozen.is_some() {
        commands.entity(e).remove::<RigidBodyDisabled>().remove::<PlayerFrozen>();
        vel.0 = Vec3::ZERO;
        p.prev_vel = Vec3::ZERO;
    }
}

fn status_line(screen: &LoadingScreen) -> String {
    let pct = (screen.progress * 100.0).round() as i32;
    if screen.progress < 0.999 {
        format!("{} {}%", screen.status, pct)
    } else {
        screen.status.to_string()
    }
}

#[allow(clippy::type_complexity)]
fn update_ui(
    time: Res<Time<Real>>,
    cfg: Res<crate::config::Config>,
    state: Res<State<GameState>>,
    screen: Res<LoadingScreen>,
    mut roots: Query<(&mut BackgroundColor, &mut Visibility), With<LoadingRoot>>,
    mut welcome_rows: Query<&mut Visibility, (With<LoadingWelcomeRow>, Without<LoadingRoot>)>,
    mut texts: Query<(
        &mut Text,
        &mut TextColor,
        Option<&LoadingWelcome>,
        Option<&LoadingName>,
        Option<&LoadingMessage>,
        Option<&LoadingStatus>,
    )>,
    mut tracks: Query<&mut BackgroundColor, (With<LoadingBarTrack>, Without<LoadingRoot>, Without<LoadingBar>)>,
    mut bars: Query<(&mut Node, &mut BackgroundColor), (With<LoadingBar>, Without<LoadingRoot>, Without<LoadingBarTrack>)>,
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
    let fade_in = ((now - screen.started) / FADE_IN_S).clamp(0.0, 1.0);
    let fade_out = screen.released.map_or(1.0, |r| (1.0 - (now - r) / FADE_S).clamp(0.0, 1.0));
    let a = fade_in.min(fade_out);
    let alpha = a * a * (3.0 - 2.0 * a);
    bg.0 = Color::srgba(0.02, 0.03, 0.05, 0.92 * alpha);

    for mut v in &mut welcome_rows {
        v.set_if_neq(if screen.name.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
    }
    for (mut t, mut color, welcome, name, message, status) in &mut texts {
        let (text, rgba) = if welcome.is_some() {
            (Some("Welcome to".to_string()), [1.0, 1.0, 1.0, 0.92])
        } else if name.is_some() {
            (Some(screen.name.clone()), [1.0, 1.0, 1.0, 1.0])
        } else if message.is_some() {
            (None, [1.0, 1.0, 1.0, 0.78])
        } else if status.is_some() {
            (Some(status_line(&screen)), [0.78, 0.82, 0.88, 0.62])
        } else {
            continue;
        };
        if let Some(s) = text {
            if t.0 != s {
                t.0 = s;
            }
        }
        color.0 = Color::srgba(rgba[0], rgba[1], rgba[2], rgba[3] * alpha);
    }
    for mut c in &mut tracks {
        c.0 = Color::srgba(1.0, 1.0, 1.0, 0.12 * alpha);
    }
    if let Ok((mut n, mut c)) = bars.single_mut() {
        n.width = Val::Percent((screen.progress * 100.0).clamp(0.0, 100.0));
        c.0 = Color::srgba(0.85, 0.62, 0.32, alpha);
    }
}
