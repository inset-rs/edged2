//! What the system's appearance settings say, and the moments they change.

use std::ptr::NonNull;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{NSColor, NSColorSpace, NSWorkspace};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSString,
};

/// What System Settings posts to every process when the appearance or the
/// accent colour changes.
const NOTIFICATIONS: [&str; 2] = [
    "AppleInterfaceThemeChangedNotification",
    "AppleColorPreferencesChangedNotification",
];

/// The accent colour the user chose in System Settings, as sRGB components
/// in 0..=1. `None` when AppKit cannot express it in sRGB.
pub fn accent_color() -> Option<(f64, f64, f64)> {
    let accent = NSColor::controlAccentColor();
    let srgb = accent.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    Some((
        srgb.redComponent(),
        srgb.greenComponent(),
        srgb.blueComponent(),
    ))
}

/// Whether the user asked for less motion in Accessibility settings, which
/// turns movement into cuts and cross-fades.
pub fn reduces_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// Calls back on the main thread whenever the appearance settings change.
/// Dropping it stops the calls.
pub struct AppearanceObserver {
    center: Retained<NSNotificationCenter>,
    tokens: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl AppearanceObserver {
    pub fn new(on_change: impl Fn() + 'static) -> AppearanceObserver {
        let on_change: Rc<dyn Fn()> = Rc::new(on_change);
        let center = Retained::into_super(NSDistributedNotificationCenter::defaultCenter());
        let tokens = NOTIFICATIONS
            .iter()
            .map(|name| {
                let on_change = Rc::clone(&on_change);
                let block = block2::RcBlock::new(move |_: NonNull<NSNotification>| on_change());
                // Posted on the main thread, where this observer lives; no queue
                // means the block runs where it is posted.
                unsafe {
                    center.addObserverForName_object_queue_usingBlock(
                        Some(&NSString::from_str(name)),
                        None,
                        None,
                        &block,
                    )
                }
            })
            .collect();
        AppearanceObserver { center, tokens }
    }
}

impl Drop for AppearanceObserver {
    fn drop(&mut self) {
        for token in self.tokens.drain(..) {
            let observer: &AnyObject = (*token).as_ref();
            unsafe { self.center.removeObserver(observer) };
        }
    }
}
