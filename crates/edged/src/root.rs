//! The root: the panels, one per screen the desktop reports, at the edge the
//! settings say.

use edged_core::Core;
use inset::{App, BuildContext, IntoWidget, StatelessWidget, WidgetRef};

use crate::windows::PanelWindows;

/// Reads which screens there are and which edge the panels go at, and hands
/// the panels both; the reads are what rebuild this when a screen comes or
/// goes or the edge is changed.
#[derive(Debug)]
pub struct Root {
    pub core: Core,
}

impl StatelessWidget for Root {
    fn build(&self, app: &mut App, _context: BuildContext) -> WidgetRef {
        let screens = self.core.desktop.read(app).screens.clone();
        let side = self.core.settings.read(app).panel_side;
        PanelWindows {
            core: self.core.clone(),
            screens,
            side,
        }
        .into_widget()
    }
}
