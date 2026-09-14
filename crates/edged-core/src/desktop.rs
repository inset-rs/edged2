//! The desktop as the panel shows it, kept current as macOS reports changes,
//! and the few things the panel does to it.
//!
//! Reading a window costs a round trip to its application, and finding the
//! windows an application keeps on other Spaces costs a walk over its element
//! ids, so nothing here reads more than a few milliseconds' worth in one
//! turn. The desktop starts with the applications alone and fills in their
//! windows and icons a slice at a time; changes macOS reports are gathered
//! for a moment and applied together; and every few seconds the window
//! server's own list of windows, one call that covers every Space, says
//! which applications have windows the panel has not seen, and only those
//! are read again, and only those walked.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use edged_macos::{
    Application, Change, Screen, Space, SpaceId, SpaceKind, Watcher, Window, WindowId,
};
use inset_foundation::{App, AsyncApp, Context, Entity, Listener, Subscription, Timer, WeakEntity};

use crate::permissions::{Granted, Permission, Permissions};

/// The size an application icon is rasterised at, in pixels: twice the points
/// it is drawn at, so it stays crisp on a Retina display.
const ICON_PIXELS: usize = 48;
/// How often the window server's list is compared with what the panel has,
/// for whatever the notifications missed.
const RECONCILE: Duration = Duration::from_secs(5);
/// How often the Dock badges are read; nothing announces a change to them,
/// and reading them walks the Dock's accessibility tree, so not often.
const BADGE_POLL: Duration = Duration::from_secs(5);
/// How long one turn may spend reading applications.
const SLICE: Duration = Duration::from_millis(8);
/// A process whose listing took longer than this is busy or stuck; it is left
/// alone for [`SLOW_REST`] rather than waited for again.
const SLOW: Duration = Duration::from_millis(150);
const SLOW_REST: Duration = Duration::from_secs(30);
/// How long changes are gathered before they are applied together: a
/// window being dragged reports a move for every frame.
const GATHER: Duration = Duration::from_millis(50);
/// How long the window server takes to register a hold before a Space
/// switch carries the held window along.
const HOLD: Duration = Duration::from_millis(50);
/// How long the Space switch animation runs before a held window can be let go.
const SWITCH: Duration = Duration::from_millis(400);
/// How long the window server may take to enable a shortcut before it is pressed,
/// which is also long enough for the click that asked to be over.
const SHORTCUT_SETTLE: Duration = Duration::from_millis(30);

/// One application with the windows it has open.
pub struct AppEntry {
    pub application: Application,
    pub windows: Vec<Window>,
    /// The window this application would type into, read only for the active
    /// application: for any other it names a window nobody is looking at.
    pub focused: Option<WindowId>,
    /// The icon as PNG bytes, kept across scans because rasterising an
    /// `NSImage` is far dearer than listing windows.
    pub icon: Option<Arc<[u8]>>,
    /// The Dock tile's badge, such as an unread count.
    pub badge: Option<String>,
}

impl AppEntry {
    fn window_ids(&self) -> HashSet<WindowId> {
        self.windows.iter().map(|window| window.id).collect()
    }

    /// What the icon cache is keyed by: the bundle, so a relaunch finds it.
    fn icon_key(&self) -> String {
        icon_key(&self.application)
    }
}

/// One read of the desktop still to do, a slice at a time.
enum Job {
    /// The windows an application lists; `expected` is what the window server
    /// holds for it, and what is still missing afterwards is walked for.
    List {
        pid: i32,
        expected: HashSet<WindowId>,
    },
    /// The walk over an application's element ids, resumed from `cursor`
    /// until `expected` is accounted for or the ids run out.
    Walk {
        pid: i32,
        expected: HashSet<WindowId>,
        cursor: u64,
    },
    Icon(i32),
}

