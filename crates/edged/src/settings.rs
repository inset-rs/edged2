//! Settings use WinUI controls; desktop previews draw only schematic windows.

mod preview;

use edged_core::{
    Core, Direction, Key, PanelReveal, Permission, PreviewTrigger, ResizeCorner, Settings, Zone,
};
use edged_macos::Side;
use inset::*;
use inset_winui::{
    Button, ColumnDefinition, DropDownButton, FluentSymbol, FontIcon, Grid, GridCell, GridLength,
    MenuFlyout, MenuFlyoutItem, NavigationView, NavigationViewBackButtonVisible,
    NavigationViewItem, NavigationViewPaneDisplayMode, RadioButton, RowDefinition, TextBlockStyle,
    Theme, ThemeScope, ToggleButton, ToggleSwitch,
};

use crate::theme::Design;
use preview::{Configuration, Demo, DesktopPreview, Scenario, ZoneIcon};

/// Room for options and a desktop preview; narrow windows stack them vertically.
pub const WINDOW_SIZE: [f64; 2] = [960.0, 640.0];
const PANE_WIDTH: f64 = 180.0;
const PAGE_PADDING: f64 = 28.0;
const GENERAL: &str = "general";
const PANEL: &str = "panel";
const PREVIEWS: &str = "previews";
const MOVE: &str = "move";
const RESIZE: &str = "resize";
const RING: &str = "ring";

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
    demo: Demo,
    scenario: Scenario,
    /// Changing a ring assignment can replay even when the destination stays the same.
    revision: u64,
    /// One anchored flyout per direction; released with the settings page.
    ring_menus: Vec<Handle<MenuFlyout>>,
    /// The resize-corner picker uses the same anchored WinUI menu as ring choices.
    corner_menu: Option<Handle<MenuFlyout>>,
}

impl StatefulWidget for SettingsPages {
    type State = SettingsPagesState;

    fn create_state(&self) -> SettingsPagesState {
        SettingsPagesState {
            state: StateData::new(),
            section: GENERAL.into(),
            demo: Demo::Panel,
            scenario: Scenario::Visible,
            revision: 0,
            ring_menus: Vec::new(),
            corner_menu: None,
        }
    }
}

impl SettingsPagesState {
    fn choose_demo(self: Handle<Self>, app: &mut App, demo: Demo) {
        self.set_state(app, |state| {
            state.demo = demo;
            state.revision += 1;
        });
    }

    /// Populate immediately before opening, so checked values never lag external changes.
    fn open_direction(self: Handle<Self>, app: &mut App, direction: Direction, accent: Color) {
        let core = self.widget(app).core.clone();
        let current = core.settings.read(app).ring_zones[direction.index()];
        let items = Zone::ALL
            .into_iter()
            .map(Some)
            .chain([None])
            .map(|candidate| {
                let settings = core.settings.clone();
                MenuFlyoutItem::radio(
                    candidate.map_or("Do nothing", Zone::name),
                    candidate == current,
                    Listener::new(move |app| {
                        settings.update(app, |settings, cx| {
                            settings.set_ring_zone(cx, direction, candidate)
                        });
                        self.choose_demo(app, Demo::Ring(direction));
                    }),
                )
                .icon(zone_icon(candidate, accent))
            })
            .collect();
        app.get(self).ring_menus[direction.index()].items(app, items);
        self.choose_demo(app, Demo::Ring(direction));
    }
}

impl State for SettingsPagesState {
    type Widget = SettingsPages;
    inset::state_accessors!();

    fn init_state(self: Handle<Self>, app: &mut App) {
        let menus = Direction::CLOCKWISE
            .iter()
            .map(|_| MenuFlyout::new(app, vec![]))
            .collect();
        app.get_mut(self).ring_menus = menus;
        let corner_menu = MenuFlyout::new(app, vec![]);
        app.get_mut(self).corner_menu = Some(corner_menu);
    }

    fn dispose(self: Handle<Self>, app: &mut App) {
        if let Some(menu) = app.get_mut(self).corner_menu.take() {
            menu.dispose(app);
        }
        for menu in std::mem::take(&mut app.get_mut(self).ring_menus) {
            menu.dispose(app);
        }
    }

