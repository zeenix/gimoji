# A DOM-only render path for gimoji-web

**Status:** Implemented — option A (ratzilla), superseded by B (hand-rolled DOM),
finally a Canvas-2D backend (2026-08-11)
**Date:** 2026-06-08
**Branch:** `consolodate-web-native`

## Problem

`gimoji-web` currently renders the picker through `ratatui-wgpu`, which spins
up a full WebGL2 pipeline (wgpu surface + glyph atlas + per-frame command
buffers) to draw what is fundamentally a text UI with ~600 rows of two
columns plus a search line and a toast. The cost shows up in three places:

1. **GPU surface memory.** The canvas is sized to its CSS box at the
   device pixel ratio. Even after capping (see `style.css`,
   `crates/gimoji-web/src/lib.rs:fit_canvas`) the WebGL2 backbuffer is
   double-buffered RGBA plus depth — tens of MB on hi-DPI screens.
2. **WASM bundle.** The released `gimoji_web_bg.wasm` is ~3.0 MB. The
   bulk of that is `wgpu` + `rustybuzz` + `raqote` (text shaping and
   software rasterisation that ratatui-wgpu uses on the WebGL2 path).
3. **WASM linear memory.** Glyph caches, intermediate rasterisation
   buffers, and ratatui-wgpu's own command buffers occupy memory the
   browser cannot reclaim until the tab closes. Combined with the GPU
   surface, the tab regularly sits at 80–100 MB resident in Firefox.

For comparison: a hand-written DOM picker on the `web-interface` branch
(pre-consolidation) ran in ~6–8 MB. The two-orders-of-magnitude gap is
the price paid for the "single Rust source of truth" architecture.

Capping the canvas size and DPR has partially addressed this, but the
floor set by wgpu + WASM is still high — roughly 50 MB even on a
modestly-sized window.

## Goals

- Reduce resident memory of an idle gimoji tab to single-digit MB.
- Cut the WASM bundle by removing `wgpu`, `ratatui-wgpu`, `wgpu`'s WebGL2
  shim, `rustybuzz`, and `raqote` — ideally landing under 500 KB.
- Keep the `gimoji-core` `App` / `Action` / `Outcome` state machine as
  the single source of truth for picker behaviour. The native CLI must
  stay untouched.
- Preserve every interaction the current web build supports: keyboard
  navigation, search-as-you-type, pointer selection of rows, light/dark
  auto-detection, clipboard copy + toast.
- Render emojis with system fonts (already done via `emoji_overlay.rs`)
  and accept that the picker's visual style will diverge further from
  the native ratatui look. That trade-off was tentatively accepted when
  `emoji_overlay.rs` was introduced; this proposal extends it.

## Non-goals

- Restoring pixel parity with the native renderer. The web build is
  explicitly a browser-native take on the same picker.
- Replacing the native `ratatui` + `crossterm` stack. This is web-only.
- A redesign of the picker's information architecture. Same columns,
  same search behaviour, same keybindings.
- Mobile-specific UX changes (touch scrolling, soft keyboard handling)
  beyond what falls out of using real DOM elements.

## Candidate approaches

### A. Drop in `ratzilla` as a DOM-rendering ratatui backend

`ratzilla` implements the `ratatui::backend::Backend` trait on top of the
DOM (one element per cell, optionally a `<canvas>` 2D path). The widget
tree in `gimoji-core` (`SearchEntry`, `SelectionView`, `FilteredView`,
`Toast`) stays untouched; only `gimoji-web` swaps the backend.

The original consolidation design
(`2026-06-06-web-native-consolidation-design.md`) considered and
rejected `ratzilla` for two reasons: perceived maintenance status and
the pixel-fidelity goal. Both warrant a fresh look:

- The pixel-fidelity argument has already been weakened by
  `emoji_overlay.rs`, which deliberately diverges from native rendering
  for the emoji column.
