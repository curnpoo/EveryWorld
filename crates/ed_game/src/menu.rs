//! Main menu and pause menu.
//!
//! Main menu (on launch): Play drops you somewhere interesting at random,
//! the search box takes a place name, Browse the map lets you pick any
//! point and choose sky or ground.
//!
//! Pause (Esc while playing): fullscreen menu with tabs: Map (teleport
//! anywhere), Settings, Character, Controls. Physics freezes while paused.

use crate::body::Cosmetics;
use crate::config::Config;
use crate::mapview::{MapCanvas, MapState};
use crate::player::{ControlSettings, Player};
use crate::spawn::{Arrived, SpawnHeight, SpawnRequest, Spawner};
use avian3d::prelude::{Physics, PhysicsTime};
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::light::DirectionalLight;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<SearchText>()
            .init_resource::<MenuView>()
            .init_resource::<GraphicsSettings>()
            .add_systems(Startup, (build_menus, skip_menu_from_cli).chain())
            .add_systems(
                Update,
                (
                    open_pause,
                    menu_buttons,
                    search_typing,
                    refresh_menu,
                    enter_play_on_arrival,
                    apply_graphics,
                )
                    .chain(),
            )
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
    HomeMap,
    Map,
    Settings,
    Character,
    Controls,
}

#[derive(Resource, Default)]
pub struct SearchText(pub String);

#[derive(Resource, Clone)]
pub struct GraphicsSettings {
    pub shadows: bool,
    pub ambient_occlusion: bool,
    pub bloom: bool,
    pub hour_offset: f64,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self { shadows: true, ambient_occlusion: true, bloom: true, hour_offset: 0.0 }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Play,
    BrowseMap,
    BackHome,
    Quit,
    Resume,
    ToMainMenu,
    Tab(MenuView),
    ToggleShadows,
    ToggleAo,
    ToggleBloom,
    TimeMinus,
    TimePlus,
    TimeNow,
    SensMinus,
    SensPlus,
    InvertY,
    ToggleBackpack,
    SearchSky,
    SearchGround,
}

/// Root of each screen; shown/hidden by `MenuView` and `GameState`.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    HomeMap,
    Pause,
}

/// A tab body inside the pause menu.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct TabBody(MenuView);

#[derive(Component)]
struct SearchField;

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

fn search_box(p: &mut ChildSpawnerCommands) {
    p.spawn((
        Node {
            padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(12.0)),
            min_width: Val::Px(340.0),
            ..default()
        },
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.25)),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
    ))
    .with_children(|b| {
        b.spawn((SearchField, Text::new(""), TextFont { font_size: FontSize::Px(18.0), ..default() }, TextColor(INK)));
    });
}

