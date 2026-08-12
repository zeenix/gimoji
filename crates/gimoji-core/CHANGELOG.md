# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.4.0 - 2026-08-12

### Added
- ✨ Add a WebAssembly frontend painting through a Canvas-2D backend.
- ✨ Drive the picker via a shared `App` state machine in gimoji-core.
- ✨ Only give up on adding emoji prefix if one is already present. #32
- ✨ Copy the selected emoji to the clipboard. #7

### Changed
- 🏗️ Restructure into a Cargo workspace with a shared core library.
- 💄 Replace screenshot with a screencast demo.

### Documentation
- 📝 Round out gimoji-core's crates.io metadata.
- 📝 correct footnote notation for new link.
- 📝 document how to use `gimoji` with `lefthook`.
- 📝 Be specific about git commit message hook into docs.
- 📝 Correct minimum Fedora version.
- 📝 Drop `Core` from Fedora's name.
- 📝 Add Fedora installation instructions. #25
- 📝 Update README as par the new emoji handling mechanism/philosophy.
- 📝 Update README with correct --init flag.
- 📝 Specify path to the screenshot.
- 📝 Fix build status badge in README.
- 📝 Add README.

### Fixed
- 🚑️ Restore emojis.json at the repo root.
- 🐛 Stop arrow keys panicking when the filter matches nothing.

### Other
- Fix minor typos.
- ✏️  A minor formatting fix.
- Fix typo in link name.

### Removed
- 🔥 Replace the JS-templated bundle script with serve-web.sh.
- 🔥 Remove `gitmoji` badge.
