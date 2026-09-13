//! The Accessibility API, as much of it as a window switcher needs.
//!
//! Every attribute is addressed by name. The names are `#define`s in
//! `AXAttributeConstants.h` that the generated bindings do not re-export, so
//! they are spelled out here next to the C name they stand for.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{
    AXCopyMultipleAttributeOptions, AXError, AXUIElement, AXValue, AXValueType,
};
use objc2_core_foundation::{
    CFArray, CFBoolean, CFRetained, CFString, CFType, CFURL, CGPoint, CGSize,
};

use crate::private;

/// `kAXWindowsAttribute`
pub const WINDOWS: &str = "AXWindows";
/// `kAXFocusedWindowAttribute`
pub const FOCUSED_WINDOW: &str = "AXFocusedWindow";
/// `kAXTitleAttribute`
pub const TITLE: &str = "AXTitle";
/// `kAXRoleAttribute`
pub const ROLE: &str = "AXRole";
/// `kAXSubroleAttribute`
pub const SUBROLE: &str = "AXSubrole";
/// `kAXMinimizedAttribute`
pub const MINIMIZED: &str = "AXMinimized";
/// `kAXPositionAttribute`
pub const POSITION: &str = "AXPosition";
/// `kAXSizeAttribute`
pub const SIZE: &str = "AXSize";
/// `kAXCloseButtonAttribute`
pub const CLOSE_BUTTON: &str = "AXCloseButton";
pub const MINIMIZE_BUTTON: &str = "AXMinimizeButton";
/// `kAXChildrenAttribute`
pub const CHILDREN: &str = "AXChildren";
/// `kAXURLAttribute`
pub const URL: &str = "AXURL";
/// `kAXIsApplicationRunningAttribute`
pub const IS_APPLICATION_RUNNING: &str = "AXIsApplicationRunning";
/// Not in the headers: the Dock's badge text on an application tile.
pub const STATUS_LABEL: &str = "AXStatusLabel";
/// Not in the headers, but every window that can go full screen answers it.
pub const FULLSCREEN: &str = "AXFullScreen";
/// `kAXPressAction`
pub const PRESS: &str = "AXPress";
/// `kAXRaiseAction`
pub const RAISE: &str = "AXRaise";
/// `kAXWindowRole`
pub const WINDOW_ROLE: &str = "AXWindow";
/// `kAXListRole`
pub const LIST_ROLE: &str = "AXList";
/// `kAXStandardWindowSubrole`
pub const STANDARD_WINDOW: &str = "AXStandardWindow";
/// `kAXDialogSubrole`
pub const DIALOG: &str = "AXDialog";
/// `kAXApplicationDockItemSubrole`
pub const APPLICATION_DOCK_ITEM: &str = "AXApplicationDockItem";

/// An accessibility element: an application, a window, or a Dock tile.
/// How long one accessibility call may wait on an application, in seconds.
/// Calls take a millisecond when the application is well.
const MESSAGING_TIMEOUT: f32 = 0.5;

/// Bounds every accessibility call this process makes, whatever element it
/// goes through, to [`MESSAGING_TIMEOUT`]: an application that has stopped
/// responding otherwise keeps each call waiting for the system's six seconds.
pub fn bound_waits() {
    let system = unsafe { AXUIElement::new_system_wide() };
    unsafe { AXUIElementSetMessagingTimeout(&system, MESSAGING_TIMEOUT) };
}

/// A string attribute's value.
pub(crate) fn string_of(value: &CFType) -> Option<String> {
    Some(value.downcast_ref::<CFString>()?.to_string())
}

pub(crate) fn bool_of(value: &CFType) -> Option<bool> {
    Some(value.downcast_ref::<CFBoolean>()?.as_bool())
}

pub(crate) fn point_of(value: &CFType) -> Option<(f64, f64)> {
    let mut point = CGPoint::new(0.0, 0.0);
    decode_value(value, AXValueType::CGPoint, &mut point)?;
    Some((point.x, point.y))
}

pub(crate) fn size_of(value: &CFType) -> Option<(f64, f64)> {
    let mut size = CGSize::new(0.0, 0.0);
    decode_value(value, AXValueType::CGSize, &mut size)?;
    Some((size.width, size.height))
}

/// An `AXValue` is a boxed C structure; this unwraps one of a known type.
fn decode_value<T>(value: &CFType, kind: AXValueType, out: &mut T) -> Option<()> {
    let value = value.downcast_ref::<AXValue>()?;
    let slot = NonNull::from(out).cast::<c_void>();
    unsafe { value.value(kind, slot) }.then_some(())
}

// The binding crate does not carry this call; the framework it lives in is linked.
unsafe extern "C-unwind" {
    fn AXUIElementSetMessagingTimeout(element: &AXUIElement, timeout_in_seconds: f32) -> AXError;
}

#[derive(Clone)]
pub struct Element(CFRetained<AXUIElement>);

impl Element {
    /// The element standing for a running process.
    ///
    /// Answers nothing useful until the user grants accessibility access. An
    /// application that has stopped responding keeps every call to it waiting
    /// for the system's six seconds; this one waits [`MESSAGING_TIMEOUT`].
    pub fn application(pid: i32) -> Element {
        let element = unsafe { AXUIElement::new_application(pid) };
        unsafe { AXUIElementSetMessagingTimeout(&element, MESSAGING_TIMEOUT) };
        Element(element)
    }

