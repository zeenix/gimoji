# Consolidating gimoji's web and native interfaces via a shared Rust core

**Status:** Design — pending implementation plan
**Date:** 2026-06-06
**Branch:** `consolodate-web-native`

## Problem

gimoji today has two divergent UIs sharing nothing:

- A native Rust TUI (`src/`, ~300 LOC, ratatui + crossterm + arboard).
- A hand-written HTML/CSS/JS reimplementation on the `web-interface` branch that
  fetches `emojis.json` at runtime and re-renders the picker in the DOM.

Every UI behaviour — selection, search, color scheme, keyboard handling — exists twice
and drifts over time. The goal is to eliminate the duplication by producing the web
interface from the same Rust source, compiled to WebAssembly, while keeping the native
CLI behaviourally unchanged.

## Goals

- Single source of truth in Rust for the picker UI: widgets, search/filter, keyboard
  semantics, color scheme, and (where possible) the visual rendering.
- The web version is a fully working tool: visitors search, navigate with keyboard or
  pointer, pick an emoji, and have it copied to their clipboard. No regression versus
  the existing `gh-pages` JS site.
- The native crate stays `gimoji` on crates.io. `cargo install gimoji` continues to
  work, behaves identically to today's binary, ships with the same flags
  (`--init`, `--hook`, `--stdout`, `--color-scheme`).
- Pixel-faithful look across native and web — same box-drawing, same monospace, same
  color emoji glyphs — accepting the cost of bundling a font.

## Non-goals

- Mobile-first redesign. The web version should work on mobile (touch + small screens)
  but the design language stays "terminal in a browser."
- Server-side anything. The web build is a static bundle deployable to GitHub Pages.
- Service-worker / offline support. Not in scope for v1.
- Reading from clipboard, image clipboard, or any clipboard operation beyond
  programmatic `writeText`.
- Replacing the git-hook installer or any native-only feature with a web equivalent.

## Approach summary

A Cargo workspace introduces `gimoji-core` (shared library, all picker logic and
widgets, no native deps) consumed by the existing `gimoji` binary (native, unchanged
externally) and a new `gimoji-web` cdylib (WASM, rendered by `ratatui-wgpu` into a
single full-page `<canvas>`). Backends differ only in how raw input maps to a
backend-agnostic `Action` enum and in how `Outcome::Picked` translates to a
side-effect (native: copy + print + exit; web: copy + toast).

`ratatui-wgpu` is chosen for the web backend with the `web` feature enabled so it
falls back to WebGL2 where WebGPU is unavailable (notably Firefox on Linux, Android,
and Intel macOS as of mid-2026; Firefox stable shipped WebGPU on Windows in v142 and
macOS Apple Silicon in v147, with Linux and Android targeted for later in 2026).
WebGL2 has universal browser support, eliminating the WebGPU availability concern.

`ratzilla` (a DOM-rendering alternative) was considered and rejected: the user
preferred the maximum-Rust path, perceives `ratzilla` as less actively maintained,
and the pixel-fidelity goal pushes toward a real graphics backend rather than
styled DOM. Native emoji font rendering (which DOM gives for free) is sacrificed
for visual consistency across browsers/OSes.

## Crate layout