/// Everything the panel draws, as of the last change macOS reported.
///
/// Owns the watcher that hears those changes and the timers that read what
/// macOS does not announce. Every change ends in a notify, so a build that
/// read the desktop is rebuilt.
pub struct Desktop {
    pub screens: Vec<Screen>,
    pub spaces: Vec<Space>,
    pub applications: Vec<AppEntry>,
    /// The Space each display shows, by display uuid.
    showing: HashMap<String, SpaceId>,
    icons: HashMap<String, Arc<[u8]>>,
    /// The Dock's badges as last read, by bundle path.
    badges: HashMap<String, String>,
    watcher: Rc<RefCell<Watcher>>,
    jobs: VecDeque<Job>,
    scanning: Option<Timer>,
    /// Windows the window server lists for a process that a whole walk did not
    /// find: windows accessibility never shows, not to be walked for again
    /// until the process's list changes.
    unresolved: HashMap<i32, HashSet<WindowId>>,
    /// Processes that answered late, and when they may be asked again.
    slow: HashMap<i32, Instant>,
    gathered: HashSet<Change>,
    gathering: Option<Timer>,
    reconciling: Option<Timer>,
    badge_poll: Option<Timer>,
    _granted: Subscription,
}

impl Desktop {
    /// Reads the applications and starts following the desktop; their windows
    /// and icons follow a slice at a time. Without accessibility access no
    /// application reports any window; the grant, when it lands, is what
    /// reads them.
    pub fn start(cx: &mut Context<Desktop>, permissions: &Entity<Permissions>) -> Desktop {
        let watcher = watcher_for(cx.weak_entity(), cx.to_async());
        let granted = cx.subscribe(
            permissions,
            |desktop: &mut Desktop, _permissions, event: &Granted, cx| {
                if event.0 == Permission::Accessibility {
                    desktop.refresh(cx);
                }
            },
        );
        let mut desktop = Desktop {
            screens: edged_macos::screens(),
            spaces: Vec::new(),
            applications: Vec::new(),
            showing: HashMap::new(),
            icons: HashMap::new(),
            badges: HashMap::new(),
            watcher,
            jobs: VecDeque::new(),
            scanning: None,
            unresolved: HashMap::new(),
            slow: HashMap::new(),
            gathered: HashSet::new(),
            gathering: None,
            reconciling: None,
            badge_poll: None,
            _granted: granted,
        };
        desktop.rescan_spaces();
        let expected = edged_macos::window_ids_by_process();
        desktop.sync_applications(&expected);
        desktop.follow();
        desktop.schedule_scan(cx);
        desktop.schedule_reconcile(cx);
        desktop.schedule_badges(cx);
        desktop
    }

    /// Reads everything again: what the menu's Refresh asks for, and what the
    /// accessibility grant is answered with.
    pub fn refresh(&mut self, cx: &mut Context<Desktop>) {
        self.rescan_screens();
        self.rescan_spaces();
        let expected = edged_macos::window_ids_by_process();
        self.sync_applications(&expected);
        for entry in &self.applications {
            let pid = entry.application.pid;
            self.jobs.push_back(Job::List {
                pid,
                expected: expected.get(&pid).cloned().unwrap_or_default(),
            });
        }
        self.schedule_scan(cx);
        cx.notify();
    }

    /// Takes one change macOS reported, to be applied with whatever else
    /// arrives in the next moment.
    pub fn apply(&mut self, change: Change, cx: &mut Context<Desktop>) {
        self.gathered.insert(change);
        if self.gathering.is_none() {
            self.gathering = Some(schedule(cx, GATHER, |desktop, cx| {
                desktop.gathering = None;
                desktop.flush(cx);
            }));
        }
    }

    /// Brings a window to the front, as clicking it would.
    pub fn focus(&self, window: &Window) {
        window.focus();
    }

    /// Brings an application forward with all its windows, as its Dock tile would.
    pub fn activate(&self, application: &Application) {
        application.activate();
    }

    pub fn close(&self, window: &Window) {
        window.close();
    }

    pub fn minimize(&self, window: &Window) {
        window.minimize();
    }

    pub fn unminimize(&self, window: &Window) {
        window.unminimize();
    }

    pub fn leave_fullscreen(&self, window: &Window) {
        window.leave_fullscreen();
    }

    pub fn quit(&self, application: &Application) {
        application.quit();
    }

