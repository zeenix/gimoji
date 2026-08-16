use std::{cell::RefCell, rc::Rc};

use canvas_backend::CanvasBackend;
use gimoji_core::{Action, App, Outcome, EMOJIS};
use ratatui::{
    layout::{Position, Rect},
    Terminal,
};
use text_input::TextInput;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{
    AddEventListenerOptions, CssStyleDeclaration, Document, HtmlElement, KeyboardEvent,
    PointerEvent, VisualViewport, WheelEvent,
};

/// Maximum picker dimensions in cells. The canvas fills its container, so
/// the picker is centred inside it via `App::render_in_area` and the
/// surrounding cells stay blank (transparent so the page background shows
/// through).
const MAX_PICKER_COLS: u16 = 110;
const MAX_PICKER_ROWS: u16 = 36;

mod canvas_backend;
mod clipboard;
mod color_scheme;
mod input;
mod text_input;

/// The element id of the `<canvas>` the picker paints into. Matches the
/// markup in `web/index.html`.
const CANVAS_ID: &str = "gimoji-canvas";

/// The element id of the offscreen `<input>` that owns text entry. Also
/// from `web/index.html`; see [`text_input`] for why it exists.
const INPUT_ID: &str = "gimoji-input";

/// Cadence of the toast countdown driver. The toast lifetime is measured in
/// seconds, so 250 ms is fine-grained enough that expiry feels snappy without
/// waking the wasm runtime more often than necessary.
const TICK_MS: i32 = 250;

/// Toast prefix for a clipboard write the browser confirmed.
const COPIED_PREFIX: &str = "Copied";
/// Toast prefix for one it refused.
const COPY_FAILED_PREFIX: &str = "Copy failed";

/// How far a pointer may travel and still count as a tap rather than a
/// scroll. Roughly the slop a browser itself allows before turning a touch
/// into a pan: tight enough that a deliberate tap picks, loose enough that
/// a finger resting on a row doesn't pick something else on the way up.
const TAP_SLOP_PX: f64 = 8.0;

/// Rows a `DOM_DELTA_PAGE` wheel notch scrolls. Those arrive without a
/// pixel measurement to convert, so this stands in for "a screenful" —
/// close to the list height on a typical viewport.
const WHEEL_PAGE_ROWS: f64 = 20.0;

struct State {
    app: App<'static>,
    clipboard: clipboard::WebClipboard,
    text_input: TextInput,
    last_perf_ms: f64,
}

#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let performance = window
        .performance()
        .ok_or_else(|| JsValue::from_str("no performance"))?;

    let colors = color_scheme::detect();
    // `with_emoji_overlay` keeps the emoji column blank in ratatui's
    // buffer so the (often VS16/ZWJ) emoji symbols don't break
    // unicode-width-based column accounting; we paint the glyphs on top
    // of the canvas in the RAF loop instead. See
    // `CanvasBackend::paint_emoji_overlay`.
    let app = App::with_emoji_overlay(EMOJIS, colors);

    let backend = CanvasBackend::new(CANVAS_ID)
        .map_err(|e| JsValue::from_str(&format!("CanvasBackend init failed: {e}")))?;
    let terminal =
        Terminal::new(backend).map_err(|e| JsValue::from_str(&format!("terminal init: {e}")))?;

    let text_input = TextInput::new(&document, INPUT_ID)
        .map_err(|e| JsValue::from_str(&format!("text input init failed: {e}")))?;

    let state = Rc::new(RefCell::new(State {
        app,
        clipboard: clipboard::WebClipboard,
        text_input,
        last_perf_ms: performance.now(),
    }));
    let terminal = Rc::new(RefCell::new(terminal));

    install_keydown(&window, &document, &state);
    install_text_input(&state);
    install_pointer_gestures(&document, &terminal, &state);
    install_wheel(&terminal, &state);
    install_color_scheme_listener(&state);
    install_tick(&window, &state);
    install_resize(&window, &terminal);
    install_viewport_sync(&window, &document, &terminal);
    install_raf_loop(&window, &terminal, &state);

    // Take focus up front so a visitor with a keyboard can type straight
    // away — but only where a keyboard is already attached. Pre-focusing a
    // touch device would park `document.activeElement` on the input without
    // ever showing its on-screen keyboard, and the tap that is supposed to
    // raise it would then be a no-op focus change on an already-focused
    // element. Touch devices take focus from the tap instead, in
    // `install_pointer_gestures`.
    if !has_coarse_pointer(&window) {
        state.borrow().text_input.focus();
    }

    Ok(())
}

