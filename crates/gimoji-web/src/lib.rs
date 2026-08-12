use std::{cell::RefCell, rc::Rc};

use canvas_backend::CanvasBackend;
use gimoji_core::{Action, App, Outcome, EMOJIS};
use ratatui::{layout::Rect, Terminal};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{KeyboardEvent, PointerEvent};

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

/// The element id of the `<canvas>` the picker paints into. Matches the
/// markup in `web/index.html`.
const CANVAS_ID: &str = "gimoji-canvas";

/// Cadence of the toast countdown driver. The toast lifetime is measured in
/// seconds, so 250 ms is fine-grained enough that expiry feels snappy without
/// waking the wasm runtime more often than necessary.
const TICK_MS: i32 = 250;

/// Toast prefix for a clipboard write the browser confirmed.
const COPIED_PREFIX: &str = "Copied";
/// Toast prefix for one it refused.
const COPY_FAILED_PREFIX: &str = "Copy failed";

struct State {
    app: App<'static>,
    clipboard: clipboard::WebClipboard,
    last_perf_ms: f64,
}

#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
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

    let state = Rc::new(RefCell::new(State {
        app,
        clipboard: clipboard::WebClipboard,
        last_perf_ms: performance.now(),
    }));
    let terminal = Rc::new(RefCell::new(terminal));

    install_keydown(&window, &state);
    install_pointerdown(&terminal, &state);
    install_color_scheme_listener(&state);
    install_tick(&window, &state);
    install_resize(&window, &terminal);
    install_raf_loop(&window, &terminal, &state);

    Ok(())
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

fn install_keydown(window: &web_sys::Window, state: &Rc<RefCell<State>>) {
    let st = state.clone();
    let cb = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        let search_empty = st.borrow().app.search_text().is_empty();
        let Some(action) = input::from_keyboard(&event, search_empty) else {
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

fn install_pointerdown(
    terminal: &Rc<RefCell<Terminal<CanvasBackend>>>,
    state: &Rc<RefCell<State>>,
) {
    let st = state.clone();
    let term = terminal.clone();
    let cb = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
        let Some((cx, cy)) = pixel_to_cell(
            &term.borrow(),
            event.client_x() as f64,
            event.client_y() as f64,
        ) else {
            return;
        };
        // `hit_test` accounts for the list's scroll offset, so the index is
        // valid against the filtered list `PickAt` indexes into.
        let Some(index) = st.borrow().app.hit_test(cx, cy) else {
            return;
        };
        event.prevent_default();
        drive(&st, Action::PickAt(index));
    });
    web_sys::window()
        .and_then(|w| w.document())
        .expect("document")
        .add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())
        .expect("pointerdown listener install");
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
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
        let mut term = term.borrow_mut();
        if let Err(e) = term.backend_mut().refresh_geometry() {
            web_sys::console::error_1(&JsValue::from_str(&format!("refresh_geometry: {e}")));
            return;
        }
        let _ = term.clear();
    });
    window
        .add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref())
        .expect("resize listener install");
    cb.forget();
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
    let outcome = state.borrow_mut().app.handle(action);
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