    /// Switches a display to one of its Spaces the way the keyboard does it,
    /// through Mission Control's own shortcut, so the animation and the Dock
    /// agree with what happened; a Space the shortcuts do not reach, a
    /// full-screen one or a desktop past the sixteenth, is left where it is.
    ///
    /// The Space is looked up afresh first: with Spaces rearranged by recent
    /// use, the desktop number a heading was built with may name another
    /// desktop by the time it is clicked. The shortcut is enabled a moment
    /// before it is pressed, and pressed on a later turn than the click,
    /// outside the pointer event AppKit is still dispatching.
    pub fn switch_to(&self, cx: &mut Context<Desktop>, space: &Space) {
        let Some(target) = edged_macos::spaces()
            .into_iter()
            .find(|fresh| fresh.id == space.id)
        else {
            eprintln!("edged: Space {} is gone", space.id);
            return;
        };
        if target.is_current() {
            return;
        }
        if !target.enable_shortcut() {
            eprintln!("edged: no shortcut switches to Space {}", target.id);
            return;
        }
        let _ = Timer::new(
            cx,
            SHORTCUT_SETTLE,
            Listener::new(move |_app: &mut App| {
                target.press_shortcut();
            }),
        );
    }

    /// Moves a window to another desktop the way a user does it: holds it by
    /// its title bar, switches Space, lets go. Each step waits for the one
    /// before it to land, on the app's timers.
    pub fn move_to_space(&self, cx: &mut Context<Desktop>, window: Window, space: Space) {
        let Some(point) = window.grab() else {
            return;
        };
        let this = cx.weak_entity();
        let _ = Timer::new(
            cx,
            HOLD,
            Listener::new(move |app: &mut App| {
                if !space.press_shortcut() {
                    eprintln!("edged: no shortcut switches to Space {}", space.id);
                }
                let this = this.clone();
                let window = window.clone();
                let _ = Timer::new(
                    app,
                    SWITCH,
                    Listener::new(move |app: &mut App| {
                        window.release(point);
                        if let Some(desktop) = this.upgrade() {
                            desktop.update(app, |desktop, cx| desktop.apply(Change::Spaces, cx));
                        }
                    }),
                );
            }),
        );
    }

    /// Applications with nothing to switch to, alphabetically: they can only be
    /// activated, not focused window by window.
    pub fn windowless(&self) -> Vec<&AppEntry> {
        let mut entries: Vec<&AppEntry> = self
            .applications
            .iter()
            .filter(|entry| entry.windows.is_empty())
            .collect();
        entries.sort_by_key(|entry| entry.application.name.to_lowercase());
        entries
    }

    /// The screen with this display id, while it is attached.
    pub fn screen(&self, display_id: u32) -> Option<&Screen> {
        self.screens
            .iter()
            .find(|screen| screen.display_id == display_id)
    }

    /// The Spaces of one screen, in Mission Control order.
    pub fn spaces_of(&self, screen: &Screen) -> Vec<&Space> {
        self.spaces
            .iter()
            .filter(|space| space.display_uuid == screen.uuid)
            .collect()
    }

    /// The desktops of one screen a window could move to: every Space of that
    /// screen except full-screen ones, which belong to one window.
    pub fn desktops_of(&self, screen: &Screen) -> Vec<&Space> {
        self.spaces_of(screen)
            .into_iter()
            .filter(|space| space.kind == SpaceKind::Desktop)
            .collect()
    }

    /// The windows on one Space, by title: a window keeps its place whatever
    /// the user does with it, so the hand learns where things are.
    pub fn windows_on(&self, space: SpaceId) -> Vec<(&AppEntry, &Window)> {
        let mut windows: Vec<(&AppEntry, &Window)> = self
            .applications
            .iter()
            .flat_map(|entry| entry.windows.iter().map(move |window| (entry, window)))
            .filter(|(_, window)| window.spaces.contains(&space))
            .collect();
        windows.sort_by(|(left_entry, left), (right_entry, right)| {
            window_order(
                (&left_entry.application.name, &left.title, left.id),
                (&right_entry.application.name, &right.title, right.id),
            )
        });
        windows
    }

