use gimoji_core::Action;
use web_sys::KeyboardEvent;

pub fn from_keyboard(event: &KeyboardEvent, search_is_empty: bool) -> Option<Action> {
    if event.ctrl_key() && event.key().as_str() == "c" {
        return Some(Action::Cancel);
    }
    match event.key().as_str() {
        "Enter" => Some(Action::PickFocused),
        "Escape" => Some(if search_is_empty {
            Action::Cancel
        } else {
            Action::ClearSearch
        }),
        "ArrowDown" => Some(Action::MoveDown),
        "ArrowUp" => Some(Action::MoveUp),
        "Backspace" => Some(Action::Backspace),
        k => {
            let mut chars = k.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if !event.ctrl_key() && !event.alt_key() && !event.meta_key() => {
                    Some(Action::Append(c))
                }
                _ => None,
            }
        }
    }
}
