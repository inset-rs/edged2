//! What the user set about Edged itself.

use inset_foundation::Context;

pub struct Settings {
    /// Whether macOS starts Edged at login.
    pub launches_at_login: bool,
}

impl Settings {
    pub fn read() -> Settings {
        Settings {
            launches_at_login: edged_macos::launches_at_login(),
        }
    }

    /// Registers or unregisters Edged as a login item.
    pub fn set_launches_at_login(
        &mut self,
        cx: &mut Context<Settings>,
        launches: bool,
    ) -> Result<(), String> {
        edged_macos::set_launches_at_login(launches)?;
        self.launches_at_login = launches;
        cx.notify();
        Ok(())
    }
}