```
crates/
  gimoji-core/
    Cargo.toml         deps: ratatui, regex; no native deps
    build.rs           bakes emojis.json -> const EMOJIS (today's build.rs, relocated)
    src/lib.rs
    src/emoji.rs       Emoji struct + EMOJIS (unchanged from today)
    src/colors.rs      Colors light()/dark() (unchanged)
    src/search_entry.rs   SearchEntry widget (unchanged)
    src/selection_view.rs SelectionView / FilteredView (mostly unchanged;
                          exposes rendered-row rects after render — see Data Flow)
    src/toast.rs       NEW: Toast widget (transient overlay)
    src/app.rs         NEW: App, Action, Outcome, Clipboard trait
  gimoji/
    Cargo.toml         name = "gimoji" (crates.io name preserved)
                       deps: gimoji-core, clap, crossterm, arboard, terminal-light,
                             nix (unix)
    src/main.rs        args parsing, hook install, hook mode, picker mode loop
    src/terminal.rs    crossterm Terminal wrapper (unchanged)
    src/clipboard.rs   arboard-backed impl of gimoji_core::Clipboard
  gimoji-web/
    Cargo.toml         publish = false
                       deps: gimoji-core, ratatui-wgpu (features = ["web"]),
                             wasm-bindgen, web-sys, js-sys,
                             console_error_panic_hook
    build.rs           font subsetting; emits subset OTF as include_bytes!
    assets/fonts/      NotoColorEmoji.ttf, JetBrainsMono-Regular.ttf, NOTICE
    src/lib.rs         wasm-bindgen(start), canvas setup, input wiring, rAF loop
    src/clipboard.rs   web_sys/inline-JS impl of gimoji_core::Clipboard
    src/color_scheme.rs   prefers-color-scheme detection + change subscription
    src/input.rs       keydown / pointerdown / resize handlers
    web/
      index.html       minimal shell; loads wasm-bindgen module
      style.css        full-viewport canvas, touch-action: none
```

The repo root becomes a Cargo workspace with `members = ["crates/*"]`. The native
binary's published name stays `gimoji`; its version bumps to 1.4.0 to mark the
internal restructuring.

## Components and data flow

### `App` and `Action` (the shared state machine)

```rust
pub enum Action {
    Append(char),       // search input character
    Backspace,
    ClearSearch,        // Esc when search is non-empty
    MoveUp,
    MoveDown,
    PickFocused,        // Enter on highlighted row
    PickAt(usize),      // click on row N (web-only producer)
    Cancel,             // Esc when search is empty; Ctrl+C
}

pub enum Outcome {
    Continue,
    Picked(String),
    Cancelled,
}

pub struct App<'a> {
    search: SearchEntry<'a>,
    selection: SelectionView<'a>,
    toast: Option<Toast>,
    colors: &'a Colors,
    last_rendered_rows: Vec<Rect>,   // for web pointer hit-testing
}

impl<'a> App<'a> {
    pub fn new(emojis: &'static [Emoji], colors: &'a Colors) -> Self;
    pub fn handle(&mut self, action: Action) -> Outcome;
    pub fn tick(&mut self, dt: Duration);
    pub fn render(&mut self, frame: &mut ratatui::Frame);
    pub fn row_rect(&self, row_index: usize) -> Option<Rect>;
}
```

`Action` is the boundary between "input origin" (per-backend) and "behaviour"
(shared). The native backend produces `Action`s from `crossterm::event::KeyEvent`;
the web backend produces them from DOM `KeyboardEvent` and `PointerEvent`. The
behaviour of every `Action` is defined exactly once, in `App::handle`.

`Outcome` lets `handle` communicate "user picked X" or "user cancelled" without
the core deciding what those mean. Native maps `Picked(s)` to copy+print+exit and
`Cancelled` to plain exit. Web maps `Picked(s)` to copy+toast+keep-looping and
`Cancelled` to no-op (browsers can't reliably close tabs; clearing the search is
already what `ClearSearch` does, so `Cancel` on web just stays put).

`tick(dt)` advances time-dependent UI state (the toast fade). The native binary
never calls it — native exits the moment `Picked` is returned, before any frame
where a toast would render. The web binary calls it at the top of every
`requestAnimationFrame`.

`row_rect(i)` is a new accessor needed by the web backend for pointer hit-testing.
After `render()`, the `App` records the rect of each visible emoji row; the web
backend's `pointerdown` handler walks them to translate a click position to a row
index. Native ignores this method.

### `Clipboard` trait

