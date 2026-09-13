//! What the system's appearance settings say, kept current.

use edged_macos::AppearanceObserver;
use inset_foundation::Context;

/// The accent colour and the motion setting, refreshed when System Settings
/// announces a change. The light or dark appearance itself reaches the
/// interface through its windows, so it is not here.
pub struct Appearance {
    /// The accent as sRGB components in 0..=1, or `None` when macOS cannot
    /// express it that way.
    pub accent: Option<(f64, f64, f64)>,
    /// Whether the user asked for less motion.
    pub reduces_motion: bool,
    _observer: AppearanceObserver,
}

impl Appearance {
    pub fn start(cx: &mut Context<Appearance>) -> Appearance {
        let this = cx.weak_entity();
        let async_app = cx.to_async();
        let observer = AppearanceObserver::new(move || {
            let this = this.clone();
            async_app.post(move |app| {
                if let Some(appearance) = this.upgrade() {
                    appearance.update(app, |appearance, cx| appearance.refresh(cx));
                }
            });
        });
        Appearance {
            accent: edged_macos::accent_color(),
            reduces_motion: edged_macos::reduces_motion(),
            _observer: observer,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Appearance>) {
        let accent = edged_macos::accent_color();
        let reduces_motion = edged_macos::reduces_motion();
        if accent == self.accent && reduces_motion == self.reduces_motion {
            return;
        }
        self.accent = accent;
        self.reduces_motion = reduces_motion;
        cx.notify();
    }
}