/// Whether the device's primary pointer is coarse, i.e. a finger. Used to
/// keep startup from stealing the focus transition a tap needs; assumes
/// coarse when the query can't be run, since that's the case with something
/// to lose.
fn has_coarse_pointer(window: &web_sys::Window) -> bool {
    window
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .map(|query| query.matches())
        .unwrap_or(true)
}

/// Compute the cell rectangle the picker should render into given the
/// frame's full area. The picker is capped at `MAX_PICKER_COLS × ROWS`
/// and centred inside the full frame so the surrounding cells stay
/// transparent and the page background shows through.
fn picker_area(full: Rect) -> Rect {
    let w = MAX_PICKER_COLS.min(full.width);
    let h = MAX_PICKER_ROWS.min(full.height);
    let x = full.width.saturating_sub(w) / 2;
    let y = full.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

/// Shared handle to the RAF callback so it can re-schedule itself. The
/// `Option` is initialised after we build the closure that references the
/// handle, which is why it can't be a one-shot binding.
type RafHandle = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

/// Install a self-rescheduling `requestAnimationFrame` loop that redraws
/// the terminal every frame. The closure holds an `Rc` to itself via
/// `next` so it can call `requestAnimationFrame` again from inside its
/// body; we leak the outermost closure handle so it survives for the
/// lifetime of the page.
fn install_raf_loop(
    window: &web_sys::Window,
    terminal: &Rc<RefCell<Terminal<CanvasBackend>>>,
    state: &Rc<RefCell<State>>,
) {
    let document = window.document();
    let next: RafHandle = Rc::new(RefCell::new(None));
    let cb = {
        let next = next.clone();
        let terminal = terminal.clone();
        let state = state.clone();
        let document = document.clone();
        // Track the page visibility across RAF ticks. Browsers drop the
        // canvas backing store while the tab is hidden (especially when
        // a new tab is opened and the renderer is pushed out of the
        // foreground), so when we transition from hidden back to visible
        // we have to force a full repaint: refresh the canvas geometry
        // (which calls `set_width`, re-allocating a fresh backing store
        // and resetting context state) and then `Terminal::clear` so the
        // diff treats every picker cell as new. RAF-timestamp-based
        // detection doesn't work because some browsers throttle RAF to
        // 1Hz while hidden, so the gap on the first foreground tick is
        // small. `document.hidden` is the canonical signal.
        let mut was_hidden = false;
        // Track the toast across RAF ticks. On the frame the toast expires
        // the picker cells it covered come back in the buffer and the diff
        // repaints them — but the overlay pass then wipes the rect the
        // toast's emoji occupied, gouging a transparent hole out of those
        // just-painted pixels. Neither ratatui's buffer nor the backend
        // shadow considers those cells dirty afterwards, so the hole would
        // stay until something else happened to touch them. Force a full
        // repaint on the expiry transition instead.
        let mut had_toast = false;
        Closure::<dyn FnMut(f64)>::new(move |_t: f64| {
            let hidden = document.as_ref().is_some_and(|d| d.hidden());
            let resumed = was_hidden && !hidden;
            was_hidden = hidden;
            let has_toast = state.borrow().app.has_toast();
            let toast_expired = had_toast && !has_toast;
            had_toast = has_toast;
            // Scope the draw call so the `CompletedFrame` (which borrows
            // from `term`) is dropped before we paint the overlay glyphs.
            let err: Option<String> = {
                let mut term = terminal.borrow_mut();
                if resumed {
                    if let Err(e) = term.backend_mut().refresh_geometry() {
                        web_sys::console::error_1(&JsValue::from_str(&format!(
                            "refresh_geometry on resume: {e}"
                        )));
                    }
                }
                if resumed || toast_expired {
                    // Drop the previous pass's overlay rects first: the
                    // clear below wipes them anyway, and letting the overlay
                    // pass re-apply them after the fresh draw is exactly the
                    // hole described above.
                    term.backend_mut().clear_overlay_rects();
                    let _ = term.clear();
                }
                let draw_res = term.draw(|frame| {
                    let area = picker_area(frame.area());
                    state.borrow_mut().app.render_in_area(frame, area);
                });
                match draw_res {
                    Ok(_) => {
                        // Overlay emoji glyphs on top of the cells the
                        // diff just painted. The picker keeps the emoji
                        // column blank in the buffer (see `App::with_
                        // emoji_overlay`), so the underlying bg is correct
                        // and we just stamp the glyphs in. The backend
                        // also clears previous overlay positions so old
                        // glyphs vanish when the visible list shrinks.
                        let s = state.borrow();
                        let band = s.app.emoji_overlay_band();
                        let overlays = s
                            .app
                            .visible_emojis()
                            .iter()
                            .map(|ve| (ve.cell, ve.emoji))
                            .chain(s.app.toast_overlay_emoji());
                        term.backend_mut().paint_emoji_overlays(band, overlays);
                        None
                    }
                    Err(e) => Some(e.to_string()),
                }
            };
            if let Some(msg) = err {
                web_sys::console::error_1(&JsValue::from_str(&format!("draw error: {msg}")));
            }
            if let Some(window) = web_sys::window() {
                if let Some(cb) = next.borrow().as_ref() {
                    let _ = window
                        .request_animation_frame(cb.as_ref().unchecked_ref::<js_sys::Function>());
                }
            }
        })
    };
    *next.borrow_mut() = Some(cb);
    let _ = window.request_animation_frame(
        next.borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .unchecked_ref::<js_sys::Function>(),
    );
    // The `next` `Rc` is kept alive by the closure itself referencing it;
    // we additionally leak our outer `Rc` clone via `Box::leak` to make
    // sure it stays alive across event loop turns even if all other
    // references happen to drop.
    Box::leak(Box::new(next));
}

fn install_keydown(window: &web_sys::Window, document: &Document, state: &Rc<RefCell<State>>) {
    let st = state.clone();
    let doc = document.clone();
    let cb = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        let (search_empty, text_input_focused) = {
            let s = st.borrow();
            (
                s.app.search_text().is_empty(),
                s.text_input.is_focused(&doc),
            )
        };
        let Some(action) = input::from_keyboard(&event, search_empty, text_input_focused) else {
            return;
        };
        event.prevent_default();
        drive(&st, action);
    });
    window
        .add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())
        .expect("keydown listener install");
    cb.forget();
}

