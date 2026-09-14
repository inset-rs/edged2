//! One line of the panel: a title, an icon at the screen's edge, and what
//! choosing it does.
//!
//! The icon sits in the strip's cell at the trailing end of the row, so it
//! stays under the pointer while the panel slides out and the title unfurls
//! beside it. The hover pill follows the window: as wide as the icon's box
//! while the panel is in, the whole row once it is out.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use edged_core::{AppEntry, Desktop, Previews, Rest};
use edged_macos::Window;
use inset::MouseRegion;
use inset::{
    AnimatedContainer, AnimatedOpacity, App, Border, BorderRadius, BorderSide, BorderStyle,
    BoxDecoration, BuildContext, Center, Clip, Color, CrossAxisAlignment, Curve, DecoratedBox,
    EdgeInsetsGeometry, Entity, Expanded, FontWeight, GestureDetector, Image, IntoWidget, KeyRef,
    Listener, Padding, Positioned, RepaintBoundary, Row, SizedBox, Stack, StackFit,
    StatelessWidget, Text, TextAlign, TextOverflow, ValueKey, WidgetRef,
};
use inset_winui::{CommonState, CommonStates, ControlStates};

use crate::menus;
use crate::panel::{PANEL_WIDTH, PanelGeometry, STRIP_WIDTH};
use crate::theme::{Design, TRANSPARENT};

/// The height every row takes: the icon's box and its gutters.
const ROW_HEIGHT: f64 = 28.0;
/// The icon's box, centred in the strip with two points to spare each side.
const ICON_BOX: f64 = 24.0;
const ICON_RADIUS: f64 = 6.0;
/// The ring around the icon of the window being looked at, drawn over the
/// icon's own transparent margin.
const RING_WIDTH: f64 = 1.5;
/// The pill's corners: the icon's, so that in the strip the pill is the icon's
/// own outline, and against the panel's corner the two curves are concentric.
const PILL_RADIUS: f64 = ICON_RADIUS;
/// The pill's inset from every edge: the icon's gutter, so the pill sits as
/// far from the panel's edges as the icon does from the strip's.
const PILL_INSET: f64 = (STRIP_WIDTH - ICON_BOX) / 2.0;
/// The pill's inset from the panel's far edge when the panel is out.
const PILL_LEADING: f64 = PILL_INSET;
/// The pill's inset from the screen's edge.
const PILL_TRAILING: f64 = PILL_INSET;
/// Room between the title and the icon.
const TITLE_GAP: f64 = 6.0;
/// Room between the pill's far edge and the title.
const TITLE_INSET: f64 = 6.0;
const LABEL_SIZE: f64 = 13.0;
const BADGE_HEIGHT: f64 = 14.0;
const BADGE_DOT: f64 = 8.0;
const BADGE_SIZE: f64 = 10.0;
const BADGE_PAD: f64 = 4.0;
/// How faint the icon of a minimized window or a hidden application is drawn.
const DIMMED: f64 = 0.45;

/// Opens a menu for this row.
pub type SecondaryAction = Rc<dyn Fn(&mut App)>;
/// Hears whether the pointer is on the row.
pub type HoverAction = Rc<dyn Fn(&mut App, bool)>;

/// A row of the panel.
pub struct PanelRow {
    /// The application's icon as PNG bytes, absent while it is being read.
    pub icon: Option<Arc<[u8]>>,
    pub label: String,
    /// The Dock badge to draw over the icon.
    pub badge: Option<String>,
    /// Whether this row is the window the user is looking at.
    pub is_focused: bool,
    /// Whether the window is minimized or its application hidden.
    pub is_dimmed: bool,
    pub click: Listener,
    /// What a right-click opens, when there is anything to open.
    pub secondary: Option<SecondaryAction>,
    /// Told when the pointer comes to the row and when it leaves.
    pub hover: Option<HoverAction>,
    pub key: Option<KeyRef>,
    pub design: Design,
}

impl PanelRow {
    pub fn new(label: impl Into<String>, click: Listener, design: Design) -> PanelRow {
        PanelRow {
            icon: None,
            label: label.into(),
            badge: None,
            is_focused: false,
            is_dimmed: false,
            click,
            secondary: None,
            hover: None,
            key: None,
            design,
        }
    }

    pub fn icon(mut self, icon: Option<Arc<[u8]>>) -> PanelRow {
        self.icon = icon;
        self
    }

    pub fn badge(mut self, badge: Option<String>) -> PanelRow {
        self.badge = badge;
        self
    }

    pub fn focused(mut self, is_focused: bool) -> PanelRow {
        self.is_focused = is_focused;
        self
    }

