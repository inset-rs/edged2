//! The moments the desktop changes, delivered on the main thread.
//!
//! macOS reports them three ways: accessibility notifications from each
//! application and its windows, workspace notifications for launches, quits
//! and Space switches, and an application notification for display changes.
//! All three arrive on the main run loop, which is the thread the app's own
//! loop runs on, so the callback may reach the app directly.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSWorkspace,
    NSWorkspaceActiveSpaceDidChangeNotification, NSWorkspaceDidActivateApplicationNotification,
    NSWorkspaceDidHideApplicationNotification, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification, NSWorkspaceDidUnhideApplicationNotification,
};
use objc2_application_services::{AXError, AXObserver, AXUIElement};
use objc2_core_foundation::{CFRetained, CFRunLoop, CFString, kCFRunLoopCommonModes};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSNotificationName};

use crate::application::Application;
use crate::ax::Element;
use crate::private::WindowId;
use crate::window::Window;

/// What changed, as finely as the source reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    /// An application launched, quit, became active, or was hidden or shown.
    Applications,
    /// A window of this process opened or closed: its window list is stale.
    Windows(i32),
    /// One window of this process was renamed, minimized, restored, moved or resized.
    Window(i32, WindowId),
    /// Keyboard focus moved to another window of this process.
    Focus(i32),
    /// A display started showing a different Space.
    Spaces,
    /// A display was added, removed or rearranged.
    Screens,
}

/// `kAXApplicationActivatedNotification` and its siblings, as strings; the
/// generated bindings do not export the constants.
const APPLICATION_NOTIFICATIONS: [&str; 7] = [
    "AXApplicationActivated",
    "AXApplicationDeactivated",
    "AXApplicationHidden",
    "AXApplicationShown",
    "AXFocusedWindowChanged",
    "AXMainWindowChanged",
    "AXWindowCreated",
];
const WINDOW_NOTIFICATIONS: [&str; 6] = [
    "AXUIElementDestroyed",
    "AXTitleChanged",
    "AXWindowMiniaturized",
    "AXWindowDeminiaturized",
    "AXWindowMoved",
    "AXWindowResized",
];

type OnChange = Rc<dyn Fn(Change)>;
/// A notification-center subscription and the center that hands it back.
type Subscription = (
    Retained<NSNotificationCenter>,
    Retained<ProtocolObject<dyn NSObjectProtocol>>,
);

/// Listens to the desktop and reports each change to one callback.
///
/// Lives on the main thread. Dropping it stops every subscription.
pub struct Watcher {
    on_change: OnChange,
    applications: HashMap<i32, ApplicationObserver>,
    tokens: Vec<Subscription>,
}

impl Watcher {
    /// Starts listening for launches, quits, Space switches and display
    /// changes. Applications and their windows are added with [`watch`](Self::watch).
    pub fn new(on_change: impl Fn(Change) + 'static) -> Watcher {
        let on_change: OnChange = Rc::new(on_change);
        let mut watcher = Watcher {
            on_change,
            applications: HashMap::new(),
            tokens: Vec::new(),
        };
        let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
        for name in unsafe {
            [
                NSWorkspaceDidLaunchApplicationNotification,
                NSWorkspaceDidTerminateApplicationNotification,
                NSWorkspaceDidActivateApplicationNotification,
                NSWorkspaceDidHideApplicationNotification,
                NSWorkspaceDidUnhideApplicationNotification,
            ]
        } {
            watcher.subscribe(&workspace, name, Change::Applications);
        }
        watcher.subscribe(
            &workspace,
            unsafe { NSWorkspaceActiveSpaceDidChangeNotification },
            Change::Spaces,
        );
        let application = NSNotificationCenter::defaultCenter();
        watcher.subscribe(
            &application,
            unsafe { NSApplicationDidChangeScreenParametersNotification },
            Change::Screens,
        );
        watcher
    }

