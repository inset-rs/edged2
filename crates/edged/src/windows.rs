//! One panel window per screen: opened as screens appear, closed as they go,
//! each with its own subtree. The set of screens is the widget's
//! configuration; the windows follow it in `did_update_widget`, so a build
//! here only lists them. What a panel does while its screen shows a
//! full-screen window is the panel's own affair.
//!
//! And one preview window, made once and kept off screen, that moves to
//! wherever the window the pointer rests on would be and comes forward
//! without taking the keyboard.

use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use edged_core::{Core, SettingsRequested};
use edged_macos::Screen;
use inset::{
    App, Brightness, BuildContext, FrameCallback, Handle, IntoWidget, KeyRef, MediaQuery,
    PageRoute, PageRouteBuilder, Rect, RouteSettingsRef, SchedulerBinding, State, StateData,
    StatefulWidget, StatelessWidget, Subscription, ValueKey, ViewCollection, WidgetRef, WidgetsApp,
    Window, WindowBackground, WindowConfig, WindowLevel, WindowRef,
};
use inset_winui::{AccentPalette, Theme, ThemeScope};

use crate::panel::{self, EdgePanel};
use crate::preview::{self, PreviewContent};
use crate::ring::{self, RingContent};
use crate::settings::{self, SettingsHost};
use crate::target::{self, TargetContent};

#[derive(Debug)]
pub struct PanelWindows {
    pub core: Core,
    pub screens: Vec<Screen>,
}

/// One screen's panel window.
struct Panel {
    display_id: u32,
    window: WindowRef,
}

pub struct PanelWindowsState {
    state: StateData<PanelWindows>,
    panels: Vec<Panel>,
    /// Screens whose window the host is still making.
    opening: HashSet<u32>,
    preview: Option<WindowRef>,
    /// Where the preview window is, while it is on screen.
    previewing: Option<Rect>,
    following_previews: Option<Subscription>,
    /// The ring's window, made once and kept off screen.
    ring: Option<WindowRef>,
    /// Whether the ring is on screen.
    ringing: bool,
    /// The window that shows where the ring would put the held window.
    target: Option<WindowRef>,
    /// Where the target window is, while it is on screen.
    targeting: Option<Rect>,
    following_holds: Option<Subscription>,
    /// The settings window, while it is open.
    settings: Option<WindowRef>,
    settings_requests: Option<Subscription>,
}

impl StatefulWidget for PanelWindows {
    type State = PanelWindowsState;

    fn create_state(&self) -> PanelWindowsState {
        PanelWindowsState {
            state: StateData::new(),
            panels: Vec::new(),
            opening: HashSet::new(),
            preview: None,
            previewing: None,
            following_previews: None,
            ring: None,
            ringing: false,
            target: None,
            targeting: None,
            following_holds: None,
            settings: None,
            settings_requests: None,
        }
    }
}

impl State for PanelWindowsState {
    type Widget = PanelWindows;
    inset::state_accessors!();

    fn init_state(self: Handle<Self>, app: &mut App) {
        follow(self, app);
        open_preview(self, app);
        let previews = self.widget(app).core.previews.clone();
        let following = app.observe(&previews, move |app| follow_preview(self, app));
        app.get_mut(self).following_previews = Some(following);
        open_overlay(self, app, target_config(), |state, window| {
            state.target = Some(window)
        });
        open_overlay(self, app, ring_config(), |state, window| {
            state.ring = Some(window)
        });
        let grab = self.widget(app).core.grab.clone();
        let following = app.observe(&grab, move |app| follow_hold(self, app));
        app.get_mut(self).following_holds = Some(following);
        let settings = self.widget(app).core.settings.clone();
        let requests = app.subscribe(&settings, move |app, _: &SettingsRequested| {
            open_settings(self, app)
        });
        app.get_mut(self).settings_requests = Some(requests);
    }

    fn did_update_widget(self: Handle<Self>, app: &mut App, old_widget: &PanelWindows) {
        if self.widget(app).screens != old_widget.screens {
            follow(self, app);
        }
    }

