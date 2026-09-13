//! What macOS lets Edged do, and the asking for it.

use std::time::Duration;

use inset_foundation::{App, Context, EventEmitter, Listener, Timer};

/// How often to ask again whether access was granted, while it has not been.
const POLL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// Listing the windows of other applications; without it every
    /// application looks like it has none.
    Accessibility,
    /// Taking pictures of windows, for previews.
    ScreenRecording,
}

/// A permission the user just granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Granted(pub Permission);

/// Which permissions are granted now. Polls while the one the panel cannot
/// work without is missing, and announces the grant when it lands.
pub struct Permissions {
    pub accessibility: bool,
    pub screen_recording: bool,
    poll: Option<Timer>,
}

impl EventEmitter<Granted> for Permissions {}

impl Permissions {
    pub fn start(cx: &mut Context<Permissions>) -> Permissions {
        let mut permissions = Permissions {
            accessibility: edged_macos::is_trusted(),
            screen_recording: edged_macos::has_screen_recording(),
            poll: None,
        };
        permissions.schedule_poll(cx);
        permissions
    }

    pub fn has(&self, permission: Permission) -> bool {
        match permission {
            Permission::Accessibility => self.accessibility,
            Permission::ScreenRecording => self.screen_recording,
        }
    }

    /// Puts macOS's own prompt on screen for the permission.
    pub fn request(&self, permission: Permission) {
        match permission {
            Permission::Accessibility => {
                edged_macos::request_trust();
            }
            Permission::ScreenRecording => {
                edged_macos::request_screen_recording();
            }
        }
    }

    /// Opens the pane of System Settings where the permission is granted.
    pub fn open_settings(&self, permission: Permission) {
        match permission {
            Permission::Accessibility => edged_macos::open_accessibility_settings(),
            Permission::ScreenRecording => edged_macos::open_screen_recording_settings(),
        }
    }

    /// Reads both again, telling observers and subscribers of any grant.
    pub fn refresh(&mut self, cx: &mut Context<Permissions>) {
        let fresh = [
            (Permission::Accessibility, edged_macos::is_trusted()),
            (
                Permission::ScreenRecording,
                edged_macos::has_screen_recording(),
            ),
        ];
        for (permission, granted) in fresh {
            if granted == self.has(permission) {
                continue;
            }
            match permission {
                Permission::Accessibility => self.accessibility = granted,
                Permission::ScreenRecording => self.screen_recording = granted,
            }
            cx.notify();
            if granted {
                cx.emit(Granted(permission));
            }
        }
    }

    /// Asks again every second until accessibility access is granted, which
    /// macOS notes for the running process a moment after the user grants it.
    fn schedule_poll(&mut self, cx: &mut Context<Permissions>) {
        if self.accessibility {
            self.poll = None;
            return;
        }
        let this = cx.weak_entity();
        self.poll = Some(Timer::new(
            cx,
            POLL,
            Listener::new(move |app: &mut App| {
                if let Some(permissions) = this.upgrade() {
                    permissions.update(app, |permissions, cx| {
                        permissions.refresh(cx);
                        permissions.schedule_poll(cx);
                    });
                }
            }),
        ));
    }
}