    fn build(self: Handle<Self>, app: &mut App, context: BuildContext) -> WidgetRef {
        let core = self.widget(app).core.clone();
        let section = app.get(self).section.clone();
        let brightness = MediaQuery::platform_brightness_of(app, context);
        let design = Design::of(app, brightness, &core.appearance);
        let colors = Colors {
            foreground: design.palette.label,
            secondary: design.palette.secondary_label,
            accent: design.palette.accent,
        };
        let reduced_motion = core.appearance.read(app).reduces_motion;
        let (heading, description, enabled) = {
            let settings = core.settings.read(app);
            match section.as_str() {
                PANEL => (
                    "Panel",
                    "Choose where the panel appears and how it opens.",
                    None,
                ),
                PREVIEWS => ("Previews", "Choose when to show a window preview.", None),
                MOVE => (
                    "Move",
                    "Hold a shortcut and move the pointer. No click needed.",
                    Some(settings.move_enabled),
                ),
                RESIZE => (
                    "Resize",
                    "Hold a shortcut and move the pointer. No click needed.",
                    Some(settings.resize_enabled),
                ),
                RING => (
                    "Ring",
                    "Hold the shortcut, move in a direction, then release.",
                    Some(settings.ring_enabled),
                ),
                _ => ("General", "Startup and access.", None),
            }
        };
        let header = column(
            vec![
                Text::new(heading)
                    .style(TextBlockStyle::Title.text_style(colors.foreground))
                    .into_widget(),
                muted(description, colors.secondary),
            ],
            8.0,
        );
        let options = match section.as_str() {
            PANEL => panel_page(app, &core, colors),
            PREVIEWS => previews_page(self, app, &core, colors),
            MOVE => move_page(app, &core, colors),
            RESIZE => resize_page(self, app, &core, colors),
            RING => ring_page(self, app, &core, colors),
            _ => general_page(app, &core, colors),
        };
        let options = if let Some(enabled) = enabled {
            let settings = core.settings.clone();
            let feature = section.clone();
            let enable = switch(enabled, move |app, on| {
                settings.update(app, |settings, cx| match feature.as_str() {
                    MOVE => settings.set_move_enabled(cx, on),
                    RESIZE => settings.set_resize_enabled(cx, on),
                    _ => settings.set_ring_enabled(cx, on),
                });
            });
            column(vec![row("Enabled", enable, colors), options], 0.0)
        } else {
            options
        };
        let body = if section == GENERAL {
            options
        } else {
            let config = Configuration::new(
                core.settings.read(app),
                app.get(self).demo,
                app.get(self).scenario,
                reduced_motion,
            );
            let preview = DesktopPreview {
                config,
                revision: app.get(self).revision,
                secondary: colors.secondary,
                accent: colors.accent,
            }
            .into_widget();
            LayoutBuilder::new(move |_, _, constraints| {
                if constraints.max_width < 680.0 {
                    column(vec![options.clone(), preview.clone()], 28.0)
                } else {
                    Row::new()
                        .cross_axis_alignment(CrossAxisAlignment::Start)
                        .spacing(28.0)
                        .children(vec![
                            Expanded::new(options.clone()).into_widget(),
                            Expanded::new(preview.clone()).into_widget(),
                        ])
                        .into_widget()
                }
            })
            .into_widget()
        };
        let page = SingleChildScrollView::new().child(
            Padding::new(EdgeInsetsGeometry::all(PAGE_PADDING))
                .child(column(vec![header, body], 28.0)),
        );
        let item = |id: &str, label: &str, symbol: FluentSymbol| {
            NavigationViewItem::text(id, label).icon(FontIcon::symbol(symbol).font_size(16.0))
        };
        NavigationView::new(
            vec![
                item(GENERAL, "General", FluentSymbol::Settings),
                item(PANEL, "Panel", FluentSymbol::Navigation),
                item(PREVIEWS, "Previews", FluentSymbol::Eye),
                item(MOVE, "Move", FluentSymbol::Layer),
                item(RESIZE, "Resize", FluentSymbol::Tab),
                item(RING, "Ring", FluentSymbol::Circle),
            ],
            Some(section),
            move |app, args| {
                if let Some(item) = args.item {
                    self.set_state(app, |state| {
                        state.demo = match item.id.as_str() {
                            PANEL => Demo::Panel,
                            PREVIEWS => Demo::Preview,
                            MOVE => Demo::Move,
                            RESIZE => Demo::Resize,
                            RING => Demo::Ring(Direction::Up),
                            _ => Demo::Panel,
                        };
                        state.section = item.id;
                    });
                }
            },
            page,
        )
        .pane_display_mode(NavigationViewPaneDisplayMode::Left)
        .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
        .is_settings_visible(false)
        .is_pane_toggle_button_visible(false)
        .open_pane_length(PANE_WIDTH)
        .into_widget()
    }
}

