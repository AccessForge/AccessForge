# Contributing to AccessForge

Thanks for your interest in contributing. AccessForge is a community project — all contributions are welcome, no matter the size.

## Ways to contribute

- **Build a mod** — pick a game, create an accessibility mod, publish it with AccessForge
- **Report bugs** — open an issue with steps to reproduce
- **Fix bugs** — check the issues for things to work on
- **Improve docs** — better guides help everyone
- **Add features** — see the "Future (v2+)" section in [progress.md](docs/progress.md) for ideas

## Development setup

### Prerequisites

- Rust (latest stable via [rustup](https://rustup.rs))
- CMake
- LLVM (set `LIBCLANG_PATH=C:\Program Files\LLVM\bin`)
- Visual Studio 2022 Build Tools with C++ workload and Windows 11 SDK
- NVDA (for accessibility testing)

### Build and test

```
cargo build
cargo test
```

### Run in mock mode

To test the UI without needing real mods or a network connection:

```
cargo run -- --mock
```

This loads fake mod data so you can interact with the full UI.

### Run the CLI

```
cargo run -- help
cargo run -- init
cargo run -- install
cargo run -- package
```

## Project structure

```
src/
  main.rs          — entry point, clap CLI
  manifest/        — YAML manifest parsing and types
  state/           — state.json read/write
  installer/       — loader definitions, install logic
  registry/        — GitHub discovery, manifest fetching
  steam/           — Steam game detection
  worker/          — background threads, progress messaging
  ui/              — wxDragon GUI
  cli/             — CLI commands (init, install, package)
  updater.rs       — self-update system
  path_setup.rs    — PATH management
```

For architecture details, see [design-notes.md](docs/technical/design-notes.md).

## Code style

- `cargo fmt` — run before committing
- `cargo clippy -- -D warnings` — no warnings allowed
- No `unwrap()` in production code — use `?` or `.context("message")?`
- English for logs and comments
- Keep UI and business logic separated (the `worker/` module bridges them)

## Testing accessibility

If you're making UI changes, test with NVDA:

1. Install [NVDA](https://www.nvaccess.org/download/) (free)
2. Run AccessForge with `cargo run -- --mock`
3. Tab through the UI — every element should be announced
4. Test the install flow — progress should be read out via the log list and gauge

## Pull requests

- Open an issue first for larger changes so we can discuss the approach
- Keep PRs focused — one feature or fix per PR
- Include tests for new functionality
- Make sure `cargo fmt`, `cargo clippy`, and `cargo test` all pass
- Describe what changed and why in the PR description

## License

By contributing, you agree that your contributions will be licensed under GPL-3.0, the same license as the project.
