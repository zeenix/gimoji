use gimoji_core::Action;
use web_sys::KeyboardEvent;

/// Translate a browser [`KeyboardEvent`] into a picker [`Action`].
///
/// Returns `None` when the key is uninteresting (lets the browser handle
/// it). `search_is_empty` is consulted so Escape clears a non-empty search
/// box before it cancels the picker.
///
/// `text_input_focused` says whether the offscreen `<input>` (see
/// `crate::text_input`) currently has focus. When it does, editing keys are
/// left alone: the element applies them itself and reports the result as an
/// `input` event, which becomes an [`Action::SetSearch`]. Claiming them here
/// too would either double-apply the edit or, via `preventDefault`, stop the
/// element from ever seeing it.
pub fn from_keyboard(
    event: &KeyboardEvent,
    search_is_empty: bool,
    text_input_focused: bool,
) -> Option<Action> {
    // Browsers keep delivering `keydown` while an IME is composing, flagged
    // with `isComposing`. Those presses belong to the candidate window —
    // Enter confirms a candidate, Escape cancels the composition, the arrows
    // walk the candidate list — so claiming them here (and the caller then
    // calling `preventDefault`) would hijack the composition instead of
    // supporting it.
    //
    // `isComposing` alone misses the two edges: `compositionstart` can arrive
    // after the keydown that opened the composition and `compositionend`
    // before the one that closed it, leaving those unflagged even though the
    // IME, not the picker, is what they belong to. Both still carry the
    // legacy `keyCode` 229, which is why MDN has you check the pair.
    if event.is_composing() || event.key_code() == IME_KEY_CODE {
        return None;
    }

    let key = event.key();
    let ctrl = event.ctrl_key();
    let alt = event.alt_key();
    let meta = event.meta_key();

    // Ctrl-C cancels regardless of search state, matching the native binding.
    if ctrl && matches!(key.as_str(), "c" | "C") {
        return Some(Action::Cancel);
    }

    match key.as_str() {
        "Enter" => Some(Action::PickFocused),
        "Escape" => Some(if search_is_empty {
            Action::Cancel
        } else {
            Action::ClearSearch
        }),
        "ArrowDown" => Some(Action::MoveDown),
        "ArrowUp" => Some(Action::MoveUp),
        "Backspace" if !text_input_focused => Some(Action::Backspace),
        // Meta is checked alongside ctrl/alt so macOS `Cmd+letter` shortcuts
        // reach the browser instead of typing into the search box.
        s if !text_input_focused && !ctrl && !alt && !meta && s.chars().count() == 1 => {
            s.chars().next().map(Action::Append)
        }
        _ => None,
    }
}

/// Legacy `keyCode` an IME reports for a key it is handling itself. No real
/// key produces it, so treating it as "not ours" costs nothing.
const IME_KEY_CODE: u32 = 229;