fn map_canvas(p: &mut ChildSpawnerCommands) {
    p.spawn((
        MapCanvas,
        RelativeCursorPosition::default(),
        Node { flex_grow: 1.0, width: Val::Percent(100.0), height: Val::Percent(100.0), overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(14.0)), ..default() },
        BackgroundColor(Color::srgb(0.61, 0.79, 0.88)),
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
                button(c, "Play  (somewhere random)", MenuAction::Play, true);
                text(c, "Or type a place and press Enter", 14.0, DIM);
                search_box(c);
                c.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|r| {
                    button(r, "Drop from the sky", MenuAction::SearchSky, false);
                    button(r, "Start on the ground", MenuAction::SearchGround, false);
                });
                button(c, "Browse the map", MenuAction::BrowseMap, false);
                button(c, "Quit", MenuAction::Quit, false);
                c.spawn((StatusLine, Text::new(""), TextFont { font_size: FontSize::Px(15.0), ..default() }, TextColor(ORANGE)));
            });
        });

    // ---- Home: map picker ----
    commands
        .spawn((
            Screen::HomeMap,
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
            ZIndex(11),
        ))
        .with_children(|p| {
            p.spawn(Node { column_gap: Val::Px(14.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                button(r, "Back", MenuAction::BackHome, false);
                text(r, "Pick anywhere on Earth: drag to move, scroll to zoom, click to choose.", 16.0, DIM);
            });
            map_canvas(p);
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
                text(r, "Paused", 26.0, INK);
                r.spawn(Node { width: Val::Px(16.0), ..default() });
                for (label, v) in [("Map", MenuView::Map), ("Settings", MenuView::Settings), ("Character", MenuView::Character), ("Controls", MenuView::Controls)] {
                    button(r, label, MenuAction::Tab(v), false);
                }
                r.spawn(Node { flex_grow: 1.0, ..default() });
                button(r, "Main menu", MenuAction::ToMainMenu, false);
                button(r, "Resume", MenuAction::Resume, true);
            });
            // Map tab.
            p.spawn((TabBody(MenuView::Map), Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), ..default() })).with_children(|t| {
                t.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                    search_box(r);
                    button(r, "Drop from the sky", MenuAction::SearchSky, false);
                    button(r, "Start on the ground", MenuAction::SearchGround, false);
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
                ] {
                    setting_row(t, label, &[("Toggle", a)], a);
                }
                setting_row(t, "Time of day", &[("-1 h", MenuAction::TimeMinus), ("+1 h", MenuAction::TimePlus), ("Now", MenuAction::TimeNow)], MenuAction::TimePlus);
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
                    "F: parachute while falling",
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
    let zoom = args.iter().position(|a| a == "--map-zoom").and_then(|i| args.get(i + 1)).and_then(|z| z.parse().ok()).unwrap_or(5.0);
    map.focus(cfg.start, zoom);
    match menu_view.as_deref() {
        Some("map") => *view = MenuView::HomeMap,
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

fn open_pause(keys: Res<ButtonInput<KeyCode>>, state: Res<State<GameState>>, mut next: ResMut<NextState<GameState>>, mut view: ResMut<MenuView>) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match state.get() {
        GameState::Playing => {
            *view = MenuView::Map;
            next.set(GameState::Paused);
        }
        GameState::Paused => next.set(GameState::Playing),
        GameState::Menu if *view == MenuView::HomeMap => *view = MenuView::Home,
        GameState::Menu => {}
    }
}

#[allow(deprecated, clippy::too_many_arguments)]
fn menu_buttons(
    mut q: Query<(&Interaction, &MenuAction, &mut BackgroundColor), Changed<Interaction>>,
    mut view: ResMut<MenuView>,
    mut next: ResMut<NextState<GameState>>,
    mut spawns: MessageWriter<SpawnRequest>,
    mut exit: MessageWriter<AppExit>,
    mut gfx: ResMut<GraphicsSettings>,
    mut controls: ResMut<ControlSettings>,
    mut cosmetics: ResMut<Cosmetics>,
    search: Res<SearchText>,
    mut map: ResMut<MapState>,
    origin: Res<crate::origin::WorldOrigin>,
    players: Query<&Transform, With<Player>>,
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
            MenuAction::Play => {
                spawns.write(SpawnRequest::Random { height: SpawnHeight::Sky });
            }
            MenuAction::SearchSky | MenuAction::SearchGround => {
                let q = search.0.trim().to_string();
                if !q.is_empty() {
                    let height = if *a == MenuAction::SearchSky { SpawnHeight::Sky } else { SpawnHeight::Ground };
                    spawns.write(SpawnRequest::Search { query: q, height });
                }
            }
            MenuAction::BrowseMap => *view = MenuView::HomeMap,
            MenuAction::BackHome => *view = MenuView::Home,
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
                        map.focus(origin.to_geodetic(pt.translation), 12.0);
                    }
                }
            }
            MenuAction::ToggleShadows => gfx.shadows = !gfx.shadows,
            MenuAction::ToggleAo => gfx.ambient_occlusion = !gfx.ambient_occlusion,
            MenuAction::ToggleBloom => gfx.bloom = !gfx.bloom,
            MenuAction::TimeMinus => gfx.hour_offset -= 1.0,
            MenuAction::TimePlus => gfx.hour_offset += 1.0,
            MenuAction::TimeNow => gfx.hour_offset = 0.0,
            MenuAction::SensMinus => controls.mouse_sensitivity = (controls.mouse_sensitivity - 0.1).max(0.2),
            MenuAction::SensPlus => controls.mouse_sensitivity = (controls.mouse_sensitivity + 0.1).min(3.0),
            MenuAction::InvertY => controls.invert_y = !controls.invert_y,
            MenuAction::ToggleBackpack => cosmetics.backpack = !cosmetics.backpack,
        }
    }
}