    /// A window by its id, whichever application it belongs to.
    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.applications
            .iter()
            .flat_map(|entry| entry.windows.iter())
            .find(|window| window.id == id)
    }

    /// Whether a Space is the one its display is showing.
    pub fn is_showing(&self, space: &Space) -> bool {
        self.showing.get(&space.display_uuid) == Some(&space.id)
    }

    /// The Space a screen is showing.
    pub fn showing_on(&self, screen: &Screen) -> Option<&Space> {
        let id = *self.showing.get(&screen.uuid)?;
        self.spaces.iter().find(|space| space.id == id)
    }

    /// Whether a screen is showing a full-screen window, which is when the
    /// panel keeps out of the way.
    pub fn is_full_screen(&self, screen: &Screen) -> bool {
        self.showing_on(screen)
            .is_some_and(|space| space.kind == SpaceKind::Fullscreen)
    }

    /// The screen a point is on.
    pub fn screen_at(&self, (x, y): (f64, f64)) -> Option<&Screen> {
        self.screens
            .iter()
            .find(|screen| screen.frame.contains(x, y))
    }

    /// The screen a window's centre is on.
    pub fn screen_of(&self, window: &Window) -> Option<&Screen> {
        let x = window.frame.x + window.frame.width / 2.0;
        let y = window.frame.y + window.frame.height / 2.0;
        self.screens
            .iter()
            .find(|screen| screen.frame.contains(x, y))
    }

    /// Applies the changes gathered since the last time, then reads what they
    /// left to read.
    fn flush(&mut self, cx: &mut Context<Desktop>) {
        let changes = std::mem::take(&mut self.gathered);
        let needs_list = changes
            .iter()
            .any(|change| matches!(change, Change::Applications | Change::Windows(_)));
        let expected = if needs_list {
            edged_macos::window_ids_by_process()
        } else {
            HashMap::new()
        };
        for change in changes {
            match change {
                Change::Applications => self.sync_applications(&expected),
                Change::Windows(pid) => self.jobs.push_back(Job::List {
                    pid,
                    expected: expected.get(&pid).cloned().unwrap_or_default(),
                }),
                Change::Window(pid, id) => self.refresh_window(pid, id),
                Change::Closed(pid, id) => self.remove_window(pid, id),
                Change::Focus(pid) => {
                    self.refresh_active();
                    if let Some(entry) = self.entry_mut(pid) {
                        entry.focused = focused_of(&entry.application);
                    }
                }
                Change::Spaces => {
                    self.rescan_spaces();
                    for entry in &mut self.applications {
                        for window in &mut entry.windows {
                            window.refresh_spaces();
                        }
                    }
                }
                Change::Screens => self.rescan_screens(),
            }
        }
        self.follow();
        self.schedule_scan(cx);
        cx.notify();
    }

    /// Does the reads still queued, for one slice of a turn, and comes back
    /// next turn for the rest.
    fn scan(&mut self, cx: &mut Context<Desktop>) {
        let started = Instant::now();
        let mut changed = false;
        while let Some(job) = self.jobs.pop_front() {
            match job {
                Job::List { pid, expected } => {
                    if self
                        .slow
                        .get(&pid)
                        .is_some_and(|until| Instant::now() < *until)
                    {
                        continue;
                    }
                    let asked = Instant::now();
                    self.list_windows(pid, expected);
                    if asked.elapsed() > SLOW {
                        self.slow.insert(pid, Instant::now() + SLOW_REST);
                    }
                    changed = true;
                }
                Job::Walk {
                    pid,
                    expected,
                    mut cursor,
                } => {
                    let budget = SLICE.saturating_sub(started.elapsed());
                    let (found, done) = edged_macos::walk_step(pid, &mut cursor, budget);
                    let found: Vec<Window> = found
                        .into_iter()
                        .filter(|window| expected.contains(&window.id))
                        .collect();
                    if !found.is_empty() {
                        self.add_windows(pid, found);
                        changed = true;
                    }
                    let known = self
                        .entry(pid)
                        .map(AppEntry::window_ids)
                        .unwrap_or_default();
                    if expected.is_subset(&known) {
                        self.unresolved.remove(&pid);
                    } else if done {
                        // Walked to the end and still missing: the window server lists
                        // what accessibility does not show, and asking again would find
                        // no more until the list changes.
                        self.unresolved
                            .insert(pid, expected.difference(&known).copied().collect());
                    } else {
                        self.jobs.push_front(Job::Walk {
                            pid,
                            expected,
                            cursor,
                        });
                        break;
                    }
                }
                Job::Icon(pid) => changed |= self.load_icon(pid),
            }
            if started.elapsed() >= SLICE {
                break;
            }
        }
        if changed {
            self.follow();
            cx.notify();
        }
        if self.jobs.is_empty() {
            self.scanning = None;
        } else {
            self.scanning = Some(schedule(cx, Duration::ZERO, |desktop, cx| desktop.scan(cx)));
        }
    }

    fn schedule_scan(&mut self, cx: &mut Context<Desktop>) {
        if self.scanning.is_none() && !self.jobs.is_empty() {
            self.scanning = Some(schedule(cx, Duration::ZERO, |desktop, cx| desktop.scan(cx)));
        }
    }

    /// Reads what an application lists, keeps the windows on other Spaces the
    /// list leaves out as long as the window server still holds them, and
    /// queues a walk for whatever is still unaccounted for.
    fn list_windows(&mut self, pid: i32, expected: HashSet<WindowId>) {
        let trusted = edged_macos::is_trusted();
        let Some(entry) = self.entry(pid) else {
            return;
        };
        // The window server's list is the judge of what is a window: accessibility
        // also reports the ones an application keeps ordered out. Only a hidden
        // application's windows are taken on accessibility's word, since the server
        // has none of them while the application is hidden.
        let hidden = entry.application.is_hidden;
        let listed: Vec<Window> = edged_macos::listed_windows(pid)
            .into_iter()
            .filter(|window| hidden || expected.contains(&window.id))
            .collect();
        let Some(entry) = self.entry_mut(pid) else {
            return;
        };
        let listed_ids: HashSet<WindowId> = listed.iter().map(|window| window.id).collect();
        let mut windows = listed;
        windows.extend(
            entry
                .windows
                .drain(..)
                .filter(|window| !listed_ids.contains(&window.id) && expected.contains(&window.id)),
        );
        entry.windows = windows;
        entry.focused = focused_of(&entry.application);
        let known = entry.window_ids();
        let walking = self
            .jobs
            .iter()
            .any(|job| matches!(job, Job::Walk { pid: walked, .. } if *walked == pid));
        let missing: HashSet<WindowId> = expected.difference(&known).copied().collect();
        let worth_walking = self
            .unresolved
            .get(&pid)
            .is_none_or(|unresolved| !missing.is_subset(unresolved));
        if trusted && !walking && !missing.is_empty() && worth_walking {
            self.jobs.push_back(Job::Walk {
                pid,
                expected,
                cursor: 0,
            });
        }
    }

    fn add_windows(&mut self, pid: i32, found: Vec<Window>) {
        let Some(entry) = self.entry_mut(pid) else {
            return;
        };
        let known = entry.window_ids();
        entry.windows.extend(
            found
                .into_iter()
                .filter(|window| !known.contains(&window.id)),
        );
    }

    /// Drops a window macOS reported destroyed: the application is back among
    /// the windowless ones at once when it was the last.
    fn remove_window(&mut self, pid: i32, id: WindowId) {
        if let Some(entry) = self.entry_mut(pid) {
            entry.windows.retain(|window| window.id != id);
        }
    }

    /// Re-reads one window, and drops it once it is gone.
    fn refresh_window(&mut self, pid: i32, id: WindowId) {
        if let Some(entry) = self.entry_mut(pid)
            && let Some(index) = entry.windows.iter().position(|w| w.id == id)
            && !entry.windows[index].refresh()
        {
            entry.windows.remove(index);
        }
    }

    fn load_icon(&mut self, pid: i32) -> bool {
        let Some(entry) = self.entry(pid) else {
            return false;
        };
        if entry.icon.is_some() {
            return false;
        }
        let key = entry.icon_key();
        let icon = match self.icons.get(&key) {
            Some(icon) => Arc::clone(icon),
            None => {
                let Some(png) = entry.application.icon_png(ICON_PIXELS) else {
                    return false;
                };
                let png: Arc<[u8]> = Arc::from(png);
                self.icons.insert(key, Arc::clone(&png));
                png
            }
        };
        if let Some(entry) = self.entry_mut(pid) {
            entry.icon = Some(icon);
        }
        true
    }

    /// Adds applications that launched, drops those that quit, and re-reads
    /// which one is active; windows of applications already listed are kept,
    /// and a new application's are queued.
    fn sync_applications(&mut self, expected: &HashMap<i32, HashSet<WindowId>>) {
        let mut previous: HashMap<i32, AppEntry> = self
            .applications
            .drain(..)
            .map(|entry| (entry.application.pid, entry))
            .collect();
        let mut arrived = Vec::new();
        self.applications = edged_macos::running_applications()
            .into_iter()
            .map(|application| match previous.remove(&application.pid) {
                Some(mut entry) => {
                    entry.focused = focused_of(&application);
                    entry.application = application;
                    entry
                }
                None => {
                    arrived.push(application.pid);
                    self.entry_for(application)
                }
            })
            .collect();
        let living = self.pids();
        self.unresolved.retain(|pid, _| living.contains(pid));
        self.slow.retain(|pid, _| living.contains(pid));
        for pid in arrived {
            self.jobs.push_back(Job::List {
                pid,
                expected: expected.get(&pid).cloned().unwrap_or_default(),
            });
            self.jobs.push_back(Job::Icon(pid));
        }
        self.forget_gone_icons();
    }

    /// Compares the window server's list with what the panel has and reads
    /// again only the applications that differ.
    fn reconcile(&mut self, cx: &mut Context<Desktop>) {
        let expected = edged_macos::window_ids_by_process();
        let before = self.pids();
        self.sync_applications(&expected);
        let mut changed = self.pids() != before;
        for entry in &self.applications {
            let pid = entry.application.pid;
            let Some(want) = expected.get(&pid) else {
                continue;
            };
            let queued = self.jobs.iter().any(|job| match job {
                Job::List { pid: p, .. } | Job::Walk { pid: p, .. } => *p == pid,
                Job::Icon(_) => false,
            });
            let known = entry.window_ids();
            let missing: HashSet<WindowId> = want.difference(&known).copied().collect();
            // A window the server no longer lists was ordered out or closed without
            // a word; the list is read again to drop it.
            let stale = !entry.application.is_hidden && !known.is_subset(want);
            let settled = self
                .unresolved
                .get(&pid)
                .is_some_and(|unresolved| missing.is_subset(unresolved));
            if !queued && (stale || (!missing.is_empty() && !settled)) {
                self.jobs.push_back(Job::List {
                    pid,
                    expected: want.clone(),
                });
                changed = true;
            }
        }
        if changed {
            self.follow();
            cx.notify();
        }
        self.schedule_scan(cx);
    }

    fn schedule_reconcile(&mut self, cx: &mut Context<Desktop>) {
        self.reconciling = Some(schedule(cx, RECONCILE, |desktop, cx| {
            desktop.reconcile(cx);
            desktop.schedule_reconcile(cx);
        }));
    }

    /// Re-reads the Dock badges, telling observers only when one changed.
    fn schedule_badges(&mut self, cx: &mut Context<Desktop>) {
        self.badge_poll = Some(schedule(cx, BADGE_POLL, |desktop, cx| {
            if desktop.refresh_badges() {
                cx.notify();
            }
            desktop.schedule_badges(cx);
        }));
    }

    fn refresh_badges(&mut self) -> bool {
        self.badges = edged_macos::badges();
        let mut changed = false;
        for entry in &mut self.applications {
            let badge = badge_of(&entry.application, &self.badges);
            if entry.badge != badge {
                entry.badge = badge;
                changed = true;
            }
        }
        changed
    }

    fn refresh_active(&mut self) {
        let active: HashMap<i32, bool> = edged_macos::running_applications()
            .into_iter()
            .map(|application| (application.pid, application.is_active))
            .collect();
        for entry in &mut self.applications {
            if let Some(is_active) = active.get(&entry.application.pid) {
                entry.application.is_active = *is_active;
            }
        }
    }

    fn rescan_screens(&mut self) {
        self.screens = edged_macos::screens();
    }

    fn rescan_spaces(&mut self) {
        self.spaces = edged_macos::spaces();
        self.showing = self
            .spaces
            .iter()
            .map(|space| space.display_uuid.clone())
            .collect::<HashSet<String>>()
            .into_iter()
            .map(|uuid| {
                let current = edged_macos::current_space(&uuid);
                (uuid, current)
            })
            .collect();
    }

    /// The processes the panel lists, for the watcher to follow.
    fn pids(&self) -> HashSet<i32> {
        self.applications
            .iter()
            .map(|entry| entry.application.pid)
            .collect()
    }

    /// Observes every application listed and every window it has, and stops
    /// observing the ones that quit.
    fn follow(&self) {
        let mut watcher = self.watcher.borrow_mut();
        watcher.retain(&self.pids());
        for entry in &self.applications {
            watcher.watch(&entry.application, &entry.windows);
        }
    }

    /// An application as first seen: no windows and no icon yet, both queued.
    fn entry_for(&self, application: Application) -> AppEntry {
        AppEntry {
            focused: focused_of(&application),
            badge: badge_of(&application, &self.badges),
            icon: self.icons.get(&icon_key(&application)).map(Arc::clone),
            windows: Vec::new(),
            application,
        }
    }

    fn entry(&self, pid: i32) -> Option<&AppEntry> {
        self.applications
            .iter()
            .find(|entry| entry.application.pid == pid)
    }

    fn entry_mut(&mut self, pid: i32) -> Option<&mut AppEntry> {
        self.applications
            .iter_mut()
            .find(|entry| entry.application.pid == pid)
    }

    fn forget_gone_icons(&mut self) {
        let keys: HashSet<String> = self
            .applications
            .iter()
            .map(|entry| entry.icon_key())
            .collect();
        self.icons.retain(|key, _| keys.contains(key));
    }
}

