//! Main menu and pause menu.
//!
//! Main menu (on launch): just Play. It opens the same tabbed screen as the
//! pause menu, starting on the map; click a spot there and choose sky or
//! ground to start the 3D world.
//!
//! Pause (Esc while playing): fullscreen menu with tabs: Map (teleport
//! anywhere), Settings, Character, Controls. Physics freezes while paused.

use crate::body::Cosmetics;
use crate::clouds::CloudOverride;
use crate::config::Config;
use crate::sky::{Clock, CurrentWeather};
use crate::mapview::{MapCanvas, MapState};
use crate::player::{ControlSettings, Player};
use crate::spawn::{Arrived, Spawner};
use avian3d::prelude::{Physics, PhysicsTime};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::light::DirectionalLight;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::post_process::bloom::Bloom;
use bevy::light::ShadowFilteringMethod;
use bevy::pbr::ContactShadows;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<MenuView>()
            .init_resource::<GraphicsSettings>()
            .init_resource::<WeatherPick>()
            .init_resource::<CurrentWeather>()
            // --fly skips PlayerPlugin/BodyPlugin; the menu still reads these.
            .init_resource::<ControlSettings>()
            .init_resource::<Cosmetics>()
            .add_systems(Startup, (build_menus, skip_menu_from_cli).chain())
            .add_systems(
                Update,
                (
                    open_pause,
                    menu_buttons,
                    refresh_menu,
                    time_slider,
                    enter_play_on_arrival,
                    apply_graphics,
                )
                    .chain(),
            )
            .add_systems(Startup, pause_physics)
            .add_systems(OnEnter(GameState::Playing), (capture_cursor, unpause_physics))
            .add_systems(OnExit(GameState::Playing), (release_cursor, pause_physics));
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Menu,
    Playing,
    Paused,
}

/// Which screen/tab of the menus is showing.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuView {
    #[default]
    Home,
    Map,
    Settings,
    Character,
    Controls,
}

#[derive(Resource, Clone)]
pub struct GraphicsSettings {
    pub shadows: bool,
    pub ambient_occlusion: bool,
    pub bloom: bool,
    /// Render resolution as a percentage of the display's native pixels.
    pub render_pct: u32,
    /// Temporal anti-aliasing.
    pub aa: bool,
    /// Extended debug/performance stats in the HUD.
    pub show_stats: bool,
    /// The clock was last set to real current time by the "Now" button.
    pub time_live: bool,
    pub hour_offset: f64,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        let post = !std::env::args().any(|a| a == "--no-post");
        let mut g = Self { shadows: !std::env::args().any(|a| a == "--no-shadows"), ambient_occlusion: post, bloom: post, render_pct: 67, aa: post, show_stats: false, time_live: false, hour_offset: 0.0 };
        // Perf bisecting: `EW_GFX=ao=0,bloom=0,aa=0,shadows=0,res=50,stats=1`.
        if let Ok(spec) = std::env::var("EW_GFX") {
            for kv in spec.split(',') {
                let Some((k, v)) = kv.split_once('=') else { continue };
                let b = v != "0";
                match k {
                    "ao" => g.ambient_occlusion = b,
                    "bloom" => g.bloom = b,
                    "aa" => g.aa = b,
                    "shadows" => g.shadows = b,
                    "stats" => g.show_stats = b,
                    "res" => g.render_pct = v.parse().unwrap_or(g.render_pct),
                    _ => {}
                }
            }
        }
        g
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Play,
    Quit,
    Resume,
    ToMainMenu,
    Tab(MenuView),
    ToggleShadows,
    ToggleAo,
    ToggleBloom,
    CycleResolution,
    ToggleAa,
    ToggleStats,
    TimeMinus,
    TimePlus,
    TimeNow,
    SetWeather(&'static str),
    SensMinus,
    SensPlus,
    InvertY,
    ToggleBackpack,
}

/// The weather picker's state: which preset is forced, and the live report to
/// restore when the player goes back to "Live".
#[derive(Resource)]
struct WeatherPick {
    name: &'static str,
    live: Option<Option<ed_data::weather::Weather>>,
}

impl Default for WeatherPick {
    fn default() -> Self {
        Self { name: "Live", live: None }
    }
}

/// The draggable track of the time-of-day slider (local solar hour 0..24).
#[derive(Component)]
struct TimeTrack;
#[derive(Component)]
struct TimeKnob;
#[derive(Component)]
struct TimeReadout;

/// Root of each screen; shown/hidden by `MenuView` and `GameState`.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Pause,
}