    pub(crate) fn from_retained(element: CFRetained<AXUIElement>) -> Element {
        Element(element)
    }

    pub(crate) fn from_ref(element: &AXUIElement) -> Element {
        Element(unsafe { CFRetained::retain(NonNull::from(element)) })
    }

    /// The raw element, for the private calls in [`crate::private`] and observers.
    pub(crate) fn raw(&self) -> &AXUIElement {
        &self.0
    }

    fn attribute(&self, name: &str) -> Option<CFRetained<CFType>> {
        let name = CFString::from_str(name);
        let mut value: *const CFType = std::ptr::null();
        let status = unsafe {
            AXUIElement::copy_attribute_value(
                &self.0,
                &name,
                NonNull::from(&mut value).cast::<*const CFType>(),
            )
        };
        if status.0 != 0 || value.is_null() {
            return None;
        }
        Some(unsafe { CFRetained::from_raw(NonNull::new(value.cast_mut())?) })
    }

    pub fn string(&self, name: &str) -> Option<String> {
        let value = self.attribute(name)?;
        Some(value.downcast_ref::<CFString>()?.to_string())
    }

    pub fn boolean(&self, name: &str) -> Option<bool> {
        let value = self.attribute(name)?;
        Some(value.downcast_ref::<CFBoolean>()?.as_bool())
    }

    /// A file URL attribute as a path.
    pub fn path(&self, name: &str) -> Option<String> {
        let value = self.attribute(name)?;
        let url = value.downcast_ref::<CFURL>()?;
        Some(url.path()?.to_string())
    }

    pub fn point(&self, name: &str) -> Option<(f64, f64)> {
        let mut point = CGPoint::new(0.0, 0.0);
        self.decode(name, AXValueType::CGPoint, &mut point)?;
        Some((point.x, point.y))
    }

    pub fn size(&self, name: &str) -> Option<(f64, f64)> {
        let mut size = CGSize::new(0.0, 0.0);
        self.decode(name, AXValueType::CGSize, &mut size)?;
        Some((size.width, size.height))
    }

    fn decode<T>(&self, name: &str, kind: AXValueType, out: &mut T) -> Option<()> {
        let value = self.attribute(name)?;
        decode_value(&value, kind, out)
    }

    /// Several attributes in one round trip, in the order asked; one the
    /// element lacks, or cannot answer, reads as `None`.
    pub fn values(&self, names: &[&str]) -> Vec<Option<CFRetained<CFType>>> {
        let keys: Vec<CFRetained<CFString>> = names.iter().map(|n| CFString::from_str(n)).collect();
        let Some(attributes) = private::array_of(&keys) else {
            return vec![None; names.len()];
        };
        let mut values: *const CFArray = std::ptr::null();
        let status = unsafe {
            self.0.copy_multiple_attribute_values(
                &attributes,
                AXCopyMultipleAttributeOptions(0),
                NonNull::from(&mut values),
            )
        };
        let Some(values) = NonNull::new(values.cast_mut()).filter(|_| status.0 == 0) else {
            return vec![None; names.len()];
        };
        let values = unsafe { CFRetained::from_raw(values) };
        (0..names.len() as isize)
            .map(|index| {
                let item = unsafe { values.value_at_index(index) };
                let item = NonNull::new(item.cast_mut())?;
                let item: &CFType = unsafe { item.cast().as_ref() };
                // An attribute that could not be read comes back as an error boxed in a value.
                let failed = item
                    .downcast_ref::<AXValue>()
                    .is_some_and(|value| unsafe { value.r#type() } == AXValueType::AXError);
                if failed {
                    return None;
                }
                Some(unsafe { CFRetained::retain(NonNull::from(item)) })
            })
            .collect()
    }

    pub fn element(&self, name: &str) -> Option<Element> {
        let value = self.attribute(name)?;
        let element = value.downcast_ref::<AXUIElement>()?;
        Some(Element::from_ref(element))
    }

    pub fn elements(&self, name: &str) -> Vec<Element> {
        let Some(value) = self.attribute(name) else {
            return Vec::new();
        };
        let Some(array) = value.downcast_ref::<CFArray>() else {
            return Vec::new();
        };
        (0..array.count())
            .filter_map(|index| {
                let item = unsafe { array.value_at_index(index) };
                let item = NonNull::new(item.cast_mut())?;
                let item: &CFType = unsafe { item.cast().as_ref() };
                let element = item.downcast_ref::<AXUIElement>()?;
                Some(Element::from_ref(element))
            })
            .collect()
    }

    pub fn set_boolean(&self, name: &str, value: bool) {
        let name = CFString::from_str(name);
        let value = CFBoolean::new(value);
        let _ = unsafe { AXUIElement::set_attribute_value(&self.0, &name, value) };
    }

    /// Sets a `CGSize` attribute, the way a resize does.
    pub fn set_size(&self, name: &str, width: f64, height: f64) {
        let mut size = CGSize::new(width, height);
        let slot = NonNull::from(&mut size).cast::<c_void>();
        let Some(value) = (unsafe { AXValue::new(AXValueType::CGSize, slot) }) else {
            return;
        };
        let name = CFString::from_str(name);
        let _ = unsafe { AXUIElement::set_attribute_value(&self.0, &name, &value) };
    }

    pub fn perform(&self, action: &str) {
        let action = CFString::from_str(action);
        let _ = unsafe { AXUIElement::perform_action(&self.0, &action) };
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Element) -> bool {
        self.0 == other.0
    }
}
