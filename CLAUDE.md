# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this
repository.

## Project Overview

Gimoji is a Rust CLI tool for adding emojis to git commit messages. It provides a terminal UI for
emoji selection and integrates with git as a prepare-commit-msg hook.

## Development Commands

- **Build**: `cargo build` or `cargo build --release`
- **Run**: `cargo run` (launches emoji picker UI)
- **Run with args**: `cargo run -- --help` (see all options)
- **Test**: `cargo test`
- **Install locally**: `cargo install --path .`
- **Format code**: `cargo fmt`
- **Lint**: `cargo clippy`

## Architecture

### Core Components

- **main.rs**: Entry point with CLI argument parsing, hook installation, and main application flow
- **emoji.rs**: Emoji data structure and search functionality. Contains pre-compiled emoji database
  from emojis.json
- **terminal.rs**: Terminal setup and management using crossterm/ratatui
- **selection_view.rs**: Grid-based emoji selection UI component
- **search_entry.rs**: Text search input component with filtering
- **colors.rs**: Color scheme definitions for light/dark terminal themes

### Key Features

- Full-screen terminal UI using ratatui for emoji selection
- Pre-compiled emoji database (no runtime downloads)
- Git hook integration for automatic emoji prompting
- Clipboard integration for standalone usage
- Auto-detection of terminal color scheme
- Search/filter functionality across emoji names, codes, and descriptions

### Build Process

The build.rs script processes emojis.json at compile time using databake to generate a static EMOJIS
array, eliminating runtime JSON parsing and network dependencies.

### Usage Modes

1. **Standalone**: `gimoji` - launches picker, copies selection to clipboard
2. **Git hook**: `gimoji --hook <commit-file>` - prepends emoji to commit message
3. **Initialize**: `gimoji --init` - installs git prepare-commit-msg hook
4. **Stdout**: `gimoji --stdout` - outputs selection to stdout instead of clipboard
