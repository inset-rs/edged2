//! Whether the application starts when the user logs in.

use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// Whether macOS will launch this application at login.
///
/// Only a bundled application can be a login item; a bare executable answers
/// false and cannot be registered.
pub fn launches_at_login() -> bool {
    let service = unsafe { SMAppService::mainAppService() };
    let status = unsafe { service.status() };
    status == SMAppServiceStatus::Enabled
}

/// Registers or unregisters the application as a login item.
///
/// macOS may ask the user to approve the change in System Settings; the
/// answer to [`launches_at_login`] then changes once they do.
pub fn set_launches_at_login(enabled: bool) -> Result<(), String> {
    let service = unsafe { SMAppService::mainAppService() };
    let result = if enabled {
        unsafe { service.registerAndReturnError() }
    } else {
        unsafe { service.unregisterAndReturnError() }
    };
    result.map_err(|error| error.localizedDescription().to_string())
}
