//! Pictures of windows, for the panel to show while the pointer rests on a
//! row: what the window looks like, where it would be.
//!
//! A picture costs the system some forty milliseconds and needs screen
//! recording access; the list of windows it will picture costs more and
//! changes rarely, so both are kept. Nothing is taken until the pointer has
//! rested on a row for a moment, and a picture is taken again only while the
//! pointer stays and the window can have changed.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use edged_macos::{CaptureTarget, ModifierObserver, Picture, Window, WindowId};
use inset_foundation::{App, Context, Entity, Listener, Timer};

use crate::permissions::Permissions;
use crate::settings::{PreviewTrigger, Settings};

/// How long the pointer rests on a row before its window is pictured.
const REST: Duration = Duration::from_millis(400);
/// How long the pointer rests on the next row before the preview follows:
/// a sweep down the list moves nothing.
const FOLLOW: Duration = Duration::from_millis(80);
/// How long the preview stays after the pointer leaves a row, so that between
/// two rows it does not go and come back.
const LINGER: Duration = Duration::from_millis(160);
/// How old a picture may be before the pointer, still resting, has another taken.
const REFRESH: Duration = Duration::from_secs(2);
/// How old the list of windows the system will picture may be before it is
/// read again for a window it lacks.
const LIST_AGE: Duration = Duration::from_secs(5);
/// How many pictures are kept.
const KEEP: usize = 20;
/// The most pixels a picture is taken with on either side: the window's own
/// pixels up to what a texture may be, so a picture is as sharp as the window.
const MAX_SIDE: f64 = 8192.0;

/// A picture of a window and when it was taken.
pub struct Preview {
    pub picture: Rc<Picture>,
    pub taken: Instant,
}

/// What the pointer rests on: the window, and the pixels per point of the
/// screen it is shown on, which is how large a picture is worth taking.
#[derive(Clone)]
pub struct Rest {
    pub window: Window,
    pub scale: f64,
}

pub struct Previews {
    permissions: Entity<Permissions>,
    settings: Entity<Settings>,
    /// Whether the Command key is held now, for the setting that wants it.
    command_held: bool,
    _modifiers: ModifierObserver,
    /// The window the pointer rests on now, whether or not it is shown, so a
    /// change of the Command key can show or take away its preview.
    under_pointer: Option<Rest>,
    targets: HashMap<WindowId, CaptureTarget>,
    listed: Option<Instant>,
    listing: bool,
    pictures: HashMap<WindowId, Preview>,
    in_flight: HashSet<WindowId>,
    /// The window the pointer has rested on long enough to show.
    pub showing: Option<Rest>,
    resting: Option<Timer>,
    leaving: Option<Timer>,
    refreshing: Option<Timer>,
}

impl Previews {
    pub fn start(
        cx: &mut Context<Previews>,
        permissions: &Entity<Permissions>,
        settings: &Entity<Settings>,
    ) -> Previews {
        let this = cx.weak_entity();
        let async_app = cx.to_async();
        let modifiers = ModifierObserver::new(move |command_held| {
            let this = this.clone();
            async_app.post(move |app| {
                if let Some(previews) = this.upgrade() {
                    previews.update(app, |previews, cx| {
                        previews.command_changed(cx, command_held)
                    });
                }
            });
        });
        Previews {
            permissions: permissions.clone(),
            settings: settings.clone(),
            command_held: edged_macos::command_held(),
            _modifiers: modifiers,
            under_pointer: None,
            targets: HashMap::new(),
            listed: None,
            listing: false,
            pictures: HashMap::new(),
            in_flight: HashSet::new(),
            showing: None,
            resting: None,
            leaving: None,
            refreshing: None,
        }
    }

    /// Whether pictures can be taken at all.
    pub fn available(&self, app: &App) -> bool {
        self.permissions.read(app).screen_recording
    }

    /// The pointer came to rest on a window's row, or left one. The picture
    /// follows after a moment, so a pointer passing through takes none; and
    /// when the setting asks for the Command key, only while it is held.
    pub fn rest_on(&mut self, cx: &mut Context<Previews>, rest: Option<Rest>) {
        self.under_pointer = rest.clone();
        let rest = rest.filter(|_| self.allowed(cx));
        self.consider(cx, rest);
    }

