use gimoji_core::Colors;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::MediaQueryListEvent;

pub fn detect() -> Colors {
    let window = web_sys::window().expect("no window");
    let mq = window
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten();
    let dark = mq.as_ref().map(|m| m.matches()).unwrap_or(true);
    if dark {
        Colors::dark()
    } else {
        Colors::light()
    }
}

pub fn subscribe<F: 'static + FnMut(Colors)>(mut on_change: F) {
    let window = web_sys::window().expect("no window");
    let Some(mq) = window
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten()
    else {
        return;
    };
    let cb = Closure::<dyn FnMut(MediaQueryListEvent)>::new(move |e: MediaQueryListEvent| {
        on_change(if e.matches() {
            Colors::dark()
        } else {
            Colors::light()
        });
    });
    mq.add_event_listener_with_callback("change", cb.as_ref().unchecked_ref())
        .expect("addEventListener");
    cb.forget();
}