#[derive(Clone, Copy)]
struct Colors {
    foreground: Color,
    secondary: Color,
    accent: Color,
}

fn column(children: Vec<WidgetRef>, spacing: f64) -> WidgetRef {
    Column::new()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .spacing(spacing)
        .children(children)
        .into_widget()
}

fn text(value: &str, color: Color) -> WidgetRef {
    Text::new(value.to_owned())
        .style(TextBlockStyle::Body.text_style(color))
        .into_widget()
}

fn muted(value: &str, color: Color) -> WidgetRef {
    Text::new(value.to_owned())
        .style(TextBlockStyle::Caption.text_style(color))
        .into_widget()
}

fn switch(on: bool, set: impl Fn(&mut App, bool) + 'static) -> WidgetRef {
    ToggleSwitch::new(on, set)
        .on_content(SizedBox::shrink())
        .off_content(SizedBox::shrink())
        .into_widget()
}

/// A shared label/control grid gives every settings section the same alignment and rhythm.
fn row(label: &str, control: WidgetRef, colors: Colors) -> WidgetRef {
    setting_row(label, control, None, colors)
}

fn setting_row(label: &str, control: WidgetRef, help: Option<&str>, colors: Colors) -> WidgetRef {
    let label = text(label, colors.foreground);
    let help = help.map(|value| muted(value, colors.secondary));
    LayoutBuilder::new(move |_, _, constraints| {
        let fields = if constraints.max_width < 360.0 {
            column(vec![label.clone(), control.clone()], 10.0)
        } else {
            Row::new()
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .spacing(12.0)
                .children(vec![
                    Expanded::new(label.clone()).into_widget(),
                    SizedBox::new()
                        .width(280.0)
                        .child(
                            Align::new()
                                .alignment(Alignment::CENTER_LEFT.into())
                                .child(control.clone()),
                        )
                        .into_widget(),
                ])
                .into_widget()
        };
        let mut children = vec![fields];
        if let Some(help) = &help {
            children.push(help.clone());
        }
        column(
            vec![
                ConstrainedBox::new(BoxConstraints::new().min_height(64.0))
                    .child(
                        Padding::new(EdgeInsetsGeometry::symmetric(14.0, 0.0))
                            .child(column(children, 8.0)),
                    )
                    .into_widget(),
                SizedBox::new()
                    .height(1.0)
                    .width(f64::INFINITY)
                    .child(ColoredBox::new(Color::from_argb(25, 128, 128, 128)))
                    .into_widget(),
            ],
            0.0,
        )
    })
    .into_widget()
}

fn permission_row(
    app: &mut App,
    core: &Core,
    permission: Permission,
    label: &str,
    colors: Colors,
) -> WidgetRef {
    let granted = core.permissions.read(app).has(permission);
    let permissions = core.permissions.clone();
    let control = if granted {
        muted("Allowed", colors.secondary)
    } else {
        Button::text(
            "Allow access",
            Listener::new(move |app| {
                permissions.update(app, |permissions, cx| permissions.request(cx, permission));
            }),
        )
        .into_widget()
    };
    row(label, control, colors)
}

