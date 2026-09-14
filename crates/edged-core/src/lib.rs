//! Edged's logic, as entities: what the desktop holds, which permissions are
//! granted, what the system's appearance says, pictures of windows, a window
//! held by the keys, and the settings. An interface reads them in its builds,
//! which subscribes it to their changes, and changes them through their
//! methods. Nothing here names a widget or a window.

mod appearance;
mod clearing;
mod desktop;
mod grab;
mod permissions;
mod previews;
mod settings;
mod shortcut;
mod zone;

use inset_foundation::{App, Entity};

pub use appearance::Appearance;
pub use clearing::{Clearing, STRIP_WIDTH};
pub use desktop::{AppEntry, Desktop};
pub use grab::{Corner, Grab, Hold, Mode};
pub use permissions::{Granted, Permission, Permissions};
pub use previews::{Preview, Previews, Rest};
pub use settings::{PreviewTrigger, ResizeCorner, Settings, SettingsRequested};
pub use shortcut::{Chord, Key};
pub use zone::{Direction, RingZones, Zone};

/// Every entity the app runs on, started together and handed to the interface.
#[derive(Clone, Debug)]
pub struct Core {
    pub desktop: Entity<Desktop>,
    pub permissions: Entity<Permissions>,
    pub appearance: Entity<Appearance>,
    pub settings: Entity<Settings>,
    pub previews: Entity<Previews>,
    pub grab: Entity<Grab>,
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
        let settings = settings; // read before the previews, which consult it
        let previews = app.new_entity(|cx| Previews::start(cx, &permissions, &settings));
        let grab = app.new_entity(|cx| Grab::start(cx, desktop.clone(), settings.clone()));
        Core {
            desktop,
            permissions,
            appearance,
            settings,
            previews,
            grab,
            _clearing: clearing,
        }
    }
}
