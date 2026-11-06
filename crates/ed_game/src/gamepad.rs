//! Gamepad support.
//!
//! One `Pad` snapshot is read per frame (sticks with a deadzone, triggers,
//! and button presses), so every system asks the same question the same way.
//! `InputMode` flips to `Controller` on the first stick or button input and
//! back to `Keyboard` on the first key or mouse input, so the crosshair and
//! the cursor follow whichever device is in use.
//!
//! Gameplay: left stick move (WASD), right stick look, RT jump (Space),
//! LT sprint and afterburner (Shift; the pull amount sets the boost, and RT sets the rocket thrust), A wingsuit (F), B parachute (P),
//! Start pause (Esc).
//! Menus: LB and RB switch tabs. Map: left stick pans, right stick zooms,
//! A picks the spot under the centre crosshair; with the drop popup open,
//! A drops from the sky and B cancels.

use crate::menu::{GameState, MenuView};
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

/// Stick travel below this (of full) is ignored, so a resting stick is still.
const DEADZONE: f32 = 0.18;
/// A trigger counts as held past this much travel.
pub const TRIGGER_ON: f32 = 0.06;

/// Camera look speed for the right stick, radians per second at sensitivity 1.
pub const PAD_LOOK_RATE: f32 = 2.4;
/// Map pan speed for the left stick, in screen widths per second.
pub const PAD_PAN_SCREENS_PER_S: f32 = 0.6;
/// Map zoom speed for the right stick, in map zoom levels per second.
pub const PAD_ZOOM_LEVELS_PER_S: f32 = 1.2;

/// This frame's controller state, merged over every connected pad.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Pad {
    /// x: right, y: up. Each in -1..1 after the deadzone.
    pub left: Vec2,
    pub right: Vec2,
    pub lt_held: bool,
    pub rt_held: bool,
    pub rt_just: bool,
    pub rt_released: bool,
    pub a_just: bool,
    pub b_just: bool,
    pub start_just: bool,
    pub lb_just: bool,
    pub rb_just: bool,
    /// Menu navigation step this frame (D-pad press or left-stick flick), screen
    /// axes: x right, y down. Zero when nothing was pressed.
    pub nav: IVec2,
    /// Analog trigger travel, 0..1. A little pull is a little boost.
    pub lt: f32,
    pub rt: f32,
}

impl Pad {
    fn any_press(&self) -> bool {
        self.a_just
            || self.b_just
            || self.start_just
            || self.lb_just
            || self.rb_just
            || self.rt_just
            || self.nav != IVec2::ZERO
            || self.lt > TRIGGER_ON
    }
}

/// Which device the player is using right now.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputMode {
    #[default]
    Keyboard,
    Controller,
}

/// The centre crosshair on the map. Shown only in controller mode.
#[derive(Component)]
pub struct Crosshair;

pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pad>()
            .init_resource::<InputMode>()
            .add_systems(PreUpdate, (read_pad, apply_script).chain().after(bevy::input::InputSystems))
            .add_systems(Update, (announce_gamepads, detect_input_mode, sync_cursor, sync_crosshair, update_hints).chain());
    }
}

fn dead(v: Vec2) -> Vec2 {
    let len = v.length();
    if len < DEADZONE {
        Vec2::ZERO
    } else {
        v / len * ((len - DEADZONE) / (1.0 - DEADZONE)).min(1.0)
    }
}

fn read_pad(gamepads: Query<&Gamepad>, mut pad: ResMut<Pad>, mut prev_rt: Local<bool>, mut prev_stick: Local<IVec2>) {
    let mut p = Pad::default();
    let mut raw_left = Vec2::ZERO;
    for g in &gamepads {
        p.left += dead(g.left_stick());
        p.right += dead(g.right_stick());
        raw_left += g.left_stick();
        p.lt = p.lt.max(g.get(GamepadButton::LeftTrigger2).unwrap_or(0.0));
        p.rt = p.rt.max(g.get(GamepadButton::RightTrigger2).unwrap_or(0.0));
        p.a_just |= g.just_pressed(GamepadButton::South);
        p.b_just |= g.just_pressed(GamepadButton::East);
        p.start_just |= g.just_pressed(GamepadButton::Start);
        p.lb_just |= g.just_pressed(GamepadButton::LeftTrigger);
        p.rb_just |= g.just_pressed(GamepadButton::RightTrigger);
        if g.just_pressed(GamepadButton::DPadRight) {
            p.nav.x += 1;
        }
        if g.just_pressed(GamepadButton::DPadLeft) {
            p.nav.x -= 1;
        }
        if g.just_pressed(GamepadButton::DPadDown) {
            p.nav.y += 1;
        }
        if g.just_pressed(GamepadButton::DPadUp) {
            p.nav.y -= 1;
        }
    }
    p.left = p.left.clamp_length_max(1.0);
    p.right = p.right.clamp_length_max(1.0);
    p.lt = p.lt.clamp(0.0, 1.0);
    p.rt = p.rt.clamp(0.0, 1.0);
    p.lt_held = p.lt > TRIGGER_ON;
    p.rt_held = p.rt > TRIGGER_ON;
    p.rt_just = p.rt_held && !*prev_rt;
    p.rt_released = !p.rt_held && *prev_rt;
    *prev_rt = p.rt_held;
    // A stick flick past 0.6 steps menu focus once, on its dominant axis.
    let flick = if raw_left.length() > 0.6 {
        if raw_left.x.abs() > raw_left.y.abs() { IVec2::new(raw_left.x.signum() as i32, 0) } else { IVec2::new(0, -raw_left.y.signum() as i32) }
    } else {
        IVec2::ZERO
    };
    if flick != IVec2::ZERO && flick != *prev_stick && p.nav == IVec2::ZERO {
        p.nav = flick;
    }
    *prev_stick = flick;
    *pad = p;
}

