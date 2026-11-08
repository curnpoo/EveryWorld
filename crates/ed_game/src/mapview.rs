//! The world map used by the main menu and the pause menu.
//!
//! The map itself is a satellite globe rendered by `crate::globe` into any UI
//! node carrying a `MapCanvas`. This module keeps the parts that are plain UI:
//! the shared `MapState` (what is selected, where to fly to next), the
//! "drop here" popup and its buttons.

use crate::spawn::{SpawnHeight, SpawnRequest};
use bevy::input::{
    ButtonState,
    keyboard::KeyboardInput,
    mouse::{AccumulatedMouseScroll, MouseScrollUnit},
};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, block_on, futures_lite::future};
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use ed_geo::Geodetic;

pub struct MapViewPlugin;

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapState>()
            // After `menu::open_pause` so an Escape that blurs the search field is
            // never also the frame that closes the menu.
            .add_systems(
                Update,
                (search_input, search_paint, waypoint_button_input)
                    .after(crate::menu::open_pause)
                    .before(crate::globe::GlobeControl),
            )
            .add_systems(
                Update,
                (popup_buttons, pad_popup.after(crate::globe::GlobeControl)),
            )
            .add_systems(Update, ease_map_time.before(crate::menu::apply_graphics));
    }
}

/// Marks the UI node the map draws into.
#[derive(Component)]
pub struct MapCanvas;

/// The orange pin shown where the user clicked.
#[derive(Component)]
pub(crate) struct Marker;

#[derive(Component)]
pub(crate) struct WaypointMarker;

#[derive(Component)]
pub(crate) struct Popup;

#[derive(Component)]
pub(crate) struct PopupTitle;

// ---- search bar (mouse only) ---------------------------------------------------------

/// Panel at the map's top-left: text field + search button + status line.
#[derive(Component)]
pub(crate) struct SearchBar;

#[derive(Component)]
struct SearchField;

#[derive(Component)]
struct SearchFieldText;

#[derive(Component)]
struct SearchAsk;

#[derive(Component)]
struct SearchStatus;

/// The little map pin of a search result, while it is falling.
#[derive(Component)]
pub(crate) struct SearchPin;

#[derive(Component)]
pub(crate) struct WaypointPlaceButton;

/// Expanding impact ring, visible during the pin's landing bounce.
#[derive(Component)]
pub(crate) struct SearchRing;

// Search widget constants. The bar docks in the canvas' top-left corner.
const FIELD_W: f32 = 260.0;
const SEARCH_MAX_CHARS: usize = 80;
const SEARCH_PLACEHOLDER: &str = "Search places, regions, addresses…";
#[derive(Component, Clone, Copy, PartialEq)]
enum PopupAction {
    Sky,
    Ground,
    Waypoint,
    Close,
}

/// Shared between the menus, the globe and the spawn flow.
#[derive(Resource, Default)]
pub struct MapState {
    /// The place picked on the globe, waiting for "Drop in here".
    pub selected: Option<Geodetic>,
    /// Place name for the current selection (from the search bar), shown in
    /// the popup title and carried into the spawn.
    pub(crate) selected_name: String,
    /// A fly-to the globe has not consumed yet: place and zoom level.
    pub(crate) focus_req: Option<(Geodetic, f64)>,
    /// A place found by the search bar: the pin is falling towards it. When it
    /// lands it hands over to `selected` and the drop popup opens.
    pub(crate) landing: Option<Landing>,
    /// When true, the next globe click places a waypoint instead of selecting
    /// a drop point.
    pub waypoint_mode: bool,
    /// Pending waypoint from a globe click; consumed by the waypoint system.
    pub pending_waypoint: Option<Geodetic>,
    /// The search text field owns the keyboard.
    pub(crate) text_focus: bool,
    /// What is typed in the search field.
    pub(crate) search_query: String,
    /// Search progress for the small line under the bar ("Finding…", errors).
    pub(crate) search_status: String,
    /// Target hour-offset for the time-of-day ease triggered by a map
    /// selection: keeps the local solar hour of the place the camera was
    /// looking at before, so a daytime origin stays daytime at the destination.
    pub(crate) time_target_offset: Option<f64>,
}

