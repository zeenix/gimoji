use gimoji_core::Action;
use web_sys::KeyboardEvent;

/// Translate a browser [`KeyboardEvent`] into a picker [`Action`].
///
/// Returns `None` when the key is uninteresting (lets the browser handle
/// it). `search_is_empty` is consulted so Escape clears a non-empty search
/// box before it cancels the picker.
pub fn from_keyboard(event: &KeyboardEvent, search_is_empty: bool) -> Option<Action> {
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
        "Backspace" => Some(Action::Backspace),
        // Meta is checked alongside ctrl/alt so macOS `Cmd+letter` shortcuts
        // reach the browser instead of typing into the search box.
        s if !ctrl && !alt && !meta && s.chars().count() == 1 => {
            s.chars().next().map(Action::Append)
        }
        _ => None,
    }
}