/// Any controller input switches to controller mode; any key, mouse button or
/// real mouse movement switches back. Nothing happening keeps the current mode.
fn detect_input_mode(
    pad: Res<Pad>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut mode: ResMut<InputMode>,
) {
    let pad_active = pad.left != Vec2::ZERO
        || pad.right != Vec2::ZERO
        || pad.lt_held
        || pad.rt_held
        || pad.any_press();
    let keys_active = keys.get_just_pressed().next().is_some()
        || mouse.get_just_pressed().next().is_some()
        || motion.delta.length_squared() > 0.25;
    let want = if pad_active {
        InputMode::Controller
    } else if keys_active {
        InputMode::Keyboard
    } else {
        *mode
    };
    mode.set_if_neq(want);
}

/// In menus, hide the mouse pointer while a controller is in use. Gameplay
/// keeps the cursor captured, which `menu.rs` handles on state entry.
fn sync_cursor(
    mode: Res<InputMode>,
    state: Res<State<GameState>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if *state.get() == GameState::Playing {
        return;
    }
    let visible = *mode == InputMode::Keyboard;
    for mut c in &mut cursors {
        if c.visible != visible {
            c.visible = visible;
        }
    }
}

fn sync_crosshair(
    mode: Res<InputMode>,
    loading: Option<Res<crate::loading::LoadingScreen>>,
    mut q: Query<&mut Visibility, With<Crosshair>>,
) {
    let want = match *mode {
        InputMode::Controller if !loading.is_some_and(|l| l.active) => Visibility::Inherited,
        _ => Visibility::Hidden,
    };
    for mut v in &mut q {
        v.set_if_neq(want);
    }
}

/// A small white crosshair centred in the map canvas. Its parent is a zero-size
/// node at the canvas centre, and the arms are offset from it to straddle that point.
pub(crate) fn spawn_crosshair(p: &mut bevy::ecs::hierarchy::ChildSpawnerCommands) {
    const ARM: f32 = 26.0;
    const THICK: f32 = 2.0;
    p.spawn((
        Crosshair,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            width: Val::Px(0.0),
            height: Val::Px(0.0),
            ..default()
        },
        ZIndex(5),
    ))
    .with_children(|c| {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-ARM / 2.0),
                top: Val::Px(-THICK / 2.0),
                width: Val::Px(ARM),
                height: Val::Px(THICK),
                ..default()
            },
            BackgroundColor(Color::WHITE),
        ));
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-THICK / 2.0),
                top: Val::Px(-ARM / 2.0),
                width: Val::Px(THICK),
                height: Val::Px(ARM),
                ..default()
            },
            BackgroundColor(Color::WHITE),
        ));
    });
}


/// Test hook: `EW_PAD_SCRIPT="40:down,60:a,80:ly=1,120:ly=0,150:shot=/tmp/x.png"`.
/// Each entry is `frame:token`. Button tokens press for one frame; `ly=`, `lx=`,
/// `ry=`, `rx=` hold a stick value until changed; `shot=path` saves a screenshot.
/// There is no controller on the dev machine, so this is how menus get driven.
fn apply_script(mut pad: ResMut<Pad>, mut commands: Commands, mut frame: Local<u32>, mut held: Local<[f32; 4]>) {
    let Ok(script) = std::env::var("EW_PAD_SCRIPT") else { return };
    *frame += 1;
    for ent in script.split(',') {
        let Some((f, tok)) = ent.split_once(':') else { continue };
        if f.trim().parse::<u32>().ok() != Some(*frame) {
            continue;
        }
        let tok = tok.trim();
        if let Some(path) = tok.strip_prefix("shot=") {
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(std::path::PathBuf::from(path)));
        } else if let Some((axis, v)) = tok.split_once('=') {
            let v: f32 = v.parse().unwrap_or(0.0);
            match axis {
                "lx" => held[0] = v,
                "ly" => held[1] = v,
                "rx" => held[2] = v,
                "ry" => held[3] = v,
                _ => {}
            }
        } else {
            match tok {
                "a" => pad.a_just = true,
                "b" => pad.b_just = true,
                "start" => pad.start_just = true,
                "lb" => pad.lb_just = true,
                "rb" => pad.rb_just = true,
                "down" => pad.nav = IVec2::new(0, 1),
                "up" => pad.nav = IVec2::new(0, -1),
                "right" => pad.nav = IVec2::new(1, 0),
                "left" => pad.nav = IVec2::new(-1, 0),
                _ if tok.starts_with("rt=") => {
                    pad.rt = tok[3..].parse().unwrap_or(0.0);
                    pad.rt_held = pad.rt > TRIGGER_ON;
                }
                _ if tok.starts_with("lt=") => {
                    pad.lt = tok[3..].parse().unwrap_or(0.0);
                    pad.lt_held = pad.lt > TRIGGER_ON;
                }
                _ => {}
            }
        }
    }
    pad.left += Vec2::new(held[0], held[1]);
    pad.right += Vec2::new(held[2], held[3]);
}


