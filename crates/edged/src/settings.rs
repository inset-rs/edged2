//! The settings window: a pane of sections down the side, so that each
//! feature to come has a place for its options, and the page of the section
//! chosen. Every option is a setting on the `Settings` entity; the page only
//! shows it and asks for the change.

use edged_core::{Core, Direction, Key, PreviewTrigger, ResizeCorner, Settings, Zone};
use inset::{
    App, Brightness, BuildContext, Column, Context, CrossAxisAlignment, EdgeInsetsGeometry, Handle,
    IntoWidget, Listener, MediaQuery, Padding, Row, SizedBox, State, StateData, StatefulWidget,
    StatelessWidget, Text, WidgetRef,
};
use inset_winui::{
    Button, FluentSymbol, FontIcon, NavigationView, NavigationViewItem,
    NavigationViewPaneDisplayMode, RadioButton, Theme, ThemeScope, ToggleButton, ToggleSwitch,
};

use crate::menus::Menu;

/// The size the settings window opens at.
/// The width the chord rows' labels take, so the keys line up.
const CHORD_LABEL_WIDTH: f64 = 90.0;

pub const WINDOW_SIZE: [f64; 2] = [560.0, 380.0];
const GENERAL: &str = "general";
const PANE_WIDTH: f64 = 180.0;
const PAGE_PADDING: f64 = 24.0;

/// The settings under the theme scope the WinUI controls read.
#[derive(Debug)]
pub struct SettingsHost {
    pub core: Core,
}

impl StatelessWidget for SettingsHost {
    fn build(&self, app: &mut App, context: BuildContext) -> WidgetRef {
        let theme = match MediaQuery::platform_brightness_of(app, context) {
            Brightness::Dark => Theme::Dark,
            Brightness::Light => Theme::Light,
        };
        ThemeScope::new(
            theme,
            SettingsPages {
                core: self.core.clone(),
            },
        )
        .into_widget()
    }
}

#[derive(Debug)]
pub struct SettingsPages {
    pub core: Core,
}

pub struct SettingsPagesState {
    state: StateData<SettingsPages>,
    section: String,
}

impl StatefulWidget for SettingsPages {
    type State = SettingsPagesState;

    fn create_state(&self) -> SettingsPagesState {
        SettingsPagesState {
            state: StateData::new(),
            section: GENERAL.to_owned(),
        }
    }
}

impl State for SettingsPagesState {
    type Widget = SettingsPages;
    inset::state_accessors!();

    fn build(self: Handle<Self>, app: &mut App, _context: BuildContext) -> WidgetRef {
        let core = self.widget(app).core.clone();
        let section = app.get(self).section.clone();
        let page = match section.as_str() {
            GENERAL => general_page(app, &core),
            _ => SizedBox::new().into_widget(),
        };
        NavigationView::new(
            vec![
                NavigationViewItem::text(GENERAL, "General")
                    .icon(FontIcon::symbol(FluentSymbol::Settings).font_size(16.0)),
            ],
            Some(section),
            move |app, args| {
                if let Some(item) = args.item {
                    self.set_state(app, |state| state.section = item.id);
                }
            },
            page,
        )
        .pane_display_mode(NavigationViewPaneDisplayMode::Left)
        .is_settings_visible(false)
        .is_pane_toggle_button_visible(false)
        .open_pane_length(PANE_WIDTH)
        .into_widget()
    }
}

/// When window previews show.
fn general_page(app: &mut App, core: &Core) -> WidgetRef {
    let trigger = core.settings.read(app).preview_trigger;
    let choice = |wanted: PreviewTrigger, label: &str| {
        let settings = core.settings.clone();
        RadioButton::new(trigger == wanted, move |app: &mut App| {
            settings.update(app, |settings, cx| settings.set_preview_trigger(cx, wanted));
        })
        .content(Text::new(label.to_owned()))
        .into_widget()
    };
    Padding::new(EdgeInsetsGeometry::all(PAGE_PADDING))
        .child(
            Column::new()
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .children(vec![
                    Text::new("Window previews").into_widget(),
                    SizedBox::new().height(12.0).into_widget(),
                    choice(PreviewTrigger::Hover, "When the pointer rests on a window"),
                    SizedBox::new().height(4.0).into_widget(),
                    choice(PreviewTrigger::CommandKey, "Only while ⌘ is held"),
                    SizedBox::new().height(24.0).into_widget(),
                    Text::new("Move and resize windows").into_widget(),
                    SizedBox::new().height(12.0).into_widget(),
                    grab_switch(app, core),
                    SizedBox::new().height(12.0).into_widget(),
                    chord_row(app, core, "Move with", Chord::MOVE),
                    SizedBox::new().height(8.0).into_widget(),
                    chord_row(app, core, "Resize with", Chord::RESIZE),
                    SizedBox::new().height(8.0).into_widget(),
                    chord_row(app, core, "Arrange with", Chord::ARRANGE),
                    SizedBox::new().height(12.0).into_widget(),
                    corner_choice(
                        app,
                        core,
                        ResizeCorner::BottomRight,
                        "Resize from the bottom-right corner",
                    ),
                    SizedBox::new().height(4.0).into_widget(),
                    corner_choice(
                        app,
                        core,
                        ResizeCorner::Nearest,
                        "Resize from the corner nearest the pointer",
                    ),
                    SizedBox::new().height(12.0).into_widget(),
                    stays_on_screen_switch(app, core),
                    SizedBox::new().height(24.0).into_widget(),
                    Text::new("The ring").into_widget(),
                    SizedBox::new().height(12.0).into_widget(),
                    ring_zones(app, core),
                ]),
        )
        .into_widget()
}

