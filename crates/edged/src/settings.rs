//! The settings window: a pane of sections down the side, one per feature, and the
//! page of the section chosen. Every option is a setting on the `Settings` entity; the
//! page only shows it and asks for the change.

use edged_core::{Core, Direction, Key, PanelReveal, PreviewTrigger, ResizeCorner, Settings, Zone};
use edged_macos::Side;
use inset::{
    App, Brightness, BuildContext, Column, Context, CrossAxisAlignment, EdgeInsetsGeometry, Handle,
    IntoWidget, Listener, MediaQuery, Padding, Row, SingleChildScrollView, SizedBox, State,
    StateData, StatefulWidget, StatelessWidget, Text, WidgetRef,
};
use inset_winui::{
    Button, FluentSymbol, FontIcon, NavigationView, NavigationViewItem,
    NavigationViewPaneDisplayMode, RadioButton, Theme, ThemeScope, ToggleButton, ToggleSwitch,
};

use crate::menus::Menu;

/// The size the settings window opens at; it can be made larger.
pub const WINDOW_SIZE: [f64; 2] = [640.0, 520.0];
/// The width the chord and direction rows' labels take, so the controls line up.
const LABEL_WIDTH: f64 = 90.0;
const PANE_WIDTH: f64 = 180.0;
const PAGE_PADDING: f64 = 24.0;
/// Between a section's title and its options, and between options.
const TITLE_GAP: f64 = 12.0;
const OPTION_GAP: f64 = 8.0;
const CHOICE_GAP: f64 = 4.0;

