# Working on Edged 2

Edged 2 is a macOS app built with published Inset and inset-winui crates. Setup is in [README.md](README.md). An original Swift Edged checkout is useful for historical comparisons but is not required to build.

## Ownership

- `crates/edged-macos` owns AppKit, Accessibility, ScreenCaptureKit, and SkyLight integration. It is the only crate allowed to use `unsafe`. Objective-C types must not cross its public boundary.
- `crates/edged-core` owns application state, timers, watchers, and subscriptions through Inset entities. It has no widget or rendering dependencies.
- `crates/edged` owns widgets and presentation state. Read entities during build and change them through entity methods. Calls to `edged-macos` here are limited to the app's own windows and rendering integration; actions on other apps go through the core model.

## Implementation and documentation

Use Inset and inset-winui controls and established mechanisms. If a required framework capability is missing, record the gap and proposed API in [docs/inset-gaps.md](docs/inset-gaps.md), using Flutter's mechanism where applicable. Propose framework changes for review instead of silently adding a workaround.

Leave blank lines between functions and distinct parts of a function. Document non-obvious types, fields, and behavior. In design notes, name the concrete type or feature first, then explain the behavior and reason in ordinary language. Avoid unexplained symbols and invented jargon.

Keep the README short and useful to users and new contributors. Put behavior and intent in `docs/design.md`, ownership and data flow in `docs/architecture.md`, and working instructions here. Archived research is historical evidence, not a current specification.

Run `cargo fmt --all --check`, `cargo test --workspace --locked`, and `cargo clippy --workspace --all-targets --locked -- -D warnings` as appropriate. See [CONTRIBUTING.md](CONTRIBUTING.md) for platform and visual checks.

Do not commit, push, package, sign, notarize, or publish unless the current task authorizes it.
