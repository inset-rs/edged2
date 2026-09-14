//! Keeps the strip at the screen's edge reachable: a window that covers it is
//! narrowed until it does not.

use std::time::Duration;

use edged_macos::{Frame, Screen, Side};
use inset_foundation::{App, Context, Entity, Listener, Timer};

use crate::desktop::Desktop;
use crate::settings::Settings;

/// How wide the collapsed panel is, and so the strip it must be able to sit in.
pub const STRIP_WIDTH: f64 = 28.0;
/// A window narrower than this is left alone: shrinking it would help no one.
const MIN_WIDTH: f64 = 100.0;
/// How often windows covering a strip are pushed off it.
const EVERY: Duration = Duration::from_secs(3);

/// Narrows, every few seconds, whatever covers a strip.
pub struct Clearing {
    desktop: Entity<Desktop>,
    settings: Entity<Settings>,
    timer: Option<Timer>,
}

impl Clearing {
    pub fn start(
        cx: &mut Context<Clearing>,
        desktop: Entity<Desktop>,
        settings: Entity<Settings>,
    ) -> Clearing {
        let mut clearing = Clearing {
            desktop,
            settings,
            timer: None,
        };
        clearing.schedule(cx);
        clearing
    }

    fn schedule(&mut self, cx: &mut Context<Clearing>) {
        let this = cx.weak_entity();
        self.timer = Some(Timer::new(
            cx,
            EVERY,
            Listener::new(move |app: &mut App| {
                if let Some(clearing) = this.upgrade() {
                    clearing.update(app, |clearing, cx| {
                        let side = clearing.settings.read(cx).panel_side;
                        keep_clear(clearing.desktop.read(cx), side);
                        clearing.schedule(cx);
                    });
                }
            }),
        ));
    }
}

/// The strip at one edge of a screen the panel occupies when collapsed.
fn strip_of(screen: &Screen, side: Side) -> Frame {
    let x = match side {
        Side::Right => screen.frame.right() - STRIP_WIDTH,
        Side::Left => screen.frame.x,
    };
    Frame {
        x,
        y: screen.frame.y,
        width: STRIP_WIDTH,
        height: screen.frame.height,
    }
}

/// The frame that keeps a window clear of the strip: only when the window
/// covers it, only by narrowing it from the strip's side, and only if what
/// remains is usable.
fn cleared(window: &Frame, strip: &Frame, side: Side) -> Option<Frame> {
    if !window.intersects(strip) {
        return None;
    }
    let (x, width) = match side {
        Side::Right => (window.x, strip.x - window.x),
        Side::Left => (strip.right(), window.right() - strip.right()),
    };
    (width > MIN_WIDTH && width < window.width).then_some(Frame {
        x,
        width,
        ..*window
    })
}

/// Narrows every window on a screen's current Space that covers its strip.
fn keep_clear(desktop: &Desktop, side: Side) {
    for screen in &desktop.screens {
        let Some(space) = desktop.showing_on(screen) else {
            continue;
        };
        let strip = strip_of(screen, side);
        for (_, window) in desktop.windows_on(space.id) {
            if window.is_minimized || window.is_fullscreen {
                continue;
            }
            if desktop.screen_of(window).map(|s| s.display_id) != Some(screen.display_id) {
                continue;
            }
            if let Some(frame) = cleared(&window.frame, &strip, side) {
                if frame.x != window.frame.x {
                    window.set_position(frame.x, frame.y);
                }
                window.set_size(frame.width, frame.height);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f64, y: f64, width: f64, height: f64) -> Frame {
        Frame {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_window_over_the_strip_is_narrowed_to_its_edge() {
        let strip = frame(1412.0, 0.0, 28.0, 900.0);
        assert_eq!(
            cleared(&frame(400.0, 100.0, 1040.0, 600.0), &strip, Side::Right),
            Some(frame(400.0, 100.0, 1012.0, 600.0))
        );
        let strip = frame(0.0, 0.0, 28.0, 900.0);
        assert_eq!(
            cleared(&frame(10.0, 100.0, 1040.0, 600.0), &strip, Side::Left),
            Some(frame(28.0, 100.0, 1022.0, 600.0)),
            "on the left the window keeps its right edge"
        );
    }

    #[test]
    fn windows_clear_of_the_strip_or_too_narrow_are_left_alone() {
        let strip = frame(1412.0, 0.0, 28.0, 900.0);
        assert_eq!(
            cleared(&frame(100.0, 100.0, 800.0, 600.0), &strip, Side::Right),
            None
        );
        assert_eq!(
            cleared(&frame(1400.0, 100.0, 300.0, 600.0), &strip, Side::Right),
            None
        );
    }
}
