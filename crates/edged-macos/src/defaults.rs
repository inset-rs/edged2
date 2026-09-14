//! What the user set, kept where macOS keeps an application's settings.

use objc2::runtime::AnyObject;
use objc2_foundation::{NSString, NSUserDefaults};

/// The setting stored under `key`, if the user set one.
pub fn read_default(key: &str) -> Option<String> {
    let defaults = NSUserDefaults::standardUserDefaults();
    defaults
        .stringForKey(&NSString::from_str(key))
        .map(|value| value.to_string())
}

/// Stores `value` under `key`.
pub fn write_default(key: &str, value: &str) {
    let defaults = NSUserDefaults::standardUserDefaults();
    let value = NSString::from_str(value);
    let object: &AnyObject = value.as_ref();
    // SAFETY: a string is a property-list value, which is all the defaults take.
    unsafe { defaults.setObject_forKey(Some(object), &NSString::from_str(key)) };
}
