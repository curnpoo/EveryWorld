//! Gamepad support.
//!
//! One `Pad` snapshot is read per frame (sticks with a deadzone, triggers,
//! and button presses), so every system asks the same question the same way.
//! `InputMode` flips to `Controller` on the first stick or button input and
//! back to `Keyboard` on the first key or mouse input, so the crosshair and
//! the cursor follow whichever device is in use.
//!
//! Gameplay: left stick move (WASD), right stick look, RT jump (Space),
//! LT sprint and afterburner (Shift), A wingsuit (F), B parachute (P),
//! Start pause (Esc).
//! Menus: LB and RB switch tabs. Map: left stick pans, right stick zooms,
//! A picks the spot under the centre crosshair; with the drop popup open,
//! A drops from the sky and B cancels.

use crate::menu::GameState;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};

/// Stick travel below this (of full) is ignored, so a resting stick is still.
const DEADZONE: f32 = 0.18;

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
    /// D-pad down or right pressed this frame: next control in menus.
    pub dpad_next_just: bool,
    /// D-pad up or left pressed this frame: previous control in menus.
    pub dpad_prev_just: bool,
}

impl Pad {
    fn any_press(&self) -> bool {
        self.a_just
            || self.b_just
            || self.start_just
            || self.lb_just
            || self.rb_just
            || self.rt_just
            || self.dpad_next_just
            || self.dpad_prev_just
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
            .add_systems(PreUpdate, read_pad.after(bevy::input::InputSystems))
            .add_systems(Update, (detect_input_mode, sync_cursor, sync_crosshair).chain());
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

fn read_pad(gamepads: Query<&Gamepad>, mut pad: ResMut<Pad>) {
    let mut p = Pad::default();
    for g in &gamepads {
        p.left += dead(g.left_stick());
        p.right += dead(g.right_stick());
        p.lt_held |= g.pressed(GamepadButton::LeftTrigger2);
        p.rt_held |= g.pressed(GamepadButton::RightTrigger2);
        p.rt_just |= g.just_pressed(GamepadButton::RightTrigger2);
        p.rt_released |= g.just_released(GamepadButton::RightTrigger2);
        p.a_just |= g.just_pressed(GamepadButton::South);
        p.b_just |= g.just_pressed(GamepadButton::East);
        p.start_just |= g.just_pressed(GamepadButton::Start);
        p.lb_just |= g.just_pressed(GamepadButton::LeftTrigger);
        p.rb_just |= g.just_pressed(GamepadButton::RightTrigger);
        p.dpad_next_just |= g.just_pressed(GamepadButton::DPadDown) || g.just_pressed(GamepadButton::DPadRight);
        p.dpad_prev_just |= g.just_pressed(GamepadButton::DPadUp) || g.just_pressed(GamepadButton::DPadLeft);
    }
    p.left = p.left.clamp_length_max(1.0);
    p.right = p.right.clamp_length_max(1.0);
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

fn sync_crosshair(mode: Res<InputMode>, mut q: Query<&mut Visibility, With<Crosshair>>) {
    let want = match *mode {
        InputMode::Controller => Visibility::Inherited,
        InputMode::Keyboard => Visibility::Hidden,
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
