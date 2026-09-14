//! Where the window would go: a translucent frame over the zone the ring has picked,
//! shown while the keys are still held, so what letting go will do is in view first.

use edged_core::{Core, Mode};
use inset::{
    App, Border, BorderRadius, BorderSide, BorderStyle, BoxDecoration, BuildContext, DecoratedBox,
    IntoWidget, MediaQuery, SizedBox, StatelessWidget, WidgetRef,
};

use crate::theme::Design;

/// The size the target's window is made with, before the first zone.
pub const INITIAL_SIZE: [f64; 2] = [400.0, 300.0];
const STROKE: f64 = 2.0;
const RADIUS: f64 = 10.0;
/// How much of the accent shows through the frame's fill.
const FILL_ALPHA: i32 = 0x30;

#[derive(Debug)]
pub struct TargetContent {
    pub core: Core,
}

impl StatelessWidget for TargetContent {
    fn build(&self, app: &mut App, context: BuildContext) -> WidgetRef {
        let brightness = MediaQuery::platform_brightness_of(app, context);
        let design = Design::of(app, brightness, &self.core.appearance);
        let showing = {
            let grab = self.core.grab.read(app);
            grab.hold
                .as_ref()
                .is_some_and(|hold| hold.mode == Mode::Arrange && hold.target().is_some())
        };
        if !showing {
            return SizedBox::new().into_widget();
        }
        let accent = design.palette.accent;
        DecoratedBox::new(
            BoxDecoration::new()
                .color(accent.with_alpha(FILL_ALPHA))
                .border(Border::all(
                    accent,
                    STROKE,
                    BorderStyle::Solid,
                    BorderSide::STROKE_ALIGN_INSIDE,
                ))
                .border_radius(BorderRadius::circular(RADIUS)),
        )
        .child(SizedBox::expand())
        .into_widget()
    }
}
