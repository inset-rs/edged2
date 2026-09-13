//! The root: the panels, one per screen the desktop reports.

use edged_core::Core;
use inset::{App, BuildContext, IntoWidget, StatelessWidget, WidgetRef};

use crate::windows::{PanelScreen, PanelWindows};

/// Reads which screens there are and which show a full-screen window, and
/// hands the panels the list; the read is what rebuilds this on any change.
#[derive(Debug)]
pub struct Root {
    pub core: Core,
}

impl StatelessWidget for Root {
    fn build(&self, app: &mut App, _context: BuildContext) -> WidgetRef {
        let screens = {
            let desktop = self.core.desktop.read(app);
            desktop
                .screens
                .iter()
                .map(|screen| PanelScreen {
                    screen: screen.clone(),
                    hidden: desktop.is_full_screen(screen),
                })
                .collect()
        };
        PanelWindows {
            core: self.core.clone(),
            screens,
        }
        .into_widget()
    }
}
