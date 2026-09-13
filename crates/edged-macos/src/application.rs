//! The applications the user is running, as the Dock counts them.

use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSApplicationActivationPolicy, NSBitmapImageFileType,
    NSBitmapImageRep, NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration,
};
use objc2_foundation::{NSDictionary, NSSize};

/// One running application.
///
/// The identity is the process id: `NSRunningApplication` objects for the same
/// process compare equal, but a fresh list hands out fresh objects each time.
#[derive(Clone)]
pub struct Application {
    /// The process this application runs as.
    pub pid: i32,
    /// `CFBundleIdentifier`, absent for processes that have no bundle.
    pub bundle_id: Option<String>,
    /// The bundle's path, which is how the Dock names the application.
    pub bundle_path: Option<String>,
    /// The name the Dock and the switcher show.
    pub name: String,
    /// Whether this application is the one currently receiving key events.
    pub is_active: bool,
    /// Whether the user hid it with Command-H.
    pub is_hidden: bool,
    handle: Retained<NSRunningApplication>,
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application")
            .field("pid", &self.pid)
            .field("name", &self.name)
            .field("bundle_id", &self.bundle_id)
            .field("is_active", &self.is_active)
            .finish()
    }
}

impl PartialEq for Application {
    fn eq(&self, other: &Application) -> bool {
        self.pid == other.pid
    }
}

impl Application {
    /// Brings this application to the front with all its windows, as clicking
    /// its Dock tile does: opening the bundle again is what makes an
    /// application with no window open one. Without a bundle, activation is
    /// all there is.
    pub fn activate(&self) {
        let Some(url) = self.handle.bundleURL() else {
            self.handle
                .activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
            return;
        };
        let configuration = NSWorkspaceOpenConfiguration::configuration();
        NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(
            &url,
            &configuration,
            None,
        );
    }

    /// Asks the application to quit, as choosing Quit from its menu would.
    pub fn quit(&self) {
        self.handle.terminate();
    }

    /// Whether the process has exited since this record was made.
    pub fn has_terminated(&self) -> bool {
        self.handle.isTerminated()
    }

    /// The application's icon as PNG bytes at `size` points square.
    ///
    /// `NSImage` is resolution-independent; drawing it into a bitmap of a known
    /// size is what turns it into pixels a renderer can upload.
    pub fn icon_png(&self, size: f64) -> Option<Vec<u8>> {
        let icon = self.handle.icon()?;
        icon.setSize(NSSize::new(size, size));
        let tiff = icon.TIFFRepresentation()?;
        let representation = NSBitmapImageRep::imageRepWithData(&tiff)?;
        let png = unsafe {
            representation.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }?;
        Some(png.to_vec())
    }

    fn from_handle(handle: Retained<NSRunningApplication>) -> Application {
        Application {
            pid: handle.processIdentifier(),
            bundle_id: handle.bundleIdentifier().map(|id| id.to_string()),
            bundle_path: handle
                .bundleURL()
                .and_then(|url| url.path())
                .map(|path| path.to_string()),
            name: handle
                .localizedName()
                .map(|name| name.to_string())
                .unwrap_or_default(),
            is_active: handle.isActive(),
            is_hidden: handle.isHidden(),
            handle,
        }
    }
}

/// Every application with a Dock presence, in no particular order.
///
/// Background and accessory processes are left out: they have no windows to
/// switch to and the Dock does not list them either.
pub fn running_applications() -> Vec<Application> {
    let workspace = NSWorkspace::sharedWorkspace();
    workspace
        .runningApplications()
        .iter()
        .filter(|handle| handle.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .map(Application::from_handle)
        .collect()
}
