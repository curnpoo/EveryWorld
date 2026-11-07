//! Main menu and pause menu.
//!
//! Main menu (on launch): just Play. It opens the same tabbed screen as the
//! pause menu, starting on the map; click a spot there and choose sky or
//! ground to start the 3D world.
//!
//! Pause (Esc while playing): fullscreen menu with tabs: Map (teleport
//! anywhere), Settings, Audio, Character, Controls, Debug. Physics freezes
//! while paused.

use crate::body::Cosmetics;
use crate::clouds::CloudOverride;
use crate::config::Config;
use crate::sky::{Clock, CurrentWeather};
use crate::gamepad::{spawn_crosshair, InputMode, Pad};
use crate::mapview::{MapCanvas, MapState};
use crate::player::{ControlSettings, Player};
use crate::settings::AudioSettings;
use crate::spawn::{Arrived, Spawner};
use avian3d::prelude::{Physics, PhysicsTime};
use bevy::asset::embedded_asset;
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
        embedded_asset!(app, "menu_assets/Chewy-Regular.ttf");
        embedded_asset!(app, "menu_assets/menu_bg.png");
        app.init_state::<GameState>()
            .init_resource::<MenuView>()
            .init_resource::<MenuFocus>()
            .init_resource::<GraphicsSettings>()
            .init_resource::<DebugSettings>()
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
                    pad_tabs,
                    pad_focus,
                    update_badges,
                    menu_buttons,
                    refresh_menu,
                    time_slider,
                    audio_sliders,
                    enter_play_on_arrival,
                    apply_graphics,
                    update_debug_readout,
                    highlight_tabs,
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
    Audio,
    Character,
    Controls,
    Debug,
    Multiplayer,
}

/// How the frame is anti-aliased: the temporal upscaler (renders low, AA included), plain AA at native
/// resolution, or neither. Mutually exclusive: the upscaler already does the AA.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AaMode {
    Upscale,
    Aa,
    Off,
}

impl AaMode {
    pub fn next(self) -> Self {
        match self {
            Self::Upscale => Self::Aa,
            Self::Aa => Self::Off,
            Self::Off => Self::Upscale,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Upscale => "MetalFX upscaler",
            Self::Aa => "Anti-aliasing",
            Self::Off => "Off",
        }
    }
}

#[derive(Resource, Clone)]
pub struct GraphicsSettings {
    pub aa_mode: AaMode,
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
    pub hour_offset_applied: f64,
    /// The local solar hour the player wants wherever they go: 15.0 is 3 pm.
    /// The time slider and the +/- buttons set it; map clicks and drops apply
    /// it, so a click on the globe lands in the same daylight you picked.
    pub solar_hour: f64,
}

/// Live renderer/debug controls. These deliberately do not persist: a debug
/// session should not silently change the next normal launch.
#[derive(Resource, Clone, Debug)]
pub struct DebugSettings {
    pub probe_gi: bool,
    pub probe_view: ProbeView,
    pub native_view: NativeDebugView,
    pub ocean_fft: OceanFft,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ProbeView {
    #[default]
    Off,
    Colors,
    Lit,
}

impl ProbeView {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Colors,
            Self::Colors => Self::Lit,
            Self::Lit => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Colors => "Probe colours",
            Self::Lit => "Lit spheres",
        }
    }

    fn from_debug_value(v: u8) -> Self {
        match v % 3 {
            1 => Self::Colors,
            2 => Self::Lit,
            _ => Self::Off,
        }
    }
}

/// Native `world.metal` diagnostic views. Values match `EW_NATIVE_DBG` so the
/// environment switch remains useful in automated runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NativeDebugView {
    #[default]
    Off,
    Normals,
    ShadowVisibility,
    AmbientGi,
    IndirectDiffuse,
    ProbeWeight,
    ProbeVisibility,
    ProbeBounce,
    ProbeXzCoverage,
    ProbeYCoverage,
    CascadeTint,
    SunNdotL,
    Shadow0,
    Shadow1,
    Shadow2,
    Shadow3,
    Shadow4,
    Shadow5,
    ShadowDepth,
}

