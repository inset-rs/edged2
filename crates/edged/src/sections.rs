//! The panel's headings: Edged's own menu at the top, and a Space's mark and
//! name at the head of its windows.
//!
//! A heading is a line like a row, with a mark in the strip's cell where a
//! row has its icon, so the panel reads as a column of icons with a numbered
//! mark at the head of each group, the current one in the accent; slid out,
//! the whole line is the target, as a row is.

use edged_core::Desktop;
use edged_macos::{Side, Space, SpaceKind};
use inset::{
    AnimatedContainer, App, Border, BorderRadius, BorderSide, BorderStyle, BoxDecoration, Center,
    Column, CrossAxisAlignment, EdgeInsetsGeometry, Entity, FontWeight, IntoWidget, Listener,
    MainAxisSize, Padding, SizedBox, Text, WidgetRef,
};
use inset_winui::{CommonStates, FluentSymbol, FontIcon};

use crate::rows::{self, Tile};
use crate::theme::{Design, TRANSPARENT};

const MARKER_SIDE: f64 = 18.0;
const MARKER_RADIUS: f64 = 5.0;
const MARKER_SIZE: f64 = 11.0;
/// A heading's line is as high as a row's, so its pill in the strip is the
/// same square an icon's is, with the smaller mark centred in it.
const HEADING_HEIGHT: f64 = 28.0;
const MORE_SIZE: f64 = 12.0;
/// Room above a group, between it and whatever came before: the heading's
/// own line is the break, so none.
const GROUP_GAP: f64 = 0.0;

/// A Space's mark: one digit or letter in a small box, filled with the
/// accent for the Space the screen is showing, bordered for the others.
fn marker(label: &str, current: bool, design: &Design) -> WidgetRef {
    let palette = &design.palette;
    let (fill, border, color) = if current {
        (palette.accent, TRANSPARENT, palette.on_accent)
    } else {
        (
            TRANSPARENT,
            palette.secondary_label,
            palette.secondary_label,
        )
    };
    AnimatedContainer::new(design.motion.highlight)
        .curve(design.motion.moving())
        .width(MARKER_SIDE)
        .height(MARKER_SIDE)
        .decoration(
            BoxDecoration::new()
                .color(fill)
                .border(Border::all(
                    border,
                    1.0,
                    BorderStyle::Solid,
                    BorderSide::STROKE_ALIGN_INSIDE,
                ))
                .border_radius(BorderRadius::circular(MARKER_RADIUS)),
        )
        .child(
            Center::new().child(Text::new(label.to_owned()).style(palette.text(
                MARKER_SIZE,
                FontWeight::W600,
                color,
            ))),
        )
        .into_widget()
}

/// The first line: Edged's own menu behind a "more" mark, reachable in the strip.
pub fn header(design: Design, menu: Listener, icon_side: Side) -> WidgetRef {
    let color = design.palette.secondary_label;
    let line = CommonStates::new(menu, move |_app, _context, states| {
        let (fill, duration) = rows::plain_fill(states, &design);
        Tile {
            text: SizedBox::new().into_widget(),
            cell: FontIcon::symbol(FluentSymbol::More)
                .font_size(MORE_SIZE)
                .foreground(color)
                .into_widget(),
            fill,
            duration,
            curve: design.motion.arriving(),
            height: HEADING_HEIGHT,
            icon_side,
        }
        .build()
    });
    line.into_widget()
}

/// One Space's heading and windows. The Space its screen is showing has its
/// mark in the accent, which moves as the user switches.
pub fn space_section(
    desktop: &Entity<Desktop>,
    space: &Space,
    showing: bool,
    rows: Vec<WidgetRef>,
    design: Design,
    icon_side: Side,
) -> WidgetRef {
    let mark = match (space.kind, space.index) {
        (SpaceKind::Fullscreen, _) => "F".to_owned(),
        (_, Some(index)) => (index + 1).to_string(),
        (_, None) => "·".to_owned(),
    };
    let switch = {
        let (desktop, target) = (desktop.clone(), space.clone());
        Listener::new(move |app: &mut App| {
            desktop.update(app, |desktop, cx| desktop.switch_to(cx, &target))
        })
    };
    let heading = CommonStates::new(switch, move |_app, _context, states| {
        let (fill, duration) = rows::plain_fill(states, &design);
        // Nothing on the text side: the empty line is the break between one
        // Space's windows and the next.
        Tile {
            text: SizedBox::new().into_widget(),
            cell: marker(&mark, showing, &design),
            fill,
            duration,
            curve: design.motion.arriving(),
            height: HEADING_HEIGHT,
            icon_side,
        }
        .build()
    });
    let mut children = vec![heading.into_widget()];
    children.extend(rows);
    // The current Space is told by its mark alone: a band behind its rows
    // fought the hover and selection pills and read as a slab.
    Padding::new(EdgeInsetsGeometry::only(0.0, GROUP_GAP, 0.0, 0.0))
        .child(
            Column::new()
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .children(children),
        )
        .into_widget()
}