- Maintenance status should be re-evaluated against the current crate
  ecosystem before deciding. If `ratzilla` looks healthy enough, this is
  by far the cheapest path.

**Pros:** Minimal new code. Keeps the consolidation architecture intact
(same widgets feeding two backends). Bundle shrinks dramatically:
`wgpu` + raster stack go away.

**Cons:** External dependency on a less-popular crate. If `ratzilla`
later goes unmaintained, the project absorbs the maintenance burden of
a forked DOM backend.

### B. Write a custom `ratatui::backend::Backend` that emits DOM

Implement `Backend` directly in `gimoji-web`. The trait API is small —
write a cell, move the cursor, hide/show it, query size, flush. The
implementation translates cell writes into a fixed grid of `<span>`s
(one per cell), with style memoisation matching the pattern in
`emoji_overlay.rs:SpanState` to skip writes when content+style is
unchanged.

**Pros:** No external dependency. Tight control over diffing strategy
and DOM shape. Reusable knowledge from `emoji_overlay.rs` — the diff +
memo pattern there already shows the right shape.

**Cons:** Real implementation effort: cursor model, color conversion,
modifier handling (bold/italic/reverse-video are uncommon in the picker
but must work for the `SearchEntry` cursor). Re-implements something
`ratzilla` already provides.

### C. Use `par-term-emu-core-rust` as an intermediate cell-grid