impl NativeDebugView {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Normals,
            Self::Normals => Self::ShadowVisibility,
            Self::ShadowVisibility => Self::AmbientGi,
            Self::AmbientGi => Self::IndirectDiffuse,
            Self::IndirectDiffuse => Self::ProbeWeight,
            Self::ProbeWeight => Self::ProbeVisibility,
            Self::ProbeVisibility => Self::ProbeBounce,
            Self::ProbeBounce => Self::ProbeXzCoverage,
            Self::ProbeXzCoverage => Self::ProbeYCoverage,
            Self::ProbeYCoverage => Self::CascadeTint,
            Self::CascadeTint => Self::SunNdotL,
            Self::SunNdotL => Self::Shadow0,
            Self::Shadow0 => Self::Shadow1,
            Self::Shadow1 => Self::Shadow2,
            Self::Shadow2 => Self::Shadow3,
            Self::Shadow3 => Self::Shadow4,
            Self::Shadow4 => Self::Shadow5,
            Self::Shadow5 => Self::ShadowDepth,
            Self::ShadowDepth => Self::Off,
        }
    }

    pub fn value(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Normals => 1,
            Self::ShadowVisibility => 2,
            Self::AmbientGi => 3,
            Self::IndirectDiffuse => 18,
            Self::ProbeWeight => 19,
            Self::ProbeVisibility => 20,
            Self::ProbeBounce => 21,
            Self::ProbeXzCoverage => 22,
            Self::ProbeYCoverage => 23,
            Self::CascadeTint => 4,
            Self::SunNdotL => 5,
            Self::Shadow0 => 6,
            Self::Shadow1 => 7,
            Self::Shadow2 => 8,
            Self::Shadow3 => 9,
            Self::Shadow4 => 16,
            Self::Shadow5 => 17,
            Self::ShadowDepth => 10,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Normals => "Normals",
            Self::ShadowVisibility => "Shadow visibility",
            Self::AmbientGi => "Ambient + GI",
            Self::IndirectDiffuse => "Indirect diffuse (grey)",
            Self::ProbeWeight => "Probe coverage",
            Self::ProbeVisibility => "Probe visibility",
            Self::ProbeBounce => "Probe bounce",
            Self::ProbeXzCoverage => "Probe XZ coverage",
            Self::ProbeYCoverage => "Probe Y coverage",
            Self::CascadeTint => "Cascade tint",
            Self::SunNdotL => "Sun N dot L",
            Self::Shadow0 => "Shadow map 0",
            Self::Shadow1 => "Shadow map 1",
            Self::Shadow2 => "Shadow map 2",
            Self::Shadow3 => "Shadow map 3",
            Self::Shadow4 => "Shadow map 4",
            Self::Shadow5 => "Shadow map 5",
            Self::ShadowDepth => "Shadow depth difference",
        }
    }

    fn from_debug_value(v: u32) -> Self {
        match v {
            1 => Self::Normals,
            2 => Self::ShadowVisibility,
            3 => Self::AmbientGi,
            18 => Self::IndirectDiffuse,
            19 => Self::ProbeWeight,
            20 => Self::ProbeVisibility,
            21 => Self::ProbeBounce,
            22 => Self::ProbeXzCoverage,
            23 => Self::ProbeYCoverage,
            4 => Self::CascadeTint,
            5 => Self::SunNdotL,
            6 => Self::Shadow0,
            7 => Self::Shadow1,
            8 => Self::Shadow2,
            9 => Self::Shadow3,
            10..=13 => Self::ShadowDepth,
            16 => Self::Shadow4,
            17 => Self::Shadow5,
            _ => Self::Off,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OceanFft {
    #[default]
    Auto,
    On,
    Off,
}

impl OceanFft {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::On,
            Self::On => Self::Off,
            Self::Off => Self::Auto,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::On => "On",
            Self::Off => "Off",
        }
    }
}

impl Default for DebugSettings {
    fn default() -> Self {
        let probe_gi = !std::env::var("EW_PROBE_GI").is_ok_and(|v| v == "0");
        let probe_view = ProbeView::from_debug_value(std::env::var("EW_PROBE_DEBUG").ok().and_then(|v| v.parse().ok()).unwrap_or(0));
        let native_view = NativeDebugView::from_debug_value(std::env::var("EW_NATIVE_DBG").ok().and_then(|v| v.parse().ok()).unwrap_or(0));
        Self { probe_gi, probe_view, native_view, ocean_fft: OceanFft::Auto }
    }
}

/// The hour every drop-in starts in, unless the player picks another one.
pub const DEFAULT_SOLAR_HOUR: f64 = 15.0;

impl Default for GraphicsSettings {
    fn default() -> Self {
        let post = !std::env::args().any(|a| a == "--no-post");
        let mut g = Self { shadows: !std::env::args().any(|a| a == "--no-shadows"), ambient_occlusion: post, bloom: post, render_pct: 67, aa: post, aa_mode: if post { AaMode::Upscale } else { AaMode::Off }, show_stats: false, time_live: false, hour_offset: 0.0, hour_offset_applied: 0.0, solar_hour: DEFAULT_SOLAR_HOUR };
        // Perf bisecting: `EW_GFX=ao=0,bloom=0,aa=0,shadows=0,res=50,stats=1`.
        if let Ok(spec) = std::env::var("EW_GFX") {
            for kv in spec.split(',') {
                let Some((k, v)) = kv.split_once('=') else { continue };
                let b = v != "0";
                match k {
                    "ao" => g.ambient_occlusion = b,
                    "bloom" => g.bloom = b,
                    "aa" => {
                        g.aa = b;
                        g.aa_mode = if b { AaMode::Aa } else { AaMode::Off };
                    }
                    "shadows" => g.shadows = b,
                    "stats" => g.show_stats = b,
                    "res" => {
                        g.render_pct = v.parse().unwrap_or(g.render_pct);
                        g.aa_mode = if g.render_pct < 100 { AaMode::Upscale } else if g.aa { AaMode::Aa } else { AaMode::Off };
                    }
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
    ToggleProbes,
    CycleProbeView,
    CycleNativeView,
    CycleOceanFft,
    TimeMinus,
    TimePlus,
    TimeNow,
    SetWeather(&'static str),
    SensMinus,
    SensPlus,
    InvertY,
    ToggleBackpack,
    ToggleMute,
    ToggleCalm,
    ConnectServer,
    DisconnectServer,
}

/// One audio mix slider; the track carries it, the knob and readout are children of the row.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Knob {
    Master,
    World,
    Body,
    Air,
    Vehicles,
    Alerts,
}

impl Knob {
    fn value(self, a: &AudioSettings) -> f32 {
        match self {
            Knob::Master => a.master,
            Knob::World => a.world,
            Knob::Body => a.body,
            Knob::Air => a.air,
            Knob::Vehicles => a.vehicles,
            Knob::Alerts => a.alerts,
        }
    }
    fn slot(self, a: &mut AudioSettings) -> &mut f32 {
        match self {
            Knob::Master => &mut a.master,
            Knob::World => &mut a.world,
            Knob::Body => &mut a.body,
            Knob::Air => &mut a.air,
            Knob::Vehicles => &mut a.vehicles,
            Knob::Alerts => &mut a.alerts,
        }
    }
}

#[derive(Component)]
struct KnobDot(Knob);
#[derive(Component)]
struct KnobReadout(Knob);

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
#[derive(Component)]
pub(crate) struct NetStatusText;

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

#[derive(Component)]
struct DebugReadout;

pub(crate) const INK: Color = Color::srgb(0.96, 0.96, 0.95);
pub(crate) const DIM: Color = Color::srgba(0.96, 0.96, 0.95, 0.6);
pub(crate) const ORANGE: Color = Color::srgb(0.98, 0.45, 0.12);
pub(crate) const PANEL: Color = Color::srgba(0.06, 0.07, 0.09, 0.9);
pub(crate) const BTN: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);
pub(crate) const BTN_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.16);

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
        badge(b);
    });
}

