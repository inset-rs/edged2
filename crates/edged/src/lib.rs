//! Edged: the windows and applications of the desktop, on a panel at its edge.
//!
//! This crate is the interface. The app itself is `edged-core`: entities the
//! panel reads in its builds and changes through their methods.
#![feature(arbitrary_self_types)]

mod menus;
mod panel;
mod permission;
mod preview;
mod ring;
mod root;
mod rows;
mod sections;
mod settings;
mod target;
mod theme;

#[cfg(test)]
mod test_support;

mod windows;

use edged_core::Core;
use inset::{App, DefaultEmbedder, IntoWidget, Shell, run_widget};
use inset_winui::install_icon_font;

pub use panel::{PANEL_WIDTH, STRIP_WIDTH};

/// The desktop entry point; `src/main.rs` calls it. No implicit window: every window Edged
/// has is a panel it opens itself, one per screen.
pub fn main() {
    DefaultEmbedder::default()
        .implicit_view(None)
        .run(|platform| Shell::new(platform, run));
}

/// Starts the app, then the panels in whatever the host provides.
pub fn run(app: &mut App) {
    install_icon_font(app);
    let core = Core::start(app);
    run_widget(app, root::Root { core }.into_widget());
}