    /// Whether the setting lets a preview show now.
    fn allowed(&self, cx: &Context<Previews>) -> bool {
        match self.settings.read(cx).preview_trigger {
            PreviewTrigger::Hover => true,
            PreviewTrigger::CommandKey => self.command_held,
        }
    }

    /// The Command key went down or up: the window under the pointer may now
    /// show, or must go.
    fn command_changed(&mut self, cx: &mut Context<Previews>, held: bool) {
        self.command_held = held;
        if self.settings.read(cx).preview_trigger != PreviewTrigger::CommandKey {
            return;
        }
        let rest = self.under_pointer.clone().filter(|_| held);
        self.consider(cx, rest);
    }

    fn consider(&mut self, cx: &mut Context<Previews>, rest: Option<Rest>) {
        if let Some(timer) = self.resting.take() {
            timer.cancel(cx);
        }
        if let Some(timer) = self.leaving.take() {
            timer.cancel(cx);
        }
        let Some(rest) = rest else {
            if self.showing.is_some() {
                self.leaving = Some(schedule(cx, LINGER, |previews, cx| {
                    previews.leaving = None;
                    previews.hide(cx);
                }));
            }
            return;
        };
        if self
            .showing
            .as_ref()
            .is_some_and(|showing| showing.window.id == rest.window.id)
        {
            return;
        }
        let delay = if self.showing.is_some() { FOLLOW } else { REST };
        self.resting = Some(schedule(cx, delay, move |previews, cx| {
            previews.resting = None;
            previews.show(cx, rest.clone());
        }));
    }

    /// Takes the preview away at once: the window it showed was brought
    /// forward, or the panel it belonged to slid in.
    pub fn dismiss(&mut self, cx: &mut Context<Previews>) {
        for timer in [self.resting.take(), self.leaving.take()]
            .into_iter()
            .flatten()
        {
            timer.cancel(cx);
        }
        self.hide(cx);
    }

    /// The picture of a window, if one has been taken.
    pub fn picture_of(&self, id: WindowId) -> Option<&Preview> {
        self.pictures.get(&id)
    }

    fn show(&mut self, cx: &mut Context<Previews>, mut rest: Rest) {
        // The frame the row was built with may be a moment old; the preview
        // goes where the window is now.
        rest.window.refresh();
        self.showing = Some(rest.clone());
        cx.notify();
        self.ensure_picture(cx, &rest);
        self.schedule_refresh(cx);
    }

    fn hide(&mut self, cx: &mut Context<Previews>) {
        if self.showing.take().is_some() {
            if let Some(timer) = self.refreshing.take() {
                timer.cancel(cx);
            }
            cx.notify();
        }
    }

    /// Takes the picture again while the pointer stays, unless the window is
    /// minimized, whose picture cannot change.
    fn schedule_refresh(&mut self, cx: &mut Context<Previews>) {
        if let Some(timer) = self.refreshing.take() {
            timer.cancel(cx);
        }
        self.refreshing = Some(schedule(cx, REFRESH, |previews, cx| {
            previews.refreshing = None;
            let Some(rest) = previews.showing.clone() else {
                return;
            };
            if !rest.window.is_minimized {
                previews.ensure_picture(cx, &rest);
            }
            previews.schedule_refresh(cx);
        }));
    }

    fn ensure_picture(&mut self, cx: &mut Context<Previews>, rest: &Rest) {
        if !self.available(cx) {
            return;
        }
        let id = rest.window.id;
        if self.in_flight.contains(&id) {
            return;
        }
        let fresh = self
            .pictures
            .get(&id)
            .is_some_and(|preview| preview.taken.elapsed() < REFRESH);
        if fresh {
            return;
        }
        if self.targets.contains_key(&id) {
            self.capture(cx, rest);
            return;
        }
        let stale = self.listed.is_none_or(|listed| listed.elapsed() > LIST_AGE);
        if stale {
            self.list(cx);
        }
    }