// ---- Button prompts ----

/// The strip of "(A) Select  (B) Back" prompts along the bottom. Controller mode only.
#[derive(Component)]
struct HintBar;

/// One button face: an Xbox-style coloured disc for A/B/X/Y, a pill for the rest.
fn glyph(p: &mut bevy::ecs::hierarchy::ChildSpawnerCommands, label: &str) {
    let (bg, round) = match label {
        "A" => (Color::srgb(0.30, 0.68, 0.31), true),
        "B" => (Color::srgb(0.85, 0.27, 0.25), true),
        "X" => (Color::srgb(0.20, 0.50, 0.85), true),
        "Y" => (Color::srgb(0.92, 0.75, 0.15), true),
        _ => (Color::srgba(1.0, 1.0, 1.0, 0.22), false),
    };
    p.spawn((
        Node {
            min_width: Val::Px(24.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(if round { 0.0 } else { 8.0 })),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(bg),
    ))
    .with_children(|g| {
        g.spawn((Text::new(label), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::WHITE)));
    });
}

/// What the buttons do right now.
fn hints_for(state: GameState, view: MenuView, popup: bool) -> &'static [(&'static str, &'static str)] {
    match (state, view) {
        (_, _) if popup => &[("A", "Drop in"), ("B", "Cancel")],
        (GameState::Playing, _) => &[("L", "Move"), ("R", "Look"), ("RT", "Rocket"), ("LT", "Boost"), ("A", "Wingsuit"), ("B", "Parachute"), ("Start", "Pause")],
        (GameState::Menu, MenuView::Home) => &[],
        (GameState::Paused, MenuView::Map) => &[("L", "Pan"), ("R", "Zoom"), ("A", "Choose spot"), ("LB", "Tab"), ("RB", "Tab"), ("B", "Resume")],
        (GameState::Menu, MenuView::Map) => &[("L", "Pan"), ("R", "Zoom"), ("A", "Choose spot"), ("LB", "Tab"), ("RB", "Tab"), ("B", "Back")],
        // Button screens: the prompts sit on the buttons themselves (`menu::update_badges`).
        (GameState::Paused | GameState::Menu, _) => &[],
    }
}

fn update_hints(
    mut commands: Commands,
    mode: Res<InputMode>,
    state: Res<State<GameState>>,
    view: Res<MenuView>,
    map: Res<crate::mapview::MapState>,
    loading: Option<Res<crate::loading::LoadingScreen>>,
    bars: Query<Entity, With<HintBar>>,
    mut last: Local<Option<(InputMode, GameState, MenuView, bool, bool)>>,
) {
    let loading = loading.is_some_and(|l| l.active);
    let key = (*mode, *state.get(), *view, map.selected.is_some(), loading);
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    for e in &bars {
        commands.entity(e).despawn();
    }
    if *mode != InputMode::Controller || key.4 {
        return;
    }
    let items = hints_for(key.1, key.2, key.3);
    if items.is_empty() {
        return;
    }
    // In game the prompts stack in a top-right panel so the bottom HUD stays clear;
    // menus keep them along the bottom.
    let in_game = key.1 == GameState::Playing && !key.3;
    let layout = if in_game {
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            right: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            align_items: AlignItems::FlexEnd,
            ..default()
        }
    } else {
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(18.0),
            ..default()
        }
    };
    commands
        .spawn((HintBar, layout, ZIndex(40), Pickable::IGNORE))
        .with_children(|bar| {
            for (g, label) in items {
                bar.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(14.0)), ..default() })
                    .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)))
                    .with_children(|c| {
                        glyph(c, g);
                        c.spawn((Text::new(*label), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.96, 0.96, 0.95))));
                    });
            }
        });
}

/// Plugging in or waking a pad switches to controller mode at once (so the prompts show
/// before the first button press) and logs it, which also tells you whether the OS
/// handed the pad to the game at all.
fn announce_gamepads(new: Query<(&Gamepad, Option<&Name>), Added<Gamepad>>, mut mode: ResMut<InputMode>) {
    for (_, name) in &new {
        info!("gamepad connected: {}", name.map(|n| n.as_str()).unwrap_or("unnamed"));
        mode.set_if_neq(InputMode::Controller);
    }
}
