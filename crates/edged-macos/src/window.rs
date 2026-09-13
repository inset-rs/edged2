//! The windows an application has open, as the Accessibility API reports them.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFDictionary, CGPoint};
use objc2_core_graphics::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, CGWindowListCopyWindowInfo,
    CGWindowListOption, kCGNullWindowID,
};

use crate::ax::{self, Element};
use crate::private::{self, SpaceId, WindowId};
use crate::screen::Frame;

/// One window of one application.
#[derive(Clone)]
pub struct Window {
    /// The window-server id, stable for the window's lifetime.
    pub id: WindowId,
    /// The process owning it.
    pub pid: i32,
    /// The title bar's text, empty for windows that have none.
    pub title: String,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
    /// Top-left origin, in the same coordinates [`crate::Screen`] reports.
    pub frame: Frame,
    /// The Spaces it appears on; more than one when the user has pinned it.
    pub spaces: Vec<SpaceId>,
    element: Element,
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("id", &self.id)
            .field("pid", &self.pid)
            .field("title", &self.title)
            .field("is_minimized", &self.is_minimized)
            .field("is_fullscreen", &self.is_fullscreen)
            .field("spaces", &self.spaces)
            .finish()
    }
}

impl PartialEq for Window {
    fn eq(&self, other: &Window) -> bool {
        self.id == other.id
    }
}

impl Window {
    /// Brings this window to the front and gives it keyboard focus, leaving
    /// the application's other windows where they are: what clicking the
    /// window would do.
    pub fn focus(&self) {
        private::focus_window(self.pid, self.id);
        self.element.perform(ax::RAISE);
    }

    /// Presses the window's close button, as clicking it would. A full-screen
    /// window leaves full screen first, since its close button is hidden.
    pub fn close(&self) {
        if self.is_fullscreen {
            self.element.set_boolean(ax::FULLSCREEN, false);
        }
        let Some(button) = self.element.element(ax::CLOSE_BUTTON) else {
            return;
        };
        button.perform(ax::PRESS);
    }

    pub fn minimize(&self) {
        self.element.set_boolean(ax::MINIMIZED, true);
    }

    pub fn unminimize(&self) {
        self.element.set_boolean(ax::MINIMIZED, false);
    }

    pub fn leave_fullscreen(&self) {
        self.element.set_boolean(ax::FULLSCREEN, false);
    }

    /// Resizes the window, keeping its origin.
    pub fn set_size(&self, width: f64, height: f64) {
        self.element.set_size(ax::SIZE, width, height);
    }

    /// The accessibility element, for observers.
    /// Takes hold of the window by its title bar, as a user starting to drag
    /// it would: the pointer moves there and the button goes down. Switching
    /// Space while held carries the window along; `release` lets go. Returns
    /// the point held, for `release`.
    pub fn grab(&self) -> Option<(f64, f64)> {
        let button = self.element.element(ax::MINIMIZE_BUTTON)?;
        let (x, y) = button.point(ax::POSITION)?;
        let (width, height) = button.size(ax::SIZE)?;
        let point = (x + width + GRAB_OFFSET, y + height / 2.0);
        post_mouse(CGEventType::MouseMoved, point);
        post_mouse(CGEventType::LeftMouseDown, point);
        post_mouse(
            CGEventType::LeftMouseDragged,
            (point.0 + 1.0, point.1 + 1.0),
        );
        Some(point)
    }

    /// Lets go of a window `grab` took hold of.
    pub fn release(&self, point: (f64, f64)) {
        post_mouse(CGEventType::LeftMouseUp, point);
    }

    pub(crate) fn element(&self) -> &Element {
        &self.element
    }

    /// Re-reads what the window reports about itself. Returns false once the
    /// window is gone.
    pub fn refresh(&mut self) -> bool {
        match read_window(self.element.clone(), self.pid) {
            Some(fresh) => {
                *self = fresh;
                true
            }
            None => false,
        }
    }

    /// Re-reads which Spaces the window is on, after a Space switch or a move.
    pub fn refresh_spaces(&mut self) {
        self.spaces = private::spaces_for_windows(&[self.id]);
    }
}

/// The window of a process that has keyboard focus.
///
/// Only the active application's answer means anything: every application keeps
/// a focused window of its own, whether or not it is the one in front.
pub fn focused_window_of(pid: i32) -> Option<WindowId> {
    let focused = Element::application(pid).element(ax::FOCUSED_WINDOW)?;
    private::window_id(focused.raw())
}

/// How far right of the minimize button a drag takes hold: past the zoom
/// button, on the empty title bar.
const GRAB_OFFSET: f64 = 40.0;

/// How many element ids a walk tries; long-lived processes can outgrow it.
const WALK_IDS: u64 = 1000;

/// A window narrower or lower than this is a tool window or an indicator,
/// not a destination.
const MIN_WIDTH: f64 = 100.0;
const MIN_HEIGHT: f64 = 50.0;

