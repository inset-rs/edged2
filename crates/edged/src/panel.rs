//! One screen's panel: a window against the screen's right edge that shows a
//! strip and slides out to the screen's applications and windows when the
//! pointer reaches it.
//!
//! The window is the host's; the panel decides its frame. The content is laid
//! out at the panel's full width whatever the window shows and anchored to
//! the screen's edge, so the strip is the content's own trailing cell and
//! nothing moves as the window widens. The window is as tall as the content,
//! centred on the screen's usable height.
//!
//! The pointer's arrival comes from the window; its leaving is watched for
//! while the panel is out, because a window's own enter and exit events go
//! astray while it is resizing under a still pointer.

use std::rc::Rc;
use std::time::Duration;

use edged_core::Core;
use edged_macos::Screen;
use inset::{
    AlignmentGeometry, App, BuildContext, Column, CrossAxisAlignment, Handle, InheritedWidget,
    IntoWidget, LayoutBuilder, LimitedBox, Listener, MainAxisSize, MediaQuery, MouseRegion,
    OverflowBox, Rect, SingleChildScrollView, Size, SizedBox, State, StateData, StatefulWidget,
    Timer, WidgetRef, WindowBackground, WindowConfig, WindowLevel, WindowRef, WindowScope,
};
use inset_winui::{SizeChangedCallback, SizeObserver};

use crate::menus;
use crate::permission::PermissionView;
use crate::rows;
use crate::sections;
use crate::theme::{Design, Motion};

/// The panel's width when slid out.
pub const PANEL_WIDTH: f64 = 250.0;
/// The strip left showing at the edge when the panel is slid in.
pub const STRIP_WIDTH: f64 = edged_core::STRIP_WIDTH;
/// The panel's corners on the side away from the screen's edge.
pub const PANEL_RADIUS: f64 = 8.0;
/// The height a panel opens with, until its content has been measured.
const INITIAL_HEIGHT: f64 = 400.0;
/// Room the panel leaves at the top and bottom of the screen's usable area.
const MARGIN: f64 = 8.0;
/// How often the pointer is looked for while the panel is out.
const POINTER_POLL: Duration = Duration::from_millis(110);
/// How many looks in a row may miss the pointer before the panel slides in:
/// the grace that lets it cross a corner.
const AWAY_LOOKS: u32 = 2;
/// Room around the window the pointer still counts as on it: the pointer
/// stops a point short of the screen's edge.
const POINTER_MARGIN: f64 = 1.0;

/// The window a screen's panel lives in: bare, floating, on every Space, at
/// the strip's width until the pointer reaches it. See-through, with no
/// background of the host's: the panel puts its own glass behind it, shaped
/// as it wants, through the window's native handle.
pub fn window_config(screen: &Screen) -> WindowConfig {
    let frame = frame_for(screen, STRIP_WIDTH, INITIAL_HEIGHT);
    WindowConfig {
        title: "Edged".to_owned(),
        size: [frame.width(), frame.height()],
        position: Some([frame.left, frame.top]),
        decorations: false,
        resizable: false,
        transparent: true,
        level: WindowLevel::AlwaysOnTop,
        activating: false,
        all_spaces: true,
        background: WindowBackground::Transparent,
        shadow: false,
        visible: true,
    }
}

/// Where a panel of this width and height sits: against the right edge of the
/// screen's usable area, centred on its height, never taller than that area
/// less a margin.
pub fn frame_for(screen: &Screen, width: f64, height: f64) -> Rect {
    let usable = &screen.visible_frame;
    let height = height.min(usable.height - 2.0 * MARGIN).max(1.0);
    Rect::from_ltwh(
        usable.right() - width,
        usable.y + (usable.height - height) / 2.0,
        width,
        height,
    )
}

/// Whether a point is on the frame, or within the margin of it.
fn is_on(frame: &Rect, x: f64, y: f64) -> bool {
    x >= frame.left - POINTER_MARGIN
        && x <= frame.right + POINTER_MARGIN
        && y >= frame.top - POINTER_MARGIN
        && y <= frame.bottom + POINTER_MARGIN
}

fn width_for(slid_out: bool) -> f64 {
    if slid_out { PANEL_WIDTH } else { STRIP_WIDTH }
}

/// A slide the host animates, or none when there is no motion to show.
fn slide(duration: Duration) -> Option<Duration> {
    (!duration.is_zero()).then_some(duration)
}

/// The width of the panel the window shows now, for the rows to set their
/// pill by.
pub struct PanelGeometry {
    pub visible_width: f64,
    pub child: WidgetRef,
}

