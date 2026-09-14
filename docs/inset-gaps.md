# What Edged needs from Inset

Edged's panel is a window Inset cannot make yet. This note lists each gap the port met, the shape Flutter gives the same thing where it has one, and a proposal. Nothing here is implemented in Inset; it is for review.

## 1. Windows the app creates

**Need.** One panel per screen: a non-activating floating window (clicking it must not take focus from the application the user is in), above ordinary windows, present on every Space and staying put across Space switches, placed and sized by the app (right edge, vertically centred, height from content, width sliding between 28 and 250 points in 100 ms), transparent with the system's blur behind it, left corners rounded.

**Landed.** `Platform::windowing_owner` with `WindowingOwner::create(WindowConfig)`, `HostWindow` (view, frame, animated `set_frame`, show, hide, close, native handle, close request), and `adopt` answering unsupported; `ViewCollection`, `ViewAnchor`, `Window` and `WindowScope` in inset-widgets; the winit host making windows on its next loop turn with the implicit view's device, applying Space behaviour and a vibrancy view through AppKit, and animating frames with `NSAnimationContext`. edged2 runs with no implicit window and one panel window per screen.

**Still open.** Non-activation needs an `NSPanel`, so `adopt` and a host that drives an app-made window. The panel no longer hides and shows for full-screen Spaces: it tucks to a hair and sets its own alpha through the native handle, so winit's `set_visible(true)`, which orders a window front and key, is never called on it.

