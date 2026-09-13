//! Mission Control's Spaces, per display.

use objc2_core_foundation::{CFArray, CFDictionary, CFString};
use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventTapLocation};

use crate::private::{self, SpaceId};

/// What a Space holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceKind {
    /// A desktop the user can put windows on.
    Desktop,
    /// A Space owned by one full-screen window.
    Fullscreen,
}

/// One Space on one display.
#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub id: SpaceId,
    /// The display this Space belongs to, as [`crate::Screen::uuid`] spells it.
    pub display_uuid: String,
    /// Its place in Mission Control, counting desktops only; a full-screen
    /// Space has no number there and so has none here.
    pub index: Option<usize>,
    pub kind: SpaceKind,
}

/// Mission Control's "Switch to Desktop 1" shortcut id; the next fifteen follow it.
const FIRST_DESKTOP_HOTKEY: i32 = 118;
const DESKTOP_HOTKEYS: usize = 16;

impl Space {
    /// Whether this Space is the one its display shows now.
    pub fn is_current(&self) -> bool {
        private::current_space(&self.display_uuid) == self.id
    }

    /// The Mission Control shortcut that switches to this Space: a desktop
    /// among the first sixteen has one, and only when it is bound to a key.
    fn shortcut(&self) -> Option<i32> {
        match (self.kind, self.index) {
            (SpaceKind::Desktop, Some(index)) if index < DESKTOP_HOTKEYS => {
                let hotkey = FIRST_DESKTOP_HOTKEY + index as i32;
                private::symbolic_hotkey(hotkey).map(|_| hotkey)
            }
            _ => None,
        }
    }

    /// Makes sure the shortcut for this Space is enabled, if it has one, and
    /// says whether it does. Ask a moment before pressing: the window server
    /// enables it in its own time.
    pub fn enable_shortcut(&self) -> bool {
        let Some(hotkey) = self.shortcut() else {
            return false;
        };
        private::enable_symbolic_hotkey(hotkey);
        true
    }

    /// Presses this Space's shortcut, as the keyboard would, so the animation
    /// and the Dock agree with what happened. Returns false when there is none.
    pub fn press_shortcut(&self) -> bool {
        let Some((keycode, modifiers)) = self.shortcut().and_then(private::symbolic_hotkey) else {
            return false;
        };
        press_key(keycode, modifiers);
        true
    }
}

/// Presses and releases a key with modifiers, as the keyboard would.
fn press_key(keycode: u16, modifiers: u32) {
    let Some(down) = CGEvent::new_keyboard_event(None, keycode, true) else {
        return;
    };
    CGEvent::set_flags(Some(&down), CGEventFlags(modifiers as u64));
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&down));
    let Some(up) = CGEvent::new_keyboard_event(None, keycode, false) else {
        return;
    };
    CGEvent::set_flags(Some(&up), CGEventFlags(0));
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&up));
}

/// Every Space the window server knows, display by display, in Mission Control
/// order. Desktops are numbered across displays, as Mission Control numbers
/// them and as its shortcuts name them.
pub fn spaces() -> Vec<Space> {
    let Some(displays) = private::managed_display_spaces() else {
        return Vec::new();
    };
    let mut spaces = Vec::new();
    let mut desktops = 0;
    for index in 0..displays.count() {
        let Some(display) = private::value_at(&displays, index) else {
            continue;
        };
        let Some(display) = display.downcast_ref::<CFDictionary>() else {
            continue;
        };
        spaces.extend(spaces_of_display(display, &mut desktops));
    }
    spaces
}

/// The Space each display is currently showing.
pub fn current_space(display_uuid: &str) -> SpaceId {
    private::current_space(display_uuid)
}

fn spaces_of_display(display: &CFDictionary, desktops: &mut usize) -> Vec<Space> {
    let Some(uuid) = private::entry(display, "Display Identifier") else {
        return Vec::new();
    };
    let Some(uuid) = uuid.downcast_ref::<CFString>() else {
        return Vec::new();
    };
    let uuid = uuid.to_string();
    let Some(list) = private::entry(display, "Spaces") else {
        return Vec::new();
    };
    let Some(list) = list.downcast_ref::<CFArray>() else {
        return Vec::new();
    };
    let mut spaces = Vec::new();
    for index in 0..list.count() {
        let Some(space) = private::value_at(list, index) else {
            continue;
        };
        let Some(space) = space.downcast_ref::<CFDictionary>() else {
            continue;
        };
        let Some(id) = private::entry(space, "id64").and_then(private::as_number) else {
            continue;
        };
        // "type" is 0 for a desktop and 4 for the Space a full-screen window owns.
        let kind = match private::entry(space, "type").and_then(private::as_number) {
            Some(0) => SpaceKind::Desktop,
            _ => SpaceKind::Fullscreen,
        };
        let position = (kind == SpaceKind::Desktop).then(|| {
            *desktops += 1;
            *desktops - 1
        });
        spaces.push(Space {
            id: id as SpaceId,
            display_uuid: uuid.clone(),
            index: position,
            kind,
        });
    }
    spaces
}