fn general_page(app: &mut App, core: &Core, colors: Colors) -> WidgetRef {
    let launches = core.settings.read(app).launches_at_login;
    let settings = core.settings.clone();
    Align::new()
        .alignment(Alignment::TOP_LEFT.into())
        .child(
            ConstrainedBox::new(BoxConstraints::new().max_width(520.0)).child(column(
                vec![
                    row(
                        "Open Edged at login",
                        switch(launches, move |app, on| {
                            settings.update(app, |settings, cx| {
                                if let Err(error) = settings.set_launches_at_login(cx, on) {
                                    eprintln!("launch at login: {error}");
                                }
                            });
                        }),
                        colors,
                    ),
                    Padding::new(EdgeInsetsGeometry::from_ltrb(0.0, 20.0, 0.0, 4.0))
                        .child(
                            Text::new("Permissions")
                                .style(TextBlockStyle::BodyStrong.text_style(colors.foreground)),
                        )
                        .into_widget(),
                    permission_row(
                        app,
                        core,
                        Permission::Accessibility,
                        "Window control",
                        colors,
                    ),
                    permission_row(
                        app,
                        core,
                        Permission::ScreenRecording,
                        "Window previews",
                        colors,
                    ),
                ],
                0.0,
            )),
        )
        .into_widget()
}

fn panel_page(app: &mut App, core: &Core, colors: Colors) -> WidgetRef {
    let settings = core.settings.read(app);
    let (side, reveal) = (settings.panel_side, settings.panel_reveal);
    let sides = [Side::Left, Side::Right].map(|candidate| {
        let settings = core.settings.clone();
        RadioButton::new(side == candidate, move |app| {
            settings.update(app, |settings, cx| settings.set_panel_side(cx, candidate));
        })
        .content(Text::new(if candidate == Side::Left {
            "Left"
        } else {
            "Right"
        }))
        .into_widget()
    });
    let styles = [PanelReveal::Unfold, PanelReveal::Slide].map(|candidate| {
        let settings = core.settings.clone();
        RadioButton::new(reveal == candidate, move |app| {
            settings.update(app, |settings, cx| settings.set_panel_reveal(cx, candidate));
        })
        .content(Text::new(if candidate == PanelReveal::Unfold {
            "Unfold"
        } else {
            "Slide"
        }))
        .into_widget()
    });
    column(
        vec![
            row("Screen edge", column(sides.into(), 8.0), colors),
            setting_row(
                "Opening style",
                column(styles.into(), 8.0),
                Some(match reveal {
                    PanelReveal::Unfold => {
                        "Icons stay at the edge; window titles unfold beside them."
                    }
                    PanelReveal::Slide => "The whole panel slides out, icons first.",
                }),
                colors,
            ),
        ],
        0.0,
    )
}

fn previews_page(
    owner: Handle<SettingsPagesState>,
    app: &mut App,
    core: &Core,
    colors: Colors,
) -> WidgetRef {
    let trigger = core.settings.read(app).preview_trigger;
    let choices = [PreviewTrigger::Hover, PreviewTrigger::CommandKey].map(|candidate| {
        let settings = core.settings.clone();
        RadioButton::new(trigger == candidate, move |app| {
            settings.update(app, |settings, cx| {
                settings.set_preview_trigger(cx, candidate)
            });
        })
        .content(Text::new(if candidate == PreviewTrigger::Hover {
            "On hover"
        } else {
            "⌘ + hover"
        }))
        .into_widget()
    });
    let scenarios = [
        (Scenario::Visible, "Visible window"),
        (Scenario::Minimized, "Minimized window"),
        (Scenario::OtherSpace, "Another Space"),
    ]
    .map(|(candidate, label)| {
        RadioButton::new(app.get(owner).scenario == candidate, move |app| {
            owner.set_state(app, |state| {
                state.scenario = candidate;
                state.revision += 1;
            })
        })
        .content(Text::new(label))
        .into_widget()
    });
    column(
        vec![
            row("Show preview", column(choices.into(), 8.0), colors),
            permission_row(
                app,
                core,
                Permission::ScreenRecording,
                "Screen Recording",
                colors,
            ),
            text("Try a window", colors.foreground),
            column(scenarios.into(), 8.0),
        ],
        22.0,
    )
}

#[derive(Clone, Copy)]
enum Chord {
    Move,
    Resize,
    Arrange,
}

impl Chord {
    fn read(self, settings: &Settings) -> edged_core::Chord {
        match self {
            Self::Move => settings.move_chord,
            Self::Resize => settings.resize_chord,
            Self::Arrange => settings.arrange_chord,
        }
    }

