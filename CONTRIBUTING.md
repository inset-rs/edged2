# Contributing to Edged 2

Follow the checkout instructions in [README.md](README.md). Inset and inset-winui are published dependencies; the lockfile records the resolved versions. No framework checkout is required.

## Checks

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Core tests exercise the app model without interacting with the desktop. Interface tests render through a local headless GPU fixture and need an available Metal adapter. Set `EDGED_CAPTURE_DIR` to save their PNG captures. CI runs formatting, core/platform tests, interface test compilation, and Clippy on macOS. Rendered interface tests run locally on a Mac with Metal; the standard hosted runner is not assumed to provide a GPU.

Use `cargo inset run -p edged` for interactive testing. Check both panel edges, multiple Spaces, minimized windows, shortcut release, narrow settings windows, and Reduce Motion when a change affects them. Do not change real user preferences from automated tests.

## Signing and distribution

`cargo inset run` uses an available Apple Development identity so Accessibility permission can survive rebuilds. A fresh checkout can compile and run tests without a signing identity. Packaging and permission behavior should be tested separately on a developer machine.

Distribution uses a Developer ID Application identity and Apple notarization credentials. Supply these through the environment or your credential store, never tracked files. Building, signing, notarizing, publishing, and changing a user's installed app are separate from the checks above.

## Making changes

Keep changes focused and explain the behavior changed and how it was verified. Read [AGENTS.md](AGENTS.md) for repository conventions, [architecture](docs/architecture.md) for ownership boundaries, and [design](docs/design.md) for interaction behavior.

When a framework capability is missing, describe the gap in [docs/inset-gaps.md](docs/inset-gaps.md) before adding an app-specific substitute. Prefer established Inset or Flutter mechanisms.
