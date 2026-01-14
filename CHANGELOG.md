# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.2.0 - 2026-01-14

### Added
- ✨ Add flag to output selection to stdout (#223).

### CI
- 💚 Consistent use of PAT.
- 💚 Drop setting of repository labels.
- 💚 More robust GH token handling in emoji update job.

### Changed
- 💄 Really fix the kbd vs mouse selection conflict.
- 💄 Allow kbd nav when mouse is over the emoji list.
- 💄 Show the copied emoji in the notification.
- 💄 Copy the emoji instead of its code.
- 💄 Consolidate selection with mouse and kbd.
- 💄 GH action to create a web interface.
- 🎨 Minor formatting fix.
- 🚚 Add emoji for "fuzzing": 🐰. #190

### Dependencies
- ⬆️  Update serde_json to v1.0.149 (#298).
- ⬆️  Update clap to v4.5.54 (#296).
- ⬆️  Update ratatui to 0.30.0 (#293).
- ⬆️  Update peter-evans/create-pull-request action to v8.
- ⬆️  Update actions/checkout action to v6.
- ⬆️  Update regex to v1.12.2 (#279).
- ⬆️  Update serde to v1.0.228 (#277).
- ⬆️  Update arboard to v3.6.1 (#261).
- ⬆️  Update actions/checkout action to v5.
- ⬆️  Update terminal-light to v1.8.0 (#225).
- ⬆️  Update nix to 0.30 (#224).
- ⬆️  Update crossterm to 0.29.0 (#219).
- ⬆️  Update databake to 0.2.0 (#189).

### Documentation
- 📝 correct footnote notation for new link.

### Fixed
- 🐛 Fix action to crate web interface.

### Other
- 🤖 Add release-plz configuration.
- 🚨 Remove an unused import.
- 🤖 Shell script for generating HTML.
- 🚸 Always keep search input field focused.
- 🚨 Fix against latest clippy.
- 🔄 Update emoji database from gitmoji upstream.
- 🤖 Automate emoji DB update.
- 🤖 Add CLAUDE.md.