/// A timer that runs `run` on the desktop once, if the desktop is still there.
fn schedule(
    cx: &mut Context<Desktop>,
    delay: Duration,
    run: impl Fn(&mut Desktop, &mut Context<Desktop>) + 'static,
) -> Timer {
    let this = cx.weak_entity();
    Timer::new(
        cx,
        delay,
        Listener::new(move |app: &mut App| {
            if let Some(desktop) = this.upgrade() {
                desktop.update(app, |desktop, cx| run(desktop, cx));
            }
        }),
    )
}

/// A watcher whose every change reaches the desktop at the app's next
/// checkpoint: macOS delivers notifications outside any event the host
/// drives, sometimes while the app is busy, as during a menu.
fn watcher_for(this: WeakEntity<Desktop>, async_app: AsyncApp) -> Rc<RefCell<Watcher>> {
    Rc::new(RefCell::new(Watcher::new(move |change| {
        let this = this.clone();
        async_app.post(move |app| {
            if let Some(desktop) = this.upgrade() {
                desktop.update(app, |desktop, cx| desktop.apply(change, cx));
            }
        });
    })))
}

/// Windows by title, case aside, and by id when the titles agree.
/// Windows by application, then by title within one, so an application's windows sit
/// together whatever they are called.
fn window_order(left: (&str, &str, WindowId), right: (&str, &str, WindowId)) -> Ordering {
    left.0
        .to_lowercase()
        .cmp(&right.0.to_lowercase())
        .then_with(|| title_order((left.1, left.2), (right.1, right.2)))
}