    fn dispose(self: Handle<Self>, app: &mut App) {
        let state = app.get_mut(self);
        state.following_previews.take();
        for panel in state.panels.drain(..) {
            panel.window.close();
        }
        if let Some(preview) = state.preview.take() {
            preview.close();
        }
        state.following_holds.take();
        if let Some(ring) = state.ring.take() {
            ring.close();
        }
        if let Some(target) = state.target.take() {
            target.close();
        }
        state.settings_requests.take();
        if let Some(settings) = state.settings.take() {
            settings.close();
        }
    }

    fn build(self: Handle<Self>, app: &mut App, _context: BuildContext) -> WidgetRef {
        let core = self.widget(app).core.clone();
        let windows: Vec<WidgetRef> = app
            .get(self)
            .panels
            .iter()
            .map(|panel| {
                let key = Rc::new(ValueKey::new(panel.display_id)) as KeyRef;
                let host = PanelHost {
                    core: core.clone(),
                    display_id: panel.display_id,
                };
                Window::new(Rc::clone(&panel.window), per_window_app(host))
                    .key(key)
                    .into_widget()
            })
            .collect();
        let mut windows = windows;
        if let Some(preview) = app.get(self).preview.clone() {
            windows.push(
                Window::new(
                    preview,
                    per_window_app(PreviewContent { core: core.clone() }),
                )
                .key(Rc::new(ValueKey::new("preview")) as KeyRef)
                .into_widget(),
            );
        }
        if let Some(target) = app.get(self).target.clone() {
            windows.push(
                Window::new(target, per_window_app(TargetContent { core: core.clone() }))
                    .key(Rc::new(ValueKey::new("target")) as KeyRef)
                    .into_widget(),
            );
        }
        if let Some(ring) = app.get(self).ring.clone() {
            windows.push(
                Window::new(ring, per_window_app(RingContent { core: core.clone() }))
                    .key(Rc::new(ValueKey::new("ring")) as KeyRef)
                    .into_widget(),
            );
        }
        if let Some(settings) = app.get(self).settings.clone() {
            windows.push(
                Window::new(settings, per_window_app(SettingsHost { core }))
                    .key(Rc::new(ValueKey::new("settings")) as KeyRef)
                    .into_widget(),
            );
        }
        ViewCollection::new(windows).into_widget()
    }
}

/// Makes the preview window, off screen, with clicks going through it.
fn open_preview(this: Handle<PanelWindowsState>, app: &mut App) {
    let Some(owner) = app.platform().windowing_owner() else {
        return;
    };
    let future = owner.create(preview_config());
    drop(app.spawn(async move |cx| {
        let opened = future.await;
        cx.update(|app| match opened {
            Ok(window) => {
                window.set_close_requested(Some(Rc::new(|| {})));
                if let Some(handle) = window.native_handle() {
                    edged_macos::pass_pointer_through(handle);
                    // The blur behind the picture, rounded as the picture's frame is,
                    // so the corners outside the frame show nothing of their own.
                    edged_macos::install_rounded_backdrop(handle, preview::FRAME_RADIUS);
                }
                this.set_state(app, |state| state.preview = Some(window));
                follow_preview(this, app);
            }
            Err(error) => eprintln!("edged: preview window: {error}"),
        });
    }));
}

/// Makes one of the hold's windows, off screen, with clicks going through it, and
/// keeps it where `keep` puts it.
fn open_overlay(
    this: Handle<PanelWindowsState>,
    app: &mut App,
    config: WindowConfig,
    keep: impl Fn(&mut PanelWindowsState, WindowRef) + 'static,
) {
    let Some(owner) = app.platform().windowing_owner() else {
        return;
    };
    let future = owner.create(config);
    drop(app.spawn(async move |cx| {
        let opened = future.await;
        cx.update(|app| match opened {
            Ok(window) => {
                window.set_close_requested(Some(Rc::new(|| {})));
                if let Some(handle) = window.native_handle() {
                    edged_macos::pass_pointer_through(handle);
                    edged_macos::raise_to_pop_up_level(handle);
                }
                this.set_state(app, |state| keep(state, window));
                follow_hold(this, app);
            }
            Err(error) => eprintln!("edged: overlay window: {error}"),
        });
    }));
}