impl PanelGeometry {
    /// The visible width above the `context`, or the full panel outside one.
    pub fn of(app: &mut App, context: BuildContext) -> f64 {
        context
            .depend_on_inherited_widget_of_exact_type::<PanelGeometry>(app)
            .map(|geometry| geometry.visible_width)
            .unwrap_or(PANEL_WIDTH)
    }
}

impl std::fmt::Debug for PanelGeometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanelGeometry")
            .field("visible_width", &self.visible_width)
            .finish_non_exhaustive()
    }
}

impl InheritedWidget for PanelGeometry {
    fn child(&self) -> &WidgetRef {
        &self.child
    }

    fn update_should_notify(&self, old_widget: &PanelGeometry) -> bool {
        (self.visible_width - old_widget.visible_width).abs() > 0.01
    }
}

/// One screen's panel, keyed by the screen so it survives the list being rebuilt.
pub struct EdgePanel {
    pub core: Core,
    pub display_id: u32,
}

impl std::fmt::Debug for EdgePanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgePanel")
            .field("display_id", &self.display_id)
            .finish_non_exhaustive()
    }
}

pub struct EdgePanelState {
    state: StateData<EdgePanel>,
    core: Core,
    display_id: u32,
    /// Whether the pointer is on the panel, which slides it out.
    expanded: bool,
    /// What the content last measured, which is the window's height.
    content_height: f64,
    /// The look for the pointer that runs while the panel is out.
    watching: Option<Timer>,
    /// How many looks in a row have missed the pointer.
    away: u32,
}

impl StatefulWidget for EdgePanel {
    type State = EdgePanelState;

    fn create_state(&self) -> EdgePanelState {
        EdgePanelState {
            state: StateData::new(),
            core: self.core.clone(),
            display_id: self.display_id,
            expanded: false,
            content_height: INITIAL_HEIGHT,
            watching: None,
            away: 0,
        }
    }
}

impl State for EdgePanelState {
    type Widget = EdgePanel;
    inset::state_accessors!();

    fn dispose(self: Handle<Self>, app: &mut App) {
        if let Some(timer) = app.get_mut(self).watching.take() {
            timer.cancel(app);
        }
    }

    fn build(self: Handle<Self>, app: &mut App, context: BuildContext) -> WidgetRef {
        let window = WindowScope::of(app, context);
        let (core, display_id) = {
            let state = app.get(self);
            (state.core.clone(), state.display_id)
        };
        let brightness = MediaQuery::platform_brightness_of(app, context);
        let design = Design::of(app, brightness, &core.appearance);
        let trusted = core.permissions.read(app).accessibility;
        let Some(screen) = core.desktop.read(app).screen(display_id).cloned() else {
            return SizedBox::new().into_widget();
        };
        // The permission request needs the panel's full width to be read.
        let content = if trusted {
            content(app, &core, &screen, design)
        } else {
            PermissionView {
                design,
                permissions: core.permissions.clone(),
            }
            .into_widget()
        };

        let this = self;
        let enter = {
            let (window, screen) = (Rc::clone(&window), screen.clone());
            Rc::new(move |app: &mut App, _event| slide_out(this, app, &window, &screen))
        };
        let measured: SizeChangedCallback = {
            let (window, screen) = (Rc::clone(&window), screen.clone());
            Rc::new(move |app: &mut App, _was: Size, now: Size| {
                follow_height(this, app, &window, &screen, trusted, now.height())
            })
        };
        MouseRegion::new()
            .on_enter(enter)
            .child(body(content, height_cap(&screen), measured))
            .into_widget()
    }
}

/// The content laid out at full width and its own height whatever the window
/// shows, against the screen's edge, so the strip is the content's own
/// trailing cell; measured, so the window can follow the content's height;
/// and told the width the window shows, so the rows can set their pill by it.
fn body(content: WidgetRef, cap: f64, measured: SizeChangedCallback) -> WidgetRef {
    LayoutBuilder::new(move |_app, _context, constraints| {
        PanelGeometry {
            visible_width: constraints.max_width,
            child: OverflowBox::new()
                .alignment(AlignmentGeometry::TOP_RIGHT)
                .min_width(PANEL_WIDTH)
                .max_width(PANEL_WIDTH)
                .min_height(0.0)
                .max_height(f64::INFINITY)
                .child(SizeObserver::new(
                    // The height the screen allows, applied only where the
                    // parent leaves it unbounded, which the overflow box does.
                    LimitedBox::new()
                        .max_height(cap)
                        .child(SingleChildScrollView::new().child(content.clone())),
                    measured.clone(),
                ))
                .into_widget(),
        }
        .into_widget()
    })
    .into_widget()
}