/// The controller prompt that sits on a button's corner: "A" on the focused one,
/// "B" on Resume, "LB"/"RB" on the neighbouring tabs. `update_badges` fills it in.
#[derive(Component)]
struct PadBadge;

fn badge(b: &mut ChildSpawnerCommands) {
    b.spawn((
        PadBadge,
        Text::new(""),
        TextFont { font_size: FontSize::Px(15.0), ..default() },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(-10.0),
            right: Val::Px(-8.0),
            min_width: Val::Px(26.0),
            padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(13.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Visibility::Hidden,
        ZIndex(5),
        Pickable::IGNORE,
    ));
}

/// Big, rounded main-menu button in the title font.
#[allow(deprecated)]
fn home_button(p: &mut ChildSpawnerCommands, font: &Handle<Font>, label: &str, action: MenuAction, accent: bool) {
    p.spawn((
        Interaction::None,
        action,
        Node {
            padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(34.0)),
            ..default()
        },
        BackgroundColor(if accent { ORANGE } else { BTN }),
    ))
    .with_children(|b| {
        b.spawn((Text::new(label), TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(34.0), ..default() }, TextColor(INK)));
        badge(b);
    });
}

/// A/B/LB/RB prompts on the buttons, only while a controller is in use.
#[allow(clippy::type_complexity)]
fn update_badges(
    mode: Res<InputMode>,
    state: Res<State<GameState>>,
    view: Res<MenuView>,
    focus: Res<MenuFocus>,
    actions: Query<&MenuAction>,
    mut badges: Query<(&ChildOf, &mut Visibility, &mut Text, &mut BackgroundColor), With<PadBadge>>,
) {
    let pad = *mode == InputMode::Controller && menu_showing(state.get(), *view);
    let cur = TAB_ORDER.iter().position(|t| *t == *view);
    let n = TAB_ORDER.len();
    for (parent, mut vis, mut text, mut bg) in &mut badges {
        let owner = parent.parent();
        let label: Option<&str> = if !pad {
            None
        } else if focus.0 == Some(owner) {
            Some("A")
        } else {
            match actions.get(owner) {
                Ok(MenuAction::Resume) if *state.get() == GameState::Paused => Some("B"),
                Ok(MenuAction::Tab(t)) => cur.and_then(|c| {
                    if *t == TAB_ORDER[(c + n - 1) % n] {
                        Some("LB")
                    } else if *t == TAB_ORDER[(c + 1) % n] {
                        Some("RB")
                    } else {
                        None
                    }
                }),
                _ => None,
            }
        };
        match label {
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
            Some(l) => {
                let col = match l {
                    "A" => Color::srgb(0.30, 0.68, 0.31),
                    "B" => Color::srgb(0.85, 0.27, 0.25),
                    _ => Color::srgb(0.35, 0.37, 0.42),
                };
                if text.0 != l {
                    text.0 = l.to_string();
                }
                if bg.0 != col {
                    bg.0 = col;
                }
                vis.set_if_neq(Visibility::Inherited);
            }
        }
    }
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
    ))
    .with_children(spawn_crosshair);
}