    fn write(self, settings: &mut Settings, cx: &mut Context<Settings>, chord: edged_core::Chord) {
        match self {
            Self::Move => settings.set_move_chord(cx, chord),
            Self::Resize => settings.set_resize_chord(cx, chord),
            Self::Arrange => settings.set_arrange_chord(cx, chord),
        }
    }
}

fn chord_row(
    app: &mut App,
    core: &Core,
    label: &str,
    which: Chord,
    enabled: bool,
    colors: Colors,
) -> WidgetRef {
    let chord = which.read(core.settings.read(app));
    let choices = Key::ALL.map(|key| {
        let settings = core.settings.clone();
        let name = match key {
            Key::Control => "Ctrl",
            Key::Option => "Opt",
            Key::Shift => "Shift",
            Key::Command => "Cmd",
        };
        ToggleButton::new(Some(chord.has(key)), move |app, checked| {
            settings.update(app, |settings, cx| {
                let chord = which.read(settings).set(key, checked.unwrap_or(false));
                which.write(settings, cx, chord);
            });
        })
        .is_enabled(enabled)
        .content(Text::new(format!("{} {name}", key.symbol())))
        .into_widget()
    });
    let picker = shortcut_picker(choices);

    row(label, picker.into_widget(), colors)
}

fn move_page(app: &mut App, core: &Core, colors: Colors) -> WidgetRef {
    let settings = core.settings.read(app);
    let enabled = settings.move_enabled;
    let stays = settings.move_stays_on_screen;
    let settings = core.settings.clone();

    column(
        vec![
            chord_row(app, core, "Move with", Chord::Move, enabled, colors),
            setting_row(
                "Within screen",
                ToggleSwitch::new(stays, move |app, on| {
                    settings.update(app, |settings, cx| {
                        settings.set_move_stays_on_screen(cx, on)
                    })
                })
                .on_content(SizedBox::shrink())
                .off_content(SizedBox::shrink())
                .is_enabled(enabled)
                .into_widget(),
                Some("Keep the whole window visible while moving it."),
                colors,
            ),
        ],
        0.0,
    )
}

fn resize_page(
    owner: Handle<SettingsPagesState>,
    app: &mut App,
    core: &Core,
    colors: Colors,
) -> WidgetRef {
    let settings = core.settings.read(app);
    let (enabled, stays, corner) = (
        settings.resize_enabled,
        settings.resize_stays_on_screen,
        settings.resize_corner,
    );
    let conflict = settings.move_enabled
        && settings.resize_enabled
        && !settings.move_chord.is_empty()
        && settings.move_chord == settings.resize_chord;
    let menu = app.get(owner).corner_menu.expect("created in init_state");
    let flyout = menu.as_flyout(app);
    let corner_settings = core.settings.clone();
    let corner_picker = DropDownButton::text(if corner == ResizeCorner::BottomRight {
        "Bottom-right"
    } else {
        "Nearest pointer"
    })
    .is_enabled(enabled)
    .flyout(flyout)
    .click(Listener::new(move |app| {
        let current = corner_settings.read(app).resize_corner;
        let items = [ResizeCorner::BottomRight, ResizeCorner::Nearest].map(|candidate| {
            let settings = corner_settings.clone();
            MenuFlyoutItem::radio(
                if candidate == ResizeCorner::BottomRight {
                    "Bottom-right"
                } else {
                    "Nearest pointer"
                },
                current == candidate,
                Listener::new(move |app| {
                    settings.update(app, |settings, cx| {
                        settings.set_resize_corner(cx, candidate)
                    });
                    owner.choose_demo(app, Demo::Resize);
                }),
            )
        });
        menu.items(app, items.into());
    }));
    let settings = core.settings.clone();
    let mut children = vec![
        chord_row(app, core, "Resize with", Chord::Resize, enabled, colors),
        setting_row(
            "Resize corner",
            corner_picker.into_widget(),
            Some(if corner == ResizeCorner::BottomRight {
                "The bottom-right corner follows the pointer; the top-left stays fixed."
            } else {
                "The nearest corner follows the pointer; the opposite corner stays fixed."
            }),
            colors,
        ),
        setting_row(
            "Within screen",
            ToggleSwitch::new(stays, move |app, on| {
                settings.update(app, |settings, cx| {
                    settings.set_resize_stays_on_screen(cx, on)
                })
            })
            .on_content(SizedBox::shrink())
            .off_content(SizedBox::shrink())
            .is_enabled(enabled)
            .into_widget(),
            Some("Keep the window within the screen while resizing it."),
            colors,
        ),
    ];
    if conflict {
        children.push(muted(
            "These shortcuts match. Move takes priority; resize will not run.",
            colors.secondary,
        ));
    }

    column(children, 0.0)
}

