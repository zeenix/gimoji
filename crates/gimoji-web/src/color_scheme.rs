use gimoji_core::Colors;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::MediaQueryListEvent;

/// The two palettes, as statics rather than values handed out by copy: the
/// picker borrows its palette for the lifetime of the page, so a scheme
/// change re-points it at the other static instead of allocating a fresh
/// palette that would have to be leaked to stay borrowable.
static DARK: Colors = Colors::dark();
static LIGHT: Colors = Colors::light();

pub fn detect() -> &'static Colors {
    let window = web_sys::window().expect("no window");
    let mq = window
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten();
    let dark = mq.as_ref().map(|m| m.matches()).unwrap_or(true);

    palette(dark)
}

pub fn subscribe<F>(mut on_change: F)
where
    F: 'static + FnMut(&'static Colors),
{
    let window = web_sys::window().expect("no window");
    let Some(mq) = window
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten()
    else {
        return;
    };
    let cb = Closure::<dyn FnMut(MediaQueryListEvent)>::new(move |e: MediaQueryListEvent| {
        on_change(palette(e.matches()));
    });
    mq.add_event_listener_with_callback("change", cb.as_ref().unchecked_ref())
        .expect("addEventListener");
    cb.forget();
}

fn palette(dark: bool) -> &'static Colors {
    if dark {
        &DARK
    } else {
        &LIGHT
    }
}