/// The pin-drop for a search result.
pub(crate) struct Landing {
    pub place: Geodetic,
    pub name: String,
    /// Elapsed seconds when the result arrived; the fall starts on sight.
    pub created: f32,
    /// Elapsed seconds when the pin first appeared; `None` while waiting.
    pub started: Option<f32>,
}

impl MapState {
    /// Fly the globe to a place. `zoom` is a map-style level: 0 shows the
    /// whole planet, each +1 halves the height above the surface.
    pub fn focus(&mut self, g: Geodetic, zoom: f64) {
        self.focus_req = Some((g, zoom));
    }

    /// A map click: highlight the spot and open the drop popup next to it.
    /// Cancels any search pin that is still in the air.
    pub fn select(&mut self, g: Geodetic) {
        self.selected = Some(g);
        self.selected_name.clear();
        self.landing = None;
    }

    /// A search result arrives: remember the place, the globe handles zooming
    /// in, and the pin falls on it before the popup opens (small delay).
    pub(crate) fn start_landing(&mut self, place: Geodetic, name: impl Into<String>, now: f32) {
        self.selected = None;
        self.selected_name.clear();
        self.landing = Some(Landing {
            place,
            name: name.into(),
            created: now,
            started: None,
        });
    }

    /// The search pin has landed: hand the highlight to the normal flow.
    pub(crate) fn land(&mut self) {
        if let Some(l) = self.landing.take() {
            self.selected = Some(l.place);
            self.selected_name = l.name;
        }
    }
}