```rust
pub trait Clipboard {
    type Error: std::error::Error;
    fn copy(&mut self, text: &str) -> Result<(), Self::Error>;
}
```

Defined in `gimoji-core` so `App` *could* be tested against a fake. Implementations
live in the backend crates: native uses `arboard` and exits-after-set on
Linux/BSD (preserving today's daemon-then-set semantics); web calls
`navigator.clipboard.writeText` via a small inline-JS shim to avoid the unstable
`web_sys` Clipboard binding. The web implementation is fire-and-forget: it returns
`Ok(())` synchronously after kicking off the Promise; if it ultimately rejects the
user already moved on.

Crucially, the web `copy()` must be invoked from within a user-gesture event
handler — browsers reject programmatic clipboard writes otherwise. The design
satisfies this: `copy()` is called inside `app.handle(Action::PickFocused)` or
`Action::PickAt`, which run inside the `keydown` or `pointerdown` handler.

### Data flow per backend

**Native** (`gimoji/src/main.rs`, post-refactor):

```
parse args ──> args == --init?           ──> install hook, exit
            ── args == --hook?           ──> read commit file, prepend, exit
            ── otherwise                 ──> picker mode:
   pick_loop:
     terminal.draw(|f| app.render(f))
     event = crossterm::event::read()
     match event_to_action(event) {
         Some(Action::Cancel) => exit(130)
         Some(action) => match app.handle(action) {
             Outcome::Continue => continue
             Outcome::Picked(s) => clipboard.copy(&s)?; println!(...); exit(0)
             Outcome::Cancelled => exit(0)
         }
         None => continue
     }
```

The shape is identical to today's loop. Lines moved around. The only behavioural
difference is that the inline `match event.code` becomes `event_to_action(event)`,
which is now a small function in `gimoji/src/main.rs` (or `gimoji-core::native` if
we want to share keycode mappings between platforms in the future — not in v1).

**Web** (`gimoji-web/src/lib.rs`):

```
wasm-bindgen(start) -> async fn run():
  set panic hook
  fetch <canvas>, sized to viewport
  detect color scheme from prefers-color-scheme
  build WgpuBackend (font bytes embedded, target = canvas)
  build ratatui Terminal
  build App, Clipboard
  wrap (app, terminal, clipboard, last_tick) in Rc<RefCell<_>>
  attach window keydown handler -> event_to_action -> drive(state, action)
  attach canvas pointerdown handler -> hit-test via app.row_rect()
                                    -> Action::PickAt -> drive(state, action)
  attach window resize handler -> terminal.resize(...)
  attach prefers-color-scheme change handler -> rebuild Colors, force redraw
  start requestAnimationFrame loop:
    each frame: app.tick(dt); terminal.draw(|f| app.render(f)); rAF(next)

drive(state, action):
  match app.handle(action) {
      Continue => mark dirty (next rAF will draw)
      Picked(s) => clipboard.copy(&s) (kicks off browser Promise);
                   app.show_toast(format!("Copied {s}"));
                   mark dirty
      Cancelled => no-op (browsers can't close tabs reliably)
  }
```

### Rendering

`ratatui-wgpu` renders into the `<canvas>` element via a wgpu surface. With the
`web` cargo feature enabled, `wgpu` selects WebGL2 on browsers that don't expose
WebGPU. Browser detection happens transparently inside `wgpu`; from
`ratatui-wgpu`'s perspective there is one backend.

The bundled font (see "Font pipeline") covers monospace and color emoji glyphs.
`ratatui-wgpu` supports stacking multiple fonts so that text glyphs are sourced
from JetBrains Mono and emoji glyphs from the Noto Color Emoji subset.

The native binary renders via `crossterm` to the user's terminal as today. The
terminal's font choices apply — there is no font bundling on native.

### Color scheme

Native: `terminal-light::luma()` (current behaviour), overridable by
`$GIMOJI_COLOR_SCHEME` and `--color-scheme`.

Web: `window.matchMedia("(prefers-color-scheme: dark)").matches` at startup; a
`change` event listener triggers a `Colors` rebuild and a forced redraw when the
user toggles their OS preference. No URL parameter override in v1 (could add
`?theme=dark` in a follow-up if requested).

### Toast widget

New widget in `gimoji-core::toast`. State: `text`, `started: Instant` (or
`f64` ms since epoch on web — `Instant` doesn't work on `wasm32-unknown-unknown`
without `instant` crate or equivalent). Renders as a centered overlay box for
~1500 ms with a short fade-out. Receives time via `App::tick(dt)`. Disappears
when its timer elapses.

To avoid pulling `instant` into the native build for code-sharing reasons,
`gimoji-core` will define a small `Now` trait or accept `dt: Duration` from the
caller (cleaner — backends own their clock). The web backend uses
`performance.now()` to compute `dt`; the native backend never advances the
toast clock.

## Font pipeline (build-time)

Goal: a single subsetted OTF blob ≤ 300 KB compressed containing only the glyphs
gimoji ever renders, for both monospace and color emoji.

Inputs (vendored under `gimoji-web/assets/fonts/` with licenses):
- **JetBrains Mono Regular** (Apache-2.0), monospace for text and box-drawing.
- **Noto Color Emoji** (SIL OFL-1.1), COLR-v1 vector emoji.

Codepoint set, computed at build time by `gimoji-web/build.rs`:
- Printable ASCII (0x20..0x7E).
- Ratatui box-drawing characters used by `Block`/`Borders`/`Table`/`Padding`
  (`─│┌┐└┘├┤┬┴┼` and the row/column variants).
- Special glyphs: `❯` (selection cursor), `…`.
- Every codepoint sequence appearing in any `EMOJIS[i].emoji` (parsed by reading
  `emojis.json` directly from the build script, the same input the existing
  `gimoji-core/build.rs` consumes). Handles single codepoints and ZWJ sequences
  (`🧑‍💻`) by including all component codepoints plus ZWJ (U+200D) and
  variation selectors (U+FE0F).

Tool: the [`subsetter`](https://docs.rs/subsetter) crate (typst project, pure
Rust). Outputs a valid OTF containing only requested glyphs.

The `build.rs` writes the resulting OTF blob to `$OUT_DIR/subset.otf` (one file
combining the two source fonts using `subsetter`'s merge, or two separate files
if merging is not supported — `ratatui-wgpu`'s `FontFamily` API will be confirmed
during the plan phase to support stacking either way).

`src/lib.rs` embeds it: `static FONT_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/subset.otf"));`

**Risks and fallbacks** (see "Risks" below).

## Build, CI, and deploy

### Workspace migration

Repo root `Cargo.toml` becomes a workspace:

```toml
[workspace]
members = ["crates/gimoji-core", "crates/gimoji", "crates/gimoji-web"]
resolver = "2"

[workspace.package]
version = "1.4.0"
edition = "2021"
license = "MIT"
authors = ["Zeeshan Ali Khan <zeeshanak@gnome.org>"]
repository = "https://github.com/zeenix/gimoji"
```

`crates/gimoji/Cargo.toml` keeps `name = "gimoji"`. `cargo install gimoji` from
crates.io continues to work; users see version 1.4.0 instead of 1.3.0.

### Native CI

Existing `.github/workflows/ci.yml` (or equivalent) updated to operate on the
workspace: `cargo test --workspace --exclude gimoji-web`, `cargo clippy --workspace
--exclude gimoji-web --all-targets`, `cargo fmt --check`. The `gimoji-web` crate
is excluded from the default workspace test/clippy because it requires the
`wasm32-unknown-unknown` target.

### Web build and deploy

New workflow `.github/workflows/deploy-web.yml`:

1. Checkout.
2. Install Rust stable + `wasm32-unknown-unknown` target.
3. Install `wasm-bindgen-cli` (pinned version matching the workspace's
   `wasm-bindgen` dep) and `wasm-opt` (binaryen).
4. `cargo build -p gimoji-web --target wasm32-unknown-unknown --release` with
   `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1` from a `[profile.release]`
   in the workspace root.
5. `wasm-bindgen --out-dir web/dist --target web --no-typescript target/wasm32-unknown-unknown/release/gimoji_web.wasm`
6. `wasm-opt -Oz -o web/dist/gimoji_web_bg.wasm web/dist/gimoji_web_bg.wasm`
7. Copy `crates/gimoji-web/web/index.html` and `crates/gimoji-web/web/style.css`
   to `web/dist/`.
8. `peaceiris/actions-gh-pages` deploys `web/dist/` to the `gh-pages` branch.

This replaces the existing JS-templated `deploy-pages.yml`. The `web-interface`
branch and the `scripts/index.html.template`, `scripts/styles.css.template`, and
`scripts/generate-web.sh` files become historical artifacts (left in place but
not used by the new deploy).

### Bundle-size target

Compressed bundle (HTML + CSS + JS glue + `.wasm` + font) target: **≤ 1 MB**. Stretch
goal: ≤ 500 KB. Composition guess:
- Font subset (combined monospace + emoji subset): ~250 KB compressed
- `.wasm` after `wasm-opt -Oz`: ~400-600 KB compressed (gimoji-core + ratatui-wgpu +
  wgpu WebGL2 backend + regex)
- HTML/CSS/JS glue: ~10 KB compressed

If we blow the 1 MB stretch, mitigations in order: swap `regex` for hand-rolled
case-insensitive substring search (saves ~150 KB); drop wgpu's `dx12`/`vulkan`
backend deps via tighter feature flags; ship the font as a separate file the
browser fetches in parallel rather than embedded in the `.wasm`.

## Testing

- **`gimoji-core` unit tests** (highest leverage): every `Action` variant against a
  fixture emoji set, verifying `App::handle`'s output `Outcome` and subsequent
  state. ~15 tests. Run on every CI build. Both backends inherit these tests
  for free.
- **`gimoji-core` search/filter tests**: keep today's tests (currently colocated
  with `selection_view`), relocated as the module moves.
