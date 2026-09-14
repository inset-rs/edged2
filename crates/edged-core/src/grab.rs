//! A window held by the modifier keys and moved, resized or arranged by the pointer,
//! with no click: hold the move chord, ⌥ unless set otherwise, and the window under the
//! pointer follows it; hold the resize chord, ⌥⌃, and a corner of it does; hold the
//! arrange chord, ⌃⌘, and the direction the pointer goes picks a zone of the screen the
//! window is put in when the keys are let go, which a ring at the pointer shows. Letting
//! go of the keys lets go of the window.
//!
//! The hold keeps where the pointer took it and where it is now, so what shows it reads
//! the same state the gesture does.

use edged_macos::{Frame, Input, InputObserver, Modifiers, Screen, Window, WindowId};
use inset_foundation::{Context, Entity};

use crate::desktop::Desktop;
use crate::settings::{ResizeCorner, Settings};
use crate::zone::{Direction, RingZones, Zone};

/// Pointer travel before a hold moves anything: a tap of the keys alone does nothing.
const DEAD_ZONE: f64 = 4.0;
/// The smallest a window is resized to.
const MIN_SIZE: f64 = 100.0;

/// What a hold does to its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Move,
    Resize,
    /// Puts the window in the zone the pointer's direction picks, once the keys are
    /// let go.
    Arrange,
}

/// The corner a resize drags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corner {
    pub left: bool,
    pub top: bool,
}

impl Corner {
    const BOTTOM_RIGHT: Corner = Corner {
        left: false,
        top: false,
    };

    fn nearest(frame: &Frame, (x, y): (f64, f64)) -> Corner {
        Corner {
            left: x - frame.x < frame.width / 2.0,
            top: y - frame.y < frame.height / 2.0,
        }
    }

    /// The corner a resize drags, by the setting: a fixed one, or the one nearest the
    /// pointer.
    fn for_setting(setting: ResizeCorner, frame: &Frame, pointer: (f64, f64)) -> Corner {
        match setting {
            ResizeCorner::BottomRight => Corner::BOTTOM_RIGHT,
            ResizeCorner::Nearest => Corner::nearest(frame, pointer),
        }
    }
}

/// A window held by the keys.
#[derive(Clone, Debug)]
pub struct Hold {
    pub window: Window,
    pub mode: Mode,
    /// Where the pointer was when the hold began, in screen space.
    pub origin: (f64, f64),
    /// Where the pointer is now.
    pub pointer: (f64, f64),
    /// The window's frame when the hold began; the move is measured from it.
    pub frame: Frame,
    pub corner: Corner,
    /// Whether the pointer has travelled past the dead zone.
    pub moving: bool,
    /// The usable area of the screen the window was on when the hold began: what the
    /// ring's zones are cut from.
    pub usable: Frame,
    /// The zone each direction of the ring picks, as set when the hold began.
    pub zones: RingZones,
}

impl Hold {
    fn offset(&self) -> (f64, f64) {
        (
            self.pointer.0 - self.origin.0,
            self.pointer.1 - self.origin.1,
        )
    }

    /// The frame the window is given for where the pointer is now; an arranging hold
    /// leaves the frame as it is until the keys are let go.
    pub fn frame_now(&self) -> Frame {
        let (dx, dy) = self.offset();
        match self.mode {
            Mode::Move => Frame {
                x: self.frame.x + dx,
                y: self.frame.y + dy,
                ..self.frame
            },
            Mode::Resize => resized(&self.frame, self.corner, dx, dy),
            Mode::Arrange => self.frame,
        }
    }

    /// The zone the pointer's direction picks, for an arranging hold; `None` while the
    /// pointer has not left the ring's dead zone, or in a direction set to nothing.
    pub fn zone(&self) -> Option<Zone> {
        if self.mode != Mode::Arrange {
            return None;
        }
        let direction = Direction::of(self.offset())?;
        self.zones[direction.index()]
    }

    /// Where the window would go if the keys were let go now.
    pub fn target(&self) -> Option<Frame> {
        self.zone().map(|zone| zone.frame(&self.usable))
    }
}