    pub fn dimmed(mut self, is_dimmed: bool) -> PanelRow {
        self.is_dimmed = is_dimmed;
        self
    }

    pub fn secondary(mut self, secondary: SecondaryAction) -> PanelRow {
        self.secondary = Some(secondary);
        self
    }

    pub fn hover(mut self, hover: HoverAction) -> PanelRow {
        self.hover = Some(hover);
        self
    }

    pub fn key(mut self, key: KeyRef) -> PanelRow {
        self.key = Some(key);
        self
    }
}

impl std::fmt::Debug for PanelRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanelRow")
            .field("label", &self.label)
            .field("is_focused", &self.is_focused)
            .finish_non_exhaustive()
    }
}

impl StatelessWidget for PanelRow {
    fn key(&self) -> Option<&KeyRef> {
        self.key.as_ref()
    }

    fn build(&self, _app: &mut App, _context: BuildContext) -> WidgetRef {
        let icon = self.icon.clone();
        let badge = self.badge.clone();
        let label = self.label.clone();
        let focused = self.is_focused;
        let dimmed = self.is_dimmed;
        let design = self.design;
        let states = CommonStates::new(self.click.clone(), move |_app, _context, states| {
            template(
                &Look {
                    icon: icon.clone(),
                    badge: badge.clone(),
                    label: &label,
                    focused,
                    dimmed,
                },
                &design,
                states,
            )
        })
        .into_widget();
        let line = states;
        let line = match self.hover.clone() {
            Some(hover) => {
                let (entered, left) = (Rc::clone(&hover), hover);
                MouseRegion::new()
                    .on_enter(Rc::new(move |app: &mut App, _event| entered(app, true)))
                    .on_exit(Rc::new(move |app: &mut App, _event| left(app, false)))
                    .child(line)
                    .into_widget()
            }
            None => line,
        };
        let line = match self.secondary.clone() {
            Some(secondary) => GestureDetector::new()
                .child(line)
                .on_secondary_tap_down(Rc::new(move |app, _details| secondary(app)))
                .into_widget(),
            None => line,
        };
        // Its own layer: a change to this line, a hover fill or a ring, repaints this
        // line and not the panel.
        RepaintBoundary::new().child(line).into_widget()
    }
}

struct Look<'a> {
    icon: Option<Arc<[u8]>>,
    badge: Option<String>,
    label: &'a str,
    focused: bool,
    dimmed: bool,
}

/// The row for one set of states: the pill behind, the title and icon over it.
fn template(look: &Look, design: &Design, states: ControlStates) -> WidgetRef {
    let palette = &design.palette;
    let motion = &design.motion;
    let (fill, duration) = match (states.common, look.focused) {
        (CommonState::Pressed, true) => (palette.focused_hover, motion.pressed),
        (CommonState::Pressed, false) => (palette.pressed, motion.pressed),
        (CommonState::PointerOver, true) => (palette.focused_hover, motion.hover_on),
        (CommonState::PointerOver, false) => (palette.hover, motion.hover_on),
        (_, true) => (palette.focused, motion.hover_off),
        (_, false) => (TRANSPARENT, motion.hover_off),
    };
    let label_color = if look.dimmed {
        palette.secondary_label
    } else {
        palette.label
    };
    let ring = if look.focused {
        palette.accent
    } else {
        TRANSPARENT
    };
    Tile {
        text: title(look.label, label_color, design),
        cell: glyph(look, ring, design),
        fill,
        duration,
        curve: motion.arriving(),
        height: ROW_HEIGHT,
    }
    .build()
}

/// What every line of the panel is made of: a pill that follows the window,
/// text hugging the strip, and the line's cell in the strip at the screen's
/// edge. Rows and headings differ only in what they put in the two places.
pub struct Tile {
    pub text: WidgetRef,
    pub cell: WidgetRef,
    /// The pill's fill for the line's state now.
    pub fill: Color,
    /// How long the fill takes to change.
    pub duration: Duration,
    pub curve: Rc<dyn Curve>,
    pub height: f64,
}

impl Tile {
    /// The line. Only its pill reads the width the window shows, so a step of the
    /// slide rebuilds the pills and nothing else of the lines.
    pub fn build(self) -> WidgetRef {
        let pill = Pill {
            fill: self.fill,
            duration: self.duration,
            curve: self.curve,
        };
        let content = Row::new()
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .children(vec![
                SizedBox::new()
                    .width(PILL_LEADING + TITLE_INSET)
                    .into_widget(),
                Expanded::new(self.text).into_widget(),
                SizedBox::new().width(TITLE_GAP).into_widget(),
                strip_cell(self.cell),
            ]);
        SizedBox::new()
            .height(self.height)
            .child(
                Stack::new()
                    .fit(StackFit::Expand)
                    .clip_behavior(Clip::None)
                    .children(vec![pill.into_widget(), content.into_widget()]),
            )
            .into_widget()
    }
}