- **`gimoji` (native) tests**: none added — today there appear to be no integration
  tests on the binary, and adding pty-based ones is out of scope for this work.
- **`gimoji-web` tests**: `wasm-bindgen-test` cases covering `event_to_action`
  (the DOM-event → Action mapping) and `App::row_rect` hit-testing. Run via
  `wasm-pack test --node` in CI. The rAF loop, canvas resize, and color-scheme
  change handlers are tested manually.
- **Manual browser matrix** (pre-merge gate, documented in the plan):
  Firefox stable on Linux, Firefox stable on Windows, Chrome on Linux, Safari on
  macOS, Mobile Safari on iOS, Firefox on Android. For each: page loads, search
  filters as expected, arrow keys navigate, Enter copies, click copies,
  paste-anywhere yields the right emoji, color-scheme switching works.

## Error handling

- **WebGL2/WebGPU init failure** on the web side: `WgpuBackend::builder().build`
  returns an error. We render a static HTML fallback `<div>` (kept hidden in the
  default page; shown on init failure) explaining the browser is unsupported,
  with a link to a basic emoji list or the GitHub repo. This is the only
  scenario where the web build degrades to non-canvas UI.
- **Clipboard write failure** on web (e.g., permission denied, no user
  gesture): the `writeText` Promise rejects. v1 logs to `console.error` and the
  toast renders as "Copy failed — press Ctrl+C" rather than "Copied". v2 could
  switch to a one-shot prompt asking the user to press a key to retry.