    fn subscribe(
        &mut self,
        center: &Retained<NSNotificationCenter>,
        name: &NSNotificationName,
        change: Change,
    ) {
        let on_change = Rc::clone(&self.on_change);
        let block = block2::RcBlock::new(move |_: NonNull<NSNotification>| on_change(change));
        // Posted on the main thread, which is the only thread this watcher
        // ever runs on; no queue means the block runs where it is posted.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
        };
        self.tokens.push((center.clone(), token));
    }

    /// Observes an application and the windows it has now. Calling it again
    /// after the window list changes picks up the new windows.
    pub fn watch(&mut self, application: &Application, windows: &[Window]) {
        let pid = application.pid;
        if !self.applications.contains_key(&pid)
            && let Some(observer) = ApplicationObserver::new(pid, Rc::clone(&self.on_change))
        {
            self.applications.insert(pid, observer);
        }
        if let Some(observer) = self.applications.get_mut(&pid) {
            observer.observe_windows(windows);
        }
    }

    /// Stops observing every application not in `pids`.
    pub fn retain(&mut self, pids: &HashSet<i32>) {
        self.applications.retain(|pid, _| pids.contains(pid));
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for (center, token) in self.tokens.drain(..) {
            let observer: &AnyObject = (*token).as_ref();
            unsafe { center.removeObserver(observer) };
        }
    }
}

/// What the accessibility callback is handed back: whose process it watches
/// and where to report.
struct Refcon {
    pid: i32,
    on_change: OnChange,
}

struct ApplicationObserver {
    observer: CFRetained<AXObserver>,
    /// Boxed so its address stays fixed for the life of the observer.
    _refcon: Box<Refcon>,
    element: Element,
    windows: HashSet<WindowId>,
}

impl ApplicationObserver {
    fn new(pid: i32, on_change: OnChange) -> Option<ApplicationObserver> {
        let mut raw: *mut AXObserver = std::ptr::null_mut();
        let status = unsafe { AXObserver::create(pid, Some(callback), NonNull::from(&mut raw)) };
        if status != AXError::Success {
            return None;
        }
        let observer = unsafe { CFRetained::from_raw(NonNull::new(raw)?) };
        let refcon = Box::new(Refcon { pid, on_change });
        let element = Element::application(pid);
        let observer = ApplicationObserver {
            observer,
            _refcon: refcon,
            element,
            windows: HashSet::new(),
        };
        for name in APPLICATION_NOTIFICATIONS {
            observer.add(observer.element.raw(), name);
        }
        let source = unsafe { observer.observer.run_loop_source() };
        if let Some(main) = CFRunLoop::main() {
            main.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        }
        Some(observer)
    }

    fn observe_windows(&mut self, windows: &[Window]) {
        let current: HashSet<WindowId> = windows.iter().map(|window| window.id).collect();
        for window in windows {
            if self.windows.insert(window.id) {
                for name in WINDOW_NOTIFICATIONS {
                    self.add(window.element().raw(), name);
                }
            }
        }
        // A destroyed window's element is gone with it; only the id set
        // needs forgetting so a reused id is observed again.
        self.windows.retain(|id| current.contains(id));
    }

    fn add(&self, element: &AXUIElement, notification: &str) {
        let name = CFString::from_str(notification);
        let refcon = NonNull::from(&*self._refcon).as_ptr().cast::<c_void>();
        let _ = unsafe { self.observer.add_notification(element, &name, refcon) };
    }
}

impl Drop for ApplicationObserver {
    fn drop(&mut self) {
        let source = unsafe { self.observer.run_loop_source() };
        if let Some(main) = CFRunLoop::main() {
            main.remove_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        }
    }
}

unsafe extern "C-unwind" fn callback(
    _observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    refcon: *mut c_void,
) {
    if refcon.is_null() {
        return;
    }
    let refcon = unsafe { &*refcon.cast::<Refcon>() };
    let pid = refcon.pid;
    let name = unsafe { notification.as_ref() }.to_string();
    let change = match name.as_str() {
        "AXFocusedWindowChanged" | "AXMainWindowChanged" => Change::Focus(pid),
        "AXApplicationActivated"
        | "AXApplicationDeactivated"
        | "AXApplicationHidden"
        | "AXApplicationShown" => Change::Applications,
        "AXTitleChanged"
        | "AXWindowMiniaturized"
        | "AXWindowDeminiaturized"
        | "AXWindowMoved"
        | "AXWindowResized" => match crate::private::window_id(unsafe { element.as_ref() }) {
            Some(id) => Change::Window(pid, id),
            None => Change::Windows(pid),
        },
        _ => Change::Windows(pid),
    };
    (refcon.on_change)(change);
}