/// Feed the offscreen `<input>`'s edits into the picker.
///
/// Its whole value is resent on every edit rather than a per-key delta:
/// mobile keyboards rewrite arbitrary spans (autocorrect, swipe typing,
/// IME composition) and often report characters through `input` alone,
/// with no `keydown` to translate.
fn install_text_input(state: &Rc<RefCell<State>>) {
    let st = state.clone();
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
        let value = st.borrow().text_input.value();
        drive(&st, Action::SetSearch(value));
    });
    state
        .borrow()
        .text_input
        .element()
        .add_event_listener_with_callback("input", cb.as_ref().unchecked_ref())
        .expect("input listener install");
    cb.forget();
}

/// Wire up tap-to-pick and drag-to-scroll on the canvas.
///
/// Both gestures start the same way, so a press is only resolved on
/// release: one that moved more than [`TAP_SLOP_PX`] scrolled the list and
/// must not also pick an emoji out from under the finger that was scrolling
/// it. `touch-action: none` in `web/style.css` is what stops the browser
/// claiming the drag for panning or double-tap zoom before we see it.
fn install_pointer_gestures(
    document: &Document,
    terminal: &Rc<RefCell<Terminal<CanvasBackend>>>,
    state: &Rc<RefCell<State>>,
) {
    let canvas = terminal.borrow().backend().canvas_element().clone();
    let gesture: Rc<RefCell<Option<Gesture>>> = Rc::new(RefCell::new(None));

    let down = {
        let gesture = gesture.clone();
        let canvas = canvas.clone();
        Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            // Suppress the compatibility mouse events this press would
            // otherwise synthesise. They arrive *after* `pointerup`, and
            // their default action moves focus to the nearest focusable
            // ancestor — i.e. off the offscreen input and onto `<body>`,
            // undoing the focus a tap on the search box just took and
            // dismissing the on-screen keyboard with it.
            event.prevent_default();
            // Capture so a drag that wanders off the canvas keeps scrolling
            // and still ends with a `pointerup` we get to see.
            let _ = canvas.set_pointer_capture(event.pointer_id());
            let y = event.client_y() as f64;
            *gesture.borrow_mut() = Some(Gesture {
                pointer_id: event.pointer_id(),
                start_x: event.client_x() as f64,
                start_y: y,
                last_y: y,
                residual_rows: 0.0,
                dragged: false,
            });
        })
    };

    let mv = {
        let gesture = gesture.clone();
        let term = terminal.clone();
        let st = state.clone();
        Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let cell_h = term.borrow().backend().geometry().cell_h;
            if cell_h <= 0.0 {
                return;
            }
            let rows = {
                let mut held = gesture.borrow_mut();
                let Some(g) = held.as_mut().filter(|g| g.pointer_id == event.pointer_id()) else {
                    return;
                };
                let y = event.client_y() as f64;
                if (event.client_x() as f64 - g.start_x).abs() > TAP_SLOP_PX
                    || (y - g.start_y).abs() > TAP_SLOP_PX
                {
                    g.dragged = true;
                }
                g.residual_rows += (y - g.last_y) / cell_h;
                g.last_y = y;
                g.take_whole_rows()
            };
            // Dragging downwards pulls earlier rows into view, so the list
            // offset moves the opposite way and the content tracks the
            // finger.
            if rows != 0 {
                drive(&st, Action::Scroll(-rows));
            }
        })
    };

    let up = {
        let gesture = gesture.clone();
        let canvas = canvas.clone();
        let term = terminal.clone();
        let st = state.clone();
        let doc = document.clone();
        Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let _ = canvas.release_pointer_capture(event.pointer_id());
            let Some(finished) = take_gesture(&gesture, event.pointer_id()) else {
                return;
            };
            if finished.dragged {
                return;
            }
            let Some((cx, cy)) = pixel_to_cell(
                &term.borrow(),
                event.client_x() as f64,
                event.client_y() as f64,
            ) else {
                return;
            };
            let position = Position { x: cx, y: cy };

            // A tap on the search box focuses the offscreen input, which is
            // what raises the on-screen keyboard. It has to happen straight
            // out of this handler: browsers only show the keyboard for a
            // focus change a user gesture drove.
            let in_search = st
                .borrow()
                .app
                .search_area()
                .is_some_and(|area| area.contains(position));
            if in_search {
                st.borrow().text_input.focus_from_gesture(&doc);
                return;
            }

            // `hit_test` accounts for the list's scroll offset, so the index
            // is valid against the filtered list `PickAt` indexes into.
            let Some(index) = st.borrow().app.hit_test(cx, cy) else {
                return;
            };
            drive(&st, Action::PickAt(index));
        })
    };

    let cancel = {
        let gesture = gesture.clone();
        let canvas = canvas.clone();
        Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let _ = canvas.release_pointer_capture(event.pointer_id());
            take_gesture(&gesture, event.pointer_id());
        })
    };

    for (name, cb) in [
        ("pointerdown", &down),
        ("pointermove", &mv),
        ("pointerup", &up),
        ("pointercancel", &cancel),
    ] {
        canvas
            .add_event_listener_with_callback(name, cb.as_ref().unchecked_ref())
            .expect("pointer listener install");
    }
    for cb in [down, mv, up, cancel] {
        cb.forget();
    }
}