/// A tab body inside the pause menu.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct TabBody(MenuView);

/// The pause screen's heading ("Paused" in game, the game name from the main menu).
#[derive(Component)]
struct PauseTitle;

/// Only shown when there is a game to go back to.
#[derive(Component)]
struct ResumeButton;

#[derive(Component)]
struct StatusLine;

#[derive(Component)]
struct SettingValue(MenuAction);

const INK: Color = Color::srgb(0.96, 0.96, 0.95);
const DIM: Color = Color::srgba(0.96, 0.96, 0.95, 0.6);
const ORANGE: Color = Color::srgb(0.98, 0.45, 0.12);
const PANEL: Color = Color::srgba(0.06, 0.07, 0.09, 0.9);
const BTN: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);
const BTN_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.16);

/// A menu button. `accent` makes it the orange primary action.
#[allow(deprecated)]
pub fn button<A: Component>(p: &mut ChildSpawnerCommands, label: &str, action: A, accent: bool) {
    p.spawn((
        Interaction::None,
        action,
        Node {
            padding: UiRect::axes(Val::Px(18.0), Val::Px(11.0)),
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(if accent { ORANGE } else { BTN }),
    ))
    .with_children(|b| {
        b.spawn((Text::new(label), TextFont { font_size: FontSize::Px(17.0), ..default() }, TextColor(INK)));
    });
}

fn text(p: &mut ChildSpawnerCommands, s: &str, size: f32, color: Color) {
    p.spawn((Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color)));
}

fn map_canvas(p: &mut ChildSpawnerCommands) {
    p.spawn((
        MapCanvas,
        RelativeCursorPosition::default(),
        Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_height: Val::Px(240.0), width: Val::Percent(100.0), overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(14.0)), ..default() },
        BackgroundColor(Color::srgb(0.01, 0.012, 0.02)),
    ));
}