/// Whether holding the chords moves and resizes the window under the pointer.
fn grab_switch(app: &mut App, core: &Core) -> WidgetRef {
    let enabled = core.settings.read(app).grab_enabled;
    let settings = core.settings.clone();
    Row::new()
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .children(vec![
            ToggleSwitch::new(enabled, move |app: &mut App, on| {
                settings.update(app, |settings, cx| settings.set_grab_enabled(cx, on));
            })
            .into_widget(),
            SizedBox::new().width(12.0).into_widget(),
            Text::new(
                "Hold the keys below to move, resize or arrange the window under the pointer",
            )
            .into_widget(),
        ])
        .into_widget()
}

/// Which of the hold's chords a row sets.
#[derive(Clone, Copy)]
enum Chord {
    Move,
    Resize,
    Arrange,
}

impl Chord {
    const MOVE: Chord = Chord::Move;
    const RESIZE: Chord = Chord::Resize;
    const ARRANGE: Chord = Chord::Arrange;

    fn read(self, settings: &Settings) -> edged_core::Chord {
        match self {
            Chord::Move => settings.move_chord,
            Chord::Resize => settings.resize_chord,
            Chord::Arrange => settings.arrange_chord,
        }
    }

    fn write(self, settings: &mut Settings, cx: &mut Context<Settings>, chord: edged_core::Chord) {
        match self {
            Chord::Move => settings.set_move_chord(cx, chord),
            Chord::Resize => settings.set_resize_chord(cx, chord),
            Chord::Arrange => settings.set_arrange_chord(cx, chord),
        }
    }
}

/// A labelled chord, set by toggling each modifier key: the picker any shortcut of
/// Edged's is set with. No key held at all turns that shortcut off.
fn chord_row(app: &mut App, core: &Core, label: &str, which: Chord) -> WidgetRef {
    let chord = which.read(core.settings.read(app));
    let mut children = vec![
        SizedBox::new()
            .width(CHORD_LABEL_WIDTH)
            .child(Text::new(label.to_owned()))
            .into_widget(),
    ];
    for key in Key::ALL {
        let settings = core.settings.clone();
        children.push(
            ToggleButton::new(Some(chord.has(key)), move |app: &mut App, checked| {
                let held = checked.unwrap_or(false);
                settings.update(app, |settings, cx| {
                    let chord = which.read(settings).set(key, held);
                    which.write(settings, cx, chord);
                });
            })
            .content(Text::new(key.symbol().to_owned()))
            .into_widget(),
        );
        children.push(SizedBox::new().width(4.0).into_widget());
    }
    Row::new()
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .children(children)
        .into_widget()
}

/// One of the corners a resize can drag, as a choice.
fn corner_choice(app: &mut App, core: &Core, wanted: ResizeCorner, label: &str) -> WidgetRef {
    let corner = core.settings.read(app).resize_corner;
    let settings = core.settings.clone();
    RadioButton::new(corner == wanted, move |app: &mut App| {
        settings.update(app, |settings, cx| settings.set_resize_corner(cx, wanted));
    })
    .content(Text::new(label.to_owned()))
    .into_widget()
}

/// Whether a moved or resized window stops at the screen's edge.
fn stays_on_screen_switch(app: &mut App, core: &Core) -> WidgetRef {
    let stays = core.settings.read(app).stays_on_screen;
    let settings = core.settings.clone();
    Row::new()
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .children(vec![
            ToggleSwitch::new(stays, move |app: &mut App, on| {
                settings.update(app, |settings, cx| settings.set_stays_on_screen(cx, on));
            })
            .into_widget(),
            SizedBox::new().width(12.0).into_widget(),
            Text::new("Keep a window within the screen while it moves or resizes").into_widget(),
        ])
        .into_widget()
}

/// One row per direction of the ring, with a button that offers the zones it can pick.
fn ring_zones(app: &mut App, core: &Core) -> WidgetRef {
    let zones = core.settings.read(app).ring_zones;
    let mut rows = Vec::new();
    for direction in Direction::CLOCKWISE {
        let zone = zones[direction.index()];
        let settings = core.settings.clone();
        let choose = Listener::new(move |app: &mut App| {
            let mut menu = Menu::new();
            for candidate in Zone::ALL.into_iter().map(Some).chain([None]) {
                let settings = settings.clone();
                menu.checked(
                    candidate.map_or("Nothing", Zone::name),
                    candidate == zone,
                    move |app| {
                        settings.update(app, |settings, cx| {
                            settings.set_ring_zone(cx, direction, candidate)
                        });
                    },
                );
            }
            menu.show(app);
        });
        rows.push(
            Row::new()
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .children(vec![
                    SizedBox::new()
                        .width(CHORD_LABEL_WIDTH)
                        .child(Text::new(direction.name().to_owned()))
                        .into_widget(),
                    Button::new(
                        Text::new(zone.map_or("Nothing", Zone::name).to_owned()),
                        choose,
                    )
                    .into_widget(),
                ])
                .into_widget(),
        );
        rows.push(SizedBox::new().height(4.0).into_widget());
    }
    Column::new()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .children(rows)
        .into_widget()
}
