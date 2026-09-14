//! The modifier keys, as they are held anywhere on the system: the panel is
//! never the key window, so it hears of them through event monitors.

use std::ptr::NonNull;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, msg_send};
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};

/// Whether the Command key is held now.
pub fn command_held() -> bool {
    // SAFETY: the class method reads the current modifier state and takes nothing.
    let flags: usize = unsafe { msg_send![NSEvent::class(), modifierFlags] };
    flags & NSEventModifierFlags::Command.0 != 0
}

/// Calls back on the main thread with whether the Command key is held, each
/// time the modifier keys change, in this application or any other. Dropping
/// it stops the calls.
pub struct ModifierObserver {
    monitors: Vec<Retained<AnyObject>>,
}

impl ModifierObserver {
    pub fn new(on_change: impl Fn(bool) + 'static) -> ModifierObserver {
        let on_change: Rc<dyn Fn(bool)> = Rc::new(on_change);
        let mut monitors = Vec::new();
        let global = {
            let on_change = Rc::clone(&on_change);
            block2::RcBlock::new(move |event: NonNull<NSEvent>| {
                // SAFETY: the system hands over a live event for the block's duration.
                let event = unsafe { event.as_ref() };
                on_change(
                    event
                        .modifierFlags()
                        .contains(NSEventModifierFlags::Command),
                );
            })
        };
        if let Some(monitor) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::FlagsChanged,
            &global,
        ) {
            monitors.push(monitor);
        }
        let local = block2::RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: as above; the event is handed back unchanged for the app to handle.
            let held = unsafe { event.as_ref() }
                .modifierFlags()
                .contains(NSEventModifierFlags::Command);
            on_change(held);
            event.as_ptr()
        });
        // SAFETY: the block returns the event it was given, a valid pointer.
        if let Some(monitor) = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::FlagsChanged, &local)
        } {
            monitors.push(monitor);
        }
        ModifierObserver { monitors }
    }
}

impl Drop for ModifierObserver {
    fn drop(&mut self) {
        for monitor in self.monitors.drain(..) {
            // SAFETY: each is a monitor `NSEvent` handed out, removed once.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }
}
