# Edged 2 architecture

Edged 2 separates macOS integration, application state, and presentation. These boundaries are implemented today; the [earlier extraction proposal](archive/architecture.md) is preserved for context.

## Crates

| Crate | Responsibility |
| --- | --- |
| `edged-macos` | Screens, Spaces, applications, windows, notifications, permissions, captures, and native window configuration. Uses objc2 internally and exposes Rust types. |
| `edged-core` | Application state and behavior using `inset-foundation` entities. Owns timers, watchers, and subscriptions. |
| `edged` | Inset widgets, inset-winui controls, app-owned windows, and rendering of window pictures. |

Only `edged-macos` may use unsafe code. SkyLight integration uses a private macOS framework, so Space operations need verification when macOS changes.

## State and updates

`Core::start` creates the shared entities before the widget tree. A widget reading an entity during build subscribes to its updates. Callbacks use entity methods to change the app. Widgets retain presentation state, such as an open flyout or the selected settings section.

Entities call `notify` for changed state and emit typed events for transitions. Native callbacks use `AsyncApp::post` to reach the app at a checkpoint rather than borrowing it during an AppKit callback.

| Entity | Owns |
| --- | --- |
| `Desktop` | The application/window model, platform watcher, reconciliation, and incremental reads |
| `Permissions` | Permission state and polling while access is requested |
| `Appearance` | System colors and accessibility appearance settings |
| `Settings` | Persisted options and requests to open the settings window |
| `Clearing` | Keeping windows clear of visible panel strips |
| `Previews` | Hover selection, capture scheduling, and retained window pictures |
| `Grab` | Shortcut-driven movement, resizing, and ring selection |

## Interface and native windows

`windows.rs` manages panel, settings, preview, ring, and destination windows. Panel widgets read the desktop model and invoke its operations. Menus use Inset's native platform menu service where they must extend outside the panel.

The preview renderer imports ScreenCaptureKit's pixel buffer through the winit host's Valo context. This is an explicit host dependency: the imported texture must use the same graphics device as the window that displays it.

Settings demonstrations draw schematic desktops and read a snapshot of the settings. They do not call desktop actions. Their animation stages separate pointer movement from effects applied on release.

## Tests

Core tests cover pure model behavior with `AppCell`. The interface's `test_support` module is compiled only for tests and provides a headless rendered view, simulated input, and frame capture. It uses published Inset crates and preserves the source notices for the adapted harness.

See [Contributing](../CONTRIBUTING.md) for commands and [Inset gaps](inset-gaps.md) for remaining platform dependencies.