fn build_menus(mut commands: Commands, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load("embedded://earthdrop/menu_assets/Chewy-Regular.ttf");
    let bg: Handle<Image> = assets.load("embedded://earthdrop/menu_assets/menu_bg.png");
    let fill = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    // ---- Home: blurred Matterhorn sunset, big bubbly title ----
    commands
        .spawn((
            Screen::Home,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.09)),
            ZIndex(10),
        ))
        .with_children(|p| {
            p.spawn((ImageNode::new(bg), fill.clone()));
            p.spawn((fill, BackgroundColor(Color::srgba(0.05, 0.03, 0.10, 0.30))));
            p.spawn((
                Text::new("EveryWorld"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(150.0), ..default() },
                TextColor(Color::srgb(1.0, 0.97, 0.90)),
                TextShadow { offset: Vec2::new(0.0, 9.0), color: Color::srgba(0.80, 0.25, 0.05, 0.9) },
            ));
            p.spawn((
                Text::new("The whole real Earth. Drop in anywhere."),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(28.0), ..default() },
                TextColor(Color::srgb(1.0, 0.93, 0.82)),
            ));
            p.spawn(Node { height: Val::Px(30.0), ..default() });
            p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(16.0), width: Val::Px(340.0), ..default() }).with_children(|c| {
                home_button(c, &font, "Play", MenuAction::Play, true);
                home_button(c, &font, "Quit", MenuAction::Quit, false);
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
                for (label, v) in [
                    ("Map", MenuView::Map),
                    ("Settings", MenuView::Settings),
                    ("Audio", MenuView::Audio),
                    ("Character", MenuView::Character),
                    ("Controls", MenuView::Controls),
                    ("Debug", MenuView::Debug),
                    ("Multiplayer", MenuView::Multiplayer),
                ] {
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
                    ("Upscaler / AA", MenuAction::ToggleAa),
                ] {
                    setting_row(t, label, &[("Toggle", a)], a);
                }
                setting_row(t, "Upscaler render scale", &[("Change", MenuAction::CycleResolution)], MenuAction::CycleResolution);
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
            // Audio tab.
            p.spawn((TabBody(MenuView::Audio), Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), ..default() }, Visibility::Hidden)).with_children(|t| {
                for (label, k) in [
                    ("Master volume", Knob::Master),
                    ("World ambience", Knob::World),
                    ("Footsteps and robot", Knob::Body),
                    ("Wind and rockets", Knob::Air),
                    ("Vehicle engines", Knob::Vehicles),
                    ("Alerts and beeps", Knob::Alerts),
                ] {
                    slider_row(t, label, k);
                }
                setting_row(t, "Mute", &[("Toggle", MenuAction::ToggleMute)], MenuAction::ToggleMute);
                setting_row(t, "Calm ambience", &[("Toggle", MenuAction::ToggleCalm)], MenuAction::ToggleCalm);
                text(t, "Calm plays only the strongest few world sounds at a time. Settings save automatically.", 14.0, DIM);
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
                    "F3: cycle GI probe spheres",
                    "Esc: pause menu (Debug tab has live renderer controls)",
                    "Controller (switches on when you use one):",
                    "Left stick: move · Right stick: look",
                    "RT: jump (hold to charge) · LT: sprint / afterburner",
                    "A: wingsuit · B: parachute · Start: pause menu",
                    "LB / RB: switch tabs",
                    "Map: left stick pans · right stick zooms · A picks at the crosshair · A again drops in · B cancels",
                ] {
                    text(t, line, 17.0, INK);
                }
            });
            // Debug tab: live renderer controls and diagnostics.
            p.spawn((
                TabBody(MenuView::Debug),
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(22.0),
                    ..default()
                },
                Visibility::Hidden,
            ))
            .with_children(|t| {
                t.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(9.0), width: Val::Px(580.0), ..default() }).with_children(|c| {
                    text(c, "Live renderer controls", 22.0, INK);
                    text(c, &format!("Renderer: {}", crate::native::renderer_name()), 17.0, INK);
                    for (label, action_label, a) in [
                        ("Probe GI (20 km)", "Toggle", MenuAction::ToggleProbes),
                        ("Probe spheres", "Next", MenuAction::CycleProbeView),
                        ("Shadow maps", "Toggle", MenuAction::ToggleShadows),
                        ("Ambient occlusion", "Toggle", MenuAction::ToggleAo),
                        ("Bloom", "Toggle", MenuAction::ToggleBloom),
                        ("Ocean FFT", "Next", MenuAction::CycleOceanFft),
                        ("Metal debug view", "Next", MenuAction::CycleNativeView),
                        ("Live diagnostics", "Toggle", MenuAction::ToggleStats),
                    ] {
                        setting_row(c, label, &[(action_label, a)], a);
                    }
                    text(c, "Changes apply immediately. Live diagnostics stay on screen while playing; this panel updates while paused.", 14.0, DIM);
                });
                t.spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(320.0),
                        padding: UiRect::all(Val::Px(14.0)),
                        border_radius: BorderRadius::all(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.03, 0.045, 0.94)),
                ))
                .with_children(|c| {
                    c.spawn((
                        DebugReadout,
                        Text::new("Collecting diagnostics..."),
                        TextFont { font_size: FontSize::Px(13.0), ..default() },
                        TextColor(Color::srgba(0.82, 0.92, 1.0, 0.92)),
                    ));
                });
            });
            // Multiplayer tab.
            p.spawn((TabBody(MenuView::Multiplayer), Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), ..default() }, Visibility::Hidden)).with_children(|t| {
                text(t, "Ghost multiplayer (LAN)", 22.0, INK);
                t.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                    button(r, "Connect", MenuAction::ConnectServer, false);
                    button(r, "Disconnect", MenuAction::DisconnectServer, false);
                });
                t.spawn((NetStatusText, Text::new("Disconnected"), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(ORANGE)));
                text(t, "Server: use --server <host:port> (default 127.0.0.1:7777) and --name <name>.", 14.0, DIM);
                text(t, "Run `earthdrop-server [port]` on one machine, then connect others.", 14.0, DIM);
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

