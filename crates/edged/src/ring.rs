//! The ring at the pointer while the arrange chord is held: the zones the window under
//! the pointer can be put in, laid out as a compass around where the keys went down,
//! and the one the pointer's direction picks.
//!
//! A disc of eight sectors, one per direction, each showing its zone as a small screen
//! with the zone's share filled in; the sector the pointer points at is lit in the
//! accent colour.

use std::any::Any;
use std::f32::consts::PI;

use edged_core::{Core, Direction, Mode, RingZones, Zone};
use inset::ui::valo::{Point, Rect};
use inset::ui::{Canvas, FillRule, Paint, PaintStyle, PathBuilder, Stroke};
use inset::{
    App, BuildContext, Color, CustomPaint, IntoWidget, MediaQuery, Size, SizedBox, StatelessWidget,
    WidgetRef,
};
use inset_rendering::CustomPainter;

use crate::theme::Design;

/// The ring's window, square and centred on where the keys went down.
pub const WINDOW_SIZE: f64 = 220.0;
/// The disc's radius.
const DISC_RADIUS: f32 = 100.0;
/// The lit sector reaches from the hub's clearance to just inside the rim.
const SECTOR_INNER: f32 = 18.0;
const SECTOR_OUTER: f32 = 96.0;
/// The dot at the centre: where the keys went down.
const HUB_RADIUS: f32 = 5.0;
/// How far from the centre a sector's icon sits.
const ICON_RADIUS: f32 = 64.0;
/// The icon: a screen, and the zone's share of it.
const ICON_WIDTH: f32 = 20.0;
const ICON_HEIGHT: f32 = 14.0;
const ICON_CORNER: f32 = 3.0;
const ICON_STROKE: f32 = 1.5;
/// How far the zone's share sits inside the screen's outline.
const ICON_INSET: f32 = 3.0;
/// Segments a sector's arc is drawn with.
const ARC_STEPS: usize = 12;

/// The disc: dark enough for white icons over any window.
const DISC_COLOR: Color = Color::from_argb(0xD9, 0x1C, 0x1C, 0x1E);
/// The hairline around the disc and the lines between sectors.
const RIM_COLOR: Color = Color::from_argb(0x40, 0xFF, 0xFF, 0xFF);
const SEPARATOR_COLOR: Color = Color::from_argb(0x1A, 0xFF, 0xFF, 0xFF);
const HUB_COLOR: Color = Color::from_argb(0x80, 0xFF, 0xFF, 0xFF);
const ICON_COLOR: Color = Color::from_argb(0xE6, 0xFF, 0xFF, 0xFF);

#[derive(Debug)]
pub struct RingContent {
    pub core: Core,
}

impl StatelessWidget for RingContent {
    fn build(&self, app: &mut App, context: BuildContext) -> WidgetRef {
        let brightness = MediaQuery::platform_brightness_of(app, context);
        let design = Design::of(app, brightness, &self.core.appearance);
        let (chosen, zones): (Option<Direction>, RingZones) = {
            let grab = self.core.grab.read(app);
            match &grab.hold {
                Some(hold) if hold.mode == Mode::Arrange => {
                    let offset = (
                        hold.pointer.0 - hold.origin.0,
                        hold.pointer.1 - hold.origin.1,
                    );
                    let chosen = Direction::of(offset)
                        .filter(|direction| hold.zones[direction.index()].is_some());
                    (chosen, hold.zones)
                }
                _ => return SizedBox::new().into_widget(),
            }
        };
        CustomPaint::new()
            .painter(RingPainter {
                zones,
                chosen,
                accent: design.palette.accent,
                on_accent: design.palette.on_accent,
            })
            .size(Size::new(WINDOW_SIZE, WINDOW_SIZE))
            .into_widget()
    }
}

/// Paints the disc, the lit sector, and each direction's zone icon.
pub(super) struct RingPainter {
    pub zones: RingZones,
    pub chosen: Option<Direction>,
    pub accent: Color,
    pub on_accent: Color,
}

impl CustomPainter for RingPainter {
    fn paint(&self, _app: &mut App, canvas: &mut Canvas, size: Size) {
        let centre = Point::new(size.width() as f32 / 2.0, size.height() as f32 / 2.0);
        canvas.draw_circle(centre, DISC_RADIUS, &fill(DISC_COLOR));
        if let Some(direction) = self.chosen {
            canvas.draw_path(
                &sector(centre, direction),
                FillRule::NonZero,
                &fill(self.accent),
            );
        }
        canvas.draw_path(
            &separators(centre),
            FillRule::NonZero,
            &stroke(SEPARATOR_COLOR, 1.0),
        );
        canvas.draw_circle(centre, DISC_RADIUS - 0.5, &stroke(RIM_COLOR, 1.0));
        canvas.draw_circle(centre, HUB_RADIUS, &fill(HUB_COLOR));
        for direction in Direction::CLOCKWISE {
            let Some(zone) = self.zones[direction.index()] else {
                continue;
            };
            let colour = if self.chosen == Some(direction) {
                self.on_accent
            } else {
                ICON_COLOR
            };
            draw_icon(
                canvas,
                zone,
                polar(centre, ICON_RADIUS, angle_of(direction)),
                colour,
            );
        }
    }

