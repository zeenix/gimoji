# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.4.0 - 2026-08-12

### Added
- ✨ Add a WebAssembly frontend painting through a Canvas-2D backend.
- ✨ Drive the picker via a shared `App` state machine in gimoji-core.

### Changed
- 🏗️ Restructure into a Cargo workspace with a shared core library.

### Documentation
- 📝 Round out gimoji-core's crates.io metadata.

### Fixed
- 🚑️ Restore emojis.json at the repo root.
- 🩹 Fall back to the crate copy when fetching emojis.json.
- 🐛 Stop arrow keys panicking when the filter matches nothing.

### Removed
- 🔥 Replace the JS-templated bundle script with serve-web.sh.