/// The pill behind a line: as wide as the icon's box while the panel is in, the
/// whole line once it is out, and every width between as the window slides.
struct Pill {
    fill: Color,
    duration: Duration,
    curve: Rc<dyn Curve>,
}

impl std::fmt::Debug for Pill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pill")
            .field("fill", &self.fill)
            .finish_non_exhaustive()
    }
}

impl StatelessWidget for Pill {
    fn build(&self, app: &mut App, context: BuildContext) -> WidgetRef {
        let visible_width = PanelGeometry::of(app, context);
        Positioned::new(
            AnimatedContainer::new(self.duration)
                .curve(Rc::clone(&self.curve))
                .decoration(
                    BoxDecoration::new()
                        .color(self.fill)
                        .border_radius(BorderRadius::circular(PILL_RADIUS)),
                ),
        )
        .left(pill_leading(visible_width))
        .top(PILL_INSET)
        .right(PILL_TRAILING)
        .bottom(PILL_INSET)
        .into_widget()
    }
}

/// The pill's fill for a line with nothing selected about it.
pub fn plain_fill(states: ControlStates, design: &Design) -> (Color, Duration) {
    let palette = &design.palette;
    let motion = &design.motion;
    match states.common {
        CommonState::Pressed => (palette.pressed, motion.pressed),
        CommonState::PointerOver => (palette.hover, motion.hover_on),
        _ => (TRANSPARENT, motion.hover_off),
    }
}

/// Where the pill starts, for the width of the panel the window shows: the
/// icon's box while the panel is in, the row less its inset once it is out,
/// and every width between as the window slides. With the inset the icon's
/// gutter, the pill in the strip is the icon's box exactly.
fn pill_leading(visible_width: f64) -> f64 {
    (PANEL_WIDTH + PILL_TRAILING - visible_width).max(PILL_LEADING)
}

/// The strip's cell: whatever a row ends with sits centred in the width the
/// collapsed panel shows, at the screen's edge.
pub fn strip_cell<K>(child: impl IntoWidget<K>) -> WidgetRef {
    SizedBox::new()
        .width(STRIP_WIDTH)
        .child(Center::new().child(child))
        .into_widget()
}

/// The application's icon in its box, ringed when its window is the one
/// being looked at, faded when it is put away, with the Dock badge over its
/// outer corner.
fn glyph(look: &Look, ring: Color, design: &Design) -> WidgetRef {
    let motion = &design.motion;
    let image = SizedBox::new().width(ICON_BOX).height(ICON_BOX);
    let image = match look.icon.clone() {
        Some(bytes) => image.child(Image::memory(bytes)),
        None => image,
    };
    let ringed = AnimatedContainer::new(motion.ring)
        .curve(motion.arriving())
        .width(ICON_BOX)
        .height(ICON_BOX)
        .foreground_decoration(
            BoxDecoration::new()
                .border(Border::all(
                    ring,
                    RING_WIDTH,
                    BorderStyle::Solid,
                    BorderSide::STROKE_ALIGN_INSIDE,
                ))
                .border_radius(BorderRadius::circular(ICON_RADIUS)),
        )
        .child(image);
    let opacity = if look.dimmed { DIMMED } else { 1.0 };
    let faded = AnimatedOpacity::new(opacity, motion.dim)
        .curve(motion.arriving())
        .child(ringed);
    let Some(badge) = &look.badge else {
        return faded.into_widget();
    };
    Stack::new()
        .clip_behavior(Clip::None)
        .children(vec![
            faded.into_widget(),
            Positioned::new(dock_badge(badge, design))
                .top(-2.0)
                .right(-1.0)
                .into_widget(),
        ])
        .into_widget()
}

/// The Dock's red badge: a count, or a dot when the label is not one.
fn dock_badge(label: &str, design: &Design) -> WidgetRef {
    let palette = &design.palette;
    let Some(count) = badge_count(label) else {
        return DecoratedBox::new(
            BoxDecoration::new()
                .color(palette.badge)
                .border_radius(BorderRadius::circular(BADGE_DOT / 2.0)),
        )
        .child(SizedBox::new().width(BADGE_DOT).height(BADGE_DOT))
        .into_widget();
    };
    DecoratedBox::new(
        BoxDecoration::new()
            .color(palette.badge)
            .border_radius(BorderRadius::circular(BADGE_HEIGHT / 2.0)),
    )
    .child(SizedBox::new().height(BADGE_HEIGHT).child(
        Padding::new(EdgeInsetsGeometry::symmetric(0.0, BADGE_PAD)).child(Center::new().child(
            Text::new(count).style(palette.text(BADGE_SIZE, FontWeight::W700, palette.on_accent)),
        )),
    ))
    .into_widget()
}

