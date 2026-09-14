//! The root: the panels, one per screen the desktop reports.

use edged_core::Core;
use inset::{App, BuildContext, IntoWidget, StatelessWidget, WidgetRef};

use crate::windows::PanelWindows;

/// Reads which screens there are and hands the panels the list; the read is
/// what rebuilds this when a screen comes or goes.
#[derive(Debug)]
pub struct Root {
    pub core: Core,
}

impl StatelessWidget for Root {
    fn build(&self, app: &mut App, _context: BuildContext) -> WidgetRef {
        let screens = self.core.desktop.read(app).screens.clone();
        PanelWindows {
            core: self.core.clone(),
            screens,
        }
        .into_widget()
    }
}