fn slider_row(p: &mut ChildSpawnerCommands, label: &str, k: Knob) {
    p.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
        r.spawn(Node { width: Val::Px(220.0), ..default() }).with_children(|c| text(c, label, 17.0, INK));
        r.spawn((KnobReadout(k), Text::new(""), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(ORANGE), Node { width: Val::Px(90.0), ..default() }));
        r.spawn((
            k,
            RelativeCursorPosition::default(),
            Node { width: Val::Px(320.0), height: Val::Px(14.0), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(BTN),
        ))
        .with_children(|tr| {
            tr.spawn((
                KnobDot(k),
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
}

/// Launching with --place / --lat / --fly skips the menu (handy for testing).
/// `--menu-view home|map|pause-map|settings|audio|controls|character|debug` opens a screen
/// directly (for screenshots).
fn skip_menu_from_cli(cfg: Res<Config>, mut next: ResMut<NextState<GameState>>, mut view: ResMut<MenuView>, mut map: ResMut<MapState>) {
    let args: Vec<String> = std::env::args().collect();
    let menu_view = args.iter().position(|a| a == "--menu-view").and_then(|i| args.get(i + 1)).cloned();
    let zoom = args.iter().position(|a| a == "--map-zoom").and_then(|i| args.get(i + 1)).and_then(|z| z.parse().ok()).unwrap_or(crate::globe::WHOLE_EARTH_ZOOM);
    map.focus(cfg.start, zoom);
    match menu_view.as_deref() {
        Some("map") => *view = MenuView::Map,
        Some("home") => {} // stay on the main menu (screenshots)
        Some(v @ ("pause-map" | "settings" | "audio" | "controls" | "character" | "debug")) => {
            *view = match v {
                "settings" => MenuView::Settings,
                "audio" => MenuView::Audio,
                "controls" => MenuView::Controls,
                "character" => MenuView::Character,
                "debug" => MenuView::Debug,
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

/// Made `pub` so the map's search systems can order themselves after it: an
/// Escape that blurs the search field is never also the frame that closes the
/// menu, because the search systems edit `text_focus` afterwards.
pub fn open_pause(
    keys: Res<ButtonInput<KeyCode>>,
    pad: Res<Pad>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut view: ResMut<MenuView>,
    mut map: ResMut<MapState>,
    players: Query<&Transform, With<Player>>,
    origin: Res<crate::origin::WorldOrigin>,
) {
    // Start on a controller does what Esc does.
    if !keys.just_pressed(KeyCode::Escape) && !pad.start_just {
        return;
    }
    // Typing in the map's search field: Esc only leaves the field, handled there.
    if keys.just_pressed(KeyCode::Escape) && map.text_focus {
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

/// The button the D-pad is on. A presses it. Empty with a mouse or keyboard.
#[derive(Resource, Default)]
pub struct MenuFocus(pub Option<Entity>);

/// Background of a focused button: brighter than hover, so it reads as the cursor.
const FOCUS_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.3);

/// Whether a menu screen with buttons is on screen (the same rule `pad_tabs` uses).
fn menu_showing(state: &GameState, _view: MenuView) -> bool {
    match state {
        GameState::Paused => true,
        GameState::Menu => true,
        GameState::Playing => false,
    }
}

/// A button's colour when it is neither focused nor hovered.
fn rest_bg(action: &MenuAction, view: MenuView) -> Color {
    match action {
        MenuAction::Play | MenuAction::Resume => ORANGE,
        MenuAction::Tab(t) if *t == view => TAB_ON,
        _ => BTN,
    }
}

/// D-pad or left stick moves the focus to the nearest button in that direction.
/// A presses the focused button by setting its `Interaction`, so `menu_buttons`
/// handles it exactly like a click. B goes back: resume from the pause menu, or
/// to the main menu from a tab. Tabs are skipped (the bumpers switch them), and
/// the Map tab has no focus because the crosshair is its cursor.
#[allow(deprecated, clippy::too_many_arguments)]
fn pad_focus(
    pad: Res<Pad>,
    mode: Res<InputMode>,
    state: Res<State<GameState>>,
    mut view: ResMut<MenuView>,
    map: Res<MapState>,
    mut focus: ResMut<MenuFocus>,
    mut next: ResMut<NextState<GameState>>,
    mut buttons: Query<(Entity, &MenuAction, &InheritedVisibility, &mut Interaction, &mut BackgroundColor, &UiGlobalTransform)>,
) {
    let popup = map.selected.is_some();
    let active = menu_showing(state.get(), *view) && !popup && *mode == InputMode::Controller && *view != MenuView::Map;
    if pad.b_just && !popup {
        match state.get() {
            GameState::Paused => next.set(GameState::Playing),
            GameState::Menu if *view != MenuView::Home => *view = MenuView::Home,
            _ => {}
        }
    }

    let cands: Vec<(Entity, Vec2)> = buttons
        .iter()
        .filter(|(_, a, v, ..)| v.get() && !matches!(a, MenuAction::Tab(_)))
        .map(|(e, _, _, _, _, t)| (e, t.translation))
        .collect();
    let cur = focus.0.filter(|e| cands.iter().any(|c| c.0 == *e));
    let want = if !active || cands.is_empty() {
        None
    } else if let Some(cur) = cur {
        let from = cands.iter().find(|c| c.0 == cur).map(|c| c.1).unwrap_or_default();
        let dir = Vec2::new(pad.nav.x as f32, pad.nav.y as f32);
        if dir == Vec2::ZERO {
            Some(cur)
        } else {
            // Nearest button whose offset lies in the pressed direction, weighting off-axis drift.
            let best = cands
                .iter()
                .filter(|c| c.0 != cur)
                .filter_map(|c| {
                    let d = c.1 - from;
                    let along = d.dot(dir);
                    let across = (d - dir * along).length();
                    (along > 4.0 && across <= along * 2.0 + 40.0).then_some((c.0, along + across * 2.0))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            Some(best.map(|b| b.0).unwrap_or(cur))
        }
    } else {
        // First open: the top-most, then left-most button.
        cands.iter().min_by(|a, b| (a.1.y.round(), a.1.x).partial_cmp(&(b.1.y.round(), b.1.x)).unwrap()).map(|c| c.0)
    };
    if focus.0 != want {
        focus.0 = want;
    }

    if active && pad.a_just {
        if let Some(e) = focus.0 {
            if let Ok((_, _, _, mut interaction, ..)) = buttons.get_mut(e) {
                *interaction = Interaction::Pressed;
            }
        }
    }

    // Repaint every button when the focus or the tab changes. In between, hover still works.
    if focus.is_changed() || view.is_changed() {
        for (e, action, _, _, mut bg, _) in &mut buttons {
            let want_bg = if focus.0 == Some(e) { FOCUS_BG } else { rest_bg(action, *view) };
            if bg.0 != want_bg {
                bg.0 = want_bg;
            }
        }
    }
}

/// Tab order for LB / RB. Home has no tabs, so bumpers do nothing there.
const TAB_ORDER: [MenuView; 7] = [MenuView::Map, MenuView::Settings, MenuView::Audio, MenuView::Character, MenuView::Controls, MenuView::Debug, MenuView::Multiplayer];
/// The active tab's button. Orange like the Play and Resume accents, but translucent.
const TAB_ON: Color = Color::srgba(0.98, 0.45, 0.12, 0.55);

/// LB / RB move to the previous or next tab while a tabbed screen is showing.
fn pad_tabs(
    pad: Res<Pad>,
    state: Res<State<GameState>>,
    mut view: ResMut<MenuView>,
    mut map: ResMut<MapState>,
    origin: Res<crate::origin::WorldOrigin>,
    players: Query<&Transform, With<Player>>,
) {
    let showing = match state.get() {
        GameState::Paused => true,
        GameState::Menu => *view != MenuView::Home,
        GameState::Playing => false,
    };
    if !showing {
        return;
    }
    let step: i32 = if pad.rb_just {
        1
    } else if pad.lb_just {
        -1
    } else {
        return;
    };
    let Some(i) = TAB_ORDER.iter().position(|t| *t == *view) else { return };
    let n = TAB_ORDER.len() as i32;
    let next = TAB_ORDER[(i as i32 + step).rem_euclid(n) as usize];
    *view = next;
    if next == MenuView::Map {
        if let Ok(pt) = players.single() {
            map.focus(origin.to_geodetic(pt.translation), crate::globe::PAUSE_ZOOM);
        }
    }
}

/// Keep the active tab lit. Other tabs go back to plain only when the tab
/// changes, so their hover highlight still works between changes.
fn highlight_tabs(view: Res<MenuView>, focus: Res<MenuFocus>, mut tabs: Query<(Entity, &MenuAction, &mut BackgroundColor)>) {
    let changed = view.is_changed();
    for (e, action, mut bg) in &mut tabs {
        let MenuAction::Tab(t) = action else { continue };
        // The D-pad highlight on a tab wins; `pad_focus` repaints it when focus moves.
        if focus.0 == Some(e) {
            continue;
        }
        if *t == *view {
            if bg.0 != TAB_ON {
                bg.0 = TAB_ON;
            }
        } else if changed && bg.0 != BTN {
            bg.0 = BTN;
        }
    }
}

#[allow(deprecated, clippy::too_many_arguments)]
fn menu_buttons(
    mut q: Query<(&Interaction, &MenuAction, &mut BackgroundColor), Changed<Interaction>>,
    mut view: ResMut<MenuView>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    mut gfx: ResMut<GraphicsSettings>,
    mut dbg: ResMut<DebugSettings>,
    mut controls: ResMut<ControlSettings>,
    mut cosmetics: ResMut<Cosmetics>,
    mut map: ResMut<MapState>,
    origin: Res<crate::origin::WorldOrigin>,
    players: Query<&Transform, With<Player>>,
    clock: Res<Clock>,
    mut weather: ResMut<CurrentWeather>,
    mut pick: ResMut<WeatherPick>,
    mut over: ResMut<CloudOverride>,
    mut audio: ResMut<AudioSettings>,
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
            MenuAction::ToggleAa => {
                gfx.aa_mode = gfx.aa_mode.next();
                gfx.aa = gfx.aa_mode != AaMode::Off;
            }
            MenuAction::ToggleStats => gfx.show_stats = !gfx.show_stats,
            MenuAction::ToggleProbes => dbg.probe_gi = !dbg.probe_gi,
            MenuAction::CycleProbeView => dbg.probe_view = dbg.probe_view.next(),
            MenuAction::CycleNativeView => dbg.native_view = dbg.native_view.next(),
            MenuAction::CycleOceanFft => dbg.ocean_fft = dbg.ocean_fft.next(),
            MenuAction::CycleResolution => gfx.render_pct = match gfx.render_pct { 100 => 90, 90 => 85, 85 => 75, 75 => 67, 67 => 50, _ => 100 },
            MenuAction::TimeMinus => {
                gfx.time_live = false;
                map.time_target_offset = None;
                gfx.hour_offset -= 1.0;
                // The chosen hour follows the clock, so later clicks and
                // drops keep the hour the player just set.
                gfx.solar_hour = (gfx.solar_hour - 1.0).rem_euclid(24.0);
            }
            MenuAction::TimePlus => {
                gfx.time_live = false;
                map.time_target_offset = None;
                gfx.hour_offset += 1.0;
                gfx.solar_hour = (gfx.solar_hour + 1.0).rem_euclid(24.0);
            }
            MenuAction::TimeNow => {
                let real = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(clock.unix_s);
                gfx.hour_offset += (real - clock.unix_s) / 3600.0;
                gfx.time_live = true;
                map.time_target_offset = None;
                // Live time: the chosen hour is the player's local hour now.
                let lon = players
                    .single()
                    .map(|pt| origin.to_geodetic(pt.translation).lon_deg)
                    .unwrap_or(0.0);
                gfx.solar_hour = ed_geo::sun::solar_hour(real, lon);
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
            MenuAction::ToggleMute => audio.muted = !audio.muted,
            MenuAction::ToggleCalm => audio.calm = !audio.calm,
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_menu(
    state: Res<State<GameState>>,
    view: Res<MenuView>,
    spawner: Res<Spawner>,
    gfx: Res<GraphicsSettings>,
    dbg: Res<DebugSettings>,
    pick: Res<WeatherPick>,
    controls: Res<ControlSettings>,
    cosmetics: Res<Cosmetics>,
    audio: Res<AudioSettings>,
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
            MenuAction::ToggleAa => gfx.aa_mode.label().to_string(),
            MenuAction::CycleResolution => if gfx.aa_mode == AaMode::Upscale { format!("{}%", gfx.render_pct) } else { "native".to_string() },
            MenuAction::ToggleStats => on(gfx.show_stats),
            MenuAction::ToggleProbes => on(dbg.probe_gi),
            MenuAction::CycleProbeView => dbg.probe_view.label().to_string(),
            MenuAction::CycleNativeView => dbg.native_view.label().to_string(),
            MenuAction::CycleOceanFft => dbg.ocean_fft.label().to_string(),
            MenuAction::TimePlus => if gfx.time_live { "Now".into() } else { "Custom".into() },
            MenuAction::SetWeather(_) => pick.name.to_string(),
            MenuAction::SensPlus => format!("{:.1}", controls.mouse_sensitivity),
            MenuAction::InvertY => on(controls.invert_y),
            MenuAction::ToggleBackpack => on(cosmetics.backpack),
            MenuAction::ToggleMute => on(audio.muted),
            MenuAction::ToggleCalm => on(audio.calm),
            _ => String::new(),
        };
    }
}

/// Live diagnostics shown beside the Debug controls. The same data can also be
/// pinned to the HUD while playing with the "Live diagnostics" toggle.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_debug_readout(
    state: Res<State<GameState>>,
    view: Res<MenuView>,
    time: Res<Time>,
    gfx: Res<GraphicsSettings>,
    dbg: Res<DebugSettings>,
    origin: Res<crate::origin::WorldOrigin>,
    mgr: Option<Res<crate::streaming::TileManager>>,
    dm: Option<Res<crate::streaming::DetailManager>>,
    players: Query<&Transform, With<Player>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    windows: Query<&bevy::window::Window, With<PrimaryWindow>>,
    mut readout: Query<&mut Text, With<DebugReadout>>,
) {
    if *state.get() != GameState::Paused || *view != MenuView::Debug {
        return;
    }
    let Ok(cam) = cams.single() else {
        if let Ok(mut t) = readout.single_mut() {
            let s = "LIVE DIAGNOSTICS\n\nNo world is running yet. Start a drop to collect live stats.";
            if t.0 != s {
                t.0 = s.to_string();
            }
        }
        return;
    };
    let pos = players.single().map(|t| t.translation).unwrap_or(cam.translation());
    let g = origin.to_geodetic(pos);
    let tile = ed_geo::tile::TileId::containing(g.lat_deg, g.lon_deg, 14);
    let (tiles_loaded, tiles_pending, tiles_desired) = mgr.as_ref().map_or((0, 0, 0), |m| (m.loaded.len(), m.pending.len(), m.desired.len()));
    let (p_io, p_build, p_apply) = mgr.as_ref().map_or((0, 0, 0), |m| m.pipeline());
    let (buildings, trees, tris) = mgr.as_ref().map_or((0, 0, 0), |m| (m.totals.buildings, m.totals.trees, m.totals.triangles));
    let (detail_loaded, detail_flight) = dm.as_ref().map_or((0, 0), |d| crate::streaming::detail_stats(d));
    let (win_w, win_h, win_scale) = windows.single().map_or((0, 0, 1.0), |w| (w.physical_width(), w.physical_height(), w.scale_factor()));
    let fps = 1.0 / time.delta_secs().max(1e-4);
    let ms = time.delta_secs_f64() * 1000.0;
    let sea = mgr.as_ref().map_or(false, |m| m.sea_near());
    let s = format!(
        "LIVE DIAGNOSTICS\n\n\
         {fps:.0} fps  /  {ms:.1} ms\n\
         render {}%   {}x{} @{win_scale:.1}x\n\
         lat {:.5}   lon {:.5}   alt {:.0} m\n\
         tile {}/{}/{}\n\n\
         tiles {} loaded / {} pending / {} desired\n\
         pipeline io {} / build {} / apply {}\n\
         detail {} loaded / {} in flight\n\
         buildings {}   trees {}   tris {:.1} M\n\n\
         probe GI {} (20 km)   view {}\n\
         shadows {}   AO {}   bloom {}\n\
         ocean FFT {}   sea near {}\n\
         Metal view {}\n\n\
         F3 cycles probe spheres. Renderer switches apply live.",
        gfx.render_pct,
        win_w,
        win_h,
        g.lat_deg,
        g.lon_deg,
        g.alt_m,
        tile.z,
        tile.x,
        tile.y,
        tiles_loaded,
        tiles_pending,
        tiles_desired,
        p_io,
        p_build,
        p_apply,
        detail_loaded,
        detail_flight,
        buildings,
        trees,
        tris as f32 / 1e6,
        if dbg.probe_gi { "On" } else { "Off" },
        dbg.probe_view.label(),
        if gfx.shadows { "On" } else { "Off" },
        if gfx.ambient_occlusion { "On" } else { "Off" },
        if gfx.bloom { "On" } else { "Off" },
        dbg.ocean_fft.label(),
        if sea { "yes" } else { "no" },
        dbg.native_view.label(),
    );
    if let Ok(mut t) = readout.single_mut() {
        if t.0 != s {
            t.0 = s;
        }
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
    mut map: ResMut<MapState>,
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
    let solar_hour = |unix_s: f64| ed_geo::sun::solar_hour(unix_s, lon);
    let Ok((rcp, vis)) = track.single() else { return };
    // Hidden nodes keep a stale RelativeCursorPosition; ignore the track off its tab.
    if buttons.just_pressed(MouseButton::Left) && vis.get() && rcp.cursor_over {
        *dragging = true;
    }
    if !buttons.pressed(MouseButton::Left) {
        *dragging = false;
    }
    if *dragging && let Some(n) = rcp.normalized {
        map.time_target_offset = None;
        let target = ((n.x as f64 + 0.5).clamp(0.0, 1.0)) * 24.0;
        let mut delta = target - solar_hour(clock.unix_s);
        delta -= 24.0 * (delta / 24.0).round();
        gfx.time_live = false;
        gfx.hour_offset += delta;
        // The slider sets the hour the player wants from here on.
        gfx.solar_hour = target.rem_euclid(24.0);
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

/// Drag (or click) any audio slider; knobs and readouts follow the settings.
#[allow(clippy::type_complexity)]
fn audio_sliders(
    state: Res<State<GameState>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut audio: ResMut<AudioSettings>,
    tracks: Query<(&Knob, &RelativeCursorPosition, &InheritedVisibility)>,
    mut dots: Query<(&KnobDot, &mut Node)>,
    mut readouts: Query<(&KnobReadout, &mut Text)>,
    mut dragging: Local<Option<Knob>>,
) {
    if *state.get() == GameState::Playing {
        *dragging = None;
        return;
    }
    if !buttons.pressed(MouseButton::Left) {
        *dragging = None;
    }
    for (k, rcp, vis) in &tracks {
        // Hidden nodes keep a stale RelativeCursorPosition; only the visible tab counts.
        if buttons.just_pressed(MouseButton::Left) && vis.get() && rcp.cursor_over {
            *dragging = Some(*k);
        }
        if *dragging == Some(*k) && let Some(n) = rcp.normalized {
            let v = ((n.x + 0.5).clamp(0.0, 1.0) * 100.0).round() / 100.0;
            if k.value(&audio) != v {
                *k.slot(&mut audio) = v;
            }
        }
    }
    // First run counts as changed too, so the readouts fill in on launch.
    if !audio.is_changed() {
        return;
    }
    for (d, mut n) in &mut dots {
        let want = Val::Percent(d.0.value(&audio) * 100.0);
        if n.left != want {
            n.left = want;
        }
    }
    for (r, mut t) in &mut readouts {
        let s = format!("{:.0}%", r.0.value(&audio) * 100.0);
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

pub fn apply_graphics(
    mut gfx: ResMut<GraphicsSettings>,
    mut clock: ResMut<crate::sky::Clock>,
    mut commands: Commands,
    cams: Query<(Entity, Has<ScreenSpaceAmbientOcclusion>, Has<Bloom>, Has<TemporalAntiAliasing>), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut suns: Query<&mut DirectionalLight, Without<crate::globe::GlobeSun>>,
) {
    if !gfx.is_changed() {
        return;
    }
    let last = gfx.hour_offset_applied;
    clock.unix_s += (gfx.hour_offset - last) * 3600.0;
    gfx.hour_offset_applied = gfx.hour_offset;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_view_cycles_through_every_mode() {
        let mut v = ProbeView::Off;
        for expected in [ProbeView::Colors, ProbeView::Lit, ProbeView::Off] {
            v = v.next();
            assert_eq!(v, expected);
        }
    }

    #[test]
    fn native_debug_views_round_trip_to_shader_values() {
        let mut v = NativeDebugView::Off;
        for _ in 0..19 {
            v = v.next();
            assert_eq!(NativeDebugView::from_debug_value(v.value()), v);
        }
        assert_eq!(v, NativeDebugView::Off);
        assert_eq!(NativeDebugView::IndirectDiffuse.value(), 18);
        assert_eq!(NativeDebugView::ProbeWeight.value(), 19);
        assert_eq!(NativeDebugView::Shadow4.value(), 16);
        assert_eq!(NativeDebugView::Shadow5.value(), 17);
    }

    #[test]
    fn ocean_fft_cycles_auto_on_off() {
        assert_eq!(OceanFft::Auto.next(), OceanFft::On);
        assert_eq!(OceanFft::On.next(), OceanFft::Off);
        assert_eq!(OceanFft::Off.next(), OceanFft::Auto);
    }
}