/// A pointer press in flight on the canvas.
struct Gesture {
    pointer_id: i32,
    start_x: f64,
    start_y: f64,
    last_y: f64,
    /// Sub-row scroll carried between moves, so a slow drag still scrolls
    /// once it has covered a whole row.
    residual_rows: f64,
    /// Set once the pointer travelled past [`TAP_SLOP_PX`]; a gesture that
    /// scrolled must not also pick on release.
    dragged: bool,
}

impl Gesture {
    /// Hand back the whole rows accumulated so far, keeping the remainder.
    fn take_whole_rows(&mut self) -> i32 {
        let rows = self.residual_rows.trunc();
        self.residual_rows -= rows;
        rows as i32
    }
}

/// End the gesture belonging to `pointer_id`, leaving any other pointer's
/// gesture (a second finger, say) alone.
fn take_gesture(gesture: &Rc<RefCell<Option<Gesture>>>, pointer_id: i32) -> Option<Gesture> {
    let mut held = gesture.borrow_mut();
    if held.as_ref().is_some_and(|g| g.pointer_id == pointer_id) {
        held.take()
    } else {
        None
    }
}

/// Scroll the list on a wheel or trackpad gesture — the desktop twin of the
/// touch drag installed by [`install_pointer_gestures`].
fn install_wheel(terminal: &Rc<RefCell<Terminal<CanvasBackend>>>, state: &Rc<RefCell<State>>) {
    let canvas = terminal.borrow().backend().canvas_element().clone();
    let term = terminal.clone();
    let st = state.clone();
    // Sub-row remainder, so trackpads that report a few pixels per event
    // still scroll instead of rounding every event away to nothing.
    let residual = Rc::new(RefCell::new(0.0f64));
    let cb = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
        let cell_h = term.borrow().backend().geometry().cell_h;
        if cell_h <= 0.0 {
            return;
        }
        event.prevent_default();
        let delta_rows = match event.delta_mode() {
            WheelEvent::DOM_DELTA_LINE => event.delta_y(),
            WheelEvent::DOM_DELTA_PAGE => event.delta_y() * WHEEL_PAGE_ROWS,
            // `DOM_DELTA_PIXEL`, and anything a future spec adds.
            _ => event.delta_y() / cell_h,
        };
        let rows = {
            let mut residual = residual.borrow_mut();
            *residual += delta_rows;
            let rows = residual.trunc();
            *residual -= rows;
            rows as i32
        };
        if rows != 0 {
            drive(&st, Action::Scroll(rows));
        }
    });
    // Wheel listeners are passive by default in some browsers, which makes
    // `preventDefault` a no-op (and logs a console error); opt out so the
    // page doesn't also scroll or zoom under the picker.
    let options = AddEventListenerOptions::new();
    options.set_passive(false);
    canvas
        .add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            cb.as_ref().unchecked_ref(),
            &options,
        )
        .expect("wheel listener install");
    cb.forget();
}