fn build_menus(mut commands: Commands) {
    // ---- Home ----
    commands
        .spawn((
            Screen::Home,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(56.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.55)),
            ZIndex(10),
        ))
        .with_children(|p| {
            text(p, "EveryWorld", 64.0, INK);
            text(p, "The whole real Earth. Drop in anywhere.", 18.0, DIM);
            p.spawn(Node { height: Val::Px(18.0), ..default() });
            p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), width: Val::Px(380.0), ..default() }).with_children(|c| {
                button(c, "Play", MenuAction::Play, true);
                button(c, "Quit", MenuAction::Quit, false);
            });
        });

    // ---- Pause: tabs ----
    commands
        .spawn((
            Screen::Pause,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(PANEL),
            Visibility::Hidden,
            ZIndex(12),
        ))
        .with_children(|p| {
            p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                r.spawn((PauseTitle, Text::new("Paused"), TextFont { font_size: FontSize::Px(26.0), ..default() }, TextColor(INK)));
                r.spawn(Node { width: Val::Px(16.0), ..default() });
                for (label, v) in [("Map", MenuView::Map), ("Settings", MenuView::Settings), ("Character", MenuView::Character), ("Controls", MenuView::Controls)] {
                    button(r, label, MenuAction::Tab(v), false);
                }
                r.spawn(Node { flex_grow: 1.0, ..default() });
                button(r, "Main menu", MenuAction::ToMainMenu, false);
                r.spawn((ResumeButton, Node::default())).with_children(|b| button(b, "Resume", MenuAction::Resume, true));
            });
            // Map tab.
            p.spawn((TabBody(MenuView::Map), Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), ..default() })).with_children(|t| {
                t.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                    text(r, "Drag to move, scroll to zoom, click anywhere to choose where to drop in.", 15.0, DIM);
                    r.spawn((StatusLine, Text::new(""), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(ORANGE)));
                });
                map_canvas(t);
            });
            // Settings tab.
            p.spawn((TabBody(MenuView::Settings), Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), ..default() }, Visibility::Hidden)).with_children(|t| {
                for (label, a) in [
                    ("Shadows", MenuAction::ToggleShadows),
                    ("Ambient occlusion", MenuAction::ToggleAo),
                    ("Bloom", MenuAction::ToggleBloom),
                    ("Anti-aliasing", MenuAction::ToggleAa),
                    ("Debug stats", MenuAction::ToggleStats),
                ] {
                    setting_row(t, label, &[("Toggle", a)], a);
                }
                setting_row(t, "Resolution", &[("Change", MenuAction::CycleResolution)], MenuAction::CycleResolution);
                setting_row(t, "Time of day", &[("-1 h", MenuAction::TimeMinus), ("+1 h", MenuAction::TimePlus), ("Now", MenuAction::TimeNow)], MenuAction::TimePlus);
                t.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                    r.spawn(Node { width: Val::Px(220.0), ..default() }).with_children(|c| text(c, "Local solar time", 17.0, INK));
                    r.spawn((TimeReadout, Text::new(""), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(ORANGE), Node { width: Val::Px(90.0), ..default() }));
                    r.spawn((
                        TimeTrack,
                        RelativeCursorPosition::default(),
                        Node { width: Val::Px(320.0), height: Val::Px(14.0), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
                        BackgroundColor(BTN),
                    ))
                    .with_children(|tr| {
                        tr.spawn((
                            TimeKnob,
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(-4.0),
                                left: Val::Percent(50.0),
                                width: Val::Px(22.0),
                                height: Val::Px(22.0),
                                margin: UiRect::left(Val::Px(-11.0)),
                                border_radius: BorderRadius::all(Val::Px(11.0)),
                                ..default()
                            },
                            BackgroundColor(ORANGE),
                        ));
                    });
                });
                setting_row(
                    t,
                    "Weather",
                    &[
                        ("Live", MenuAction::SetWeather("Live")),
                        ("Clear", MenuAction::SetWeather("Clear")),
                        ("Fair", MenuAction::SetWeather("Fair")),
                        ("Cloudy", MenuAction::SetWeather("Cloudy")),
                        ("Rain", MenuAction::SetWeather("Rain")),
                        ("Storm", MenuAction::SetWeather("Storm")),
                        ("Fog", MenuAction::SetWeather("Fog")),
                    ],
                    MenuAction::SetWeather("Live"),
                );
                setting_row(t, "Mouse sensitivity", &[("-", MenuAction::SensMinus), ("+", MenuAction::SensPlus)], MenuAction::SensPlus);
                setting_row(t, "Invert mouse Y", &[("Toggle", MenuAction::InvertY)], MenuAction::InvertY);
            });
            // Character tab.
            p.spawn((TabBody(MenuView::Character), Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), ..default() }, Visibility::Hidden)).with_children(|t| {
                text(t, "Courier robot", 22.0, INK);
                setting_row(t, "Parcel backpack", &[("Toggle", MenuAction::ToggleBackpack)], MenuAction::ToggleBackpack);
                text(t, "More customization coming: jackets, colors, screens for the CRT head.", 14.0, DIM);
            });
            // Controls tab.
            p.spawn((TabBody(MenuView::Controls), Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }, Visibility::Hidden)).with_children(|t| {
                for line in [
                    "Mouse: look around",
                    "W A S D: move",
                    "Space: jump",
                    "Shift: sprint on the ground",
                    "Shift in the air: rocket boots toward where you look (WASD steers)",
                    "Shift + Space in the air: boost harder (keep holding to climb stages)",
                    "F: toggle wingsuit (look to carve turns, W dive, S flare)",
                    "P: parachute while falling",
                    "C: crouch",
                    "V: first / third person",
                    "Scroll: camera distance",
                    "[ and ]: scrub time of day",
                    "Esc: pause menu",
                ] {
                    text(t, line, 17.0, INK);
                }
            });
        });
}

fn setting_row(p: &mut ChildSpawnerCommands, label: &str, buttons: &[(&str, MenuAction)], value_of: MenuAction) {
    p.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
        r.spawn(Node { width: Val::Px(220.0), ..default() }).with_children(|c| text(c, label, 17.0, INK));
        r.spawn((SettingValue(value_of), Text::new(""), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(ORANGE), Node { width: Val::Px(90.0), ..default() }));
        for (l, a) in buttons {
            button(r, l, *a, false);
        }
    });
}

