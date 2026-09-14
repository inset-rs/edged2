//! The panel's colours, type and motion: macOS's own, in the system's light or
//! dark appearance, with the accent the user chose in System Settings and the
//! motion they allow in Accessibility.

use std::rc::Rc;
use std::time::Duration;

use edged_core::Appearance;
use inset::{App, Brightness, Color, Curve, Curves, Entity, FontWeight, TextStyle};

/// The system font: the same family the Cupertino kit names, which the
/// host's font manager resolves to San Francisco on macOS.
const FONT: &str = "CupertinoSystemText";

pub const TRANSPARENT: Color = Color::from_argb(0, 0, 0, 0);

/// Everything the panel draws with, for one appearance.
#[derive(Clone, Copy, Debug)]
pub struct Design {
    pub palette: Palette,
    pub motion: Motion,
}

impl Design {
    /// The design for the appearance the system reports now, in the brightness
    /// a window is drawn in. Reading it in a build subscribes the build to
    /// changes of the accent and the motion setting.
    pub fn of(app: &App, brightness: Brightness, appearance: &Entity<Appearance>) -> Design {
        let appearance = appearance.read(app);
        Design::new(brightness, appearance.accent, appearance.reduces_motion)
    }

    /// The design for a brightness, an accent as sRGB components, and whether
    /// the user asks for less motion.
    pub fn new(
        brightness: Brightness,
        accent: Option<(f64, f64, f64)>,
        reduces_motion: bool,
    ) -> Design {
        Design {
            palette: Palette::for_brightness(brightness, accent),
            motion: Motion::for_setting(reduces_motion),
        }
    }
}

/// Over glass a tint reads as a shade, so the fills are neutral and faint,
/// and the accent is spent on the one thing that is selected: the window
/// being looked at, and the Space the screen is showing.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub label: Color,
    pub secondary_label: Color,
    pub accent: Color,
    pub on_accent: Color,
    /// A row or heading under the pointer.
    pub hover: Color,
    pub pressed: Color,
    /// The row of the window being looked at, and that row under the pointer.
    pub focused: Color,
    pub focused_hover: Color,
    pub badge: Color,
}

impl Palette {
    pub fn for_brightness(brightness: Brightness, accent: Option<(f64, f64, f64)>) -> Palette {
        let accent = accent_color(brightness, accent);
        match brightness {
            Brightness::Light => Palette {
                label: Color::from_argb(0xD9, 0x00, 0x00, 0x00),
                secondary_label: Color::from_argb(0x80, 0x00, 0x00, 0x00),
                accent,
                on_accent: Color::from_argb(0xFF, 0xFF, 0xFF, 0xFF),
                hover: Color::from_argb(0x0E, 0x00, 0x00, 0x00),
                pressed: Color::from_argb(0x1A, 0x00, 0x00, 0x00),
                focused: with_alpha(accent, 0x29),
                focused_hover: with_alpha(accent, 0x42),
                badge: Color::from_argb(0xFF, 0xFF, 0x3B, 0x30),
            },
            Brightness::Dark => Palette {
                label: Color::from_argb(0xD9, 0xFF, 0xFF, 0xFF),
                secondary_label: Color::from_argb(0x8C, 0xFF, 0xFF, 0xFF),
                accent,
                on_accent: Color::from_argb(0xFF, 0xFF, 0xFF, 0xFF),
                hover: Color::from_argb(0x16, 0xFF, 0xFF, 0xFF),
                pressed: Color::from_argb(0x24, 0xFF, 0xFF, 0xFF),
                focused: with_alpha(accent, 0x3D),
                focused_hover: with_alpha(accent, 0x57),
                badge: Color::from_argb(0xFF, 0xFF, 0x45, 0x3A),
            },
        }
    }

    pub fn text(&self, size: f64, weight: FontWeight, color: Color) -> TextStyle {
        TextStyle::new()
            .font_family(FONT)
            .font_size(size)
            .font_weight(weight)
            .color(color)
    }
}

/// How long each change takes. Movement stops and fades shorten when the
/// user asked for reduced motion.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    /// The panel sliding out to its full width.
    pub slide_out: Duration,
    /// The panel sliding back to the strip.
    pub slide_in: Duration,
    pub hover_on: Duration,
    /// Slower than on: calmer under a pointer sweeping down the list.
    pub hover_off: Duration,
    pub pressed: Duration,
    /// The ring moving to another window.
    pub ring: Duration,
    /// The current Space's band and mark moving to another section.
    pub highlight: Duration,
    /// A row fading as its window is minimized, or back.
    pub dim: Duration,
    reduced: bool,
}

impl Motion {
    pub fn for_setting(reduces_motion: bool) -> Motion {
        if reduces_motion {
            Motion::reduced()
        } else {
            Motion::full()
        }
    }

    fn full() -> Motion {
        Motion {
            slide_out: Duration::from_millis(100),
            slide_in: Duration::from_millis(100),
            hover_on: Duration::from_millis(80),
            hover_off: Duration::from_millis(140),
            pressed: Duration::from_millis(50),
            ring: Duration::from_millis(180),
            highlight: Duration::from_millis(220),
            dim: Duration::from_millis(180),
            reduced: false,
        }
    }

    /// Nothing moves; what changes cross-fades.
    fn reduced() -> Motion {
        let fade = Duration::from_millis(120);
        Motion {
            slide_out: Duration::ZERO,
            slide_in: Duration::ZERO,
            hover_on: fade,
            hover_off: fade,
            pressed: fade,
            ring: fade,
            highlight: fade,
            dim: fade,
            reduced: true,
        }
    }

    /// The curve of something arriving: most of the way at once, then settling.
    pub fn arriving(&self) -> Rc<dyn Curve> {
        if self.reduced {
            Curves::linear()
        } else {
            Curves::ease_out_cubic()
        }
    }

    /// The curve of something moving from one place to another.
    pub fn moving(&self) -> Rc<dyn Curve> {
        if self.reduced {
            Curves::linear()
        } else {
            Curves::ease_in_out_cubic()
        }
    }
}

/// The accent from System Settings, else the blue macOS ships with.
fn accent_color(brightness: Brightness, accent: Option<(f64, f64, f64)>) -> Color {
    match accent {
        Some((r, g, b)) => Color::from_argb(0xFF, channel(r), channel(g), channel(b)),
        None => match brightness {
            Brightness::Light => Color::from_argb(0xFF, 0x00, 0x7A, 0xFF),
            Brightness::Dark => Color::from_argb(0xFF, 0x0A, 0x84, 0xFF),
        },
    }
}

fn channel(component: f64) -> i32 {
    (component.clamp(0.0, 1.0) * 255.0).round() as i32
}

fn with_alpha(color: Color, alpha: i32) -> Color {
    Color::from_argb(alpha, channel(color.r), channel(color.g), channel(color.b))
}