/// The tallest the panel may be on this screen.
fn height_cap(screen: &Screen) -> f64 {
    screen.visible_frame.height - 2.0 * MARGIN
}

/// Sizes the window to the content, at whatever width the panel is at.
fn follow_height(
    this: Handle<EdgePanelState>,
    app: &mut App,
    window: &WindowRef,
    screen: &Screen,
    trusted: bool,
    height: f64,
) {
    if (app.get(this).content_height - height).abs() < 0.5 {
        return;
    }
    app.get_mut(this).content_height = height;
    let slid_out = app.get(this).expanded || !trusted;
    window.set_frame(frame_for(screen, width_for(slid_out), height), None);
}

/// Slides the panel out and starts looking for the pointer to leave.
fn slide_out(this: Handle<EdgePanelState>, app: &mut App, window: &WindowRef, screen: &Screen) {
    if app.get(this).expanded {
        return;
    }
    this.set_state(app, |state| {
        state.expanded = true;
        state.away = 0;
    });
    let height = app.get(this).content_height;
    window.set_frame(
        frame_for(screen, PANEL_WIDTH, height),
        slide(motion_of(this, app).slide_out),
    );
    look_for_pointer(this, app, Rc::clone(window), screen.clone());
}

/// Looks once for the pointer after the poll interval, and again while it is
/// found; slides the panel in once it has been missed enough times in a row.
fn look_for_pointer(
    this: Handle<EdgePanelState>,
    app: &mut App,
    window: WindowRef,
    screen: Screen,
) {
    let timer = Timer::new(
        app,
        POINTER_POLL,
        Listener::new(move |app: &mut App| {
            if !app.get(this).expanded {
                return;
            }
            let height = app.get(this).content_height;
            let frame = frame_for(&screen, PANEL_WIDTH, height);
            let found = edged_macos::pointer_location().is_some_and(|(x, y)| is_on(&frame, x, y));
            let away = if found { 0 } else { app.get(this).away + 1 };
            app.get_mut(this).away = away;
            if away >= AWAY_LOOKS {
                slide_in(this, app, &window, &screen);
            } else {
                look_for_pointer(this, app, Rc::clone(&window), screen.clone());
            }
        }),
    );
    app.get_mut(this).watching = Some(timer);
}

/// Slides the panel in, unless access is still to be granted, when it stays out.
fn slide_in(this: Handle<EdgePanelState>, app: &mut App, window: &WindowRef, screen: &Screen) {
    let permissions = app.get(this).core.permissions.clone();
    let trusted = permissions.read(app).accessibility;
    this.set_state(app, |state| {
        state.expanded = false;
        state.watching = None;
    });
    if !trusted {
        return;
    }
    let height = app.get(this).content_height;
    window.set_frame(
        frame_for(screen, STRIP_WIDTH, height),
        slide(motion_of(this, app).slide_in),
    );
}

/// The motion the system allows now.
fn motion_of(this: Handle<EdgePanelState>, app: &App) -> Motion {
    let appearance = app.get(this).core.appearance.clone();
    Motion::for_setting(appearance.read(app).reduces_motion)
}

/// Edged's menu, the applications with no windows, then each Space of the
/// screen with its windows.
fn content(app: &mut App, core: &Core, screen: &Screen, design: Design) -> WidgetRef {
    let menu = {
        let core = core.clone();
        Listener::new(move |app: &mut App| menus::edged_menu(app, &core))
    };
    let desktop = &core.desktop;
    let mut children = vec![sections::header(design, menu)];
    {
        let model = desktop.read(app);
        for entry in model.windowless() {
            children.push(rows::application_row(desktop, entry, design));
        }
        for space in model.spaces_of(screen) {
            let rows: Vec<WidgetRef> = model
                .windows_on(space.id)
                .into_iter()
                .map(|(entry, window)| rows::window_row(desktop, entry, window, design))
                .collect();
            children.push(sections::space_section(
                desktop,
                space,
                model.is_showing(space),
                rows,
                design,
            ));
        }
    }
    // Nothing above the first line or below the last: a row's own gutters
    // are the panel's margin, as they are at its sides.
    Column::new()
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .children(children)
        .into_widget()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_a_hair_past_the_edge_still_counts_as_on_the_panel() {
        let frame = Rect::from_ltwh(1412.0, 100.0, 28.0, 400.0);
        assert!(is_on(&frame, 1440.5, 300.0));
        assert!(is_on(&frame, 1412.0, 99.5));
        assert!(!is_on(&frame, 1410.0, 300.0));
        assert!(!is_on(&frame, 1420.0, 502.0));
    }
}