const GENERAL: &str = "general";
const PANEL: &str = "panel";
const PREVIEWS: &str = "previews";
const HOLD: &str = "hold";
const RING: &str = "ring";

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
            PANEL => panel_page(app, &core),
            PREVIEWS => previews_page(app, &core),
            HOLD => hold_page(app, &core),
            RING => ring_page(app, &core),
            _ => SizedBox::new().into_widget(),
        };
        let item = |id: &str, label: &str, symbol: FluentSymbol| {
            NavigationViewItem::text(id, label).icon(FontIcon::symbol(symbol).font_size(16.0))
        };
        NavigationView::new(
            vec![
                item(GENERAL, "General", FluentSymbol::Settings),
                item(PANEL, "Panel", FluentSymbol::Navigation),
                item(PREVIEWS, "Previews", FluentSymbol::Eye),
                item(HOLD, "Move and resize", FluentSymbol::Layer),
                item(RING, "Ring", FluentSymbol::Circle),
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

/// A page: its sections top to bottom, scrolling when the window is shorter than they are.
fn page(children: Vec<WidgetRef>) -> WidgetRef {
    SingleChildScrollView::new()
        .child(
            Padding::new(EdgeInsetsGeometry::all(PAGE_PADDING)).child(
                Column::new()
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .children(children),
            ),
        )
        .into_widget()
}

fn title(text: &str) -> WidgetRef {
    Text::new(text.to_owned()).into_widget()
}

fn gap(height: f64) -> WidgetRef {
    SizedBox::new().height(height).into_widget()
}

/// A switch with what it turns on beside it.
fn switch_row(on: bool, label: &str, set: impl Fn(&mut App, bool) + 'static) -> WidgetRef {
    Row::new()
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .children(vec![
            ToggleSwitch::new(on, set).into_widget(),
            SizedBox::new().width(12.0).into_widget(),
            Text::new(label.to_owned()).into_widget(),
        ])
        .into_widget()
}

/// Edged itself: whether it starts with the Mac.
fn general_page(app: &mut App, core: &Core) -> WidgetRef {
    let launches = core.settings.read(app).launches_at_login;
    let settings = core.settings.clone();
    page(vec![
        title("Startup"),
        gap(TITLE_GAP),
        switch_row(launches, "Open Edged at login", move |app, on| {
            settings.update(app, |settings, cx| {
                if let Err(error) = settings.set_launches_at_login(cx, on) {
                    eprintln!("launch at login: {error}");
                }
            });
        }),
    ])
}

/// Where the panel sits and how it comes out.
fn panel_page(app: &mut App, core: &Core) -> WidgetRef {
    let (side, reveal) = {
        let settings = core.settings.read(app);
        (settings.panel_side, settings.panel_reveal)
    };
    let side_choice = |wanted: Side, label: &str| {
        let settings = core.settings.clone();
        RadioButton::new(side == wanted, move |app: &mut App| {
            settings.update(app, |settings, cx| settings.set_panel_side(cx, wanted));
        })
        .content(Text::new(label.to_owned()))
        .into_widget()
    };
    let reveal_choice = |wanted: PanelReveal, label: &str| {
        let settings = core.settings.clone();
        RadioButton::new(reveal == wanted, move |app: &mut App| {
            settings.update(app, |settings, cx| settings.set_panel_reveal(cx, wanted));
        })
        .content(Text::new(label.to_owned()))
        .into_widget()
    };
    page(vec![
        title("Edge of the screen"),
        gap(TITLE_GAP),
        side_choice(Side::Right, "Right"),
        gap(CHOICE_GAP),
        side_choice(Side::Left, "Left"),
        gap(TITLE_GAP + OPTION_GAP),
        title("When the pointer reaches it"),
        gap(TITLE_GAP),
        reveal_choice(
            PanelReveal::Unfold,
            "The titles unfold beside the icons, which stay at the edge",
        ),
        gap(CHOICE_GAP),
        reveal_choice(
            PanelReveal::Slide,
            "The whole panel slides out, icons first",
        ),
    ])
}

/// When window previews show.
fn previews_page(app: &mut App, core: &Core) -> WidgetRef {
    let trigger = core.settings.read(app).preview_trigger;
    let choice = |wanted: PreviewTrigger, label: &str| {
        let settings = core.settings.clone();
        RadioButton::new(trigger == wanted, move |app: &mut App| {
            settings.update(app, |settings, cx| settings.set_preview_trigger(cx, wanted));
        })
        .content(Text::new(label.to_owned()))
        .into_widget()
    };
    page(vec![
        title("Window previews"),
        gap(TITLE_GAP),
        choice(PreviewTrigger::Hover, "When the pointer rests on a window"),
        gap(CHOICE_GAP),
        choice(PreviewTrigger::CommandKey, "Only while ⌘ is held"),
    ])
}

/// Moving and resizing the window under the pointer by holding keys.
fn hold_page(app: &mut App, core: &Core) -> WidgetRef {
    let (enabled, stays) = {
        let settings = core.settings.read(app);
        (settings.grab_enabled, settings.stays_on_screen)
    };
    let grab = core.settings.clone();
    let on_screen = core.settings.clone();
    page(vec![
        title("Move and resize windows"),
        gap(TITLE_GAP),
        switch_row(
            enabled,
            "Hold the keys below to move or resize the window under the pointer",
            move |app, on| {
                grab.update(app, |settings, cx| settings.set_grab_enabled(cx, on));
            },
        ),
        gap(TITLE_GAP),
        chord_row(app, core, "Move with", Chord::MOVE),
        gap(OPTION_GAP),
        chord_row(app, core, "Resize with", Chord::RESIZE),
        gap(TITLE_GAP),
        corner_choice(
            app,
            core,
            ResizeCorner::BottomRight,
            "Resize from the bottom-right corner",
        ),
        gap(CHOICE_GAP),
        corner_choice(
            app,
            core,
            ResizeCorner::Nearest,
            "Resize from the corner nearest the pointer",
        ),
        gap(TITLE_GAP),
        switch_row(
            stays,
            "Keep a window within the screen while it moves or resizes",
            move |app, on| {
                on_screen.update(app, |settings, cx| settings.set_stays_on_screen(cx, on));
            },
        ),
    ])
}

/// The ring of zones: its keys, and what each direction does.
fn ring_page(app: &mut App, core: &Core) -> WidgetRef {
    let enabled = core.settings.read(app).ring_enabled;
    let settings = core.settings.clone();
    page(vec![
        title("The ring"),
        gap(TITLE_GAP),
        switch_row(
            enabled,
            "Hold the keys below for a ring of places to put the window under the pointer",
            move |app, on| {
                settings.update(app, |settings, cx| settings.set_ring_enabled(cx, on));
            },
        ),
        gap(TITLE_GAP),
        chord_row(app, core, "Open with", Chord::ARRANGE),
        gap(TITLE_GAP + OPTION_GAP),
        title("Where each direction puts the window"),
        gap(TITLE_GAP),
        ring_zones(app, core),
    ])
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
            .width(LABEL_WIDTH)
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
                        .width(LABEL_WIDTH)
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
        rows.push(gap(CHOICE_GAP));
    }
    Column::new()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .children(rows)
        .into_widget()
}
