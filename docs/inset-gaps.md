# What Edged needs from Inset

Edged's panel is a window Inset cannot make yet. This note lists each gap the port met, the shape Flutter gives the same thing where it has one, and a proposal. Nothing here is implemented in Inset; it is for review.

## 1. Windows the app creates

**Need.** One panel per screen: a non-activating floating window (clicking it must not take focus from the application the user is in), above ordinary windows, present on every Space and staying put across Space switches, placed and sized by the app (right edge, vertically centred, height from content, width sliding between 28 and 250 points in 100 ms), transparent with the system's blur behind it, left corners rounded.

**Landed.** `Platform::windowing_owner` with `WindowingOwner::create(WindowConfig)`, `HostWindow` (view, frame, animated `set_frame`, show, hide, close, native handle, close request), and `adopt` answering unsupported; `ViewCollection`, `ViewAnchor`, `Window` and `WindowScope` in inset-widgets; the winit host making windows on its next loop turn with the implicit view's device, applying Space behaviour and a vibrancy view through AppKit, and animating frames with `NSAnimationContext`. edged2 runs with no implicit window and one panel window per screen.

**Still open.** Non-activation needs an `NSPanel`, so `adopt` and a host that drives an app-made window; winit's `set_visible(true)` on macOS orders the window front and key, so a hidden panel that returns takes focus.