/// A window of the hold's: bare, see-through, above everything, on every Space.
fn overlay_config(title: &str, size: [f64; 2]) -> WindowConfig {
    WindowConfig {
        title: title.to_owned(),
        size,
        position: Some([0.0, 0.0]),
        decorations: false,
        resizable: false,
        level: WindowLevel::AlwaysOnTop,
        activating: false,
        all_desktops: true,
        background: WindowBackground::Transparent,
        shadow: false,
        visible: false,
    }
}

fn ring_config() -> WindowConfig {
    overlay_config("Edged Ring", [ring::WINDOW_SIZE, ring::WINDOW_SIZE])
}

fn target_config() -> WindowConfig {
    overlay_config("Edged Target", target::INITIAL_SIZE)
}

/// How long the target takes to glide from one zone to the next.
const TARGET_MOVE: Duration = Duration::from_millis(80);

/// Keeps the ring where the arrange chord went down and the target over the zone the
/// pointer picks, while a window is held that way, and takes both off screen otherwise.
fn follow_hold(this: Handle<PanelWindowsState>, app: &mut App) {
    let (origin, target) = {
        let grab = self_core(this, app).grab.clone();
        let grab = grab.read(app);
        match grab.hold.as_ref() {
            Some(hold) if hold.mode == edged_core::Mode::Arrange => (
                Some(hold.origin),
                hold.target()
                    .map(|frame| Rect::from_ltwh(frame.x, frame.y, frame.width, frame.height)),
            ),
            _ => (None, None),
        }
    };
    follow_target(this, app, target);
    follow_ring(this, app, origin);
}

fn follow_ring(this: Handle<PanelWindowsState>, app: &mut App, origin: Option<(f64, f64)>) {
    let Some(window) = app.get(this).ring.clone() else {
        return;
    };
    let Some(handle) = window.native_handle() else {
        return;
    };
    let ringing = app.get(this).ringing;
    match origin {
        Some((x, y)) => {
            if !ringing {
                let half = ring::WINDOW_SIZE / 2.0;
                window.set_frame(
                    Rect::from_ltwh(x - half, y - half, ring::WINDOW_SIZE, ring::WINDOW_SIZE),
                    None,
                );
            }
            // Front again each time, so the ring stays above the target as it comes and goes.
            edged_macos::order_front(handle);
            app.get_mut(this).ringing = true;
        }
        None => {
            if ringing {
                edged_macos::order_out(handle);
                app.get_mut(this).ringing = false;
            }
        }
    }
}

fn follow_target(this: Handle<PanelWindowsState>, app: &mut App, target: Option<Rect>) {
    let Some(window) = app.get(this).target.clone() else {
        return;
    };
    let Some(handle) = window.native_handle() else {
        return;
    };
    let targeting = app.get(this).targeting;
    match target {
        Some(frame) => {
            if targeting == Some(frame) {
                return;
            }
            // Already on screen: glide to the next zone; else appear there.
            window.set_frame(frame, targeting.map(|_| TARGET_MOVE));
            if targeting.is_none() {
                edged_macos::order_front(handle);
            }
            app.get_mut(this).targeting = Some(frame);
        }
        None => {
            if targeting.is_some() {
                edged_macos::order_out(handle);
                app.get_mut(this).targeting = None;
            }
        }
    }
}