/// `frame` moved the least that keeps it within `usable`: a window wider or taller than
/// the area sits at its top-left.
fn kept_on(frame: &Frame, usable: &Frame) -> Frame {
    let x = frame
        .x
        .min(usable.x + usable.width - frame.width)
        .max(usable.x);
    let y = frame
        .y
        .min(usable.y + usable.height - frame.height)
        .max(usable.y);
    Frame { x, y, ..*frame }
}

/// `frame` cut down to what lies within `usable`: a resize stops at the screen's edge.
fn kept_within(frame: &Frame, usable: &Frame) -> Frame {
    let left = frame.x.max(usable.x);
    let top = frame.y.max(usable.y);
    let right = (frame.x + frame.width).min(usable.x + usable.width);
    let bottom = (frame.y + frame.height).min(usable.y + usable.height);
    Frame {
        x: left,
        y: top,
        width: (right - left).max(MIN_SIZE.min(usable.width)),
        height: (bottom - top).max(MIN_SIZE.min(usable.height)),
    }
}

/// `frame` with its `corner` dragged by `(dx, dy)`, never below the smallest size.
fn resized(frame: &Frame, corner: Corner, dx: f64, dy: f64) -> Frame {
    let (mut x, mut width) = (frame.x, frame.width);
    if corner.left {
        let new_width = (width - dx).max(MIN_SIZE);
        x += width - new_width;
        width = new_width;
    } else {
        width = (width + dx).max(MIN_SIZE);
    }
    let (mut y, mut height) = (frame.y, frame.height);
    if corner.top {
        let new_height = (height - dy).max(MIN_SIZE);
        y += height - new_height;
        height = new_height;
    } else {
        height = (height + dy).max(MIN_SIZE);
    }
    Frame {
        x,
        y,
        width,
        height,
    }
}

/// The mode the keys held ask for, by the chords set; the first of move, resize and
/// arrange when two are the same. Anything else holds nothing.
fn mode_for(modifiers: Modifiers, settings: &Settings) -> Option<Mode> {
    if settings.move_chord.matches(modifiers) {
        Some(Mode::Move)
    } else if settings.resize_chord.matches(modifiers) {
        Some(Mode::Resize)
    } else if settings.ring_enabled && settings.arrange_chord.matches(modifiers) {
        Some(Mode::Arrange)
    } else {
        None
    }
}

pub struct Grab {
    desktop: Entity<Desktop>,
    settings: Entity<Settings>,
    /// The window held now, if any.
    pub hold: Option<Hold>,
    /// Where the pointer was last seen while a window is held.
    pointer: (f64, f64),
    _keys: InputObserver,
    /// The watch on the pointer, kept only while a window is held: every move of the
    /// mouse anywhere would otherwise wake the app.
    pointer_watch: Option<InputObserver>,
}

impl Grab {
    /// Starts listening to the keys; nothing is held, and no pointer is watched, until ⌥
    /// goes down.
    pub fn start(
        cx: &mut Context<Grab>,
        desktop: Entity<Desktop>,
        settings: Entity<Settings>,
    ) -> Grab {
        let keys = InputObserver::keys(Grab::relay(cx));
        Grab {
            desktop,
            settings,
            hold: None,
            pointer: (0.0, 0.0),
            _keys: keys,
            pointer_watch: None,
        }
    }

