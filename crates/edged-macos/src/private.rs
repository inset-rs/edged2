//! Calls macOS makes but does not publish.
//!
//! Spaces have no public API at all; a window's id, the windows an application
//! keeps on other Spaces, and putting a window in front without activating the
//! whole application are reachable only through private calls. Each is stable
//! across releases and is what every window manager on the platform uses; each
//! is declared here with the framework it comes from.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::AXUIElement;
use objc2_core_foundation::{
    CFArray, CFData, CFDictionary, CFNumber, CFNumberType, CFRetained, CFString, CFType, CFUUID,
};
use objc2_core_graphics::CGDirectDisplayID;

/// A Space's system-wide identifier (`CGSSpaceID`).
pub type SpaceId = u64;

/// A window's Core Graphics identifier (`CGWindowID`).
pub type WindowId = u32;

/// Carbon's process handle, which the window server still speaks.
#[repr(C)]
#[derive(Default)]
pub struct ProcessSerialNumber {
    pub high: u32,
    pub low: u32,
}

unsafe extern "C" {
    /// `HIServices`, private: the `CGWindowID` behind an accessibility element.
    fn _AXUIElementGetWindow(element: &AXUIElement, out: *mut WindowId) -> i32;

    /// `HIServices`, private: an element from a 20-byte token of pid, zero,
    /// `0x636f636f`, and an element id. Walking ids finds windows the public
    /// window list leaves out, such as those on other Spaces.
    fn _AXUIElementCreateWithRemoteToken(token: &CFData) -> *mut AXUIElement;

    /// `CoreGraphics`: the identifier that survives a display being unplugged,
    /// and the only name the Spaces calls accept for a display.
    fn CGDisplayCreateUUIDFromDisplayID(display: CGDirectDisplayID) -> *mut CFUUID;

    /// `ApplicationServices`, removed from the headers but still exported.
    fn GetProcessForPID(pid: i32, psn: *mut ProcessSerialNumber) -> i32;

    /// `SkyLight`, private: this process's connection to the window server.
    fn CGSMainConnectionID() -> i32;

    /// `SkyLight`, private: every display with its Spaces, as an array of
    /// dictionaries keyed by "Display Identifier", "Current Space" and "Spaces".
    fn CGSCopyManagedDisplaySpaces(connection: i32) -> *mut CFArray;

    /// `SkyLight`, private: the Space a display is showing.
    fn CGSManagedDisplayGetCurrentSpace(connection: i32, display: &CFString) -> SpaceId;

    /// `SkyLight`, private: shows a Space on a display.

    /// `SkyLight`, private: the Spaces each of `windows` appears on.
    fn CGSCopySpacesForWindows(connection: i32, selector: u32, windows: &CFArray) -> *mut CFArray;

    /// `SkyLight`, private: a window's level; normal windows sit at 0.
    fn CGSGetWindowLevel(connection: i32, window: WindowId, level: *mut i32) -> i32;

    /// `SkyLight`, private: the key and modifiers of a system shortcut, such
    /// as Mission Control's "Switch to Desktop n".
    fn CGSGetSymbolicHotKeyValue(
        hotkey: i32,
        unicode: *mut u16,
        keycode: *mut u16,
        modifiers: *mut u32,
    ) -> i32;
    fn CGSIsSymbolicHotKeyEnabled(hotkey: i32) -> bool;
    fn CGSSetSymbolicHotKeyEnabled(hotkey: i32, enabled: bool) -> i32;

    /// `SkyLight`, private: puts a process in front, showing one window of it.
    fn _SLPSSetFrontProcessWithOptions(
        psn: *mut ProcessSerialNumber,
        window: WindowId,
        mode: u32,
    ) -> i32;

    /// `SkyLight`, private: sends an event record to a process; the way to
    /// make one of its windows key without activating the others.
    fn SLPSPostEventRecordTo(psn: *mut ProcessSerialNumber, bytes: *mut u8) -> i32;
}

/// `kCGSAllSpacesMask`: every Space, not only the visible ones.
const ALL_SPACES: u32 = 7;
/// `SLPSMode.userGenerated`
const USER_GENERATED: u32 = 0x200;

/// This process's window-server connection.
pub fn connection() -> i32 {
    unsafe { CGSMainConnectionID() }
}

/// The window id an accessibility element stands for.
pub fn window_id(element: &AXUIElement) -> Option<WindowId> {
    let mut id: WindowId = 0;
    let status = unsafe { _AXUIElementGetWindow(element, &mut id) };
    (status == 0 && id != 0).then_some(id)
}

/// The element behind one remote token: `pid`, then an element id.
pub fn element_with_token(pid: i32, element_id: u64) -> Option<CFRetained<AXUIElement>> {
    let mut token = Vec::with_capacity(20);
    token.extend_from_slice(&pid.to_le_bytes());
    token.extend_from_slice(&0i32.to_le_bytes());
    token.extend_from_slice(&0x636f_636fi32.to_le_bytes());
    token.extend_from_slice(&element_id.to_le_bytes());
    let data = CFData::from_bytes(&token);
    let element = unsafe { _AXUIElementCreateWithRemoteToken(&data) };
    Some(unsafe { CFRetained::from_raw(NonNull::new(element)?) })
}

/// A display's UUID string, as the Spaces calls spell it.
pub fn display_uuid(display: CGDirectDisplayID) -> Option<String> {
    let uuid = unsafe { CGDisplayCreateUUIDFromDisplayID(display) };
    let uuid = unsafe { CFRetained::from_raw(NonNull::new(uuid)?) };
    let string = CFUUID::new_string(None, Some(&uuid))?;
    Some(string.to_string())
}

