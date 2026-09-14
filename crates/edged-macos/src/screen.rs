//! The displays the desktop spans.
//!
//! Read through Core Graphics for the frame, which comes in the top-left-origin
//! coordinates window positions already use, and through AppKit for the area
//! the menu bar and Dock leave free.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSScreen};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGError, CGGetActiveDisplayList, CGMainDisplayID,
};
use objc2_foundation::{NSNumber, NSString};

/// A side of the screen: which edge something sits against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// More displays than any Mac has ports for.
const MAX_DISPLAYS: u32 = 16;

/// One display.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    /// The Core Graphics display this screen draws on.
    pub display_id: CGDirectDisplayID,
    /// Top-left origin, in the global space window positions share.
    pub frame: Frame,
    /// The part of the frame the menu bar and Dock leave to windows.
    pub visible_frame: Frame,
    /// Whether this is the screen holding the menu bar.
    pub is_main: bool,
    /// The identifier the Spaces calls know this display by.
    pub uuid: String,
    /// Pixels per point.
    pub scale: f64,
}

/// A rectangle in the global top-left-origin screen space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Frame {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn intersects(&self, other: &Frame) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Every active display, the main one first.
pub fn screens() -> Vec<Screen> {
    let main = CGMainDisplayID();
    let mut displays = [0 as CGDirectDisplayID; MAX_DISPLAYS as usize];
    let mut count: u32 = 0;
    let status = unsafe { CGGetActiveDisplayList(MAX_DISPLAYS, displays.as_mut_ptr(), &mut count) };
    if status != CGError::Success {
        return Vec::new();
    }
    let appkit = appkit_screens();
    let mut screens: Vec<Screen> = displays[..count as usize]
        .iter()
        .map(|display| {
            let bounds = CGDisplayBounds(*display);
            let frame = Frame {
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
            };
            let (visible_frame, scale) = appkit
                .iter()
                .find(|(id, _, _)| *id == *display)
                .map(|(_, visible, scale)| (*visible, *scale))
                .unwrap_or((frame, 2.0));
            Screen {
                display_id: *display,
                frame,
                visible_frame,
                is_main: *display == main,
                uuid: crate::private::display_uuid(*display).unwrap_or_default(),
                scale,
            }
        })
        .collect();
    screens.sort_by_key(|screen| !screen.is_main);
    screens
}

/// Where the pointer is now, in the same top-left-origin space the frames
/// use. `None` off the main thread, where AppKit will not say.
pub fn pointer_location() -> Option<(f64, f64)> {
    let mtm = MainThreadMarker::new()?;
    let primary_height = NSScreen::screens(mtm).iter().next()?.frame().size.height;
    let point = NSEvent::mouseLocation();
    Some((point.x, primary_height - point.y))
}

/// Each `NSScreen`'s display id, visible frame flipped to top-left origin, and scale.
///
/// AppKit's origin is the bottom-left of the primary screen; the flip uses the
/// primary screen's height, which is what Core Graphics' top-left space is
/// measured from.
fn appkit_screens() -> Vec<(CGDirectDisplayID, Frame, f64)> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let screens = NSScreen::screens(mtm);
    let Some(primary) = screens.iter().next() else {
        return Vec::new();
    };
    let primary_height = primary.frame().size.height;
    screens
        .iter()
        .filter_map(|screen| {
            let key = NSString::from_str("NSScreenNumber");
            let description = screen.deviceDescription();
            let number = description.objectForKey(&key)?;
            let number = number.downcast_ref::<NSNumber>()?;
            let id = number.as_u32();
            let visible = screen.visibleFrame();
            let flipped = Frame {
                x: visible.origin.x,
                y: primary_height - visible.origin.y - visible.size.height,
                width: visible.size.width,
                height: visible.size.height,
            };
            Some((id, flipped, screen.backingScaleFactor()))
        })
        .collect()
}