/// Attribution strip docked at the canvas' bottom-right corner.
///
/// Lists the open-data sources whose licences require visible credit:
/// © OpenStreetMap contributors (ODbL), OpenFreeMap, AWS Terrain.
#[allow(deprecated)]
fn build_attribution(commands: &mut Commands, canvas: Entity) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                right: Val::Px(0.0),
                padding: UiRect {
                    left: Val::Px(6.0),
                    right: Val::Px(6.0),
                    top: Val::Px(3.0),
                    bottom: Val::Px(3.0),
                },
                border_radius: BorderRadius::top_left(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.52)),
            ZIndex(5),
            Pickable::IGNORE,
            ChildOf(canvas),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("© OpenStreetMap contributors · OpenFreeMap · AWS Terrain"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.72)),
                Pickable::IGNORE,
            ));
        });
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
#[allow(deprecated)]
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
    // Waypoint marker on the globe (green, same size as Marker).
    commands.spawn((
        WaypointMarker,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(18.0),
            height: Val::Px(18.0),
            border: UiRect::all(Val::Px(4.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgb(0.2, 0.85, 0.3)),
        BorderColor::all(Color::WHITE),
        Visibility::Hidden,
        ZIndex(3),
        ChildOf(canvas),
    ));
    // Search pin: a little map pin that falls onto the searched spot. The tip
    // is the node's bottom centre, so `top` is anchored so it points at the geo.
    commands
        .spawn((
            SearchPin,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(26.0),
                height: Val::Px(38.0),
                ..default()
            },
            UiTransform::IDENTITY,
            Visibility::Hidden,
            ZIndex(6),
            Pickable::IGNORE,
            ChildOf(canvas),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(11.0),
                    top: Val::Px(22.0),
                    width: Val::Px(4.0),
                    height: Val::Px(16.0),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.82, 0.34, 0.08)),
            ));
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(1.0),
                    top: Val::Px(0.0),
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    border: UiRect::all(Val::Px(4.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.98, 0.42, 0.1)),
                BorderColor::all(Color::WHITE),
            ));
        });
    // One-shot impact ripple under the pin when it touches down.
    commands.spawn((
        SearchRing,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(30.0),
            height: Val::Px(30.0),
            border: UiRect::all(Val::Px(2.5)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::srgba(0.98, 0.42, 0.1, 0.8)),
        UiTransform::IDENTITY,
        Visibility::Hidden,
        ZIndex(2),
        Pickable::IGNORE,
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
            p.spawn((
                Text::new("Drop in here"),
                TextFont {
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            p.spawn((
                PopupTitle,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            ));
            for (label, action, accent) in [
                ("Drop from the sky", PopupAction::Sky, true),
                ("Start on the ground", PopupAction::Ground, false),
                ("Set Waypoint", PopupAction::Waypoint, false),
                ("Cancel", PopupAction::Close, false),
            ] {
                crate::menu::button(p, label, action, accent);
            }
        });
    build_search_bar(commands, canvas);
    build_attribution(commands, canvas);
    // Waypoint placement button.
    commands.spawn((
        WaypointPlaceButton,
        Interaction::None,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            top: Val::Px(14.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(crate::menu::BTN),
        ZIndex(5),
        ChildOf(canvas),
    ))
    .with_children(|p| {
        p.spawn((
            Text::new("Set Waypoint"),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(crate::menu::INK),
        ));
    });
}

/// The search bar: rounded panel docked at the canvas' top-left, a text field
/// and a search button, with a small status line underneath. Mouse only.
#[allow(deprecated)]
fn build_search_bar(commands: &mut Commands, canvas: Entity) {
    commands
        .spawn((
            SearchBar,
            RelativeCursorPosition::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(14.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.08, 0.1, 0.92)),
            ZIndex(5),
            ChildOf(canvas),
        ))
        .with_children(|p| {
            p.spawn((Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },))
                .with_children(|row| {
                    row.spawn((
                        SearchField,
                        Interaction::None,
                        Node {
                            width: Val::Px(FIELD_W),
                            height: Val::Px(38.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(0.0)),
                            align_items: AlignItems::Center,
                            overflow: Overflow::clip(),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(10.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.22)),
                        BackgroundColor(crate::menu::BTN),
                    ))
                    .with_children(|f| {
                        f.spawn((
                            SearchFieldText,
                            Text::new(SEARCH_PLACEHOLDER),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(crate::menu::DIM),
                        ));
                    });
                    row.spawn((
                        SearchAsk,
                        Interaction::None,
                        Node {
                            height: Val::Px(38.0),
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(0.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(crate::menu::ORANGE),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("Search"),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(crate::menu::INK),
                        ));
                    });
                });
            p.spawn((
                SearchStatus,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(crate::menu::DIM),
                Visibility::Hidden,
            ));
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
            PopupAction::Waypoint => {
                let sel = st.selected.unwrap();
                st.pending_waypoint = Some(sel);
                st.selected = None;
                st.selected_name.clear();
            }
            PopupAction::Sky | PopupAction::Ground => {
                let height = if matches!(a, PopupAction::Sky) {
                    SpawnHeight::Sky
                } else {
                    SpawnHeight::Ground
                };
                spawns.write(SpawnRequest::At {
                    pos: sel,
                    name: st.selected_name.clone(),
                    height,
                });
                st.selected = None;
                st.selected_name.clear();
            }
        }
    }
}

const POPUP_ORDER: [PopupAction; 4] = [PopupAction::Sky, PopupAction::Ground, PopupAction::Waypoint, PopupAction::Close];

/// Controller path through the "drop here" popup: D-pad up/down moves the
/// highlight, A presses it, B cancels. Runs after the globe's `control`, so the
/// A press that opened the popup is never also read as a choice (`was_open`).
#[allow(deprecated)]
fn pad_popup(
    pad: Res<crate::gamepad::Pad>,
    mode: Res<crate::gamepad::InputMode>,
    mut st: ResMut<MapState>,
    mut spawns: MessageWriter<SpawnRequest>,
    mut idx: Local<usize>,
    mut was_open: Local<bool>,
    mut buttons: Query<(&PopupAction, &mut BackgroundColor)>,
) {
    let open = st.selected.is_some();
    let live = open && *was_open && *mode == crate::gamepad::InputMode::Controller;
    if open && !*was_open {
        *idx = 0;
    }
    *was_open = open;
    if live {
        if pad.nav.y != 0 {
            *idx =
                (*idx as i32 + pad.nav.y.signum()).clamp(0, POPUP_ORDER.len() as i32 - 1) as usize;
        }
        if pad.b_just {
            st.selected = None;
            st.selected_name.clear();
        } else if pad.a_just {
            let (Some(sel), action) = (st.selected, POPUP_ORDER[*idx]) else {
                return;
            };
            match action {
                PopupAction::Close => {}
                PopupAction::Waypoint => {
                    st.pending_waypoint = Some(sel);
                    st.selected = None;
                    st.selected_name.clear();
                }
                PopupAction::Sky | PopupAction::Ground => {
                    let height = if action == PopupAction::Sky {
                        SpawnHeight::Sky
                    } else {
                        SpawnHeight::Ground
                    };
                    spawns.write(SpawnRequest::At {
                        pos: sel,
                        name: st.selected_name.clone(),
                        height,
                    });
                }
            }
            st.selected = None;
            st.selected_name.clear();
        }
    }
    // Paint: the highlighted row only in controller mode; otherwise leave hover alone.
    if open && *mode == crate::gamepad::InputMode::Controller {
        for (a, mut bg) in &mut buttons {
            let want = if *a == POPUP_ORDER[*idx] {
                Color::srgba(1.0, 1.0, 1.0, 0.35)
            } else if *a == PopupAction::Sky {
                Color::srgb(0.98, 0.45, 0.12)
            } else {
                Color::srgba(1.0, 1.0, 1.0, 0.08)
            };
            if bg.0 != want {
                bg.0 = want;
            }
        }
    }
}

type SearchTask = Task<Result<Vec<ed_data::geocode::SearchHit>, String>>;

/// The search field: click it to start typing, click elsewhere or Esc to stop,
/// Enter or the button runs the search. Everything lands in `MapState`.
#[allow(deprecated)]
fn search_input(
    mut keys: MessageReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    cfg: Res<crate::config::Config>,
    game: Res<State<crate::menu::GameState>>,
    bar: Query<&RelativeCursorPosition, With<SearchBar>>,
    fields: Query<&Interaction, (With<SearchField>, Without<SearchAsk>, Changed<Interaction>)>,
    asks: Query<&Interaction, (With<SearchAsk>, Without<SearchField>, Changed<Interaction>)>,
    mut st: ResMut<MapState>,
    mut task: Local<Option<SearchTask>>,
) {
    // The field only owns the keyboard while a map screen is up.
    if game.get() == &crate::menu::GameState::Playing {
        st.text_focus = false;
        st.search_status.clear();
    }
    let mut clicked_field = false;
    for i in &fields {
        if *i == Interaction::Pressed {
            clicked_field = true;
        }
    }
    let mut clicked_ask = false;
    for i in &asks {
        if *i == Interaction::Pressed {
            clicked_ask = true;
        }
    }

    // Typing (only while the field is focused). Enter submits, Esc lets go.
    let mut submit = false;
    if st.text_focus {
        for k in keys.read() {
            if k.state != ButtonState::Pressed {
                continue;
            }
            match k.key_code {
                KeyCode::Backspace => {
                    if !k.repeat {
                        st.search_query.pop();
                    }
                }
                KeyCode::Enter | KeyCode::NumpadEnter => submit = true,
                KeyCode::Escape => st.text_focus = false,
                _ => {
                    if let Some(text) = &k.text {
                        if !k.repeat {
                            st.search_query.push_str(text);
                        }
                    }
                }
            }
        }
    } else {
        for _ in keys.read() {}
    }

    // Mouse: click the field (or the button) to grab the keyboard; any other
    // click releases it, so typing never fights a map pick.
    st.text_focus = st.text_focus || clicked_field || clicked_ask;
    if mouse.just_pressed(MouseButton::Left) && !clicked_field && !clicked_ask && !over_ui(bar) {
        st.text_focus = false;
    }
    if clicked_ask {
        submit = true;
    }
    if st.search_query.chars().count() > SEARCH_MAX_CHARS {
        st.search_query = st.search_query.chars().take(SEARCH_MAX_CHARS).collect();
    }

    // Run the search.
    if submit {
        let q = st.search_query.trim().to_string();
        if !q.is_empty() {
            st.search_status = format!("Finding \"{q}\"…");
            let (cache, q) = (cfg.cache.clone(), q);
            *task = Some(IoTaskPool::get().spawn(async move {
                ed_data::geocode::forward(&cache, &q).map_err(|e| e.to_string())
            }));
        }
    }

    // Poll the in-flight search.
    if let Some(t) = task.as_mut() {
        if let Some(res) = block_on(future::poll_once(t)) {
            *task = None;
            match res {
                Ok(hits) if !hits.is_empty() => {
                    let h = &hits[0];
                    let label = if h.region.is_empty() {
                        h.name.clone()
                    } else {
                        format!("{}, {}", h.name, h.region)
                    };
                    st.search_status.clear();
                    st.start_landing(h.position, label, time.elapsed_secs());
                    st.focus(h.position, crate::globe::zoom_for_span(h.span_deg));
                }
                Ok(_) => {
                    st.search_status = format!("Nothing found for \"{}\".", st.search_query.trim());
                }
                Err(e) => st.search_status = format!("Search unavailable ({e})."),
            }
        }
    }
}

/// Handle the "Set Waypoint" button: enters waypoint placement mode.
#[allow(deprecated)]
fn waypoint_button_input(
    mut st: ResMut<MapState>,
    q: Query<&Interaction, (With<WaypointPlaceButton>, Changed<Interaction>)>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            st.waypoint_mode = true;
            st.search_status = "Click on the globe to place waypoint".to_string();
        }
    }
}

