# How Edged is put together

Edged is the reference for how an Inset app is structured, so the split has to be one worth copying: logic that knows nothing of widgets, an interface that holds no logic, and one mechanism joining them. This note is the proposal for that split, what it costs to get there from today's code, and how two features already on the horizon fit: a window preview when a row is hovered, and PowerToys-style grab-and-move.

## The three crates

```
edged-macos   what macOS knows and can do: screens, Spaces, applications, windows,
              notifications, badges, pointer, appearance, backdrop. Plain types,
              objc2 inside, no Inset. The one crate allowed unsafe.

edged-core    the app: entities on inset-foundation alone. Desktop, Permissions,
              Appearance, Clearing, and later Previews and Grab. Owns every
              timer, watcher and subscription. Testable with AppCell, no window.

edged         the interface: inset + inset-winui + edged-core. Root, panel, rows,
              sections, theme, menus, permission view. Reads entities in build,
              calls entity methods from callbacks, and that is all it does.
```

`edged-macos` stays as it is. Today `edged` holds both of the other two; the work is to carve `edged-core` out of it.

## The joining mechanism

Inset already has the gpui model, and this design uses nothing beyond it:

- A widget that reads an entity in `build` is rebuilt when that entity calls `notify`. The read is the subscription; nothing is wired by hand.
- A callback changes state through `entity.update(app, |desktop, cx| desktop.focus(window, cx))`. The interface never holds mutable state of the app's, only view state such as whether a panel is out.
- An entity that must react to another's transition, rather than its state, subscribes to a typed event: `Permissions` emits `Granted`, `Desktop` subscribes and rescans. Events are for transitions; `notify` is for state.
- Entities are created once in `edged::run`, before the widget tree, and handed to the root as handles. No widget creates an entity.
- Native callbacks reach an entity through `AsyncApp::post`, never by borrowing the app from inside a notification.

Two rules follow. Nothing in `edged-core` names a widget, a window or a colour. Nothing in `edged` calls `edged-macos` except for what concerns its own windows: putting the glass behind a panel, and reading where the pointer is relative to one.

## The entities

| Entity | State | Owns | Methods the interface calls |
|---|---|---|---|
| `Desktop` | screens, Spaces, applications with their windows, which Space each screen shows, icons | the watcher, a queue of reads done a slice per turn, the change-gathering timer, the reconcile timer, the badge timer | `refresh`, `focus(window)`, `switch_to(space)`, `move_to_space(window, space)`, the queries `windows_on`, `windowless`, `spaces_of` |
| `Permissions` | accessibility granted, screen recording granted | the poll until accessibility is granted; emits `Granted` | `request(permission)`, `open_settings`, `refresh` |
| `Appearance` | accent colour, reduce motion | refreshed on the system's appearance notification | none; read only |
| `Clearing` | nothing | the timer that narrows windows off the strips | none |
| `Settings` | launch at login, when previews show | nothing; emits `SettingsRequested` when the user asks for the window | `set_launches_at_login`, `set_preview_trigger`, `request_window` |
| `Previews` | which window the pointer rests on, the pictures taken of windows, the windows the system will picture | the rest and refresh timers | `rest_on(window)`, `picture_of(id)` |
| `Grab` | the window the keys hold, with the pointer's origin and position and the frame it started from | the input observer | none; it acts on the keys and the pointer |

`Desktop`'s queries are unit-tested where their logic is pure, such as the ordering; the rest asks macOS and is exercised by running the app. `relocate.rs` becomes `Desktop::move_to_space`, `overlap.rs` the `Clearing` entity, `watch.rs` the glue that routes each `Change` to the entity it concerns, `Change::Appearance` going to `Appearance` rather than through `Desktop`.

The interface then shrinks to presentation: `theme.rs` reads `Appearance`; the menus describe entries and call `Desktop` methods; the panel keeps only its own view state.

## Room for what comes next

**Window preview on hover.** Built as planned: the `Previews` entity keeps a picture per window with when it was taken, asks `edged-macos` for one through ScreenCaptureKit once the pointer has rested on a row, and is answered on a later turn through `post`. The interface keeps one preview window, made at start and kept off screen, that moves to the frame the window would occupy and comes forward without taking the keyboard. Screen Recording is asked for from Edged's menu. ScreenCaptureKit was chosen over the private call alt-tab-macos uses: it pictures minimized, hidden and other-Space windows on this macOS, it is public, and its forty milliseconds hide behind the rest before a preview shows; the private call is faster but the difference does not matter once per hover. The picture reaches the screen with nothing copied: the preview names its host, the winit platform, takes the renderer's image context from it and imports the buffer as a texture through valo's Apple codec, which samples the `IOSurface` where it lies.

**Grab-and-move.** Landed as a hold without a click: `edged-macos` has an `InputObserver`, global and local event monitors for the pointer's moves and the modifier keys, which Accessibility access allows, plus `Window::set_position` beside `set_size` and `window_at`, the window server's frontmost ordinary window of another process under a point. The `Grab` entity in `edged-core` is the state machine: nothing held; the move chord down, ⌥ by default, takes the window under the pointer; the pointer's travel past a dead zone brings it to the front and moves it, or with the resize chord, ⌥⌃, resizes it from the corner nearest where the hold began; the keys up let go. The chords are a `Chord` setting, the one shape any later shortcut of Edged's takes. A third chord, ⌃⌘, holds the window to arrange it: the pointer's direction from where the keys went down picks a `Zone`, eight as a compass has, and letting go puts the window in it; the ring window in `edged` reads the same `Hold` to show the zones and the one picked, and a target window frames where the window would go before the keys are let go. Each direction's zone is a setting, as is whether a moved or resized window stops at the screen's edge. The panel knows of none of this. The corner a resize drags is a setting: the bottom-right one, or the one nearest the pointer. One setting turns it off.

Both features add an entity and a window; neither touches the panel's code or the rules above. That is the test of the split.

## What a logic crate depends on

`edged-core` depends on `inset-foundation`, not on `inset`. The foundation is the entity system, the timers and the async door, and nothing else; `inset` would bring the widgets, the renderer and the host along, which a crate that never draws has no use for. An app crate depends on `inset` and sees the same types through it.

## How it went

The split landed in one pass: `edged-core` took `desktop.rs`, `watch.rs`, `overlap.rs` and `relocate.rs` from the interface, `Desktop` grew its own watcher and timers, `Permissions`, `Appearance`, `Settings` and `Clearing` became entities, and `Core::start` makes them all. The interface's files changed only where they had reached into logic; the design pass before it stayed as it was. What is left for Inset is a short "structuring an app" section in its README stating the rules above.