/// Launching with --place / --lat / --fly skips the menu (handy for testing).
/// `--menu-view map|pause-map|settings|controls|character` opens a screen
/// directly (for screenshots).
fn skip_menu_from_cli(cfg: Res<Config>, mut next: ResMut<NextState<GameState>>, mut view: ResMut<MenuView>, mut map: ResMut<MapState>) {
    let args: Vec<String> = std::env::args().collect();
    let menu_view = args.iter().position(|a| a == "--menu-view").and_then(|i| args.get(i + 1)).cloned();
    let zoom = args.iter().position(|a| a == "--map-zoom").and_then(|i| args.get(i + 1)).and_then(|z| z.parse().ok()).unwrap_or(crate::globe::WHOLE_EARTH_ZOOM);
    map.focus(cfg.start, zoom);
    match menu_view.as_deref() {
        Some("map") => *view = MenuView::Map,
        Some(v @ ("pause-map" | "settings" | "controls" | "character")) => {
            *view = match v {
                "settings" => MenuView::Settings,
                "controls" => MenuView::Controls,
                "character" => MenuView::Character,
                _ => MenuView::Map,
            };
            next.set(GameState::Paused);
        }
        _ => {
            if cfg.fly || args.iter().any(|a| a == "--place" || a == "--lat" || a == "--play") {
                next.set(GameState::Playing);
            }
        }
    }
}

fn open_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut view: ResMut<MenuView>,
    mut map: ResMut<MapState>,
    players: Query<&Transform, With<Player>>,
    origin: Res<crate::origin::WorldOrigin>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match state.get() {
        GameState::Playing => {
            *view = MenuView::Map;
            if let Ok(pt) = players.single() {
                map.focus(origin.to_geodetic(pt.translation), crate::globe::PAUSE_ZOOM);
            }
            next.set(GameState::Paused);
        }
        GameState::Paused => next.set(GameState::Playing),
        GameState::Menu => *view = MenuView::Home,
    }
}

