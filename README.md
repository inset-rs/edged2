<p align="center">
  <img src="https://edged2.app/edged.png" alt="Edged 2 logo" width="96" height="96">
</p>

<h1 align="center">Edged 2</h1>

<p align="center">
  A window switcher for macOS, built with <a href="https://github.com/inset-rs/inset">Inset</a>.
</p>

<p align="center">
  <a href="https://edged2.app">Website</a> · <a href="https://edged2.app/download">Download</a>
</p>

- Switch between apps and windows from a panel at either screen edge, grouped by Space.
- Preview windows, including minimized windows and windows on another Space.
- Move and resize windows by holding a shortcut and moving the pointer.
- Use the ring to place windows in halves, quarters, or a custom direction assignment.

Accessibility access is required to control windows. Screen Recording access is required for window previews.

## Demos

### Switch windows

https://edged2.app/videos/demo-panel.mp4

### Move and resize

https://edged2.app/videos/demo-resize.mp4

### Place with the ring

https://edged2.app/videos/demo-ring.mp4

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

[MIT](LICENSE)
