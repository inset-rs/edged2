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
//!
//! While a full-screen window owns the screen the panel is tucked: a hair
//! wide, invisible, inside the edge. A window that shows nothing hears no
//! pointer, so the edge is watched for it instead, and the panel comes out
//! from there as it does from the strip.

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
/// The panel's width while a full-screen window owns the screen: a hair
/// inside the edge, invisible, there only to be reached.
const TUCK_WIDTH: f64 = 1.0;
/// How far from the screen's edge the pointer counts as at it.
const EDGE_ZONE: f64 = 1.0;

/// The window a screen's panel lives in: bare, floating, on every Space, at
/// the strip's width until the pointer reaches it. See-through, with no
/// background of the host's: the panel puts its own glass behind it, shaped
/// as it wants, through the window's native handle. It keeps the system's
/// shadow, which is also where the hairline around a window comes from.
pub fn window_config(screen: &Screen) -> WindowConfig {
    let frame = frame_for(screen, STRIP_WIDTH, INITIAL_HEIGHT);
    WindowConfig {
        title: "Edged".to_owned(),
        size: [frame.width(), frame.height()],
        position: Some([frame.left, frame.top]),
        decorations: false,
        resizable: false,
        level: WindowLevel::AlwaysOnTop,
        activating: false,
        all_desktops: true,
        background: WindowBackground::Transparent,
        shadow: true,
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

/// Whether a point is at the screen's edge, level with the frame.
fn is_at_edge(screen: &Screen, frame: &Rect, x: f64, y: f64) -> bool {
    x >= screen.visible_frame.right() - EDGE_ZONE
        && y >= frame.top - POINTER_MARGIN
        && y <= frame.bottom + POINTER_MARGIN
}

/// The width the panel rests at: the strip, or a hair while tucked.
fn resting_width(tucked: bool) -> f64 {
    if tucked { TUCK_WIDTH } else { STRIP_WIDTH }
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
    /// Whether the screen shows a full-screen window, which owns it whole
    /// and has the panel keep out of sight.
    pub tucked: bool,
}

impl std::fmt::Debug for EdgePanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgePanel")
            .field("display_id", &self.display_id)
            .field("tucked", &self.tucked)
            .finish_non_exhaustive()
    }
}

pub struct EdgePanelState {
    state: StateData<EdgePanel>,
    core: Core,
    display_id: u32,
    /// Whether the pointer is on the panel, which slides it out.
    expanded: bool,
    /// Whether the panel rests a hair wide and invisible, for a full-screen
    /// window; as the widget last said.
    tucked: bool,
    /// Whether the first build is done, after which the window can be reached.
    placed: bool,
    /// What the content last measured, which is the window's height.
    content_height: f64,
    /// The look for the pointer: for its leaving while the panel is out, for
    /// its reaching the edge while the panel is tucked.
    watching: Option<Timer>,
    /// The moment a tucked panel that slid back in goes invisible.
    hiding: Option<Timer>,
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
            tucked: self.tucked,
            placed: false,
            content_height: INITIAL_HEIGHT,
            watching: None,
            hiding: None,
            away: 0,
        }
    }
}

impl State for EdgePanelState {
    type Widget = EdgePanel;
    inset::state_accessors!();

    /// The window is reachable from here on; a panel born under a full-screen
    /// window tucks at once.
    fn did_change_dependencies(self: Handle<Self>, app: &mut App) {
        if app.get(self).placed {
            return;
        }
        app.get_mut(self).placed = true;
        if app.get(self).tucked
            && let Some((window, screen)) = window_and_screen(self, app)
        {
            tuck(self, app, &window, &screen);
        }
    }

    fn did_update_widget(self: Handle<Self>, app: &mut App, old_widget: &EdgePanel) {
        let tucked = self.widget(app).tucked;
        if tucked == old_widget.tucked {
            return;
        }
        app.get_mut(self).tucked = tucked;
        let Some((window, screen)) = window_and_screen(self, app) else {
            return;
        };
        if tucked {
            tuck(self, app, &window, &screen);
        } else {
            untuck(self, app, &window, &screen);
        }
    }