/// The search box takes typing whenever a menu with one is open.
fn search_typing(
    state: Res<State<GameState>>,
    view: Res<MenuView>,
    mut search: ResMut<SearchText>,
    mut keys: MessageReader<KeyboardInput>,
    mut spawns: MessageWriter<SpawnRequest>,
) {
    let active = (*state.get() == GameState::Menu && *view == MenuView::Home) || (*state.get() == GameState::Paused && *view == MenuView::Map);
    for k in keys.read() {
        if !active || k.state != ButtonState::Pressed {
            continue;
        }
        match &k.logical_key {
            Key::Backspace => {
                search.0.pop();
            }
            Key::Enter => {
                let q = search.0.trim().to_string();
                if !q.is_empty() {
                    spawns.write(SpawnRequest::Search { query: q, height: SpawnHeight::Sky });
                }
            }
            Key::Character(c) if search.0.len() < 80 => search.0.push_str(c),
            Key::Space if search.0.len() < 80 => search.0.push(' '),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_menu(
    time: Res<Time>,
    state: Res<State<GameState>>,
    view: Res<MenuView>,
    search: Res<SearchText>,
    spawner: Res<Spawner>,
    gfx: Res<GraphicsSettings>,
    controls: Res<ControlSettings>,
    cosmetics: Res<Cosmetics>,
    mut screens: Query<(&Screen, &mut Visibility), Without<TabBody>>,
    mut tabs: Query<(&TabBody, &mut Visibility), Without<Screen>>,
    mut fields: Query<&mut Text, (With<SearchField>, Without<StatusLine>, Without<SettingValue>)>,
    mut status: Query<&mut Text, (With<StatusLine>, Without<SearchField>, Without<SettingValue>)>,
    mut values: Query<(&SettingValue, &mut Text), (Without<SearchField>, Without<StatusLine>)>,
) {
    let st = *state.get();
    for (s, mut v) in &mut screens {
        let show = match s {
            Screen::Home => st == GameState::Menu && *view != MenuView::HomeMap,
            Screen::HomeMap => st == GameState::Menu && *view == MenuView::HomeMap,
            Screen::Pause => st == GameState::Paused,
        };
        v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    for (t, mut v) in &mut tabs {
        v.set_if_neq(if t.0 == *view { Visibility::Inherited } else { Visibility::Hidden });
    }
    let caret = if (time.elapsed_secs() * 2.0) as i32 % 2 == 0 { "|" } else { " " };
    for mut f in &mut fields {
        f.0 = if search.0.is_empty() { format!("{caret}Zermatt, Kyoto, 40.78 -73.97...") } else { format!("{}{caret}", search.0) };
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
            MenuAction::TimePlus => if gfx.hour_offset == 0.0 { "Now".into() } else { format!("{:+} h", gfx.hour_offset) },
            MenuAction::SensPlus => format!("{:.1}", controls.mouse_sensitivity),
            MenuAction::InvertY => on(controls.invert_y),
            MenuAction::ToggleBackpack => on(cosmetics.backpack),
            _ => String::new(),
        };
    }
}

/// Once a spawn lands, start (or resume) playing.
fn enter_play_on_arrival(mut commands: Commands, arrived: Option<Res<Arrived>>, mut next: ResMut<NextState<GameState>>, mut search: ResMut<SearchText>) {
    if arrived.is_some() {
        commands.remove_resource::<Arrived>();
        search.0.clear();
        next.set(GameState::Playing);
    }
}

fn apply_graphics(
    gfx: Res<GraphicsSettings>,
    mut clock: ResMut<crate::sky::Clock>,
    mut last_offset: Local<f64>,
    mut commands: Commands,
    cams: Query<(Entity, Has<ScreenSpaceAmbientOcclusion>, Has<Bloom>), With<Camera3d>>,
    mut suns: Query<&mut DirectionalLight>,
) {
    if !gfx.is_changed() {
        return;
    }
    clock.unix_s += (gfx.hour_offset - *last_offset) * 3600.0;
    *last_offset = gfx.hour_offset;
    for mut s in &mut suns {
        s.shadow_maps_enabled = gfx.shadows;
    }
    for (e, has_ao, has_bloom) in &cams {
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
