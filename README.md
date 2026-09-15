# Edged, on Inset

A window switcher that lives at the screen's edge: a strip at the right of every screen, or the left, that slides out to the running applications and their windows, grouped by Space, when the pointer reaches it. A re-implementation of [Edged](https://github.com/tyxu/Edged) with [Inset](https://github.com/inset-rs/inset).

Three crates, described in [docs/architecture.md](docs/architecture.md):

- `edged-macos`: what macOS knows about the desktop — screens, Spaces, applications, windows, Dock badges, permissions, launch at login, the appearance — and the few things the panel does to it: focus, close, minimize, move a window to another desktop, switch Space. The only crate that writes `unsafe`.
- `edged-core`: the app, as entities on Inset's foundation: the desktop kept current, the permissions polled until granted, the appearance, the settings. No widgets.
- `edged`: the panel, on Inset with the WinUI kit for its controls. It reads the entities and calls their methods.

## Run

Inset requires nightly Rust; `rust-toolchain.toml` selects it. Inset comes from crates.io; the WinUI kit checkout is expected beside this one as `../inset-winui`.

```sh
cargo install inset-cli
cargo inset run -p edged
```

`cargo inset run` builds `Edged.app` and runs it in place, signed with the Apple Development certificate from your keychain, so the Accessibility grant macOS asks for on first run survives rebuilds. Plain `cargo run -p edged` runs the bare binary; macOS then ties the grant to that binary.

The first thing the panel shows is the Accessibility request: without that permission no application reports any window.

`cargo inset build macos -p edged` writes the bundle to `target/inset/macos/release/`.

## What is here

- One panel window per screen at its right edge, or its left, slid in to a 28-point strip and sliding out to 250 points when the pointer reaches it, over a tenth of a second, easing as Edged does; as tall as its content and centred on the screen. The icons stay at the edge and the titles unfold beside them, or, as Edged did, the whole panel slides out icons first; both are settings. While a full-screen window owns the screen the panel tucks to a hair inside the edge, invisible, and comes out when the pointer reaches the edge.
- Applications without windows, then each Space of the screen with its windows by application and then by title, so an application's windows sit together and nothing shifts; the Space the screen is showing carries a moving highlight and a filled mark. A window an application keeps ordered out, as a mail client keeps its closed main window, is no window: the window server's own list of what is on a Space or minimized decides.
- Click focuses a window or activates an application. The heading of a Space switches to it. Right-click opens the system's menu through Inset's platform service: Close, Minimize or Restore or Leave Full Screen, Quit, and Move to Desktop *n* for the other desktops of that screen, done the way a hand would (hold the title bar, switch Space, let go).
- Icons at the screen's edge with the titles unfurling beside them, so nothing moves as the panel opens; Dock badges over the icons, minimized windows and hidden applications dimmed, a ring on the window being looked at, tooltips with the full title.
- Live updates from macOS: launches, quits, activation, window creation, titles, minimizing, moves, Space and screen changes, gathered for a moment and applied together. Every five seconds the window server's own list, one call that covers every Space, says which applications have windows the panel has not seen, and only those are read again. Reads run a few milliseconds per turn, so the panel is up before the windows are in and never freezes for them. Windows that cover a strip are narrowed off it.
- Rest the pointer on a window's row and a picture of the window appears where the window would be, framed and named, even for a window on another Space or minimized; it needs Screen Recording access, asked for from the panel's menu.
- A settings window with General, Panel, Previews, Move, Resize, and Ring sections. Move and Resize have independent enable switches, shortcuts, and screen-edge settings. Desktop illustrations show the selected setup and replay its behavior without moving real windows. Each ring direction has an anchored destination menu in its compass position. Previews respect Reduce Motion; dependent controls retain their settings when a feature is disabled.
- Hold ⌃⇧ and the window under the pointer moves with it, no click needed; hold ⌥⇧ and its bottom-right corner follows the pointer instead, or the corner nearest the pointer if set so. The window comes to the front as it moves. Hold ⌃⌘ and a ring of zones opens at the pointer: move it up to fill the screen, down for 80% centred, sideways for a half, diagonally for a quarter, and let go to put the window there, as Loop does; a frame shows where that is before you let go, and each direction's zone is yours to set. A moved or resized window stops at the screen's edge, which lines it up, unless you turn that off. The chords are set in the settings, where moving and resizing, and the ring on its own, can be turned off.
- Launch at Login and Quit in the panel's own menu.

## What stands between this and Edged

The panels are real windows: one per screen, bare, floating, on every Space, see-through onto Liquid Glass, sliding between the strip and the full width with AppKit's own animation, and as tall as their content. One thing remains, on the host side and listed in [docs/inset-gaps.md](docs/inset-gaps.md): clicking a panel activates Edged. A window that never takes focus must be an `NSPanel`, which the winit host cannot make; that waits on adopting an app-made window.

Menu-bar item listing, which Edged also does, is left out: macOS now shows hidden menu-bar items itself.
