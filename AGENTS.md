# Edged on Inset

An implementation of Edged (`/Users/mac/code/Edged`, Swift) with Inset (`../inset-rs`) and the WinUI kit (`../inset-winui`), both used from their checkouts.

- `crates/edged-macos` wraps AppKit, the Accessibility API and SkyLight. It is the only crate that may write `unsafe`. Every public type is plain Rust; no Objective-C type crosses its boundary.
- `crates/edged-core` is the app: entities on `inset-foundation` alone. They own every timer, watcher and subscription, and nothing in them names a widget, a window or a colour. Tests run with `AppCell` and no display.
- `crates/edged` is the interface. It reads entities in `build`, which subscribes it to their changes, and changes them through their methods from callbacks. It calls `edged-macos` only for its own windows: the glass behind a panel and where the pointer is relative to one. Actions on the desktop go through `Desktop`'s methods, never straight to `edged-macos`.

`docs/architecture.md` says why the split is drawn there and how the next features fit it.

When the app needs something Inset does not offer, do not work around it here: write down what the framework would add, in `docs/inset-gaps.md`, with the shape Flutter gives it where Flutter has one, and leave the app in the state that best shows the gap. Framework changes are proposed for review, not made from this repository.

```sh
cargo inset run -p edged
cargo test --workspace
cargo clippy --workspace
```
