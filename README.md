# Edged 2

A window switcher for macOS, built with [Inset](https://github.com/inset-rs/inset).

- [Website](https://edged2.app)
- [Download](https://edged2.app/download)

- Switch between apps and windows from a panel at either screen edge, grouped by Space.
- Preview windows, including minimized windows and windows on another Space.
- Move and resize windows by holding a shortcut and moving the pointer.
- Use the ring to place windows in halves, quarters, or a custom direction assignment.

Accessibility access is required to control windows. Screen Recording access is required for window previews.

## Build and run

Use macOS with Rust installed. The repository's `rust-toolchain.toml` selects the required nightly toolchain.


```sh
git clone https://github.com/inset-rs/edged2.git
cd edged2
cargo install inset-cli
cargo inset run -p edged
```

To build a release app, run `cargo inset build macos -p edged`. The bundle is written to `target/inset/macos/release/`.

See [Contributing](CONTRIBUTING.md) for checks and signing notes, [Design](docs/design.md) for intended behavior, and [Architecture](docs/architecture.md) for the code structure.

## License

[MIT](LICENSE) · Copyright (c) 2026 tyxu.