fn ring_page(
    owner: Handle<SettingsPagesState>,
    app: &mut App,
    core: &Core,
    colors: Colors,
) -> WidgetRef {
    let settings = core.settings.read(app);
    let (enabled, zones) = (settings.ring_enabled, settings.ring_zones);
    let positions = [
        Direction::UpLeft,
        Direction::Up,
        Direction::UpRight,
        Direction::Left,
        Direction::Right,
        Direction::DownLeft,
        Direction::Down,
        Direction::DownRight,
    ];
    let mut cells = Vec::new();
    for (index, direction) in positions.into_iter().enumerate() {
        let index = if index >= 4 { index + 1 } else { index };
        let content = direction_content(direction, zones[direction.index()], colors);
        let flyout = app.get(owner).ring_menus[direction.index()].as_flyout(app);
        let button = DropDownButton::new(content)
            .is_enabled(enabled)
            .flyout(flyout)
            .click(Listener::new(move |app| {
                owner.open_direction(app, direction, colors.accent)
            }));
        cells.push(
            GridCell::new(button)
                .row(index / 3)
                .column(index % 3)
                .into_widget(),
        );
    }
    cells.push(
        GridCell::new(
            Center::new().child(
                FontIcon::symbol(FluentSymbol::Circle)
                    .font_size(24.0)
                    .into_widget(),
            ),
        )
        .row(1)
        .column(1)
        .into_widget(),
    );
    let grid = Grid::new()
        .column_definitions((0..3).map(|_| ColumnDefinition::new(GridLength::STAR)))
        .row_definitions((0..3).map(|_| RowDefinition::new(GridLength::AUTO)))
        .row_spacing(8.0)
        .column_spacing(8.0)
        .children(cells);
    let settings = core.settings.clone();
    column(
        vec![
            chord_row(app, core, "Open ring with", Chord::Arrange, enabled, colors),
            text("Direction assignments", colors.foreground),
            grid.into_widget(),
            Button::text(
                "Restore default directions",
                Listener::new(move |app| {
                    settings.update(app, |settings, cx| {
                        for direction in Direction::CLOCKWISE {
                            settings.set_ring_zone(cx, direction, Zone::default_for(direction));
                        }
                    });
                    owner.choose_demo(app, Demo::Ring(Direction::Up));
                }),
            )
            .is_enabled(enabled)
            .into_widget(),
        ],
        18.0,
    )
}

/// Shared content keeps direction names, destination diagrams, and labels in one place.
fn direction_content(direction: Direction, zone: Option<Zone>, colors: Colors) -> WidgetRef {
    SizedBox::new()
        .width(f64::INFINITY)
        .child(column(
            vec![
                muted(direction.name(), colors.secondary),
                CustomPaint::new()
                    .painter(ZoneIcon {
                        zone,
                        color: colors.accent,
                    })
                    .size(Size::new(32.0, 23.0))
                    .into_widget(),
                muted(zone.map_or("Do nothing", Zone::name), colors.foreground),
            ],
            6.0,
        ))
        .into_widget()
}

/// All four modifiers share one explicit row; no undeclared Grid row can overlap them.
fn shortcut_picker(choices: [WidgetRef; 4]) -> Grid {
    Grid::new()
        .column_definitions((0..4).map(|_| ColumnDefinition::new(GridLength::AUTO)))
        .row_definitions([RowDefinition::new(GridLength::AUTO)])
        .column_spacing(4.0)
        .children(
            choices
                .into_iter()
                .enumerate()
                .map(|(i, widget)| GridCell::new(widget).column(i).into_widget()),
        )
}