    fn should_repaint(&self, _app: &App, old_painter: &dyn CustomPainter) -> bool {
        old_painter
            .as_any()
            .downcast_ref::<RingPainter>()
            .is_none_or(|old| {
                old.zones != self.zones
                    || old.chosen != self.chosen
                    || old.accent != self.accent
                    || old.on_accent != self.on_accent
            })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The angle a direction lies at, clockwise from up on a screen whose y grows downwards.
fn angle_of(direction: Direction) -> f32 {
    -PI / 2.0 + direction.index() as f32 * (PI / 4.0)
}

fn polar(centre: Point, radius: f32, angle: f32) -> Point {
    Point::new(
        centre.x + radius * angle.cos(),
        centre.y + radius * angle.sin(),
    )
}

/// The wedge of a direction's sector, an eighth of the disc centred on the direction.
fn sector(centre: Point, direction: Direction) -> std::sync::Arc<inset::ui::Path> {
    let from = angle_of(direction) - PI / 8.0;
    let step = (PI / 4.0) / ARC_STEPS as f32;
    let mut path = PathBuilder::new();
    path.move_to(polar(centre, SECTOR_INNER, from));
    for i in 0..=ARC_STEPS {
        path.line_to(polar(centre, SECTOR_OUTER, from + step * i as f32));
    }
    for i in (0..=ARC_STEPS).rev() {
        path.line_to(polar(centre, SECTOR_INNER, from + step * i as f32));
    }
    path.close();
    path.build()
}

/// The lines between the sectors, from the hub's clearance to the rim.
fn separators(centre: Point) -> std::sync::Arc<inset::ui::Path> {
    let mut path = PathBuilder::new();
    for direction in Direction::CLOCKWISE {
        let angle = angle_of(direction) + PI / 8.0;
        path.move_to(polar(centre, SECTOR_INNER, angle));
        path.line_to(polar(centre, SECTOR_OUTER, angle));
    }
    path.build()
}

/// A zone's icon at `at`: a screen's outline, with the zone's share of it filled in.
fn draw_icon(canvas: &mut Canvas, zone: Zone, at: Point, colour: Color) {
    let screen = Rect::new(
        at.x - ICON_WIDTH / 2.0,
        at.y - ICON_HEIGHT / 2.0,
        ICON_WIDTH,
        ICON_HEIGHT,
    );
    canvas.draw_rrect(screen, ICON_CORNER, &stroke(colour, ICON_STROKE));
    let inner = Rect::new(
        screen.x + ICON_INSET,
        screen.y + ICON_INSET,
        screen.width - 2.0 * ICON_INSET,
        screen.height - 2.0 * ICON_INSET,
    );
    canvas.draw_rrect(share_of(zone, &inner), 1.0, &fill(colour));
}

/// The part of `screen` a zone covers.
fn share_of(zone: Zone, screen: &Rect) -> Rect {
    let (x, y, w, h) = (screen.x, screen.y, screen.width, screen.height);
    match zone {
        Zone::Fill => *screen,
        Zone::Centred => Rect::new(x + w * 0.15, y + h * 0.15, w * 0.7, h * 0.7),
        Zone::LeftHalf => Rect::new(x, y, w / 2.0, h),
        Zone::RightHalf => Rect::new(x + w / 2.0, y, w / 2.0, h),
        Zone::TopHalf => Rect::new(x, y, w, h / 2.0),
        Zone::BottomHalf => Rect::new(x, y + h / 2.0, w, h / 2.0),
        Zone::TopLeft => Rect::new(x, y, w / 2.0, h / 2.0),
        Zone::TopRight => Rect::new(x + w / 2.0, y, w / 2.0, h / 2.0),
        Zone::BottomLeft => Rect::new(x, y + h / 2.0, w / 2.0, h / 2.0),
        Zone::BottomRight => Rect::new(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0),
    }
}

fn fill(colour: Color) -> Paint {
    Paint {
        color: colour.into(),
        ..Paint::default()
    }
}

fn stroke(colour: Color, width: f32) -> Paint {
    Paint {
        color: colour.into(),
        style: PaintStyle::Stroke(Stroke::new(width)),
        ..Paint::default()
    }
}