#[allow(deprecated, clippy::too_many_arguments)]
fn menu_buttons(
    mut q: Query<(&Interaction, &MenuAction, &mut BackgroundColor), Changed<Interaction>>,
    mut view: ResMut<MenuView>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    mut gfx: ResMut<GraphicsSettings>,
    mut controls: ResMut<ControlSettings>,
    mut cosmetics: ResMut<Cosmetics>,
    mut map: ResMut<MapState>,
    origin: Res<crate::origin::WorldOrigin>,
    players: Query<&Transform, With<Player>>,
    clock: Res<Clock>,
    mut weather: ResMut<CurrentWeather>,
    mut pick: ResMut<WeatherPick>,
    mut over: ResMut<CloudOverride>,
) {
    for (i, a, mut bg) in &mut q {
        let accent = matches!(a, MenuAction::Play | MenuAction::Resume);
        match i {
            Interaction::Hovered => {
                if !accent {
                    bg.0 = BTN_HOVER;
                }
                continue;
            }
            Interaction::None => {
                if !accent {
                    bg.0 = BTN;
                }
                continue;
            }
            Interaction::Pressed => {}
        }
        match a {
            MenuAction::Play => *view = MenuView::Map,
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
            MenuAction::Resume => next.set(GameState::Playing),
            MenuAction::ToMainMenu => {
                *view = MenuView::Home;
                next.set(GameState::Menu);
            }
            MenuAction::Tab(t) => {
                *view = *t;
                if *t == MenuView::Map {
                    if let Ok(pt) = players.single() {
                        map.focus(origin.to_geodetic(pt.translation), crate::globe::PAUSE_ZOOM);
                    }
                }
            }
            MenuAction::ToggleShadows => gfx.shadows = !gfx.shadows,
            MenuAction::ToggleAo => gfx.ambient_occlusion = !gfx.ambient_occlusion,
            MenuAction::ToggleBloom => gfx.bloom = !gfx.bloom,
            MenuAction::ToggleAa => gfx.aa = !gfx.aa,
            MenuAction::ToggleStats => gfx.show_stats = !gfx.show_stats,
            MenuAction::CycleResolution => gfx.render_pct = match gfx.render_pct { 100 => 90, 90 => 85, 85 => 75, 75 => 67, 67 => 50, _ => 100 },
            MenuAction::TimeMinus => {
                gfx.time_live = false;
                gfx.hour_offset -= 1.0;
            }
            MenuAction::TimePlus => {
                gfx.time_live = false;
                gfx.hour_offset += 1.0;
            }
            MenuAction::TimeNow => {
                let real = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(clock.unix_s);
                gfx.hour_offset += (real - clock.unix_s) / 3600.0;
                gfx.time_live = true;
            }
            MenuAction::SetWeather(name) => {
                if *name == "Live" {
                    if let Some(live) = pick.live.take() {
                        weather.0 = live;
                    }
                } else {
                    if pick.live.is_none() {
                        pick.live = Some(weather.0.clone());
                    }
                    let preset = match *name {
                        "Cloudy" => "overcast",
                        other => &other.to_lowercase(),
                    };
                    if let Some(w) = ed_world::clouds::weather_preset(preset) {
                        weather.0 = Some(w);
                    }
                }
                pick.name = name;
                // Snap the clouds to the new weather instead of easing over minutes.
                over.set_changed();
            }
            MenuAction::SensMinus => controls.mouse_sensitivity = (controls.mouse_sensitivity - 0.1).max(0.2),
            MenuAction::SensPlus => controls.mouse_sensitivity = (controls.mouse_sensitivity + 0.1).min(3.0),
            MenuAction::InvertY => controls.invert_y = !controls.invert_y,
            MenuAction::ToggleBackpack => cosmetics.backpack = !cosmetics.backpack,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_menu(
    state: Res<State<GameState>>,
    view: Res<MenuView>,
    spawner: Res<Spawner>,
    gfx: Res<GraphicsSettings>,
    pick: Res<WeatherPick>,
    controls: Res<ControlSettings>,
    cosmetics: Res<Cosmetics>,
    mut screens: Query<(&Screen, &mut Visibility), Without<TabBody>>,
    mut tabs: Query<(&TabBody, &mut Visibility, &mut Node), (Without<Screen>, Without<ResumeButton>)>,
    mut status: Query<&mut Text, (With<StatusLine>, Without<PauseTitle>, Without<SettingValue>)>,
    mut title: Query<&mut Text, (With<PauseTitle>, Without<StatusLine>, Without<SettingValue>)>,
    mut resume: Query<&mut Node, (With<ResumeButton>, Without<TabBody>)>,
    mut values: Query<(&SettingValue, &mut Text), (Without<PauseTitle>, Without<StatusLine>)>,
) {
    let st = *state.get();
    for (s, mut v) in &mut screens {
        let show = match s {
            Screen::Home => st == GameState::Menu && *view == MenuView::Home,
            Screen::Pause => st == GameState::Paused || (st == GameState::Menu && *view != MenuView::Home),
        };
        v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    for (t, mut v, mut node) in &mut tabs {
        let on = t.0 == *view;
        v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
        // Hidden tabs must also leave the layout, or they steal the map's height.
        let d = if on { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
    let in_game = st == GameState::Paused;
    for mut t in &mut title {
        let want = if in_game { "Paused" } else { "EveryWorld" };
        if t.0 != want {
            t.0 = want.to_string();
        }
    }
    for mut n in &mut resume {
        let d = if in_game { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    for mut s in &mut status {
        if s.0 != spawner.status {
            s.0 = spawner.status.clone();
        }
    }
    let on = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    for (sv, mut t) in &mut values {
        t.0 = match sv.0 {
            MenuAction::ToggleShadows => on(gfx.shadows),
            MenuAction::ToggleAo => on(gfx.ambient_occlusion),
            MenuAction::ToggleBloom => on(gfx.bloom),
            MenuAction::ToggleAa => on(gfx.aa),
            MenuAction::CycleResolution => format!("{}%", gfx.render_pct),
            MenuAction::ToggleStats => on(gfx.show_stats),
            MenuAction::TimePlus => if gfx.time_live { "Now".into() } else { "Custom".into() },
            MenuAction::SetWeather(_) => pick.name.to_string(),
            MenuAction::SensPlus => format!("{:.1}", controls.mouse_sensitivity),
            MenuAction::InvertY => on(controls.invert_y),
            MenuAction::ToggleBackpack => on(cosmetics.backpack),
            _ => String::new(),
        };
    }
}

/// Drag the time-of-day slider: the knob shows the player's local solar hour,
/// and dragging shifts the game clock (via `hour_offset`) by the shortest way
/// round to the hour under the cursor.
#[allow(clippy::too_many_arguments)]
fn time_slider(
    state: Res<State<GameState>>,
    buttons: Res<ButtonInput<MouseButton>>,
    clock: Res<Clock>,
    origin: Res<crate::origin::WorldOrigin>,
    players: Query<&Transform, With<Player>>,
    mut gfx: ResMut<GraphicsSettings>,
    track: Query<(&RelativeCursorPosition, &InheritedVisibility), With<TimeTrack>>,
    mut knob: Query<&mut Node, With<TimeKnob>>,
    mut readout: Query<&mut Text, With<TimeReadout>>,
    mut dragging: Local<bool>,
) {
    if *state.get() == GameState::Playing {
        *dragging = false;
        return;
    }
    let lon = players.single().map(|pt| origin.to_geodetic(pt.translation).lon_deg).unwrap_or(0.0);
    let solar_hour = |unix_s: f64| (unix_s / 3600.0 + lon / 15.0).rem_euclid(24.0);
    let Ok((rcp, vis)) = track.single() else { return };
    // Hidden nodes keep a stale RelativeCursorPosition; ignore the track off its tab.
    if buttons.just_pressed(MouseButton::Left) && vis.get() && rcp.cursor_over {
        *dragging = true;
    }
    if !buttons.pressed(MouseButton::Left) {
        *dragging = false;
    }
    if *dragging && let Some(n) = rcp.normalized {
        let target = ((n.x as f64 + 0.5).clamp(0.0, 1.0)) * 24.0;
        let mut delta = target - solar_hour(clock.unix_s);
        delta -= 24.0 * (delta / 24.0).round();
        gfx.time_live = false;
        gfx.hour_offset += delta;
    }
    let h = solar_hour(clock.unix_s);
    if let Ok(mut n) = knob.single_mut() {
        n.left = Val::Percent((h / 24.0 * 100.0) as f32);
    }
    if let Ok(mut t) = readout.single_mut() {
        let total = (h * 60.0).round() as i64 % 1440;
        let s = format!("{:02}:{:02}", total / 60, total % 60);
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// Once a spawn lands, start (or resume) playing.
fn enter_play_on_arrival(mut commands: Commands, arrived: Option<Res<Arrived>>, mut next: ResMut<NextState<GameState>>) {
    if arrived.is_some() {
        commands.remove_resource::<Arrived>();
        next.set(GameState::Playing);
    }
}

fn apply_graphics(
    gfx: Res<GraphicsSettings>,
    mut clock: ResMut<crate::sky::Clock>,
    mut last_offset: Local<f64>,
    mut commands: Commands,
    cams: Query<(Entity, Has<ScreenSpaceAmbientOcclusion>, Has<Bloom>, Has<TemporalAntiAliasing>), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut suns: Query<&mut DirectionalLight, Without<crate::globe::GlobeSun>>,
) {
    if !gfx.is_changed() {
        return;
    }
    clock.unix_s += (gfx.hour_offset - *last_offset) * 3600.0;
    *last_offset = gfx.hour_offset;
    // Screen-space contact shadows fill in the small-scale contact the
    // cascades miss (EW_CONTACT=0 to A/B).
    let contact = gfx.shadows && std::env::var("EW_CONTACT").map(|v| v != "0").unwrap_or(true);
    for mut s in &mut suns {
        s.shadow_maps_enabled = gfx.shadows;
        s.contact_shadows_enabled = contact;
    }
    for (e, has_ao, has_bloom, has_aa) in &cams {
        // Temporal PCF jitters the shadow taps and lets TAA average them:
        // softer edges than Gaussian for fewer taps. Without TAA it would shimmer.
        commands.entity(e).insert(if gfx.aa { ShadowFilteringMethod::Temporal } else { ShadowFilteringMethod::Gaussian });
        if contact {
            commands.entity(e).insert(ContactShadows { linear_steps: 12, thickness: 0.25, length: 1.5 });
        } else {
            commands.entity(e).remove::<ContactShadows>();
        }
        match (gfx.aa, has_aa) {
            (true, false) => {
                commands.entity(e).insert(TemporalAntiAliasing::default());
            }
            (false, true) => {
                commands.entity(e).remove::<TemporalAntiAliasing>();
            }
            _ => {}
        }
        match (gfx.ambient_occlusion, has_ao) {
            (true, false) => {
                commands.entity(e).insert(ScreenSpaceAmbientOcclusion::default());
            }
            (false, true) => {
                commands.entity(e).remove::<ScreenSpaceAmbientOcclusion>();
            }
            _ => {}
        }
        match (gfx.bloom, has_bloom) {
            (true, false) => {
                commands.entity(e).insert(Bloom::NATURAL);
            }
            (false, true) => {
                commands.entity(e).remove::<Bloom>();
            }
            _ => {}
        }
    }
}

fn capture_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut cursors {
        c.grab_mode = CursorGrabMode::Locked;
        c.visible = false;
    }
}

fn release_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut cursors {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
}

fn pause_physics(mut t: ResMut<Time<Physics>>) {
    t.pause();
}

fn unpause_physics(mut t: ResMut<Time<Physics>>) {
    t.unpause();
}