/// Brings the settings window forward, making it first if there is none.
fn open_settings(this: Handle<PanelWindowsState>, app: &mut App) {
    if let Some(window) = app.get(this).settings.clone() {
        window.show();
        return;
    }
    let Some(owner) = app.platform().windowing_owner() else {
        return;
    };
    let future = owner.create(settings_config());
    drop(app.spawn(async move |cx| {
        let opened = future.await;
        cx.update(|app| match opened {
            Ok(window) => {
                // The close box closes the window for good; the next request makes a new one.
                let closer = app.to_async();
                window.set_close_requested(Some(Rc::new(move || {
                    closer.post(move |app| {
                        this.set_state(app, |state| {
                            if let Some(window) = state.settings.take() {
                                window.close();
                            }
                        });
                    });
                })));
                this.set_state(app, |state| state.settings = Some(window));
            }
            Err(error) => eprintln!("edged: settings window: {error}"),
        });
    }));
}

/// The settings window: an ordinary one, since it takes typing and clicks.
fn settings_config() -> WindowConfig {
    WindowConfig {
        title: "Edged Settings".to_owned(),
        size: settings::WINDOW_SIZE,
        position: None,
        decorations: true,
        resizable: false,
        level: WindowLevel::Normal,
        activating: true,
        all_desktops: false,
        background: WindowBackground::Solid,
        shadow: true,
        visible: true,
    }
}

/// The preview's window: bare, see-through, above the panels, on every Space.
fn preview_config() -> WindowConfig {
    WindowConfig {
        title: "Edged Preview".to_owned(),
        size: preview::INITIAL_SIZE,
        position: Some([0.0, 0.0]),
        decorations: false,
        resizable: false,
        level: WindowLevel::AlwaysOnTop,
        activating: false,
        all_desktops: true,
        // The blur behind the picture is the panel's own, put in with the window's
        // handle, so that it can be rounded to the picture's frame.
        background: WindowBackground::Transparent,
        shadow: false,
        visible: false,
    }
}

/// How long the preview takes to fade in once its first picture is drawn.
const PREVIEW_FADE: Duration = Duration::from_millis(120);
/// How long the preview takes to glide from one window's frame to the next.
const PREVIEW_MOVE: Duration = Duration::from_millis(140);

/// Keeps the preview window with the window the pointer rests on: it comes
/// forward, fading in, once the first picture has been drawn; from then on it
/// glides to each next window's frame the moment that window's picture is
/// ready, showing the last picture until then, so hovering down the list reads
/// as one window following the pointer with nothing blank between; and it
/// goes off screen when the pointer has left.
fn follow_preview(this: Handle<PanelWindowsState>, app: &mut App) {
    let Some(window) = app.get(this).preview.clone() else {
        return;
    };
    let Some(handle) = window.native_handle() else {
        return;
    };
    let (showing, pictured) = {
        let previews = self_core(this, app).previews.clone();
        let previews = previews.read(app);
        let showing = previews.showing.clone();
        let pictured = showing
            .as_ref()
            .is_some_and(|rest| previews.picture_of(rest.window.id).is_some());
        (showing, pictured)
    };
    let previewing = app.get(this).previewing;
    let Some(rest) = showing else {
        if previewing.is_some() {
            edged_macos::order_out(handle);
            app.get_mut(this).previewing = None;
        }
        return;
    };
    let frame = rest.window.frame;
    let frame = Rect::from_ltwh(frame.x, frame.y, frame.width, frame.height);
    if previewing == Some(frame) {
        return;
    }
    if !pictured {
        // Nothing moves until the next window's picture is ready: what is on screen
        // stays as it is, so the change happens once, and whole.
        return;
    }
    if previewing.is_some() {
        // Already on screen: glide to the next window as its picture takes over.
        window.set_frame(frame, Some(PREVIEW_MOVE));
        app.get_mut(this).previewing = Some(frame);
        return;
    }
    window.set_frame(frame, None);
    app.get_mut(this).previewing = Some(frame);
    // The frame the picture is drawn in is the next one; the window comes forward after it.
    SchedulerBinding::add_post_frame_callback(
        app,
        FrameCallback::new(move |app, _elapsed| {
            if app.get(this).previewing == Some(frame) {
                edged_macos::fade_in(handle, PREVIEW_FADE);
            }
        }),
    );
}

fn self_core(this: Handle<PanelWindowsState>, app: &App) -> Core {
    this.widget(app).core.clone()
}

