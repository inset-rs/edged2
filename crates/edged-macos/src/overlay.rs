//! What windows that only show something need from AppKit: to let the
//! pointer through, to come and go without taking the keyboard, and to sit
//! above one another in a set order.

use std::time::Duration;

use objc2::rc::Retained;
use objc2_app_kit::{NSAnimatablePropertyContainer, NSAnimationContext, NSView, NSWindow};
use raw_window_handle::RawWindowHandle;

fn window_of(handle: RawWindowHandle) -> Option<Retained<NSWindow>> {
    let RawWindowHandle::AppKit(appkit) = handle else {
        return None;
    };
    // SAFETY: the host hands out the NSView of a live window, and AppKit views are read
    // on the main thread.
    let view: &NSView = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    view.window()
}

/// Has clicks on the window go to whatever is beneath it.
pub fn pass_pointer_through(handle: RawWindowHandle) -> bool {
    let Some(window) = window_of(handle) else {
        return false;
    };
    window.setIgnoresMouseEvents(true);
    true
}

/// Puts the window on screen without making it key: the host's `show` would
/// also take the keyboard, which a window that only shows something must not.
pub fn order_front(handle: RawWindowHandle) -> bool {
    let Some(window) = window_of(handle) else {
        return false;
    };
    window.setAlphaValue(1.0);
    window.orderFrontRegardless();
    true
}

/// Puts the window on screen as `order_front` does, fading it in over
/// `duration` from nothing, so it arrives rather than appears.
pub fn fade_in(handle: RawWindowHandle, duration: Duration) -> bool {
    let Some(window) = window_of(handle) else {
        return false;
    };
    window.setAlphaValue(0.0);
    window.orderFrontRegardless();
    NSAnimationContext::beginGrouping();
    NSAnimationContext::currentContext().setDuration(duration.as_secs_f64());
    window.animator().setAlphaValue(1.0);
    NSAnimationContext::endGrouping();
    true
}

/// How much of the window shows, from nothing at 0 to all of it at 1. A window at
/// nothing keeps its place on screen and in the order, so it can be reached and
/// brought back without the host's `show`, which would activate the application.
pub fn set_alpha(handle: RawWindowHandle, alpha: f64) -> bool {
    let Some(window) = window_of(handle) else {
        return false;
    };
    window.setAlphaValue(alpha);
    true
}

/// Takes the window off screen.
pub fn order_out(handle: RawWindowHandle) -> bool {
    let Some(window) = window_of(handle) else {
        return false;
    };
    window.orderOut(None);
    true
}

/// The level pop-up menus draw at, above every floating window.
const POP_UP_MENU_LEVEL: isize = 101;

/// Puts the window at the level of pop-up menus, above the floating windows
/// such as a preview, whatever order they were shown in.
pub fn raise_to_pop_up_level(handle: RawWindowHandle) -> bool {
    let Some(window) = window_of(handle) else {
        return false;
    };
    window.setLevel(POP_UP_MENU_LEVEL);
    true
}
