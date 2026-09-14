//! What macOS knows about the desktop: its screens, its Spaces, the applications
//! running on it and their windows, and the moments any of that changes.
//!
//! Every call here crosses into AppKit, the Accessibility API or SkyLight, so
//! this is the one crate in the workspace allowed to write `unsafe`. Nothing
//! above it names an Objective-C type.

mod appearance;
mod application;
mod ax;
mod backdrop;
mod capture;
mod defaults;
mod dock;
mod input;
mod login_item;
mod modifiers;
mod observe;
mod overlay;
mod permission;
mod private;
mod screen;
mod space;
mod window;

pub use appearance::{AppearanceObserver, accent_color, reduces_motion};
pub use application::{Application, running_applications};
pub use ax::bound_waits;
pub use backdrop::{install_backdrop, install_rounded_backdrop};
pub use capture::{CaptureTarget, Picture, capturable_windows, capture_window};
pub use defaults::{read_default, write_default};
pub use dock::badges;
pub use input::{Input, InputObserver, Modifiers, modifiers_held};
pub use login_item::{launches_at_login, set_launches_at_login};
pub use modifiers::{ModifierObserver, command_held};
pub use observe::{Change, Watcher};
pub use overlay::{
    fade_in, order_front, order_out, pass_pointer_through, raise_to_pop_up_level, set_alpha,
};
pub use permission::{
    has_screen_recording, is_trusted, open_accessibility_settings, open_screen_recording_settings,
    request_screen_recording, request_trust,
};
pub use private::{SpaceId, WindowId};
pub use screen::{Frame, Screen, pointer_location, screens};
pub use space::{Space, SpaceKind, current_space, spaces};
pub use window::{
    Window, focused_window_of, listed_windows, walk_step, window_at, window_ids_by_process,
};
