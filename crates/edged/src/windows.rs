//! One panel window per screen: opened as screens appear, closed as they go,
//! hidden while a screen shows a full-screen window, each with its own
//! subtree. The set of screens is the widget's configuration; the windows
//! follow it in `did_update_widget`, so a build here only lists them.

use std::collections::HashSet;
use std::rc::Rc;

use edged_core::Core;
use edged_macos::Screen;
use inset::{
    App, Brightness, BuildContext, Handle, IntoWidget, KeyRef, MediaQuery, PageRoute,
    PageRouteBuilder, RouteSettingsRef, State, StateData, StatefulWidget, StatelessWidget,
    ValueKey, ViewCollection, WidgetRef, WidgetsApp, Window, WindowRef,
};
use inset_winui::{AccentPalette, Theme, ThemeScope};

use crate::panel::{self, EdgePanel};

/// A screen the panels follow.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelScreen {
    pub screen: Screen,
    /// Whether the screen shows a full-screen window, which owns it whole and
    /// has the panel keep out of the way.
    pub hidden: bool,
}

#[derive(Debug)]
pub struct PanelWindows {
    pub core: Core,
    pub screens: Vec<PanelScreen>,
}

/// One screen's panel window.
struct Panel {
    display_id: u32,
    window: WindowRef,
    hidden: bool,
}

pub struct PanelWindowsState {
    state: StateData<PanelWindows>,
    panels: Vec<Panel>,
    /// Screens whose window the host is still making.
    opening: HashSet<u32>,
}

impl StatefulWidget for PanelWindows {
    type State = PanelWindowsState;

    fn create_state(&self) -> PanelWindowsState {
        PanelWindowsState {
            state: StateData::new(),
            panels: Vec::new(),
            opening: HashSet::new(),
        }
    }
}

impl State for PanelWindowsState {
    type Widget = PanelWindows;
    inset::state_accessors!();

    fn init_state(self: Handle<Self>, app: &mut App) {
        follow(self, app);
    }

    fn did_update_widget(self: Handle<Self>, app: &mut App, old_widget: &PanelWindows) {
        if self.widget(app).screens != old_widget.screens {
            follow(self, app);
        }
    }

    fn dispose(self: Handle<Self>, app: &mut App) {
        for panel in app.get_mut(self).panels.drain(..) {
            panel.window.close();
        }
    }

    fn build(self: Handle<Self>, app: &mut App, _context: BuildContext) -> WidgetRef {
        let core = self.widget(app).core.clone();
        let windows: Vec<WidgetRef> = app
            .get(self)
            .panels
            .iter()
            .map(|panel| {
                let key = Rc::new(ValueKey::new(panel.display_id)) as KeyRef;
                let host = PanelHost {
                    core: core.clone(),
                    display_id: panel.display_id,
                };
                Window::new(Rc::clone(&panel.window), per_window_app(host))
                    .key(key)
                    .into_widget()
            })
            .collect();
        ViewCollection::new(windows).into_widget()
    }
}

/// Brings the windows in line with the screens: closes the panels of screens
/// that went, hides and shows the ones that stay, opens one for each new screen.
fn follow(this: Handle<PanelWindowsState>, app: &mut App) {
    let screens = this.widget(app).screens.clone();
    close_gone(this, app, &screens);
    follow_hiding(this, app, &screens);
    open_new(this, app, &screens);
}

fn close_gone(this: Handle<PanelWindowsState>, app: &mut App, screens: &[PanelScreen]) {
    let gone: Vec<usize> = app
        .get(this)
        .panels
        .iter()
        .enumerate()
        .filter(|(_, panel)| {
            !screens
                .iter()
                .any(|s| s.screen.display_id == panel.display_id)
        })
        .map(|(index, _)| index)
        .collect();
    if gone.is_empty() {
        return;
    }
    this.set_state(app, |state| {
        for index in gone.into_iter().rev() {
            state.panels.remove(index).window.close();
        }
    });
}

/// A change of hiding needs no rebuild: the window is the only thing that moves.
fn follow_hiding(this: Handle<PanelWindowsState>, app: &mut App, screens: &[PanelScreen]) {
    for panel in &mut app.get_mut(this).panels {
        let Some(screen) = screens
            .iter()
            .find(|s| s.screen.display_id == panel.display_id)
        else {
            continue;
        };
        if panel.hidden == screen.hidden {
            continue;
        }
        panel.hidden = screen.hidden;
        if screen.hidden {
            panel.window.hide();
        } else {
            panel.window.show();
        }
    }
}

fn open_new(this: Handle<PanelWindowsState>, app: &mut App, screens: &[PanelScreen]) {
    let Some(owner) = app.platform().windowing_owner() else {
        eprintln!("edged: this host opens no windows, so there is nowhere to put a panel");
        return;
    };
    for screen in screens {
        let display_id = screen.screen.display_id;
        let state = app.get(this);
        let known = state.panels.iter().any(|p| p.display_id == display_id)
            || state.opening.contains(&display_id);
        if known {
            continue;
        }
        app.get_mut(this).opening.insert(display_id);
        let future = owner.create(panel::window_config(&screen.screen));
        drop(app.spawn(async move |cx| {
            let opened = future.await;
            cx.update(|app| {
                app.get_mut(this).opening.remove(&display_id);
                match opened {
                    Ok(window) => opened_for(this, app, display_id, window),
                    Err(error) => eprintln!("edged: panel window: {error}"),
                }
            });
        }));
    }
}

/// Dresses a window the host just made and lists it.
fn opened_for(this: Handle<PanelWindowsState>, app: &mut App, display_id: u32, window: WindowRef) {
    // The panel has no close box; a close request is what a hotkey like ⌘W
    // would send, and the panel stays.
    window.set_close_requested(Some(Rc::new(|| {})));
    let backdrop = window
        .native_handle()
        .is_some_and(|handle| edged_macos::install_backdrop(handle, panel::PANEL_RADIUS));
    if !backdrop {
        eprintln!("edged: the panel window has no native view to put glass behind");
    }
    // The screen may have gone full screen while the window was being made.
    let hidden = this
        .widget(app)
        .screens
        .iter()
        .any(|s| s.screen.display_id == display_id && s.hidden);
    if hidden {
        window.hide();
    }
    this.set_state(app, |state| {
        state.panels.push(Panel {
            display_id,
            window,
            hidden,
        })
    });
}

/// Every window gets its own navigator and overlay, which the tooltips need.
fn per_window_app(home: PanelHost) -> WidgetRef {
    WidgetsApp::new(AccentPalette::default().base)
        .title("Edged")
        .debug_show_checked_mode_banner(false)
        .page_route_builder(|app, settings, builder| {
            let route = PageRouteBuilder::new(
                app,
                Rc::new(move |app, context, _, _| builder(app, context)),
            )
            .settings(app, RouteSettingsRef::Settings(settings.clone()));
            PageRoute::as_page_route(route)
        })
        .home(home)
        .into_widget()
}

/// The panel under the theme scope the WinUI controls read.
#[derive(Debug)]
struct PanelHost {
    core: Core,
    display_id: u32,
}

impl StatelessWidget for PanelHost {
    fn build(&self, app: &mut App, context: BuildContext) -> WidgetRef {
        let theme = match MediaQuery::platform_brightness_of(app, context) {
            Brightness::Dark => Theme::Dark,
            Brightness::Light => Theme::Light,
        };
        ThemeScope::new(
            theme,
            EdgePanel {
                core: self.core.clone(),
                display_id: self.display_id,
            },
        )
        .into_widget()
    }
}