fn zone_icon(zone: Option<Zone>, color: Color) -> WidgetRef {
    CustomPaint::new()
        .painter(ZoneIcon { zone, color })
        .size(Size::new(20.0, 16.0))
        .into_widget()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn direction_cards_share_a_leading_edge() {
        let fixture = Fixture::with_root([400, 360], move |app| {
            inset_winui::install_icon_font(app);
            run_app(
                app,
                WidgetsApp::new(Color::from_argb(255, 0, 122, 255))
                    .debug_show_checked_mode_banner(false)
                    .builder(move |_, _, _| {
                        let colors = Colors {
                            foreground: Color::from_argb(255, 30, 30, 32),
                            secondary: Color::from_argb(255, 100, 110, 120),
                            accent: Color::from_argb(255, 0, 122, 255),
                        };
                        let cards = [Direction::UpLeft, Direction::Left, Direction::DownLeft].map(
                            |direction| {
                                SizedBox::new()
                                    .width(150.0)
                                    .child(DropDownButton::new(direction_content(
                                        direction,
                                        Zone::default_for(direction),
                                        colors,
                                    )))
                                    .into_widget()
                            },
                        );
                        ThemeScope::new(
                            Theme::Light,
                            Row::new().children(vec![
                                column(cards.into(), 8.0),
                                FontIcon::symbol(FluentSymbol::Circle)
                                    .font_size(24.0)
                                    .into_widget(),
                            ]),
                        )
                        .into_widget()
                    })
                    .into_widget(),
            );
        });
        let mut app = fixture.cell.borrow_mut();
        let root = WidgetsBinding::instance(&mut app)
            .root_element(&app)
            .unwrap();
        let mut elements = vec![root];
        let mut index = 0;
        let mut left_edges = Vec::new();
        while index < elements.len() {
            let element = elements[index];
            if let Some(text) = downcast_widget::<Text>(element.widget(&app).as_ref())
                && ["Up-left", "Left", "Down-left"].contains(&text.data.as_deref().unwrap_or(""))
            {
                let object = element.render_object(&app).unwrap().as_box().unwrap();
                left_edges.push(object.local_to_global(&app, Offset::ZERO, None).dx());
            }
            elements.extend(element.children(&app));
            index += 1;
        }
        assert_eq!(left_edges.len(), 3);
        assert!(
            left_edges
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < 0.1)
        );
        drop(app);
        fixture.capture("settings_ring_alignment");
    }

    #[test]
    fn all_four_shortcuts_are_separate_and_clickable() {
        for width in [360, 480] {
            let clicked = Rc::new(Cell::new(0u8));
            let observed = clicked.clone();
            let mut fixture = Fixture::with_root([width, 200], move |app| {
                let clicked = clicked.clone();
                run_app(
                    app,
                    WidgetsApp::new(Color::from_argb(255, 0, 122, 255))
                        .debug_show_checked_mode_banner(false)
                        .builder(move |_, _, _| {
                            let choices = ["⌃ Ctrl", "⌥ Opt", "⇧ Shift", "⌘ Cmd"].map(|label| {
                                let clicked = clicked.clone();
                                let index = ["⌃ Ctrl", "⌥ Opt", "⇧ Shift", "⌘ Cmd"]
                                    .iter()
                                    .position(|value| *value == label)
                                    .unwrap();
                                ToggleButton::new(Some(false), move |_, _| {
                                    clicked.set(clicked.get() | (1 << index))
                                })
                                .content(Text::new(label))
                                .into_widget()
                            });
                            ThemeScope::new(
                                Theme::Light,
                                Padding::new(EdgeInsetsGeometry::all(20.0)).child(row(
                                    "Move with",
                                    shortcut_picker(choices).into_widget(),
                                    Colors {
                                        foreground: Color::from_argb(255, 20, 20, 20),
                                        secondary: Color::from_argb(255, 100, 100, 100),
                                        accent: Color::from_argb(255, 0, 122, 255),
                                    },
                                )),
                            )
                            .into_widget()
                        })
                        .into_widget(),
                );
            });
            let labels = ["⌃ Ctrl", "⌥ Opt", "⇧ Shift", "⌘ Cmd"];
            let points = labels.map(|label| fixture.find(label));
            for pair in points.windows(2) {
                assert!(pair[0].dx() < pair[1].dx());
                assert_eq!(pair[0].dy(), pair[1].dy());
            }
            fixture.capture(&format!("settings_shortcuts_{width}"));
            for label in labels {
                fixture.tap(label);
            }
            assert_eq!(observed.get(), 0b1111);
        }
    }

    #[test]
    fn direction_dropdown_opens_at_its_button_and_invokes_the_destination() {
        let selected = Rc::new(Cell::new(None));
        let result = selected.clone();
        let mut fixture = Fixture::with_root([440, 480], move |app| {
            inset_winui::install_icon_font(app);
            let result = result.clone();
            let menu = MenuFlyout::new(
                app,
                vec![
                    MenuFlyoutItem::radio(
                        "Right half",
                        false,
                        Listener::new(move |_| result.set(Some(Zone::RightHalf))),
                    )
                    .icon(zone_icon(
                        Some(Zone::RightHalf),
                        Color::from_argb(255, 0, 122, 255),
                    )),
                ],
            );
            let flyout = menu.as_flyout(app);
            let entry = OverlayEntry::new(
                app,
                Rc::new(move |_, _| {
                    let colors = Colors {
                        foreground: Color::from_argb(255, 30, 30, 32),
                        secondary: Color::from_argb(255, 100, 110, 120),
                        accent: Color::from_argb(255, 0, 122, 255),
                    };
                    Padding::new(EdgeInsetsGeometry::all(28.0))
                        .child(column(
                            vec![
                                SizedBox::new()
                                    .width(116.0)
                                    .child(
                                        DropDownButton::new(direction_content(
                                            Direction::Up,
                                            Some(Zone::Fill),
                                            colors,
                                        ))
                                        .flyout(flyout),
                                    )
                                    .into_widget(),
                                DesktopPreview {
                                    config: Configuration {
                                        demo: Demo::Ring(Direction::Right),
                                        side: Side::Right,
                                        reveal: PanelReveal::Unfold,
                                        trigger: PreviewTrigger::Hover,
                                        corner: ResizeCorner::BottomRight,
                                        stays: true,
                                        zone: Some(Zone::RightHalf),
                                        scenario: Scenario::Visible,
                                        enabled: true,
                                        reduced_motion: false,
                                        ring_zones: Direction::CLOCKWISE.map(Zone::default_for),
                                        shortcut: edged_core::Chord::NONE
                                            .with(Key::Control)
                                            .with(Key::Command),
                                    },
                                    revision: 0,
                                    secondary: colors.secondary,
                                    accent: colors.accent,
                                }
                                .into_widget(),
                            ],
                            24.0,
                        ))
                        .into_widget()
                }),
                false,
                true,
                false,
            );
            run_app(
                app,
                WidgetsApp::new(Color::from_argb(255, 0, 122, 255))
                    .debug_show_checked_mode_banner(false)
                    .builder(move |_, _, _| {
                        ThemeScope::new(Theme::Light, Overlay::new().initial_entries([entry]))
                            .into_widget()
                    })
                    .into_widget(),
            );
        });
        fixture.capture("settings_direction_closed");
        fixture.tap("Up");
        let point = fixture.find("Right half");
        assert!(
            point.dy() > fixture.find("Up").dy(),
            "the menu opens below its own direction button"
        );
        fixture.capture("settings_direction_open");
        fixture.tap("Right half");
        assert_eq!(selected.get(), Some(Zone::RightHalf));
        let play = {
            let mut app = fixture.cell.borrow_mut();
            let root = WidgetsBinding::instance(&mut app)
                .root_element(&app)
                .unwrap();
            let mut elements = vec![root];
            let mut index = 0;
            while index < elements.len() {
                elements.extend(elements[index].children(&app));
                index += 1;
            }
            let element = elements
                .into_iter()
                .find(|element| downcast_widget::<Button>(element.widget(&app).as_ref()).is_some())
                .expect("preview play button");
            let object = element.render_object(&app).unwrap().as_box().unwrap();
            let size = object.size(&app);
            object.local_to_global(
                &app,
                Offset::new(size.width() / 2.0, size.height() / 2.0),
                None,
            )
        };
        fixture.send(inset::ui::PointerChange::Down, play);
        fixture.send(inset::ui::PointerChange::Up, play);
        fixture.pump();
        fixture.capture("settings_keys_held");
    }
}
