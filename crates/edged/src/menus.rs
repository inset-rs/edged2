//! What a right-click offers, as the system's own menus, through the host.
//!
//! A menu is described first and shown after, so the choice comes back to
//! whoever asked with the app still in hand. Every choice is a method of an
//! entity; the menu itself decides nothing.

use edged_core::{Core, Desktop, Permission};
use edged_macos::{Application, Window};
use inset::{App, Entity, PopupMenuEntry};

type Action = Box<dyn FnOnce(&mut App)>;

/// A menu under construction: entries and what choosing each one does.
#[derive(Default)]
pub struct Menu {
    entries: Vec<PopupMenuEntry>,
    actions: Vec<Option<Action>>,
}

impl Menu {
    pub fn new() -> Menu {
        Menu::default()
    }

    pub fn item(&mut self, title: impl Into<String>, action: impl FnOnce(&mut App) + 'static) {
        self.entries.push(PopupMenuEntry::item(title));
        self.actions.push(Some(Box::new(action)));
    }

    pub fn checked(
        &mut self,
        title: impl Into<String>,
        checked: bool,
        action: impl FnOnce(&mut App) + 'static,
    ) {
        self.entries
            .push(PopupMenuEntry::item(title).checked(checked));
        self.actions.push(Some(Box::new(action)));
    }

    pub fn separator(&mut self) {
        self.entries.push(PopupMenuEntry::Separator);
        self.actions.push(None);
    }

    /// Shows the menu at the pointer and runs the chosen action.
    pub fn show(self, app: &mut App) {
        let chosen = app
            .platform()
            .popup_menus()
            .and_then(|menus| menus.show(&self.entries));
        let Some(chosen) = chosen else {
            return;
        };
        if let Some(Some(action)) = self.actions.into_iter().nth(chosen) {
            action(app);
        }
    }
}

/// Close, minimize or restore, quit, and a move to any other desktop of the
/// window's screen; leaving full screen when that applies.
pub fn window_menu(
    app: &mut App,
    desktop: &Entity<Desktop>,
    window: &Window,
    application: &Application,
) {
    let mut menu = Menu::new();
    menu.item("Close", {
        let (desktop, window) = (desktop.clone(), window.clone());
        move |app| desktop.read(app).close(&window)
    });
    if window.is_fullscreen {
        menu.item("Leave Full Screen", {
            let (desktop, window) = (desktop.clone(), window.clone());
            move |app| desktop.read(app).leave_fullscreen(&window)
        });
    } else if window.is_minimized {
        menu.item("Restore", {
            let (desktop, window) = (desktop.clone(), window.clone());
            move |app| desktop.read(app).unminimize(&window)
        });
    } else {
        menu.item("Minimize", {
            let (desktop, window) = (desktop.clone(), window.clone());
            move |app| desktop.read(app).minimize(&window)
        });
    }
    menu.separator();
    quit_item(&mut menu, desktop, application);

    let destinations: Vec<edged_macos::Space> = {
        let desktop = desktop.read(app);
        match (window.is_fullscreen, desktop.screen_of(window)) {
            (false, Some(screen)) => desktop
                .desktops_of(screen)
                .into_iter()
                .filter(|space| !window.spaces.contains(&space.id))
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    };
    if !destinations.is_empty() {
        menu.separator();
    }
    for space in destinations {
        let label = match space.index {
            Some(index) => format!("Move to Desktop {}", index + 1),
            None => "Move to Desktop".to_owned(),
        };
        let (desktop, window) = (desktop.clone(), window.clone());
        menu.item(label, move |app| {
            desktop.update(app, |desktop, cx| desktop.move_to_space(cx, window, space))
        });
    }
    menu.show(app);
}

/// An application with no windows can only be quit from here.
pub fn application_menu(app: &mut App, desktop: &Entity<Desktop>, application: &Application) {
    let mut menu = Menu::new();
    quit_item(&mut menu, desktop, application);
    menu.show(app);
}

/// Edged's own: launch at login, a fresh look at the desktop, and quit.
pub fn edged_menu(app: &mut App, core: &Core) {
    let launches = core.settings.read(app).launches_at_login;
    let mut menu = Menu::new();
    menu.checked("Launch at Login", launches, {
        let settings = core.settings.clone();
        move |app| {
            settings.update(app, |settings, cx| {
                if let Err(error) = settings.set_launches_at_login(cx, !launches) {
                    eprintln!("launch at login: {error}");
                }
            });
        }
    });
    menu.item("Refresh", {
        let desktop = core.desktop.clone();
        move |app| desktop.update(app, |desktop, cx| desktop.refresh(cx))
    });
    if !core.permissions.read(app).screen_recording {
        menu.separator();
        // A picture of a window needs screen recording access; the item asks for it.
        menu.item("Show Window Previews…", {
            let permissions = core.permissions.clone();
            move |app| {
                permissions.update(app, |permissions, cx| {
                    permissions.request(cx, Permission::ScreenRecording)
                })
            }
        });
    }
    menu.separator();
    menu.item("Settings…", {
        let settings = core.settings.clone();
        move |app| settings.update(app, |settings, cx| settings.request_window(cx))
    });
    menu.separator();
    menu.item("Quit Edged", |_app| std::process::exit(0));
    menu.show(app);
}

fn quit_item(menu: &mut Menu, desktop: &Entity<Desktop>, application: &Application) {
    let (desktop, target) = (desktop.clone(), application.clone());
    menu.item(format!("Quit {}", application.name), move |app| {
        desktop.read(app).quit(&target)
    });
}
