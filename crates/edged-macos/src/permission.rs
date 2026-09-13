//! The permissions Edged asks for: accessibility, without which a window
//! switcher cannot work, and screen recording, for pictures of windows.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{AXIsProcessTrusted, AXIsProcessTrustedWithOptions};
use objc2_core_foundation::{CFDictionary, CFRetained, kCFBooleanTrue};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

/// Whether the user has granted this application accessibility access.
///
/// Without it every application reports no windows, which is indistinguishable
/// from an empty desktop.
pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Asks the same question, and puts macOS's own prompt on screen when the
/// answer is no.
///
/// The prompt appears once per launch; System Settings is where the user
/// actually grants it. macOS notes the grant for the running process a moment
/// later, so a caller keeps asking [`is_trusted`] rather than restarting.
pub fn request_trust() -> bool {
    let Some(options) = prompting_options() else {
        return is_trusted();
    };
    unsafe { AXIsProcessTrustedWithOptions(Some(&options)) }
}

/// Opens the Accessibility pane of System Settings.
pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// Whether the user has granted this application screen recording, which
/// taking a picture of another application's window needs.
pub fn has_screen_recording() -> bool {
    CGPreflightScreenCaptureAccess()
}

/// Asks for screen recording, putting macOS's own prompt on screen when it
/// has not been granted. As with accessibility, the grant lands in System
/// Settings and is noted a moment later.
pub fn request_screen_recording() -> bool {
    CGRequestScreenCaptureAccess()
}

/// Opens the Screen Recording pane of System Settings.
pub fn open_screen_recording_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .spawn();
}

fn prompting_options() -> Option<CFRetained<CFDictionary>> {
    let key = unsafe { objc2_application_services::kAXTrustedCheckOptionPrompt };
    let value = unsafe { kCFBooleanTrue }?;
    let mut keys: [*const c_void; 1] = [NonNull::from(key).as_ptr().cast()];
    let mut values: [*const c_void; 1] = [NonNull::from(value).as_ptr().cast()];
    unsafe {
        CFDictionary::new(
            None,
            keys.as_mut_ptr(),
            values.as_mut_ptr(),
            1,
            std::ptr::null(),
            std::ptr::null(),
        )
    }
}
