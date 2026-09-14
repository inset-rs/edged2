//! The pointer and the modifier keys as they move and change anywhere on the
//! system, for a gesture that holds a window without a click: Edged is never
//! the key application, so it hears of them through event monitors, which
//! Accessibility access allows.

use std::ptr::NonNull;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};

/// The modifier keys that hold a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub option: bool,
    pub shift: bool,
    pub command: bool,
    pub control: bool,
}

impl Modifiers {
    fn of(flags: NSEventModifierFlags) -> Modifiers {
        Modifiers {
            option: flags.contains(NSEventModifierFlags::Option),
            shift: flags.contains(NSEventModifierFlags::Shift),
            command: flags.contains(NSEventModifierFlags::Command),
            control: flags.contains(NSEventModifierFlags::Control),
        }
    }
}

/// What the observer reports, on the main thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    /// The pointer moved to this point, in the top-left screen space the
    /// windows are measured in.
    Moved { x: f64, y: f64 },
    /// The modifier keys changed.
    Modifiers(Modifiers),
}

/// Calls back with what the system delivers of one kind of input, in this
/// application or any other. Dropping it stops the calls: a watch on the
/// pointer, which reports every move, is kept only while a move matters.
pub struct InputObserver {
    monitors: Vec<Retained<AnyObject>>,
}

impl InputObserver {
    /// Reports each change of the modifier keys.
    pub fn keys(on_input: impl Fn(Input) + 'static) -> InputObserver {
        InputObserver::new(NSEventMask::FlagsChanged, on_input)
    }

    /// Reports each move of the pointer, with a button held or not.
    pub fn pointer(on_input: impl Fn(Input) + 'static) -> InputObserver {
        InputObserver::new(
            NSEventMask::MouseMoved
                | NSEventMask::LeftMouseDragged
                | NSEventMask::RightMouseDragged
                | NSEventMask::OtherMouseDragged,
            on_input,
        )
    }

    fn new(mask: NSEventMask, on_input: impl Fn(Input) + 'static) -> InputObserver {
        let on_input: Rc<dyn Fn(Input)> = Rc::new(on_input);
        let mut monitors = Vec::new();
        let global = {
            let on_input = Rc::clone(&on_input);
            block2::RcBlock::new(move |event: NonNull<NSEvent>| {
                // SAFETY: the system hands over a live event for the block's duration.
                if let Some(input) = input_of(unsafe { event.as_ref() }) {
                    on_input(input);
                }
            })
        };
        if let Some(monitor) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &global)
        {
            monitors.push(monitor);
        }
        let local = block2::RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: as above; the event is handed back unchanged for the app to handle.
            if let Some(input) = input_of(unsafe { event.as_ref() }) {
                on_input(input);
            }
            event.as_ptr()
        });
        // SAFETY: the block returns the event it was given, a valid pointer.
        if let Some(monitor) =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &local) }
        {
            monitors.push(monitor);
        }
        InputObserver { monitors }
    }
}

impl Drop for InputObserver {
    fn drop(&mut self) {
        for monitor in self.monitors.drain(..) {
            // SAFETY: each is a monitor `NSEvent` handed out, removed once.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }
}

/// What an event says, if it is one of the two kinds listened for. A move's
/// position is read from the system rather than the event, whose location is
/// relative to whatever window it went to.
fn input_of(event: &NSEvent) -> Option<Input> {
    match event.r#type() {
        NSEventType::FlagsChanged => Some(Input::Modifiers(Modifiers::of(event.modifierFlags()))),
        NSEventType::MouseMoved
        | NSEventType::LeftMouseDragged
        | NSEventType::RightMouseDragged
        | NSEventType::OtherMouseDragged => {
            crate::screen::pointer_location().map(|(x, y)| Input::Moved { x, y })
        }
        _ => None,
    }
}

/// The modifier keys held now.
pub fn modifiers_held() -> Modifiers {
    Modifiers::of(NSEvent::modifierFlags_class())
}