/// Brings the windows in line with the screens: closes the panels of screens
/// that went, opens one for each new screen.
fn follow(this: Handle<PanelWindowsState>, app: &mut App) {
    let screens = this.widget(app).screens.clone();
    close_gone(this, app, &screens);
    open_new(this, app, &screens);
}

fn close_gone(this: Handle<PanelWindowsState>, app: &mut App, screens: &[Screen]) {
    let gone: Vec<usize> = app
        .get(this)
        .panels
        .iter()
        .enumerate()
        .filter(|(_, panel)| !screens.iter().any(|s| s.display_id == panel.display_id))
        .map(|(index, _)| index)
        .collect();
    if gone.is_empty() {
        return;
    }
    this.set_state(app, |state| {
        for index in gone.into_iter().rev() {
            state.panels.remove(index).window.close();
        }
    });
}

fn open_new(this: Handle<PanelWindowsState>, app: &mut App, screens: &[Screen]) {
    let Some(owner) = app.platform().windowing_owner() else {
        eprintln!("edged: this host opens no windows, so there is nowhere to put a panel");
        return;
    };
    for screen in screens {
        let display_id = screen.display_id;
        let state = app.get(this);
        let known = state.panels.iter().any(|p| p.display_id == display_id)
            || state.opening.contains(&display_id);
        if known {
            continue;
        }
        app.get_mut(this).opening.insert(display_id);
        let future = owner.create(panel::window_config(screen));
        drop(app.spawn(async move |cx| {
            let opened = future.await;
            cx.update(|app| {
                app.get_mut(this).opening.remove(&display_id);
                match opened {
                    Ok(window) => opened_for(this, app, display_id, window),
                    Err(error) => eprintln!("edged: panel window: {error}"),
                }
            });
        }));
    }
}

/// Dresses a window the host just made and lists it.
fn opened_for(this: Handle<PanelWindowsState>, app: &mut App, display_id: u32, window: WindowRef) {
    // The panel has no close box; a close request is what a hotkey like ⌘W
    // would send, and the panel stays.
    window.set_close_requested(Some(Rc::new(|| {})));
    let dressed = window.native_handle().is_some_and(|handle| {
        // Above the preview, whichever of the two was shown last.
        edged_macos::raise_to_pop_up_level(handle);
        edged_macos::install_backdrop(handle, panel::PANEL_RADIUS)
    });
    if !dressed {
        eprintln!("edged: the panel window has no native view to put glass behind");
    }
    this.set_state(app, |state| state.panels.push(Panel { display_id, window }));
}

/// Every window gets its own navigator and overlay, which the tooltips need,
/// and the reading direction and media query every widget expects.
fn per_window_app<K>(home: impl IntoWidget<K>) -> WidgetRef {
    WidgetsApp::new(AccentPalette::default().base)
        .title("Edged")
        .debug_show_checked_mode_banner(false)
        .page_route_builder(|app, settings, builder| {
            let route = PageRouteBuilder::new(
                app,
                Rc::new(move |app, context, _, _| builder(app, context)),
            )
            .settings(app, RouteSettingsRef::Settings(settings.clone()));
            PageRoute::as_page_route(route)
        })
        .home(home)
        .into_widget()
}

/// The panel under the theme scope the WinUI controls read.
#[derive(Debug)]
struct PanelHost {
    core: Core,
    display_id: u32,
}

impl StatelessWidget for PanelHost {
    fn build(&self, app: &mut App, context: BuildContext) -> WidgetRef {
        let theme = match MediaQuery::platform_brightness_of(app, context) {
            Brightness::Dark => Theme::Dark,
            Brightness::Light => Theme::Light,
        };
        let tucked = {
            let desktop = self.core.desktop.read(app);
            desktop
                .screen(self.display_id)
                .is_some_and(|screen| desktop.is_full_screen(screen))
        };
        ThemeScope::new(
            theme,
            EdgePanel {
                core: self.core.clone(),
                display_id: self.display_id,
                tucked,
            },
        )
        .into_widget()
    }
}
