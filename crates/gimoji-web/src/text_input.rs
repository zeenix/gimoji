use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlInputElement};

/// The offscreen `<input>` that owns text entry for the picker.
///
/// The picker paints into a `<canvas>`, which can't take focus and so can't
/// raise a mobile on-screen keyboard on its own — a tap on the search box
/// used to do nothing at all. This wraps a real (but visually hidden) input
/// element that stands in for it: [`Self::focus`] is what brings the
/// keyboard up, and the element's `input` events are the source of truth
/// for the search text on every platform.
///
/// Mirroring a whole value rather than replaying key presses is deliberate:
/// mobile keyboards routinely rewrite spans of text (autocorrect, swipe
/// typing, IME composition) and report characters through `input` alone,
/// with no usable `keydown`.
pub struct TextInput {
    element: HtmlInputElement,
    id: &'static str,
}

impl TextInput {
    /// Look up the element with id `id`, which the page markup is expected
    /// to provide (see `web/index.html`).
    pub fn new(document: &Document, id: &'static str) -> Result<Self, String> {
        let element = document
            .get_element_by_id(id)
            .ok_or_else(|| format!("no element with id {id:?}"))?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| format!("element {id:?} is not an <input>"))?;

        Ok(Self { element, id })
    }

    pub fn element(&self) -> &HtmlInputElement {
        &self.element
    }

    /// Whether this element currently holds DOM focus, i.e. whether typing
    /// is already being delivered to it as `input` events.
    pub fn is_focused(&self, document: &Document) -> bool {
        document
            .active_element()
            .is_some_and(|active| active.id() == self.id)
    }

    /// Take focus from inside a user gesture, raising the on-screen keyboard
    /// on mobile.
    ///
    /// Browsers raise the keyboard on a focus *transition*, and `focus()` on
    /// the element that already holds focus is a no-op — which is exactly
    /// the state left behind when someone dismisses the keyboard with its own
    /// "done" key without moving focus. Blurring first makes the transition
    /// real, so a tap always brings the keyboard back.
    ///
    /// Only a gesture counts: called from a timer or a settled promise this
    /// moves focus without showing anything.
    pub fn focus_from_gesture(&self, document: &Document) {
        if self.is_focused(document) {
            if let Err(e) = self.element.blur() {
                web_sys::console::error_1(&e);
            }
        }
        self.focus();
    }

    /// Take focus without any of the gesture handling above. Raises no
    /// keyboard on its own.
    pub fn focus(&self) {
        if let Err(e) = self.element.focus() {
            web_sys::console::error_1(&e);
        }
    }

    /// Mirror `text` into the element, unless it already matches.
    ///
    /// Guarding on equality matters: assigning `value` collapses the
    /// selection and moves the caret to the end, so doing it on every
    /// keystroke would fight the user's own editing. In practice only
    /// picker-side rewrites (Escape clearing the search) get this far.
    pub fn set_value(&self, text: &str) {
        if self.element.value() != text {
            self.element.set_value(text);
        }
    }

    pub fn value(&self) -> String {
        self.element.value()
    }
}