**Surface alpha (valo).** Landed in valo 0.4.5 (unpublished; both workspaces patch it by path): `SurfaceOptions::default().with_alpha(SurfaceAlpha::Transparent)` on the `_with_options` constructors, kept across `resize`, with `Surface::alpha()` saying what took effect. valo's output was already premultiplied at every stage, which is what CoreAnimation, Wayland and WebGPU composite, so only the mode changed. Skia and Flutter leave the layer's opacity to the host; wgpu sets it from the surface configuration on every configure, so valo has to be told. `Transparent` picks `PreMultiplied` where offered, `PostMultiplied` only on Metal (it merely clears the layer's opaque flag), then `Inherit`; WebGPU takes `PreMultiplied` unconditionally since its capabilities under-report; plain Windows windows and OpenGL offer opaque only. The winit host asks for it on every see-through window, and the panel paints no fill of its own.

**Window corners.** Settled: not a window configuration. Only macOS shapes a window through a layer mask; Windows 11 rounds every window itself with three preset sizes and no per-corner choice, and Wayland has nothing, so a `corner_radius` field would promise what two of three desktops cannot keep. The portable shape is what the app paints into a see-through surface. The one thing that needed the host's cooperation was the glass view the host puts behind the content, which is a rectangle; so the panel asks for a `Transparent` background and places the glass itself through `HostWindow::native_handle`, the AppKit view from which the window is one call away, in `edged-macos::install_backdrop`: the view is one radius wider than the window, so the corners that hang past the screen's edge are clipped square and the leading pair keep their radius. `WindowBackground::Glass` and `Vibrancy` stay for the common full-window case. The window shadow stays off as SDL keeps it off for transparent windows: AppKit derives it from the window's alpha and would need invalidating whenever the content changes shape.

**Glass.** `WindowBackground::Glass` uses `NSGlassEffectView` when the class exists at runtime and the vibrancy view otherwise, placed beside the content view in the window's frame. There Liquid Glass renders as its flat material, not as glass: a probe with a screen capture showed the glass whole only inside the content view. The content view is winit's, drawn through a Metal layer that wgpu adds as a sublayer of its root layer; putting the glass in as a subview beneath and moving the Metal layer back on top held in the probe but not in the app, where the drawing ended up under the glass. Left at the flat material, which reads as a blur; real glass waits on a host that owns its view, the same host the non-activating panel needs.

**Popup menus and the pointer.** Landed: a native menu runs its own event loop and keeps the release of the button that opened it, so the app would go on holding that button, with no hovers and no taps until the next press. After a popup the host wakes and reconciles its record of the buttons with what AppKit says is held down, sending the releases it missed; it does the same before every pointer move.

**Frame animation curve.** `HostWindow::set_frame` takes a duration and the host animates with `NSAnimationContext`'s default timing. The panel wants an ease-out on the way out and an ease-in-out on the way in; a `curve` on the call, mapped to `CAMediaTimingFunction`, would give it. Not landed.

**Flutter.** `widgets/_window.dart` (experimental): a `WindowingOwner` on the binding creates controllers of five kinds, regular, dialog, tooltip, popup, satellite, each with a delegate for close and resize; `Window`, `DialogWindow`, `TooltipWindow`, `PopupWindow`, `SatelliteWindow` widgets pair a controller with a `View`; `WindowScope` exposes the controller to the subtree; a `WindowRegistry` plus `WindowManager` render every open window into a `ViewCollection`. On mobile the owner is `_WindowingOwnerUnsupported` and the implicit view is all there is. That split is the one Inset's embedder interface wants: implicit view on mobile and web, created views on desktop.

**Proposal.** `View` stays the one seam between the framework and a host, and a window is a view the host puts on screen. That is also Flutter's real shape on macOS: `FlutterViewController` is a view the app places in a window it owns; the archetypes in `_window.dart` are framework sugar above that. So rather than port the archetypes, give the host two entry points and keep the rest in the app:

- `WindowingOwner::create(WindowConfig) -> WindowRef`, with the config every window wants: size, position, decorations, transparency, level, whether it activates, whether it appears on every Space, and its background (`solid`, `transparent`, `vibrancy(material)`, `glass` for macOS 26's Liquid Glass with vibrancy as the fallback). The `WindowRef` carries the view id, the live size, `set_frame` with an optional animation duration (the host animates, as `NSWindow.animator()` does), show, hide, close, and a close request the app answers.
- `WindowRef::native_handle()`: the raw window and view handles, so an app with a special need configures the rest itself with objc2 or the Windows API, the way winit and wgpu already expose theirs. Edged would set its collection behaviour and corner mask this way.
- `WindowingOwner::adopt(native window) -> WindowRef`: the reverse, for what no configuration covers. Non-activation needs the window to be an `NSPanel`, so the app creates the panel and hands it over; the host installs its view in it and drives rendering and input for it, which is exactly what Flutter's macOS embedder does with any window it is given.
- `Platform::windowing_owner()` returns `None` on mobile and web, where the implicit view is all there is.

The host side decides how far this goes. `inset-embedder-winit` can create windows with `with_decorations(false)`, `with_transparent(true)`, `WindowLevel::AlwaysOnTop`, position and size, register each as a `View` with its own surface, and hand out the raw handle; it cannot adopt a window it did not create, and it makes `NSWindow`s, never `NSPanel`s. Adoption, and with it the non-activating panel, needs a host that owns its AppKit view: an `inset-embedder-macos`, the counterpart of Flutter's macOS embedder, with winit kept for Windows and Linux. That is the piece to plan for; the config-plus-handle surface above is the same either way.

## 2. Menus and popups outside the window

**Need.** Right-click menus wider and taller than the 28-point strip they open from; tooltips likewise.

**Today.** WinUI's `MenuFlyout` and `ToolTip` draw inside the window and are cut off at its edge. Landed: `Platform::show_popup_menu`, the host's own menu at the pointer, which the winit host implements on macOS with `NSMenu`; edged2 describes each menu and shows it through that call. Tooltips still draw in the window.

**Flutter.** `PlatformMenuBar` is native on macOS; context menus are in-window overlays, and the windowing work above gives overlays a `PopupWindow` of their own.

**Remaining.** Once `PopupWindow` exists, WinUI's `Flyout`, `MenuFlyout` and `ToolTip` render into a popup window when the owner supports it and fall back to the in-window overlay otherwise.

## 3. Pointer arrival and departure

**The pointer in a window that is never key.** Landed. winit's macOS view tracks the mouse with a legacy tracking rect, which reports only entry and exit, and relies on `acceptsMouseMovedEvents` for moves, which AppKit delivers to the key window alone; a panel that never activates heard the pointer enter and leave but never move, and the host reported the entry at the pointer's last known position. Flutter's macOS embedder has the same as its `mouseTrackingMode`, `inKeyWindow` by default and `always` for views like this one. The host now adds an `NSTrackingArea` with `mouseEnteredAndExited | mouseMoved | activeAlways | inVisibleRect` to winit's view for a window created with `activating: false`, and reads the pointer's position from AppKit on entry.

Landed: the winit host adds the pointer before its first hover and removes it when the cursor leaves the window, as Flutter's macOS embedder does, so `MouseRegion` enter and exit fire. AppKit's own enter and exit events go astray while a window resizes under a still pointer, so the panel confirms the pointer's departure itself: while it is out it reads the pointer's location every 110 ms and slides in after two misses, which is also its grace for crossing a corner. That is the app's policy, not a host gap.

## 4. Host callbacks reaching the app

Landed: `AsyncApp::post` queues a closure for the next checkpoint and wakes the host, so a native callback never borrows the `App`, including one delivered during a menu's own event loop. edged2's watcher posts every change.

## 5. What an app has to name from the layer crates

Landed: `inset` re-exports the painting value types (`BorderRadius`, `Radius`, `Border`, `BorderSide`, `BorderStyle`, `BoxShadow`, `BoxShape`, `Clip`, `Axis`, `EdgeInsets`), `inset-widgets` re-exports the flex and stack enums from rendering as Flutter's `basic.dart` does, and `state_accessors!` refers through its own crate, so an app depends on `inset` alone.

## 6. Nightly

Every app with a `State` writes `#![feature(arbitrary_self_types)]`. Worth stating in the README next to the nightly requirement.

## What worked without friction

Entities and `observe` for the desktop model; `Timer` for the rescan, badge and drag-to-Space sequencing; implicit animations (`AnimatedContainer`, `AnimatedOpacity`) for the slide, the hover fills and the moving Space highlight; `MouseRegion`, `IgnorePointer`, `OverflowBox` for the strip; keyed per-screen subtrees; WinUI's `CommonStates`, `Button`, `InfoBadge`, `ToolTip`; `Image::memory` for icons; `cargo inset build macos` with a development signature so the Accessibility grant survives rebuilds.
