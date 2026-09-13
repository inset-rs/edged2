//! Edged's logic, as entities: what the desktop holds, which permissions are
//! granted, what the system's appearance says, and the settings. An interface
//! reads them in its builds, which subscribes it to their changes, and changes
//! them through their methods. Nothing here names a widget or a window.

mod appearance;
mod clearing;
mod desktop;
mod permissions;
mod settings;

use inset_foundation::{App, Entity};

pub use appearance::Appearance;
pub use clearing::{Clearing, STRIP_WIDTH};
pub use desktop::{AppEntry, Desktop};
pub use permissions::{Granted, Permission, Permissions};
pub use settings::Settings;

/// Every entity the app runs on, started together and handed to the interface.
#[derive(Clone, Debug)]
pub struct Core {
    pub desktop: Entity<Desktop>,
    pub permissions: Entity<Permissions>,
    pub appearance: Entity<Appearance>,
    pub settings: Entity<Settings>,
    /// Runs on its own; nothing reads it.
    _clearing: Entity<Clearing>,
}

impl Core {
    /// Reads the desktop, begins watching it, and starts polling for what
    /// macOS does not announce.
    pub fn start(app: &mut App) -> Core {
        edged_macos::bound_waits();
        let permissions = app.new_entity(Permissions::start);
        let appearance = app.new_entity(Appearance::start);
        let desktop = app.new_entity(|cx| Desktop::start(cx, &permissions));
        let clearing = app.new_entity(|cx| Clearing::start(cx, desktop.clone()));
        let settings = app.new_entity(|_cx| Settings::read());
        Core {
            desktop,
            permissions,
            appearance,
            settings,
            _clearing: clearing,
        }
    }
}