    /// A callback that brings an input to this entity on the app's next turn.
    fn relay(cx: &mut Context<Grab>) -> impl Fn(Input) + 'static {
        let this = cx.weak_entity();
        let cx_async = cx.to_async();
        move |input| {
            let this = this.clone();
            cx_async.post(move |app| {
                if let Some(grab) = this.upgrade() {
                    grab.update(app, |grab, cx| grab.input(cx, input));
                }
            });
        }
    }

    fn input(&mut self, cx: &mut Context<Grab>, input: Input) {
        match input {
            Input::Moved { x, y } => self.moved(cx, (x, y)),
            Input::Modifiers(modifiers) => self.keys_changed(cx, modifiers),
        }
    }

    fn keys_changed(&mut self, cx: &mut Context<Grab>, modifiers: Modifiers) {
        let wanted = {
            let settings = self.settings.read(cx);
            settings
                .grab_enabled
                .then(|| mode_for(modifiers, settings))
                .flatten()
        };
        match (&mut self.hold, wanted) {
            (None, Some(mode)) => self.take_hold(cx, mode),
            (Some(hold), Some(mode)) if hold.mode != mode => {
                // The keys changed mid-hold: go on from where the window is now.
                let corner = self.settings.read(cx).resize_corner;
                hold.frame = hold.frame_now();
                hold.origin = self.pointer;
                hold.corner = Corner::for_setting(corner, &hold.frame, self.pointer);
                hold.mode = mode;
                cx.notify();
            }
            (Some(_), None) => self.let_go(cx),
            _ => {}
        }
    }

    fn take_hold(&mut self, cx: &mut Context<Grab>, mode: Mode) {
        let Some(pointer) = edged_macos::pointer_location() else {
            return;
        };
        let Some(window) = self.window_under(cx, pointer) else {
            return;
        };
        if window.is_fullscreen || window.is_minimized {
            return;
        }
        let frame = window.frame;
        let usable = self.usable_area_of(cx, &window).unwrap_or(frame);
        let (corner, zones) = {
            let settings = self.settings.read(cx);
            (
                Corner::for_setting(settings.resize_corner, &frame, pointer),
                settings.ring_zones,
            )
        };
        self.pointer = pointer;
        self.hold = Some(Hold {
            window,
            mode,
            origin: pointer,
            pointer,
            frame,
            corner,
            moving: false,
            usable,
            zones,
        });
        self.pointer_watch = Some(InputObserver::pointer(Grab::relay(cx)));
        cx.notify();
    }

    /// The window under the pointer, as the desktop knows it.
    fn window_under(&self, cx: &Context<Grab>, pointer: (f64, f64)) -> Option<Window> {
        let id: WindowId = edged_macos::window_at(pointer)?;
        self.desktop.read(cx).window(id).cloned()
    }

    fn moved(&mut self, cx: &mut Context<Grab>, pointer: (f64, f64)) {
        self.pointer = pointer;
        let stays_on_screen = self.settings.read(cx).stays_on_screen;
        // The screen the pointer is on bounds the window; crossing to another display
        // takes the window along.
        let screen_under_pointer = self
            .desktop
            .read(cx)
            .screen_at(pointer)
            .map(|screen| screen.visible_frame);
        let Some(hold) = &mut self.hold else {
            return;
        };
        hold.pointer = pointer;
        if !hold.moving {
            let (dx, dy) = (pointer.0 - hold.origin.0, pointer.1 - hold.origin.1);
            if dx.abs().max(dy.abs()) < DEAD_ZONE {
                return;
            }
            hold.moving = true;
            // A window held is one the user is looking at: in front, as a click would put it.
            hold.window.focus();
        }
        let mut frame = hold.frame_now();
        if stays_on_screen {
            let usable = screen_under_pointer.unwrap_or(hold.usable);
            frame = match hold.mode {
                Mode::Move => kept_on(&frame, &usable),
                Mode::Resize => kept_within(&frame, &usable),
                Mode::Arrange => frame,
            };
        }
        match hold.mode {
            Mode::Move => hold.window.set_position(frame.x, frame.y),
            Mode::Resize => {
                if hold.corner.left || hold.corner.top {
                    hold.window.set_position(frame.x, frame.y);
                }
                hold.window.set_size(frame.width, frame.height);
            }
            // The ring shows the zone; the window moves once the keys are let go.
            Mode::Arrange => {}
        }
        cx.notify();
    }

    /// Lets go of the window, putting it in the zone the pointer picked if the hold was
    /// an arranging one.
    fn let_go(&mut self, cx: &mut Context<Grab>) {
        self.pointer_watch = None;
        let Some(hold) = self.hold.take() else {
            return;
        };
        if let Some(frame) = hold.target() {
            hold.window.set_position(frame.x, frame.y);
            hold.window.set_size(frame.width, frame.height);
            hold.window.focus();
        }
        cx.notify();
    }

    /// The usable area of the screen a window is on.
    fn usable_area_of(&self, cx: &Context<Grab>, window: &Window) -> Option<Frame> {
        self.desktop
            .read(cx)
            .screen_of(window)
            .map(|screen: &Screen| screen.visible_frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame {
            x: 100.0,
            y: 200.0,
            width: 400.0,
            height: 300.0,
        }
    }

    #[test]
    fn the_chords_set_pick_the_mode_and_the_move_chord_wins_a_tie() {
        let settings = Settings::for_test();
        let move_modifiers = Modifiers {
            control: true,
            shift: true,
            ..Modifiers::default()
        };
        let resize_modifiers = Modifiers {
            option: true,
            shift: true,
            ..Modifiers::default()
        };

        assert_eq!(mode_for(move_modifiers, &settings), Some(Mode::Move));
        assert_eq!(mode_for(resize_modifiers, &settings), Some(Mode::Resize));
        assert_eq!(
            mode_for(
                Modifiers {
                    option: true,
                    ..move_modifiers
                },
                &settings
            ),
            None
        );
        assert_eq!(mode_for(Modifiers::default(), &settings), None);

        let same = Settings {
            resize_chord: settings.move_chord,
            ..settings
        };
        assert_eq!(mode_for(move_modifiers, &same), Some(Mode::Move));
    }

    #[test]
    fn the_ring_switched_off_leaves_its_chord_unanswered() {
        let settings = Settings {
            ring_enabled: false,
            ..Settings::for_test()
        };
        let arrange = Modifiers {
            control: true,
            command: true,
            ..Modifiers::default()
        };
        assert_eq!(mode_for(arrange, &settings), None);
        assert_eq!(
            mode_for(arrange, &Settings::for_test()),
            Some(Mode::Arrange)
        );
    }

    #[test]
    fn a_window_kept_on_screen_stops_at_its_edges_and_a_resize_within_them() {
        let usable = Frame {
            x: 0.0,
            y: 25.0,
            width: 1000.0,
            height: 600.0,
        };
        let past = Frame {
            x: 800.0,
            y: -50.0,
            ..frame()
        };
        let kept = kept_on(&past, &usable);
        assert_eq!((kept.x, kept.y), (600.0, 25.0));
        assert_eq!((kept.width, kept.height), (400.0, 300.0));
        let grown = Frame {
            x: 700.0,
            y: 400.0,
            width: 500.0,
            height: 400.0,
        };
        let within = kept_within(&grown, &usable);
        assert_eq!(
            (within.x + within.width, within.y + within.height),
            (1000.0, 625.0)
        );
    }

    #[test]
    fn the_corner_setting_picks_a_fixed_corner_or_the_nearest() {
        let pointer = (120.0, 220.0);
        assert_eq!(
            Corner::for_setting(ResizeCorner::BottomRight, &frame(), pointer),
            Corner::BOTTOM_RIGHT
        );
        let nearest = Corner::for_setting(ResizeCorner::Nearest, &frame(), pointer);
        assert!(nearest.left && nearest.top);
    }

    #[test]
    fn a_resize_drags_the_nearest_corner_and_keeps_the_opposite_one_still() {
        let corner = Corner::nearest(&frame(), (120.0, 220.0));
        assert!(corner.left && corner.top);
        let dragged = resized(&frame(), corner, -10.0, 20.0);
        assert_eq!((dragged.x, dragged.y), (90.0, 220.0));
        assert_eq!((dragged.width, dragged.height), (410.0, 280.0));
        assert_eq!(dragged.x + dragged.width, frame().x + frame().width);

        let corner = Corner::nearest(&frame(), (480.0, 480.0));
        assert!(!corner.left && !corner.top);
        let dragged = resized(&frame(), corner, -500.0, -500.0);
        assert_eq!((dragged.width, dragged.height), (MIN_SIZE, MIN_SIZE));
        assert_eq!((dragged.x, dragged.y), (100.0, 200.0));
    }
}