/// Convert a viewport pixel coordinate to a terminal cell coordinate using
/// the geometry cached by the [`CanvasBackend`]. Returns `None` if the
/// click landed outside the canvas.
fn pixel_to_cell(terminal: &Terminal<CanvasBackend>, px: f64, py: f64) -> Option<(u16, u16)> {
    let geom = terminal.backend().geometry();
    if geom.cell_w <= 0.0 || geom.cell_h <= 0.0 {
        return None;
    }
    let canvas_rect = terminal
        .backend()
        .canvas_element()
        .get_bounding_client_rect();
    let rel_x = px - canvas_rect.left();
    let rel_y = py - canvas_rect.top();
    if rel_x < 0.0 || rel_y < 0.0 || rel_x >= canvas_rect.width() || rel_y >= canvas_rect.height() {
        return None;
    }
    let cx = (rel_x / geom.cell_w).floor() as i64;
    let cy = (rel_y / geom.cell_h).floor() as i64;
    if cx < 0 || cy < 0 || cx >= geom.cols as i64 || cy >= geom.rows as i64 {
        return None;
    }
    Some((cx as u16, cy as u16))
}

/// Re-colour the picker when the OS colour scheme flips. Only the palette
/// changes: whatever the user has typed, selected or scrolled to stays put.
fn install_color_scheme_listener(state: &Rc<RefCell<State>>) {
    let st = state.clone();
    color_scheme::subscribe(move |colors| st.borrow_mut().app.set_colors(colors));
}