fn title_order(left: (&str, WindowId), right: (&str, WindowId)) -> Ordering {
    left.0
        .to_lowercase()
        .cmp(&right.0.to_lowercase())
        .then_with(|| left.1.cmp(&right.1))
}

fn icon_key(application: &Application) -> String {
    application
        .bundle_path
        .clone()
        .unwrap_or_else(|| application.pid.to_string())
}

fn focused_of(application: &Application) -> Option<WindowId> {
    application
        .is_active
        .then(|| edged_macos::focused_window_of(application.pid))
        .flatten()
}

fn badge_of(application: &Application, badges: &HashMap<String, String>) -> Option<String> {
    let path = application.bundle_path.as_deref()?;
    badges.get(path.trim_end_matches('/')).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_order_by_application_before_title() {
        assert_eq!(
            window_order(("Safari", "Apple", 1), ("Finder", "Zebra", 2)),
            Ordering::Greater
        );
        assert_eq!(
            window_order(("finder", "b", 1), ("Finder", "a", 2)),
            Ordering::Greater
        );
    }

    #[test]
    fn windows_order_by_title_whatever_their_case_and_by_id_when_titled_alike() {
        assert_eq!(title_order(("alpha", 2), ("Beta", 1)), Ordering::Less);
        assert_eq!(title_order(("Beta", 1), ("alpha", 2)), Ordering::Greater);
        assert_eq!(title_order(("Same", 5), ("same", 3)), Ordering::Greater);
        assert_eq!(title_order(("Same", 3), ("same", 3)), Ordering::Equal);
    }
}