/// Is the mouse over the search widget?
fn over_ui(bar: Query<&RelativeCursorPosition, With<SearchBar>>) -> bool {
    bar.iter().any(|p| p.cursor_over())
}

/// Colours of the search field/button/status and the typed text.
#[allow(deprecated)]
fn search_paint(
    st: Res<MapState>,
    mut fields: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<SearchField>, Without<SearchAsk>),
    >,
    mut asks: Query<(&Interaction, &mut BackgroundColor), (With<SearchAsk>, Without<SearchField>)>,
    mut txt: Query<(&mut Text, &mut TextColor), (With<SearchFieldText>, Without<SearchStatus>)>,
    mut status: Query<(&mut Text, &mut Visibility), (With<SearchStatus>, Without<SearchFieldText>)>,
) {
    let focus = st.text_focus;
    for (i, mut bg, mut bc) in &mut fields {
        let mut want = if focus {
            Color::srgba(1.0, 1.0, 1.0, 0.18)
        } else {
            crate::menu::BTN
        };
        if *i == Interaction::Hovered && !focus {
            want = crate::menu::BTN_HOVER;
        }
        if bg.0 != want {
            bg.0 = want;
        }
        let border = if focus {
            Color::srgba(1.0, 1.0, 1.0, 0.55)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.22)
        };
        let want_bc = BorderColor::all(border);
        if *bc != want_bc {
            *bc = want_bc;
        }
    }
    for (i, mut bg) in &mut asks {
        let want = if *i == Interaction::Hovered {
            Color::srgb(1.0, 0.54, 0.22)
        } else {
            crate::menu::ORANGE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    for (mut t, mut col) in &mut txt {
        // A caret makes an empty field look typeable; it sits before the dim
        // placeholder when idle and after the text while focused.
        let caret = if st.text_focus { "│" } else { "" };
        let (s, color) = if st.search_query.is_empty() {
            if focus {
                (caret.to_string(), crate::menu::DIM)
            } else {
                (SEARCH_PLACEHOLDER.to_string(), crate::menu::DIM)
            }
        } else {
            (format!("{}{caret}", st.search_query), crate::menu::INK)
        };
        if t.as_str() != s {
            *t = Text::new(s);
        }
        if col.0 != color {
            col.0 = color;
        }
    }
    for (mut t, mut v) in &mut status {
        let empty = st.search_status.is_empty();
        if t.as_str() != st.search_status {
            *t = Text::new(st.search_status.clone());
        }
        let want = if empty {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *v != want {
            *v = want;
        }
    }
}

/// Ease the time-of-day toward the target set by a map selection, so the
/// destination's lighting reflects the hour we will spawn into (a daytime
/// origin stays daytime). Runs before `menu::apply_graphics`, which applies
/// the resulting hour-offset to the clock.
fn ease_map_time(
    time: Res<Time>,
    state: Res<State<crate::menu::GameState>>,
    mut map: ResMut<MapState>,
    mut gfx: ResMut<crate::menu::GraphicsSettings>,
) {
    if *state.get() == crate::menu::GameState::Playing {
        map.time_target_offset = None;
        return;
    }
    let Some(target) = map.time_target_offset else {
        return;
    };

    gfx.time_live = false;
    let cur = gfx.hour_offset;
    let diff = target - cur;
    let dt = time.delta_secs().min(0.05);
    if diff.abs() < 1e-4 {
        gfx.hour_offset = target;
        map.time_target_offset = None;
    } else {
        gfx.hour_offset = cur + diff * (1.0 - (-dt as f64 * 2.0).exp());
    }
}