- **Font subset failure at build time**: hard build failure. No runtime
  fallback.
- **Native errors**: unchanged from today.

## Risks and open items

1. **`subsetter` COLR-v1 support for Noto Color Emoji** is unverified. Plan-phase
   spike (first task after scaffolding): subset Noto Color Emoji with the actual
   codepoint set, render through `ratatui-wgpu` in a throwaway example, confirm
   glyphs render correctly. If `subsetter` fails to produce a valid COLR subset,
   fallback Plan B: ship **Twemoji COLR Mozilla** unsubsetted (~600 KB raw,
   ~300 KB after brotli). Plan C (last resort): a `pyftsubset` build-time step
   from Python's `fonttools`. The user has stated a preference against Python; C
   is only invoked if both A and B fail.

2. **`ratatui-wgpu`'s `FontFamily` API for stacking monospace + emoji** is real
   per the crate's README but the exact API has not been audited against this
   design. Plan-phase: read `ratatui-wgpu`'s docs or source for the
   `WgpuBackendBuilder::with_fonts` / `FontFamily` interface and adjust the
   build pipeline (single merged OTF vs two separate fonts) accordingly.

3. **`arboard` does not yet support `wasm32-unknown-unknown`** (PR 1Password/arboard#160
   open since 2024, unmerged as of mid-2026). The web build uses `web_sys` /
   inline-JS for clipboard instead. No code-sharing for the clipboard
   implementation; just a trait boundary. Acceptable.

