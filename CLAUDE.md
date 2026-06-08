# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this
repository.

## Project Overview

Gimoji is a Rust CLI tool for adding emojis to git commit messages. It provides a terminal UI for
emoji selection and integrates with git as a prepare-commit-msg hook. A WebAssembly build of the
same picker is also deployed to GitHub Pages.

## Development Commands

Prefer `just <recipe>` from the project root — it hides the native/wasm split.
Run `just` with no args for the full list.

- **Build native**: `just build`
- **Run native**: `just run` (or `just run -- --help` for args)
- **Test native**: `just test`
- **Install native locally**: `just install`
- **Format**: `just fmt` (rewrites) or `just fmt-check` (CI parity)
- **Lint**: `just lint` (clippy native + wasm with `-D warnings`)
- **Build WASM (debug)**: `just web-build`
- **Build WASM (size-optimised)**: `just web-release`
- **Serve WASM locally**: `just web-serve` (default port 8000)

Raw cargo also works:

- Native commands run from the repo root use the workspace `Cargo.lock`.
- WASM commands run from `crates/gimoji-web/` and pick up
  `wasm32-unknown-unknown` automatically via `crates/gimoji-web/.cargo/config.toml`.

## Architecture

### Repository Layout

Two cargo roots, both checked into the same repo:

- **Native workspace** (`Cargo.toml` at the repo root) — members:
  - **`crates/gimoji-core`** (library, published): the picker state machine
    (`App`, `Action`, `Outcome`), emoji database (`EMOJIS`), color palettes,
    ratatui widgets (`SearchEntry`, `SelectionView`, `Toast`), and the
    `Clipboard` trait used by both frontends.
  - **`crates/gimoji`** (binary, published, name unchanged): the native CLI
    that wires `crossterm` + `arboard` into the `App`. Entry point:
    `crates/gimoji/src/main.rs`.
- **Standalone wasm package** (`crates/gimoji-web/Cargo.toml`, its own
  `Cargo.lock`): cdylib that renders the picker via `ratatui-wgpu` (WebGL2)
  into a `<canvas>`. Entry point: `crates/gimoji-web/src/lib.rs`. Bundled
  fonts at `crates/gimoji-web/assets/fonts/`. Depends on `gimoji-core` via
  path; excluded from the root workspace because its dep tree
  (`ratatui-wgpu` `web` feature, `web-sys`, `wasm-bindgen`) only compiles
  for `wasm32-unknown-unknown`.

### Picker behaviour

The picker is a single `App::handle(Action) -> Outcome` state machine in
`gimoji-core`. Backends produce `Action`s from their input source
(crossterm `KeyEvent` for native, DOM `KeyboardEvent`/`PointerEvent` for
web) and interpret `Outcome::Picked(s)` according to the frontend
semantics (native copies-then-exits; web copies-and-toasts).

### Key Features

- Full-screen terminal UI using ratatui for emoji selection
- Pre-compiled emoji database (no runtime downloads)
- Git hook integration for automatic emoji prompting
- Clipboard integration for standalone usage
- Auto-detection of terminal color scheme
- Search/filter functionality across emoji names, codes, and descriptions

### Build Process

The `crates/gimoji-core/build.rs` script processes `emojis.json` at compile time using databake to
generate a static `EMOJIS` array, eliminating runtime JSON parsing and network dependencies.

### Usage Modes

1. **Standalone**: `gimoji` - launches picker, copies selection to clipboard
2. **Git hook**: `gimoji --hook <commit-file>` - prepends emoji to commit message
3. **Initialize**: `gimoji --init` - installs git prepare-commit-msg hook
4. **Stdout**: `gimoji --stdout` - outputs selection to stdout instead of clipboard
