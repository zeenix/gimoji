use std::{cell::RefCell, num::NonZeroU32, rc::Rc};

use gimoji_core::{App, Clipboard as _, Colors, Outcome, EMOJIS};
use ratatui::Terminal;
use ratatui_wgpu::{Builder, Dimensions, Font, WgpuBackend};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{HtmlCanvasElement, KeyboardEvent, PointerEvent};

mod clipboard;
mod color_scheme;
mod emoji_overlay;
mod input;

static JB_MONO: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

struct State {
    app: App<'static>,
    terminal: Terminal<WgpuBackend<'static, 'static>>,
    clipboard: clipboard::WebClipboard,
    overlay: emoji_overlay::EmojiOverlay,
    canvas: HtmlCanvasElement,
    last_perf_ms: f64,
    dirty: bool,
}

#[wasm_bindgen(start)]
pub async fn run() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas: HtmlCanvasElement = document
        .get_element_by_id("gimoji-canvas")
        .ok_or_else(|| JsValue::from_str("no canvas"))?
        .dyn_into()?;
    fit_canvas(&canvas);

    let colors: &'static Colors = Box::leak(Box::new(color_scheme::detect()));

    let font =
        Font::new(JB_MONO).ok_or_else(|| JsValue::from_str("invalid JetBrains Mono font"))?;
    let backend = Builder::from_font(font)
        .with_width_and_height(Dimensions {
            width: NonZeroU32::new(canvas.width().max(1)).unwrap(),
            height: NonZeroU32::new(canvas.height().max(1)).unwrap(),
        })
        .build_with_target(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .await
        .map_err(|e| JsValue::from_str(&format!("wgpu init failed: {e}")))?;

    let terminal =
        Terminal::new(backend).map_err(|e| JsValue::from_str(&format!("terminal init: {e}")))?;
    let app = App::with_emoji_overlay(EMOJIS, colors);
    let perf_ms = window
        .performance()
        .ok_or_else(|| JsValue::from_str("no performance"))?
        .now();
    let overlay = emoji_overlay::EmojiOverlay::new(&document)?;
    let state = Rc::new(RefCell::new(State {
        app,
        terminal,
        clipboard: clipboard::WebClipboard,
        overlay,
        canvas: canvas.clone(),
        last_perf_ms: perf_ms,
        dirty: true,
    }));

    install_keydown(&state);
    install_pointerdown(&canvas, &state);
    install_resize(&canvas, &state);
    install_color_scheme_listener(&state);
    schedule_raf(state);
    Ok(())
}

fn fit_canvas(canvas: &HtmlCanvasElement) {
    let window = web_sys::window().unwrap();
    let dpr = window.device_pixel_ratio().max(1.0);
    let w = (window.inner_width().unwrap().as_f64().unwrap() * dpr) as u32;
    let h = (window.inner_height().unwrap().as_f64().unwrap() * dpr) as u32;
    canvas.set_width(w.max(1));
    canvas.set_height(h.max(1));
}

fn install_keydown(state: &Rc<RefCell<State>>) {
    let window = web_sys::window().unwrap();
    let st = state.clone();
    let cb = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        let mut s = st.borrow_mut();
        let search_empty = s.app.search_text().is_empty();
        if let Some(action) = input::from_keyboard(&event, search_empty) {
            event.prevent_default();
            drive(&mut s, action);
        }
    });
    window
        .add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())
        .unwrap();
    cb.forget();
}

fn install_pointerdown(canvas: &HtmlCanvasElement, state: &Rc<RefCell<State>>) {
    let st = state.clone();
    let canvas_for_handler = canvas.clone();
    let cb = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
        let mut s = st.borrow_mut();
        let rect = canvas_for_handler.get_bounding_client_rect();
        let Ok(term_size) = s.terminal.size() else {
            return;
        };
        let cell_w = (rect.width() / term_size.width as f64).max(1.0);
        let cell_h = (rect.height() / term_size.height as f64).max(1.0);
        let cx = ((event.client_x() as f64 - rect.left()) / cell_w) as u16;
        let cy = ((event.client_y() as f64 - rect.top()) / cell_h) as u16;
        for i in 0.. {
            let Some(rr) = s.app.row_rect(i) else { break };
            if cx >= rr.x && cx < rr.x + rr.width && cy >= rr.y && cy < rr.y + rr.height {
                event.prevent_default();
                drive(&mut s, gimoji_core::Action::PickAt(i));
                break;
            }
        }
    });
    canvas
        .add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())
        .unwrap();
    cb.forget();
}

fn install_resize(canvas: &HtmlCanvasElement, state: &Rc<RefCell<State>>) {
    let window = web_sys::window().unwrap();
    let st = state.clone();
    let canvas = canvas.clone();
    let cb = Closure::<dyn FnMut()>::new(move || {
        fit_canvas(&canvas);
        let mut s = st.borrow_mut();
        s.overlay.invalidate_geometry();
        s.dirty = true;
    });
    window
        .add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref())
        .unwrap();
    cb.forget();
}

fn install_color_scheme_listener(state: &Rc<RefCell<State>>) {
    let st = state.clone();
    color_scheme::subscribe(move |new_colors| {
        let leaked: &'static Colors = Box::leak(Box::new(new_colors));
        let mut s = st.borrow_mut();
        s.app = App::with_emoji_overlay(EMOJIS, leaked);
        s.dirty = true;
    });
}

fn drive(s: &mut State, action: gimoji_core::Action) {
    match s.app.handle(action) {
        Outcome::Continue => s.dirty = true,
        Outcome::Picked(text) => {
            let _ = s.clipboard.copy(&text);
            s.app.show_toast("Copied", text);
            s.dirty = true;
        }
        Outcome::Cancelled => {}
    }
}

fn schedule_raf(state: Rc<RefCell<State>>) {
    fn next(state: Rc<RefCell<State>>) {
        let window = web_sys::window().unwrap();
        let st = state.clone();
        let cb = Closure::once_into_js(move |_: f64| {
            {
                let mut s = st.borrow_mut();
                let now = web_sys::window().unwrap().performance().unwrap().now();
                let dt =
                    std::time::Duration::from_secs_f64(((now - s.last_perf_ms) / 1000.0).max(0.0));
                s.last_perf_ms = now;
                s.app.tick(dt);
                if s.dirty || s.app.has_toast() {
                    let State {
                        app,
                        terminal,
                        overlay,
                        canvas,
                        ..
                    } = &mut *s;
                    // Don't propagate a render failure as a panic: that
                    // would tear down the wasm instance and freeze the
                    // page on a transient wgpu hiccup. Log and skip the
                    // frame instead — the next raf tick re-tries.
                    if let Err(e) = terminal.draw(|f| app.render(f)) {
                        web_sys::console::error_1(
                            &format!("gimoji: terminal draw failed: {e}").into(),
                        );
                    } else if let Ok(term_size) = terminal.size() {
                        overlay.sync(
                            canvas,
                            term_size,
                            app.visible_emojis(),
                            app.toast_overlay_emoji(),
                        );
                    }
                    s.dirty = false;
                }
            }
            next(st);
        });
        window
            .request_animation_frame(cb.as_ref().unchecked_ref())
            .unwrap();
    }
    next(state);
}