4. **`App::row_rect` requires the table's row positions** — a small addition.
   Today `FilteredView::render` draws rows via `ratatui::widgets::Table`
   without recording where each row landed. Resolution: `App::render` computes
   the table area itself (it already controls the outer `Layout`), then
   computes per-row rects from `Table`'s known layout (header + padding +
   `row_height * row_count`). `SelectionView`/`FilteredView`'s public API stays
   unchanged; the rect math lives in `App`.

5. **`instant`-free timekeeping for the toast widget** — `gimoji-core` should not
   pull `instant` (which has supply-chain quirks). The chosen approach is that
   backends own the clock and pass `dt: Duration` to `App::tick`. The web
   backend computes `dt` from `performance.now()` deltas. Native never calls
   `tick`. No `instant` dep needed.

6. **`wasm-bindgen` async `#[wasm_bindgen(start)]` support** requires a recent
   enough `wasm-bindgen`. Pin to `>=0.2.93` (verify at plan time).

7. **Web pointer events on iOS Safari** sometimes have idiosyncratic behaviour
   around double-tap zoom and momentum. Mitigations: `touch-action: none` on the
   canvas; explicit `event.preventDefault()` in `pointerdown` handlers; verify
   in the manual browser matrix.

8. **CSP / clipboard permission on GitHub Pages**: `navigator.clipboard.writeText`
   requires either a user gesture (covered) or a permissions-policy header
   (which GitHub Pages does not let us set). Since the call only happens inside
   user-gesture handlers, no header is needed.

9. **`web-interface` branch becomes stale.** The new design replaces it. The
   plan should explicitly state that the old `deploy-pages.yml` is removed and
   the gh-pages branch deploy switches to the new workflow. The
   `scripts/generate-web.sh` etc. on the `web-interface` branch are not deleted
   (they live on their own branch) but the project no longer references them.

## Open question deferred to plan

- Exact version pins for `ratatui-wgpu`, `wasm-bindgen`, `wgpu` (whose web
  feature flags differ between releases). Decided at plan time after running
  `cargo search` for current versions.

## Postscript: Phase 7 spike results (2026-06-06)

A throwaway research spike (not committed; see git history of this file for
context) validated the two open assumptions from the "Risks & open questions"
section. Findings:

### Emoji font plan: switch to **Plan B**

