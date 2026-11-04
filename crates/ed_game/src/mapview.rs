//! The world map used by the main menu and the pause menu.
//!
//! The map itself is a satellite globe rendered by `crate::globe` into any UI
//! node carrying a `MapCanvas`. This module keeps the parts that are plain UI:
//! the shared `MapState` (what is selected, where to fly to next), the
//! "drop here" popup and its buttons.

use crate::spawn::{SpawnHeight, SpawnRequest};
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use ed_geo::Geodetic;

pub struct MapViewPlugin;

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapState>().add_systems(Update, popup_buttons);
    }
}

/// Marks the UI node the map draws into.
#[derive(Component)]
pub struct MapCanvas;

/// The orange pin shown where the user clicked.
#[derive(Component)]
pub(crate) struct Marker;

#[derive(Component)]
pub(crate) struct Popup;

#[derive(Component)]
pub(crate) struct PopupTitle;

#[derive(Component, Clone, Copy)]
enum PopupAction {
    Sky,
    Ground,
    Close,
}

/// Shared between the menus, the globe and the spawn flow.
#[derive(Resource, Default)]
pub struct MapState {
    /// The place picked on the globe, waiting for "Drop in here".
    pub selected: Option<Geodetic>,
    /// A fly-to the globe has not consumed yet: place and zoom level.
    pub(crate) focus_req: Option<(Geodetic, f64)>,
}

impl MapState {
    /// Fly the globe to a place. `zoom` is a map-style level: 0 shows the
    /// whole planet, each +1 halves the height above the surface.
    pub fn focus(&mut self, g: Geodetic, zoom: f64) {
        self.focus_req = Some((g, zoom));
    }
}

/// Scroll this frame in wheel notches. Trackpads report pixels (tens per
/// frame), so convert and cap to keep zooming gentle.
pub(crate) fn scroll_notches(scroll: &AccumulatedMouseScroll) -> f32 {
    let n = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 80.0,
    };
    n.clamp(-1.0, 1.0)
}

/// The pick pin and the "drop here" popup, as children of the map canvas.
pub(crate) fn build_overlays(commands: &mut Commands, canvas: Entity) {
    commands.spawn((
        Marker,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(18.0),
            height: Val::Px(18.0),
            border: UiRect::all(Val::Px(4.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgb(0.98, 0.42, 0.1)),
        BorderColor::all(Color::WHITE),
        Visibility::Hidden,
        ZIndex(3),
        ChildOf(canvas),
    ));
    commands
        .spawn((
            Popup,
            RelativeCursorPosition::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(260.0),
                padding: UiRect::all(Val::Px(14.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.08, 0.1, 0.92)),
            Visibility::Hidden,
            ZIndex(4),
            ChildOf(canvas),
        ))
        .with_children(|p| {
            p.spawn((Text::new("Drop in here"), TextFont { font_size: FontSize::Px(17.0), ..default() }, TextColor(Color::WHITE)));
            p.spawn((PopupTitle, Text::new(""), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6))));
            for (label, action, accent) in [("Drop from the sky", PopupAction::Sky, true), ("Start on the ground", PopupAction::Ground, false), ("Cancel", PopupAction::Close, false)] {
                crate::menu::button(p, label, action, accent);
            }
        });
}

#[allow(deprecated)]
fn popup_buttons(
    mut st: ResMut<MapState>,
    q: Query<(&Interaction, &PopupAction), Changed<Interaction>>,
    mut spawns: MessageWriter<SpawnRequest>,
) {
    for (i, a) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        let Some(sel) = st.selected else { continue };
        match a {
            PopupAction::Close => st.selected = None,
            PopupAction::Sky | PopupAction::Ground => {
                let height = if matches!(a, PopupAction::Sky) { SpawnHeight::Sky } else { SpawnHeight::Ground };
                spawns.write(SpawnRequest::At { pos: sel, name: String::new(), height });
                st.selected = None;
            }
        }
    }
}