/// Refresh the backend's grid on `window.resize` and force a full repaint.
///
/// `refresh_geometry` assigns the canvas `width`/`height`, which wipes the
/// drawing buffer even when the new value equals the old one. Ratatui's
/// `autoresize` only forces a full redraw when the *cell* grid changed, so a
/// resize that lands on the same grid — the canvas is CSS-capped, and a drag
/// that doesn't cross a cell boundary keeps the same cols × rows — would
/// leave the picker permanently blank. Clearing the terminal makes the diff
/// treat every cell as new, matching what the visibility-resume path at the
/// top of the RAF loop does.
fn install_resize(window: &web_sys::Window, terminal: &Rc<RefCell<Terminal<CanvasBackend>>>) {
    let term = terminal.clone();
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| refresh_terminal(&term));
    window
        .add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref())
        .expect("resize listener install");
    cb.forget();
}

/// Keep the picker pinned to the *visual* viewport, so it always sits in the
/// space the user can actually see.
///
/// Raising a mobile on-screen keyboard shrinks the visual viewport, and the
/// browser may pan it as well. Android shrinks the layout viewport along
/// with it, so `100vh` and the `resize` handler above would cope on their
/// own; iOS does neither — `100vh` stays the full screen height, and a pan
/// (which is what the viewport's `scroll` event reports, as a change of
/// `offsetTop`/`offsetLeft`) leaves the page anchored at the layout
/// viewport's origin, so the picker can end up clipped or behind the
/// keyboard. Publishing the height *and* the offsets for `web/style.css` to
/// size and translate against covers both.
fn install_viewport_sync(
    window: &web_sys::Window,
    document: &Document,
    terminal: &Rc<RefCell<Terminal<CanvasBackend>>>,
) {
    let Some(viewport) = window.visual_viewport() else {
        return;
    };
    let Some(root) = document
        .document_element()
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    // The backend measured the canvas before these variables existed, so the
    // first publish can already have resized it out from under the grid.
    if publish_viewport_metrics(&root, &viewport) {
        refresh_terminal(terminal);
    }

    let term = terminal.clone();
    let vp = viewport.clone();
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
        // Panning and pinch-zoom fire these continuously. Moving the page is
        // cheap, but only a height change resizes the canvas, and a repaint
        // per event would be visible — so redraw on that alone.
        if publish_viewport_metrics(&root, &vp) {
            refresh_terminal(&term);
        }
    });
    for name in ["resize", "scroll"] {
        viewport
            .add_event_listener_with_callback(name, cb.as_ref().unchecked_ref())
            .expect("visual viewport listener install");
    }
    cb.forget();
}