    /// Reads which windows the system will picture; the window the pointer
    /// rests on then, if it is among them, is pictured.
    fn list(&mut self, cx: &mut Context<Previews>) {
        if self.listing {
            return;
        }
        self.listing = true;
        let this = cx.weak_entity();
        cx.spawn(async move |cx| {
            let targets = edged_macos::capturable_windows().await;
            let Some(previews) = this.upgrade() else {
                return;
            };
            cx.update(|app| {
                previews.update(app, |previews, cx| previews.targets_listed(cx, targets));
            });
        });
    }

    fn targets_listed(
        &mut self,
        cx: &mut Context<Previews>,
        targets: HashMap<WindowId, CaptureTarget>,
    ) {
        self.listing = false;
        self.listed = Some(Instant::now());
        self.targets = targets;
        if let Some(rest) = self.showing.clone()
            && self.targets.contains_key(&rest.window.id)
        {
            self.capture(cx, &rest);
        }
    }

    fn capture(&mut self, cx: &mut Context<Previews>, rest: &Rest) {
        let id = rest.window.id;
        let Some(target) = self.targets.get(&id).cloned() else {
            return;
        };
        let (width, height) = picture_size(rest);
        self.in_flight.insert(id);
        let this = cx.weak_entity();
        cx.spawn(async move |cx| {
            let picture = edged_macos::capture_window(target, width, height).await;
            let Some(previews) = this.upgrade() else {
                return;
            };
            cx.update(|app| {
                previews.update(app, |previews, cx| previews.captured(cx, id, picture));
            });
        });
    }

    fn captured(&mut self, cx: &mut Context<Previews>, id: WindowId, picture: Option<Picture>) {
        self.in_flight.remove(&id);
        let Some(picture) = picture else {
            return;
        };
        self.keep(id, picture);
        cx.notify();
    }

    /// Keeps a picture, letting the oldest go once there are too many.
    fn keep(&mut self, id: WindowId, picture: Picture) {
        self.pictures.insert(
            id,
            Preview {
                picture: Rc::new(picture),
                taken: Instant::now(),
            },
        );
        while self.pictures.len() > KEEP {
            let Some(oldest) = self
                .pictures
                .iter()
                .min_by_key(|(_, preview)| preview.taken)
                .map(|(id, _)| *id)
            else {
                break;
            };
            self.pictures.remove(&oldest);
        }
    }
}

/// The pixels a picture is worth: the window's size on its screen, capped.
fn picture_size(rest: &Rest) -> (u32, u32) {
    let width = rest.window.frame.width * rest.scale;
    let height = rest.window.frame.height * rest.scale;
    let longest = width.max(height).max(1.0);
    let fit = (MAX_SIDE / longest).min(1.0);
    ((width * fit).round() as u32, (height * fit).round() as u32)
}

/// A timer that runs `run` on the previews once, if they are still there.
fn schedule(
    cx: &mut Context<Previews>,
    delay: Duration,
    run: impl Fn(&mut Previews, &mut Context<Previews>) + 'static,
) -> Timer {
    let this = cx.weak_entity();
    Timer::new(
        cx,
        delay,
        Listener::new(move |app: &mut App| {
            if let Some(previews) = this.upgrade() {
                previews.update(app, |previews, cx| run(previews, cx));
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_the_window_at_its_screen_scale_capped_on_the_long_side() {
        let frame = |width, height| edged_macos::Frame {
            x: 0.0,
            y: 0.0,
            width,
            height,
        };
        assert_eq!(picture_size_of(frame(800.0, 600.0), 2.0), (1600, 1200));
        assert_eq!(picture_size_of(frame(5000.0, 2000.0), 2.0), (8192, 3277));
    }

    fn picture_size_of(frame: edged_macos::Frame, scale: f64) -> (u32, u32) {
        let width = frame.width * scale;
        let height = frame.height * scale;
        let longest = width.max(height).max(1.0);
        let fit = (MAX_SIDE / longest).min(1.0);
        ((width * fit).round() as u32, (height * fit).round() as u32)
    }
}
