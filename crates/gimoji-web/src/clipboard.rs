/// The browser clipboard the picker copies picked emojis into.
pub struct WebClipboard;

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl WebClipboard {
    /// Start writing `text` to the clipboard, handing back the promise the
    /// browser settles once the write has actually happened.
    ///
    /// The write is asynchronous and can still be refused after it starts —
    /// a denied permission or a non-secure context both reject — so the
    /// promise is the caller's only way to tell the user what really
    /// happened. `Err` covers the synchronous failures alone.
    pub fn copy(&self, text: &str) -> Result<js_sys::Promise, Error> {
        let window = web_sys::window().ok_or_else(|| Error("no window".into()))?;

        Ok(window.navigator().clipboard().write_text(text))
    }
}
