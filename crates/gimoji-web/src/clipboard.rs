use gimoji_core::Clipboard;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

pub struct WebClipboard;

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl Clipboard for WebClipboard {
    type Error = Error;

    fn copy(&mut self, text: &str) -> Result<(), Self::Error> {
        let window = web_sys::window().ok_or_else(|| Error("no window".into()))?;
        let clipboard = window.navigator().clipboard();
        let promise = clipboard.write_text(text);
        let owned = text.to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = JsFuture::from(promise).await {
                web_sys::console::error_2(
                    &JsValue::from_str(&format!("clipboard write of {owned:?} failed")),
                    &e,
                );
            }
        });
        Ok(())
    }
}