/// Publish the visual viewport's height and pan offsets as CSS variables.
///
/// Returns whether the *height* changed, which is the only one of the three
/// that resizes the canvas and so needs the grid re-measured; the offsets
/// only translate the page.
fn publish_viewport_metrics(root: &HtmlElement, viewport: &VisualViewport) -> bool {
    let style = root.style();
    set_px(&style, "--app-offset-left", viewport.offset_left());
    set_px(&style, "--app-offset-top", viewport.offset_top());

    let height = viewport.height();
    if height <= 0.0 {
        return false;
    }
    set_px(&style, "--app-height", height)
}

/// Set a pixel-valued custom property, returning whether it differed from
/// what was already there.
fn set_px(style: &CssStyleDeclaration, name: &str, value: f64) -> bool {
    let text = format!("{value}px");
    if style.get_property_value(name).ok().as_deref() == Some(text.as_str()) {
        return false;
    }
    if let Err(e) = style.set_property(name, &text) {
        web_sys::console::error_1(&e);
        return false;
    }
    true
}

/// Re-read the canvas geometry and force a full repaint.
fn refresh_terminal(terminal: &Rc<RefCell<Terminal<CanvasBackend>>>) {
    let mut term = terminal.borrow_mut();
    if let Err(e) = term.backend_mut().refresh_geometry() {
        web_sys::console::error_1(&JsValue::from_str(&format!("refresh_geometry: {e}")));
        return;
    }
    let _ = term.clear();
}

/// Drive the toast countdown on a fixed interval. The RAF loop redraws
/// every frame, so we only need to advance the toast clock here.
fn install_tick(window: &web_sys::Window, state: &Rc<RefCell<State>>) {
    let st = state.clone();
    let cb = Closure::<dyn FnMut()>::new(move || {
        let Some(perf) = web_sys::window().and_then(|w| w.performance()) else {
            return;
        };
        let now = perf.now();
        let mut s = st.borrow_mut();
        let dt = std::time::Duration::from_secs_f64(((now - s.last_perf_ms) / 1000.0).max(0.0));
        s.last_perf_ms = now;
        s.app.tick(dt);
    });
    let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
        cb.as_ref().unchecked_ref(),
        TICK_MS,
    );
    cb.forget();
}

/// Feed an action into the picker and carry out whatever it decided.
///
/// Takes the shared state by handle rather than by borrow because a pick
/// isn't done when this returns: the clipboard write settles later, and the
/// toast that reports it has to wait for that answer.
fn drive(state: &Rc<RefCell<State>>, action: Action) {
    let outcome = {
        let mut s = state.borrow_mut();
        let outcome = s.app.handle(action);
        // Keep the offscreen input in step with the picker. Rewrites that
        // came from the picker's side — Escape clearing the search — would
        // otherwise leave a stale value in the element for the next `input`
        // event to resurrect.
        s.text_input.set_value(s.app.search_text());
        outcome
    };
    let Outcome::Picked(text) = outcome else {
        return;
    };
    // Bind the result before matching on it: the failure arm re-borrows the
    // state, which would panic while the borrow behind this call is live.
    let started = state.borrow().clipboard.copy(&text);
    let promise = match started {
        Ok(promise) => promise,
        Err(e) => {
            report_copy_failure(state, &text, &JsValue::from_str(&e.to_string()));
            return;
        }
    };
    let state = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(_) => state.borrow_mut().app.show_toast(COPIED_PREFIX, text),
            Err(e) => report_copy_failure(&state, &text, &e),
        }
    });
}

/// Log a failed clipboard write and say so on screen, so a refusal can't
/// pass for a successful copy.
fn report_copy_failure(state: &Rc<RefCell<State>>, text: &str, cause: &JsValue) {
    web_sys::console::error_2(
        &JsValue::from_str(&format!("clipboard write of {text:?} failed")),
        cause,
    );
    state.borrow_mut().app.show_toast(COPY_FAILED_PREFIX, "");
}