The `par-term-emu-core-rust` crate
(<https://crates.io/crates/par-term-emu-core-rust>) implements a full
VT100/VT220/VT320/VT420/VT520 terminal emulator: bytes in, structured
cell grid out (via `term.content()` / `term.get_semantic_snapshot()`).
That suggests a pipeline:

1. Use a `ratatui::backend::Backend` impl that writes escape sequences
   into a `Vec<u8>` (the crossterm backend already does this; a thin
   custom backend could do it without crossterm's OS deps).
2. Feed those bytes to `par-term-emu-core-rust`.
3. Walk its cell grid and render to DOM.

**Pros:** Reuses a battle-tested VT parser handling the gnarly bits —
grapheme clusters, complex emoji sequences, modifier combinations. The
crate is actively maintained (40+ releases as of mid-2026).

**Cons:** The crate is explicitly targeted at native (Linux/macOS/Windows)
with Python bindings; it depends on `portable-pty` and other OS-level
crates for its streaming features. There is no WASM build target today
and no feature flag known to strip the OS dependencies. Getting it to
build for `wasm32-unknown-unknown` would require an upstream
contribution or a fork — neither of which is justified for a UI that
only needs the picker's narrow subset of terminal behaviour.

Also: this approach adds an extra layer (ratatui buffer → escape bytes
→ emu-core cell grid → DOM) that doesn't obviously beat **B** (ratatui
buffer → DOM, via a direct `Backend` impl). The picker uses a
handful of attributes — bold, reverse-video for the cursor, a few
colors — not the full VT520 attribute matrix. Paying parser overhead
for unused features is the wrong trade.

Treat this as a fallback only if **B** turns out to require
re-implementing too much terminal-emulator behaviour by hand.

### D. Bypass ratatui on the web side

`gimoji-core` exposes the picker model (filtered list, selected index,
search text, toast state) as plain data. `gimoji-web` renders that
state to bespoke DOM (a real `<input>` for search, a virtualised list
of `<div>` rows, etc.). The web build no longer involves ratatui at
all.

**Pros:** Smallest possible bundle and resident memory. Best browser
UX: real focus management, real text input, real accessibility tree,
real scroll containers. Probably 200–300 KB WASM and <10 MB resident.

**Cons:** This re-introduces the duplication the consolidation work
removed. Filter/search logic stays in `gimoji-core` (good) but visual
layout diverges between native and web (bad). Every layout change has
to be made in two places. Effectively the `web-interface` branch
rebuilt with `gimoji-core` as a library — which begs the question of
whether the rebuild is worth it relative to just option A or B.

## Recommendation

Pursue **A** if a fresh evaluation of `ratzilla` shows it is acceptably
maintained and its API surface fits the widgets in `gimoji-core`. Fall
back to **B** if not. Treat **C** as a niche fallback (requires upstream
WASM support work). Treat **D** as an escape hatch: only justified if
A, B, and C all fail to hit the memory target, since it walks back the
core consolidation premise.

## Trade-offs worth being explicit about

- **Visual divergence.** Native still uses ratatui's box-drawing through
  crossterm; DOM render will use real DOM text. The "monospace
  terminal in a browser" aesthetic survives but pixel-level parity does
  not. This is already true post-`emoji_overlay.rs`.
- **Animation frame model.** ratatui-wgpu redraws on `requestAnimationFrame`.
  A DOM render can be event-driven (redraw only on input + toast tick),
  which is both cheaper and more browser-native. The `App::tick` plumbing
  in `gimoji-core` already accommodates either model.
- **Bundle size vs. browser cache.** A 500 KB WASM + JS bundle is
  cache-friendly enough that returning visitors see effectively zero
  load cost, where the current 3 MB bundle is large enough that first
  loads on slow connections are perceptible.

## Open questions

- Does the current `ratatui` (0.30.1) `Backend` trait have any feature
  that `ratzilla` lags on (e.g. `Modifier::CROSSED_OUT`,
  `Style::underline_color`)? If yes, does the picker actually use any of
  them?
- Is there an existing crate (`ratatui-html`, `tui-web`, etc.) that
  fills the same niche and is more actively maintained?
- The current `EmojiOverlay` paints DOM emojis on top of the wgpu
  canvas. Under a DOM backend it can render emoji inline (a single
  span in the same grid), but should it? Inline is simpler; overlay
  preserves the option to swap the backend again without re-doing
  emoji rendering.
- Does removing `wgpu` break Firefox-on-Android or any browser the
  current build claims to support but didn't actually exercise?

## Migration shape (sketch)

If A is chosen:

1. Add `ratzilla` to `crates/gimoji-web/Cargo.toml`; remove
   `ratatui-wgpu`, `wgpu`, the wgpu-specific `web-sys` features, and
   the canvas resize plumbing.
2. Replace the `WgpuBackend` in `lib.rs:run` with the ratzilla
   equivalent; keep `App`, `Action`, `Outcome` calls identical.
3. Replace `schedule_raf` with the event-driven redraw model ratzilla
   prefers (likely a dirty flag flush after each input event + a
   per-second tick for toast countdown).
4. Decide whether `emoji_overlay.rs` stays (overlay) or folds into the
   grid (inline). Keeping it is the lower-risk first step.
5. Update `style.css` to drop the canvas-specific rules.
6. Re-measure: bundle size, idle RSS in Firefox/Chrome, time-to-paint.

If B is chosen, steps 1–2 are replaced by writing the backend module
(probably `crates/gimoji-web/src/dom_backend.rs`) implementing
`ratatui::backend::Backend`; the rest is the same.

The native crate (`gimoji`) is not touched in any variant.

## Outcome (2026-06-08)

Implemented option A. The ratzilla implementation landed and worked, was then replaced by a
hand-rolled per-cell DOM backend, and finally by the Canvas-2D backend described under
"Post-outcome evolution" below. The branch history was consolidated afterwards, so those
intermediate stages survive only in this narrative — and in the local `backup/pre-squash` ref —
not as commits.

### Bundle size

| Artefact          | Before (3.0 MB era) | After  | Change |
|-------------------|---------------------|--------|--------|
| `gimoji_web_bg.wasm` (raw) | ~3.0 MB    | 294 KB | −90%   |
| `gimoji_web_bg.wasm` (gzipped) | ~1.1 MB | 130 KB | −88%   |
| `gimoji_web.js`            | ~30 KB    | 32 KB  | ±0     |
| Total dist (raw)           | ~3.0 MB   | 328 KB | −89%   |

Comfortably under the 500 KB target. The `beamterm-renderer` /
WebGL transitive deps that worried planning are dead-code-eliminated
by `wasm-opt -Oz` since `DomBackend` never references them.

### Idle RSS

No Firefox `about:performance` measurement was taken — that manual smoke test was
never redone, and this section should not claim one exists. What was measured
instead (2026-08-11, Task 2 headless-Chrome smoke harness, `page.metrics()`):
`JSHeapUsedSize` ~1.76 MiB, `JSHeapTotalSize` ~3.5 MiB. Headless-Chrome process RSS
(as reported by `ps`) was also sampled but is **not** representative of the
picker's footprint — it includes each renderer process's full V8/Blink baseline
(JIT reservations, GPU/compositor buffers, per-process isolation), which dwarfs
anything attributable to this page; the JS-heap figures above are the meaningful
numbers here. By the time of this measurement the backend was already the
Canvas-2D implementation described in "Post-outcome evolution" below, not
ratzilla — no idle-memory number was ever captured for the ratzilla backend
specifically.

### Deviations from plan

- Plan sketch had `MouseEventKind::SingleClick`; ratzilla 0.3 actually
  uses `Pressed` / `Released` variants. Implementation filters on
  `Pressed` + `MouseButton::Left`.
- Plan sketch assumed mouse `col`/`row` were in cell coordinates;
  ratzilla 0.3 actually delivers pixels. Implementation added a
  `pixel_to_cell` helper that reads live geometry from ratzilla's
  rendered `<div id="gimoji-grid_ratzilla_grid">` (the suffix is
  ratzilla's `DomBackendOptions::grid_id` convention).
- Plan sketch's example `input.rs` had Backspace-with-empty cancel
  the picker. The existing behavior was actually Esc-based (Esc with
  empty cancels; Esc with non-empty clears search). Preserved
  faithfully.
- Plan didn't anticipate `critical-section` link errors on wasm32.
  ratzilla enables ratatui-core's `layout-cache` feature which pulls
  in `critical-section`. Added `critical-section = { version = "1",
  features = ["std"] }` to provide the wasm32 implementation.

### Deferred / not pursued

- Upstreaming feature flags to ratzilla to gate `beamterm-renderer`
  is **not** needed — `wasm-opt` already strips the dead code. No
  PR required.
- Mac `Cmd+letter` keybindings: no longer deferred. The frontend now
  handles raw `KeyboardEvent`s rather than ratzilla's `KeyEvent`, so
  `input::from_keyboard` checks `meta_key()` next to ctrl/alt and
  Cmd-modified chars reach the browser instead of the search box
  (fixed 2026-08-11).

### Post-outcome evolution (2026-08-11)

Ratzilla did not survive contact: 0.3.1 panics on an off-by-one between `size()` and its cell
array, and 0.3.0 mutates its size inside `draw()` after ratatui's autoresize, ghosting borders
across resizes. It was replaced by a hand-rolled per-cell DOM backend (option B), which rendered
correctly but churned inline-style strings on the wasm heap — resident memory climbed past
200 MB during heavy scrolling and never recovered, because wasm linear memory never shrinks.

The final backend paints the whole grid into a single `<canvas>` via Canvas 2D
(`crates/gimoji-web/src/canvas_backend.rs`): one DOM element, `fillRect`/`fillText` instead of
style strings, reused scratch buffers, and per-codepoint font fallback for colour emoji. Emoji
glyphs are stamped in an overlay pass (`EmojiSource::Overlay`) because `unicode-width`
under-counts VS16/ZWJ sequences and would break column alignment.

Final bundle: 214 KiB raw / 89.4 KiB gzipped wasm (from the Task 2 smoke harness's
`./scripts/serve-web.sh` build; JS glue adds another 27 KiB raw / 5.5 KiB gzipped,
for a ~240 KiB raw / ~95 KiB gzipped total dist).