/// Everything a window is read with, in one round trip.
const WINDOW_ATTRIBUTES: [&str; 7] = [
    ax::ROLE,
    ax::SUBROLE,
    ax::TITLE,
    ax::POSITION,
    ax::SIZE,
    ax::MINIMIZED,
    ax::FULLSCREEN,
];

/// The windows a process lists itself: those on the current Space, and
/// whichever others it chooses to report. Two round trips per window and one
/// for the list. Nothing at all until the user grants accessibility access,
/// which is indistinguishable here from an application that has no windows.
pub fn listed_windows(pid: i32) -> Vec<Window> {
    let mut seen = HashSet::new();
    Element::application(pid)
        .elements(ax::WINDOWS)
        .into_iter()
        .filter_map(|element| read_window(element, pid))
        .filter(|window| seen.insert(window.id))
        .collect()
}

/// One step of the walk over a process's element ids, which reaches the
/// windows on other Spaces the list leaves out. Goes on from `cursor` for at
/// most `budget`, leaves `cursor` where the next step starts, and says
/// whether the walk is over. One round trip per id.
pub fn walk_step(pid: i32, cursor: &mut u64, budget: Duration) -> (Vec<Window>, bool) {
    let started = Instant::now();
    let mut found = Vec::new();
    while *cursor < WALK_IDS && started.elapsed() < budget {
        let id = *cursor;
        *cursor += 1;
        let Some(element) = private::element_with_token(pid, id) else {
            continue;
        };
        if let Some(window) = read_window(Element::from_retained(element), pid) {
            found.push(window);
        }
    }
    (found, *cursor >= WALK_IDS)
}

/// Every window the window server holds, on every Space, by the process
/// that owns it: those at the normal level and of a size a person could
/// use. One call, and no application is asked anything, so this is what
/// says which processes have windows the accessibility list did not show.
pub fn window_ids_by_process() -> HashMap<i32, HashSet<WindowId>> {
    let Some(list) = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionAll | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    ) else {
        return HashMap::new();
    };
    let mut by_process: HashMap<i32, HashSet<WindowId>> = HashMap::new();
    for index in 0..list.count() {
        let Some(info) =
            private::value_at(&list, index).and_then(|v| v.downcast_ref::<CFDictionary>())
        else {
            continue;
        };
        let number = |key: &str| private::entry(info, key).and_then(private::as_number);
        if number("kCGWindowLayer") != Some(0) {
            continue;
        }
        let (Some(pid), Some(id)) = (number("kCGWindowOwnerPID"), number("kCGWindowNumber")) else {
            continue;
        };
        let Some(bounds) =
            private::entry(info, "kCGWindowBounds").and_then(|v| v.downcast_ref::<CFDictionary>())
        else {
            continue;
        };
        let side = |key: &str| {
            private::entry(bounds, key)
                .and_then(private::as_number)
                .unwrap_or(0)
        };
        if (side("Width") as f64) < MIN_WIDTH || (side("Height") as f64) < MIN_HEIGHT {
            continue;
        }
        by_process
            .entry(pid as i32)
            .or_default()
            .insert(id as WindowId);
    }
    by_process
}

/// Reads a window in one round trip, or nothing when the element is not a
/// window a switcher would list: panels, popovers, tool windows and the
/// caps-lock indicator are not destinations; standard windows and dialogs at
/// the normal level are.
fn read_window(element: Element, pid: i32) -> Option<Window> {
    let values: [Option<_>; 7] = element.values(&WINDOW_ATTRIBUTES).try_into().ok()?;
    let [role, subrole, title, position, size, minimized, fullscreen] = values;
    if role.as_deref().and_then(ax::string_of).as_deref() != Some(ax::WINDOW_ROLE) {
        return None;
    }
    match subrole.as_deref().and_then(ax::string_of).as_deref() {
        Some(ax::STANDARD_WINDOW) | Some(ax::DIALOG) => {}
        // A window with no subrole at all is still a window; every application
        // that sets one uses it to mark the windows that are not.
        None => {}
        Some(_) => return None,
    }
    let (width, height) = size.as_deref().and_then(ax::size_of).unwrap_or((0.0, 0.0));
    if width < MIN_WIDTH || height < MIN_HEIGHT {
        return None;
    }
    let id = private::window_id(element.raw())?;
    if private::window_level(id).is_some_and(|level| level != 0) {
        return None;
    }
    let (x, y) = position
        .as_deref()
        .and_then(ax::point_of)
        .unwrap_or((0.0, 0.0));
    Some(Window {
        id,
        pid,
        title: title.as_deref().and_then(ax::string_of).unwrap_or_default(),
        is_minimized: minimized.as_deref().and_then(ax::bool_of).unwrap_or(false),
        is_fullscreen: fullscreen.as_deref().and_then(ax::bool_of).unwrap_or(false),
        frame: Frame {
            x,
            y,
            width,
            height,
        },
        spaces: private::spaces_for_windows(&[id]),
        element,
    })
}

fn post_mouse(kind: CGEventType, (x, y): (f64, f64)) {
    let Some(event) = CGEvent::new_mouse_event(None, kind, CGPoint { x, y }, CGMouseButton::Left)
    else {
        return;
    };
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
}