/// The raw display-and-Spaces description, straight from the window server.
pub fn managed_display_spaces() -> Option<CFRetained<CFArray>> {
    let array = unsafe { CGSCopyManagedDisplaySpaces(connection()) };
    Some(unsafe { CFRetained::from_raw(NonNull::new(array)?) })
}

/// The Space a display currently shows.
pub fn current_space(display_uuid: &str) -> SpaceId {
    let display = CFString::from_str(display_uuid);
    unsafe { CGSManagedDisplayGetCurrentSpace(connection(), &display) }
}

/// Every Space the given windows appear on.
///
/// The reply is the union over `windows`, not one entry each, so a caller that
/// needs the Spaces of a particular window asks about that window alone.
pub fn spaces_for_windows(windows: &[WindowId]) -> Vec<SpaceId> {
    let numbers: Vec<CFRetained<CFNumber>> = windows
        .iter()
        .map(|id| CFNumber::new_i64(*id as i64))
        .collect();
    let Some(array) = array_of(&numbers) else {
        return Vec::new();
    };
    let spaces = unsafe { CGSCopySpacesForWindows(connection(), ALL_SPACES, &array) };
    let Some(spaces) = NonNull::new(spaces) else {
        return Vec::new();
    };
    let spaces = unsafe { CFRetained::from_raw(spaces) };
    (0..spaces.count())
        .filter_map(|index| number_at(&spaces, index))
        .map(|value| value as SpaceId)
        .collect()
}

/// A window's level; `Some(0)` for an ordinary window.
pub fn window_level(window: WindowId) -> Option<i32> {
    let mut level = 0i32;
    let status = unsafe { CGSGetWindowLevel(connection(), window, &mut level) };
    (status == 0).then_some(level)
}

/// A shortcut bound to no key reports this key code.
const NO_KEY: u16 = 0xFFFF;

/// The key code and modifier flags bound to a system shortcut, or `None` when
/// the shortcut is bound to no key.
pub fn symbolic_hotkey(hotkey: i32) -> Option<(u16, u32)> {
    let mut keycode = 0u16;
    let mut modifiers = 0u32;
    let status = unsafe {
        CGSGetSymbolicHotKeyValue(hotkey, std::ptr::null_mut(), &mut keycode, &mut modifiers)
    };
    if status != 0 || keycode == NO_KEY {
        return None;
    }
    Some((keycode, modifiers))
}

/// Enables a system shortcut the user had turned off. The window server takes
/// this in its own time, so it is asked before the shortcut is needed, not as
/// it is pressed.
pub fn enable_symbolic_hotkey(hotkey: i32) {
    if !unsafe { CGSIsSymbolicHotKeyEnabled(hotkey) } {
        unsafe { CGSSetSymbolicHotKeyEnabled(hotkey, true) };
    }
}

/// Brings one window of a process to the front and makes it key, leaving the
/// process's other windows where they are: what clicking the window would do.
///
/// The event record bytes come from Hammerspoon's reading of what the window
/// server expects; they have not changed since macOS 10.12.
pub fn focus_window(pid: i32, window: WindowId) {
    let mut psn = ProcessSerialNumber::default();
    if unsafe { GetProcessForPID(pid, &mut psn) } != 0 {
        return;
    }
    unsafe { _SLPSSetFrontProcessWithOptions(&mut psn, window, USER_GENERATED) };
    let mut bytes = [0u8; 0xf8];
    bytes[0x04] = 0xf8;
    bytes[0x3a] = 0x10;
    bytes[0x3c..0x40].copy_from_slice(&window.to_ne_bytes());
    bytes[0x20..0x30].fill(0xff);
    bytes[0x08] = 0x01;
    unsafe { SLPSPostEventRecordTo(&mut psn, bytes.as_mut_ptr()) };
    bytes[0x08] = 0x02;
    unsafe { SLPSPostEventRecordTo(&mut psn, bytes.as_mut_ptr()) };
}

/// Reads a `CFNumber` that arrived as an untyped value.
pub fn as_number(value: &CFType) -> Option<i64> {
    let number = value.downcast_ref::<CFNumber>()?;
    let mut out: i64 = 0;
    let slot = NonNull::from(&mut out).cast::<c_void>();
    unsafe { number.value(CFNumberType::SInt64Type, slot.as_ptr()) }.then_some(out)
}

/// Reads one `CFNumber` out of a `CFArray`.
pub fn number_at(array: &CFArray, index: isize) -> Option<i64> {
    as_number(value_at(array, index)?)
}

/// Reads one element out of a `CFArray`.
pub fn value_at(array: &CFArray, index: isize) -> Option<&CFType> {
    let value = unsafe { array.value_at_index(index) };
    let value = NonNull::new(value.cast_mut())?;
    Some(unsafe { value.cast::<CFType>().as_ref() })
}

/// Reads one entry out of a `CFDictionary` by string key.
pub fn entry<'a>(dictionary: &'a CFDictionary, key: &str) -> Option<&'a CFType> {
    let key = CFString::from_str(key);
    let value = unsafe { dictionary.value(NonNull::from(&*key).as_ptr().cast()) };
    let value = NonNull::new(value.cast_mut())?;
    Some(unsafe { value.cast::<CFType>().as_ref() })
}

/// A `CFArray` over already-retained CoreFoundation objects.
pub(crate) fn array_of<T>(values: &[CFRetained<T>]) -> Option<CFRetained<CFArray>> {
    let mut pointers: Vec<*const c_void> = values
        .iter()
        .map(|value| NonNull::from(&**value).as_ptr().cast_const().cast())
        .collect();
    let count = pointers.len() as isize;
    unsafe { CFArray::new(None, pointers.as_mut_ptr(), count, std::ptr::null()) }
}