**Surface alpha (valo).** Landed in valo 0.4.5: `SurfaceOptions::default().with_alpha(SurfaceAlpha::Transparent)` on the `_with_options` constructors, kept across `resize`, with `Surface::alpha()` saying what took effect. valo's output was already premultiplied at every stage, which is what CoreAnimation, Wayland and WebGPU composite, so only the mode changed. Skia and Flutter leave the layer's opacity to the host; wgpu sets it from the surface configuration on every configure, so valo has to be told. `Transparent` picks `PreMultiplied` where offered, `PostMultiplied` only on Metal (it merely clears the layer's opaque flag), then `Inherit`; WebGPU takes `PreMultiplied` unconditionally since its capabilities under-report; plain Windows windows and OpenGL offer opaque only. The winit host asks for it on every see-through window, and the panel paints no fill of its own.

**Window corners.** Settled: not a window configuration. Only macOS shapes a window through a layer mask; Windows 11 rounds every window itself with three preset sizes and no per-corner choice, and Wayland has nothing, so a `corner_radius` field would promise what two of three desktops cannot keep. The portable shape is what the app paints into a see-through surface. The one thing that needed the host's cooperation was the glass view the host puts behind the content, which is a rectangle; so the panel asks for a `Transparent` background and places the glass itself through `HostWindow::native_handle`, the AppKit view from which the window is one call away, in `edged-macos::install_backdrop`: the view is one radius wider than the window, so the corners that hang past the screen's edge are clipped square and the leading pair keep their radius. `WindowBackground::Glass` and `Blurred` stay for the common full-window case. The window shadow stays off as SDL keeps it off for transparent windows: AppKit derives it from the window's alpha and would need invalidating whenever the content changes shape.

**Glass.** `WindowBackground::Glass` uses `NSGlassEffectView` when the class exists at runtime and the vibrancy view otherwise, placed beside the content view in the window's frame. There Liquid Glass renders as its flat material, not as glass: a probe with a screen capture showed the glass whole only inside the content view. The content view is winit's, drawn through a Metal layer that wgpu adds as a sublayer of its root layer; putting the glass in as a subview beneath and moving the Metal layer back on top held in the probe but not in the app, where the drawing ended up under the glass. Left at the flat material, which reads as a blur; real glass waits on a host that owns its view, the same host the non-activating panel needs.

**Popup menus and the pointer.** Landed: a native menu runs its own event loop and keeps the release of the button that opened it, so the app would go on holding that button, with no hovers and no taps until the next press. After a popup the host wakes and reconciles its record of the buttons with what AppKit says is held down, sending the releases it missed; it does the same before every pointer move.

**Frame animation curve.** `HostWindow::set_frame` takes a duration and the host animates with AppKit's ease-in-ease-out timing, which is the curve Edged uses for its slides, over the same tenth of a second. A `curve` on the call, mapped to `CAMediaTimingFunction`, would let a window ask for another; not landed, and nothing asks for one now.

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

## 5. An image from pixels

**Need.** The window preview takes a picture through ScreenCaptureKit and has it as an `IOSurface`-backed Core Video buffer. Showing it needs an `Image` the `RawImage` widget can draw, with no codec and no copy in between.

**Flutter.** `decodeImageFromPixels(pixels, width, height, format, callback)` in `dart:ui`: raw pixels in memory in, a `ui.Image` out, shown with `RawImage`. Nothing for a buffer the GPU already holds.

**Landed, then reshaped.** The first form put a `NativeImage` enum on `Platform`, with a variant for memory and one for a Core Video buffer; that made the interface crate name Apple's types and grow a variant for every system's kind of picture. Now `Platform` is `Any` and `downcast_ref` on the trait object hands an app its host's own type; the interface keeps only `import_pixels`, Flutter's `decodeImageFromPixels`. The winit host exposes its renderer's `image_context`, on the device every window draws with, and the preview widget names that host: it imports the buffer as a texture through valo's `import_pixel_buffer`, the `IOSurface` path the Apple codec uses for decoded frames, and makes the image with `import_texture`. The texture samples the buffer where it lies and keeps it alive until wgpu is done. The facade exposes the engine layer as `inset::ui`, so the picture the `Image` widget shows is `ui::Image`, as in Flutter. The preview widget makes one image per picture taken and keeps it while that picture shows.

## 6. Windows that only show something

**Need.** The preview window must never take the keyboard and must let clicks through to what is beneath it.

**Today.** `HostWindow::show` is winit's `set_visible(true)`, which on macOS orders the window front and makes it key even for a window created with `activating: false`; and there is no way to have a window ignore the pointer. Edged does both through the native handle in `edged-macos::overlay`: `orderFrontRegardless`, `orderOut`, `setIgnoresMouseEvents`.

**Proposal.** For a window created with `activating: false`, the host's `show` and `hide` order it front and out without making it key. And `HostWindow::set_ignores_pointer(bool)`, winit's `set_cursor_hittest` on every desktop, so a window that only shows something says so once. Not landed; the panel meanwhile never hides, and the preview orders itself through the native handle.

## 7. What an app has to name from the layer crates

Landed: `inset` re-exports the painting value types (`BorderRadius`, `Radius`, `Border`, `BorderSide`, `BorderStyle`, `BoxShadow`, `BoxShape`, `Clip`, `Axis`, `EdgeInsets`), `inset-widgets` re-exports the flex and stack enums from rendering as Flutter's `basic.dart` does, and `state_accessors!` refers through its own crate, so an app depends on `inset` alone.

## 8. Frames the host did not ask for

**Found.** With the app idle, the host rendered a full frame of every window at the speed of rendering, and the framework's own trace showed no request in front of any of them. A playground harness ruled out every window configuration the panels use: transparent, non-activating, blur sibling, tracking area, present-with-transaction, all at zero draws a second. The loop was the host's own: on every frame, a view whose surface gives no texture asked winit for another redraw, and a surface gives no texture while its window is occluded, which the preview window is from the moment it is made hidden. Each retry rendered every window again. It never went through the framework, so no trace line preceded it.

**Landed.** A view that cannot draw now owes the frame and draws it when winit reports the window seen again, instead of asking for a redraw at once. Two measures taken against the storm before its cause was found, a layer-contents redraw policy of `Never` on every winit view and a gate that dropped the redraws the host had not asked winit for, were removed once it was: winit's own `request_redraw` never passes through `drawRect:`, so the policy only silenced AppKit's occasional draws, and the gate would have dropped a real expose on a host without a compositor. A redraw from the system now draws a frame, as before; the follow-up that would make that cheap is a `View::present` that hands the host a picture it can keep and present again.

## 9. What a frame costs the process after it has been idle

**Found.** With the icons fixed, a fresh instance still showed some 400 MB of graphics memory appear for a second or two with every frame drawn after a pause, and the first frame after a pause took ten to fifteen milliseconds where the next ones took three. A harness in the playground (`valo-passes`) rendered the panel's picture off screen with valo and bare wgpu passes, an idle gap before each: the Metal driver lets a process's GPU context go after about a second and a half of idleness and rebuilds it on the next submit. A bare wgpu pass with four-sample attachments, transient or not, brings back about 230 MB and costs about 10 ms; a valo frame of the same picture brings back about 400 MB, so valo's context adds some 170 MB to what the driver restores; the number of live pipelines does not change it, nor does the target's size. valo's own accounting is right: its transient attachments are memoryless, its pools hold a few megabytes, and layers add their resolve textures only. Neither the Liquid Glass nor the vibrancy backdrop costs anything when a window's content changes, measured with a bare AppKit window. A Skia or Flutter app on Metal pays the same kind of restore, only smaller.

**Landed.** A frame log in the winit host served the measurements and was removed once they were done. A resize that reports the same geometry again draws nothing. A redraw the host did not ask winit for, which AppKit sends for every step of a frame animation, no longer runs a framework frame: only a window that owes a frame gets one. The rows' pill is the one widget that reads the width the window shows, so a step of the slide rebuilds sixty pills and not sixty rows.

**Landed.** Both. A valo display list now compares by content, a list being equal to itself without a comparison and a nested one compared the same way, so a scene over the pictures its last frame retained compares in the time of its own few commands; `View::present` hands the host a shared picture, which it keeps, presents again when a window is seen again without a framework frame, and does not draw when it equals what the surface shows, so a slide step draws the panel that moved and nothing else. And valo's upload ring writes its blocks through a map on a device whose primary buffers are host-visible (`MAPPABLE_PRIMARY_BUFFERS`, which unified memory offers and the host now asks for), with the queue-write ring kept elsewhere; the `bisect` binary in the harness showed a bare pass with two `queue.write_buffer` calls, or a mapped staging buffer copied in the encoder, restoring at 415 MB after idle and the same bytes written through mapped primary buffers at 255 MB, an empty pass's figure, while draws, dynamic offsets, stencil writes and atlas textures add nothing. Both are in valo 0.4.6, unpublished; both workspaces patch it by path until it is.

## 10. Nightly

Every app with a `State` writes `#![feature(arbitrary_self_types)]`. Worth stating in the README next to the nightly requirement.

## What worked without friction

Entities and `observe` for the desktop model; `Timer` for the rescan, badge and drag-to-Space sequencing; implicit animations (`AnimatedContainer`, `AnimatedOpacity`) for the slide, the hover fills and the moving Space highlight; `MouseRegion`, `IgnorePointer`, `OverflowBox` for the strip; keyed per-screen subtrees; WinUI's `CommonStates`, `Button`, `InfoBadge`, `ToolTip`; `Image::memory` for icons; `cargo inset build macos` with a development signature so the Accessibility grant survives rebuilds.