/// A numeric badge as the Dock shows it, capped where a wider count would
/// leave the strip.
fn badge_count(label: &str) -> Option<String> {
    let count: u64 = label.trim().parse().ok()?;
    Some(if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    })
}

/// One line, hugging the icon, cut with an ellipsis at its far end: a window
/// title is often far wider than the panel.
pub fn title(label: &str, color: Color, design: &Design) -> WidgetRef {
    Text::new(label.to_owned())
        .style(design.palette.text(LABEL_SIZE, FontWeight::W400, color))
        .text_align(TextAlign::Right)
        .max_lines(1)
        .overflow(TextOverflow::Ellipsis)
        .into_widget()
}

/// A row for an application with no windows: choosing it activates the application.
pub fn application_row(desktop: &Entity<Desktop>, entry: &AppEntry, design: Design) -> WidgetRef {
    let activate = {
        let (desktop, application) = (desktop.clone(), entry.application.clone());
        Listener::new(move |app: &mut App| desktop.read(app).activate(&application))
    };
    let (menu_desktop, menu_target) = (desktop.clone(), entry.application.clone());
    PanelRow::new(entry.application.name.clone(), activate, design)
        .icon(entry.icon.clone())
        .badge(entry.badge.clone())
        .dimmed(entry.application.is_hidden)
        .secondary(Rc::new(move |app| {
            menus::application_menu(app, &menu_desktop, &menu_target)
        }))
        .key(Rc::new(ValueKey::new(entry.application.pid)) as KeyRef)
        .into_widget()
}

/// A row for one window: choosing it brings the window forward, and the
/// pointer resting on it shows a picture of the window where it would be,
/// except for the window in front on the Space the screen is showing, which
/// is in plain sight.
pub fn window_row(
    desktop: &Entity<Desktop>,
    previews: &Entity<Previews>,
    entry: &AppEntry,
    window: &Window,
    on_current_space: bool,
    scale: f64,
    design: Design,
) -> WidgetRef {
    let focused = entry.application.is_active && entry.focused == Some(window.id);
    let in_sight = focused && on_current_space && !window.is_minimized;
    let hover: HoverAction = {
        let (previews, window) = (previews.clone(), window.clone());
        Rc::new(move |app: &mut App, on: bool| {
            let rest = (on && !in_sight).then(|| Rest {
                window: window.clone(),
                scale,
            });
            previews.update(app, |previews, cx| previews.rest_on(cx, rest));
        })
    };
    let label = if window.title.is_empty() {
        entry.application.name.clone()
    } else {
        window.title.clone()
    };
    let focus = {
        let (desktop, previews, window) = (desktop.clone(), previews.clone(), window.clone());
        Listener::new(move |app: &mut App| {
            // The window comes forward; its picture would only sit on top of it.
            previews.update(app, |previews, cx| previews.dismiss(cx));
            desktop.read(app).focus(&window)
        })
    };
    let menu_window = window.clone();
    let menu_application = entry.application.clone();
    let menu_desktop = desktop.clone();
    PanelRow::new(label, focus, design)
        .icon(entry.icon.clone())
        .badge(entry.badge.clone())
        .focused(focused)
        .dimmed(window.is_minimized || entry.application.is_hidden)
        .secondary(Rc::new(move |app: &mut App| {
            menus::window_menu(app, &menu_desktop, &menu_window, &menu_application)
        }))
        .hover(hover)
        .key(Rc::new(ValueKey::new(window.id)) as KeyRef)
        .into_widget()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_is_the_icon_box_while_the_panel_is_in_and_the_row_once_it_is_out() {
        assert_eq!(
            pill_leading(STRIP_WIDTH),
            PANEL_WIDTH - PILL_TRAILING - ICON_BOX
        );
        assert_eq!(pill_leading(PANEL_WIDTH), PILL_LEADING);
        assert_eq!(pill_leading(100.0), 152.0);
        assert_eq!(PILL_INSET, 2.0);
    }

    #[test]
    fn a_badge_is_a_count_or_a_dot() {
        assert_eq!(badge_count("7").as_deref(), Some("7"));
        assert_eq!(badge_count("120").as_deref(), Some("99+"));
        assert_eq!(badge_count("new"), None);
    }
}