`subsetter` v0.2.6 cannot be used for the emoji font. Its own crate docs state
that the output is "most likely unusable in any other contexts than PDF
writing" — it drops the `cmap` table and, critically for color emoji, makes no
attempt to preserve `COLR`, `CPAL`, `CBDT`, `CBLC`, or `sbix`. We confirmed
this by subsetting Noto Color Emoji (`/usr/share/fonts/google-noto-color-emoji-fonts/Noto-COLRv1.ttf`,
4.6 MB) against a representative glyph set: the subsetter ran to completion
without error but produced a 3.3 KB file containing only `glyf`, `head`,
`hhea`, `hmtx`, `loca`, `maxp`, `name`, `post`. No color tables, no cmap.

Therefore Phase 8+ will ship **Twemoji COLR Mozilla unsubsetted** (~600 KB
raw, ~300 KB after brotli). This is well within the budget (the design
already allocates "~300 KB compressed" for emoji). Plan C (`pyftsubset`)
remains a future option if the bundle ever exceeds budget, but is not needed
at the planned scope.

### `ratatui-wgpu` v0.5.0 builder API: confirmed shape

The crate exposes a `Builder` (not `WgpuBackendBuilder`) constructed via
`Builder::from_font(Font)`. Additional fonts for fallback are layered on with
`.with_fonts(impl IntoIterator<Item = Font>)`. The builder is **async**, and
the entry point is `.build_with_target(impl Into<wgpu::SurfaceTarget<'s>>)`.
On the web, `wgpu::SurfaceTarget::Canvas(web_sys::HtmlCanvasElement)` is the
correct variant (gated by `cfg(web)` in wgpu; the `web` feature on
`ratatui-wgpu` enables `wgpu/webgl` so Firefox is supported).

`Font` is constructed with `Font::new(&[u8]) -> Option<Font>` (returns `None`
on parse failure — note: `Option`, not `Result`). The font data must outlive
the `Font<'a>`, so we use `&'static [u8]` via `include_bytes!`.

Minimal shape Phase 10 should use on the web:

```rust
use std::num::NonZeroU32;
use ratatui_wgpu::{Builder, Dimensions, Font, WgpuBackend};

static JB_MONO: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");
static EMOJI: &[u8]  = include_bytes!("../assets/TwemojiCOLRMozilla.ttf");

let last_resort = Font::new(JB_MONO).ok_or("bad mono font")?;
let emoji_font  = Font::new(EMOJI).ok_or("bad emoji font")?;

let backend: WgpuBackend<'_, '_> = Builder::from_font(last_resort)
    .with_fonts(std::iter::once(emoji_font))
    .with_width_and_height(Dimensions {
        width:  NonZeroU32::new(canvas.width()).unwrap(),
        height: NonZeroU32::new(canvas.height()).unwrap(),
    })
    .build_with_target(wgpu::SurfaceTarget::Canvas(canvas.clone()))
    .await?;
```

Notes Phase 10 must respect:

1. The first font (passed to `from_font`) is the **last-resort fallback**, not
   the preferred font. Per-cell font selection in
   `ratatui-wgpu`'s `Fonts::font_for_cell` walks the regular/bold/italic
   stacks first and only falls back to the "last resort" font if nothing else
   covers the glyph. So we pass JetBrains Mono as the last-resort font and add
   the emoji font (and any other text fonts) via `.with_fonts(...)`.
2. `with_fonts` warns at runtime if any font is not monospace. Twemoji COLR
   Mozilla is not monospace; the warning is expected and harmless (the crate
   will still render it, just with a width-adjustment caveat).
3. Backend type parameters: `WgpuBackend<'a, 's, P = DefaultPostProcessor>`
   where `'a` is the lifetime of the font byte slices (`'static` for us) and
   `'s` is the surface target lifetime. The web canvas variant produces a
   `'static`-ish surface, so `WgpuBackend<'static, 'static>` is achievable.
4. Native parity: the same `Builder::from_font(...).with_fonts(...)` chain
   works on native by calling `.build_with_target(window_arc)` instead, where
   `window_arc: Arc<winit::window::Window>` implements `WindowHandle` and
   therefore `Into<SurfaceTarget>`.