    fn dispose(self: Handle<Self>, app: &mut App) {
        for timer in [
            app.get_mut(self).watching.take(),
            app.get_mut(self).hiding.take(),
        ]
        .into_iter()
        .flatten()
        {
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
        // An exit is believed only where the pointer has left in truth: the
        // window sends one for its own resizing under a still pointer too.
        let exit = {
            let (window, screen) = (Rc::clone(&window), screen.clone());
            Rc::new(move |app: &mut App, _event| {
                if app.get(this).expanded && !pointer_on_panel(this, app, &screen) {
                    slide_in(this, app, &window, &screen);
                }
            })
        };
        let measured: SizeChangedCallback = {
            let (window, screen) = (Rc::clone(&window), screen.clone());
            Rc::new(move |app: &mut App, _was: Size, now: Size| {
                follow_height(this, app, &window, &screen, trusted, now.height())
            })
        };
        MouseRegion::new()
            .on_enter(enter)
            .on_exit(exit)
            .child(body(content, height_cap(&screen), measured))
            .into_widget()
    }
}

/// The panel's window and screen, once it is in a window and its screen is known.
fn window_and_screen(this: Handle<EdgePanelState>, app: &mut App) -> Option<(WindowRef, Screen)> {
    let context = this.context(app);
    let window = WindowScope::maybe_of(app, context)?;
    let (core, display_id) = {
        let state = app.get(this);
        (state.core.clone(), state.display_id)
    };
    let screen = core.desktop.read(app).screen(display_id).cloned()?;
    Some((window, screen))
}

/// Whether the pointer is on the panel at its full width.
fn pointer_on_panel(this: Handle<EdgePanelState>, app: &App, screen: &Screen) -> bool {
    let frame = frame_for(screen, PANEL_WIDTH, app.get(this).content_height);
    edged_macos::pointer_location().is_some_and(|(x, y)| is_on(&frame, x, y))
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
    let width = {
        let state = app.get(this);
        if state.expanded || !trusted {
            PANEL_WIDTH
        } else {
            resting_width(state.tucked)
        }
    };
    window.set_frame(frame_for(screen, width, height), None);
}

/// Slides the panel out and starts looking for the pointer to leave.
fn slide_out(this: Handle<EdgePanelState>, app: &mut App, window: &WindowRef, screen: &Screen) {
    if app.get(this).expanded {
        return;
    }
    if let Some(timer) = app.get_mut(this).hiding.take() {
        timer.cancel(app);
    }
    this.set_state(app, |state| {
        state.expanded = true;
        state.away = 0;
    });
    if app.get(this).tucked {
        set_visible(window, true);
    }
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
            let found = pointer_on_panel(this, app, &screen);
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

/// Slides the panel in, unless access is still to be granted, when it stays
/// out. A tucked panel slides in to a hair and then goes invisible, and the
/// edge is watched for the pointer again.
fn slide_in(this: Handle<EdgePanelState>, app: &mut App, window: &WindowRef, screen: &Screen) {
    let core = app.get(this).core.clone();
    let trusted = core.permissions.read(app).accessibility;
    core.previews
        .update(app, |previews, cx| previews.dismiss(cx));
    this.set_state(app, |state| {
        state.expanded = false;
        state.watching = None;
    });
    if !trusted {
        return;
    }
    let tucked = app.get(this).tucked;
    let height = app.get(this).content_height;
    let duration = motion_of(this, app).slide_in;
    window.set_frame(
        frame_for(screen, resting_width(tucked), height),
        slide(duration),
    );
    if tucked {
        hide_after(this, app, Rc::clone(window), screen.clone(), duration);
    }
}

/// Tucks the panel for a full-screen window: in to a hair, then invisible,
/// with the edge watched for the pointer. A panel that is out tucks once it
/// slides in; one waiting for access stays out.
fn tuck(this: Handle<EdgePanelState>, app: &mut App, window: &WindowRef, screen: &Screen) {
    let trusted = app.get(this).core.permissions.read(app).accessibility;
    if app.get(this).expanded || !trusted {
        return;
    }
    let height = app.get(this).content_height;
    let duration = motion_of(this, app).slide_in;
    window.set_frame(frame_for(screen, TUCK_WIDTH, height), slide(duration));
    hide_after(this, app, Rc::clone(window), screen.clone(), duration);
}

/// Brings a tucked panel back to the strip, where the pointer reaches it on
/// its own.
fn untuck(this: Handle<EdgePanelState>, app: &mut App, window: &WindowRef, screen: &Screen) {
    for timer in [
        app.get_mut(this).hiding.take(),
        (!app.get(this).expanded)
            .then(|| app.get_mut(this).watching.take())
            .flatten(),
    ]
    .into_iter()
    .flatten()
    {
        timer.cancel(app);
    }
    set_visible(window, true);
    let trusted = app.get(this).core.permissions.read(app).accessibility;
    if app.get(this).expanded || !trusted {
        return;
    }
    let height = app.get(this).content_height;
    window.set_frame(
        frame_for(screen, STRIP_WIDTH, height),
        slide(motion_of(this, app).slide_in),
    );
}

/// Once a slide in has ended, makes the tucked panel invisible and starts
/// watching the edge for the pointer.
fn hide_after(
    this: Handle<EdgePanelState>,
    app: &mut App,
    window: WindowRef,
    screen: Screen,
    duration: Duration,
) {
    let timer = Timer::new(
        app,
        duration,
        Listener::new(move |app: &mut App| {
            app.get_mut(this).hiding = None;
            let state = app.get(this);
            if state.expanded || !state.tucked {
                return;
            }
            set_visible(&window, false);
            watch_edge(this, app, Rc::clone(&window), screen.clone());
        }),
    );
    app.get_mut(this).hiding = Some(timer);
}

/// Looks for the pointer at the screen's edge, level with the panel, and
/// slides the panel out when it is there; the only way a panel that shows
/// nothing can be reached.
fn watch_edge(this: Handle<EdgePanelState>, app: &mut App, window: WindowRef, screen: Screen) {
    let timer = Timer::new(
        app,
        POINTER_POLL,
        Listener::new(move |app: &mut App| {
            let state = app.get(this);
            if state.expanded || !state.tucked {
                return;
            }
            let frame = frame_for(&screen, PANEL_WIDTH, state.content_height);
            let reached = edged_macos::pointer_location()
                .is_some_and(|(x, y)| is_at_edge(&screen, &frame, x, y));
            if reached {
                slide_out(this, app, &window, &screen);
            } else {
                watch_edge(this, app, Rc::clone(&window), screen.clone());
            }
        }),
    );
    app.get_mut(this).watching = Some(timer);
}

/// Shows or hides the window where it is, without the host's `show`, which
/// would activate the application.
fn set_visible(window: &WindowRef, visible: bool) {
    if let Some(handle) = window.native_handle() {
        edged_macos::set_alpha(handle, if visible { 1.0 } else { 0.0 });
    }
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
            let showing = model.is_showing(space);
            let rows: Vec<WidgetRef> = model
                .windows_on(space.id)
                .into_iter()
                .map(|(entry, window)| {
                    rows::window_row(
                        desktop,
                        &core.previews,
                        entry,
                        window,
                        showing,
                        screen.scale,
                        design,
                    )
                })
                .collect();
            children.push(sections::space_section(
                desktop, space, showing, rows, design,
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
    use edged_macos::Frame;

    use super::*;

    #[test]
    fn a_point_a_hair_past_the_edge_still_counts_as_on_the_panel() {
        let frame = Rect::from_ltwh(1412.0, 100.0, 28.0, 400.0);
        assert!(is_on(&frame, 1440.5, 300.0));
        assert!(is_on(&frame, 1412.0, 99.5));
        assert!(!is_on(&frame, 1410.0, 300.0));
        assert!(!is_on(&frame, 1420.0, 502.0));
    }

    #[test]
    fn the_edge_is_reached_only_level_with_the_panel() {
        let screen = Screen {
            display_id: 1,
            uuid: String::new(),
            frame: Frame {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            visible_frame: Frame {
                x: 0.0,
                y: 25.0,
                width: 1440.0,
                height: 875.0,
            },
            scale: 2.0,
            is_main: true,
        };
        let frame = frame_for(&screen, PANEL_WIDTH, 400.0);
        assert!(is_at_edge(&screen, &frame, 1439.5, frame.top + 10.0));
        assert!(!is_at_edge(&screen, &frame, 1430.0, frame.top + 10.0));
        assert!(!is_at_edge(&screen, &frame, 1439.5, frame.bottom + 20.0));
    }
}
