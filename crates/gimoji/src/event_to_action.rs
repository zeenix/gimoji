use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use gimoji_core::Action;

#[derive(Debug)]
pub struct ExitSignal;

pub fn from_key_event(
    event: KeyEvent,
    search_is_empty: bool,
) -> Result<Option<Action>, ExitSignal> {
    match event.code {
        KeyCode::Char(c) if c == 'c' && event.modifiers.contains(KeyModifiers::CONTROL) => {
            Err(ExitSignal)
        }
        KeyCode::Enter => Ok(Some(Action::PickFocused)),
        KeyCode::Esc => Ok(Some(if search_is_empty {
            Action::Cancel
        } else {
            Action::ClearSearch
        })),
        KeyCode::Down => Ok(Some(Action::MoveDown)),
        KeyCode::Up => Ok(Some(Action::MoveUp)),
        KeyCode::Backspace => Ok(Some(Action::Backspace)),
        KeyCode::Char(c) => Ok(Some(Action::Append(c))),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn enter_maps_to_pick_focused() {
        assert_eq!(
            from_key_event(key(KeyCode::Enter), true).unwrap(),
            Some(Action::PickFocused)
        );
    }

    #[test]
    fn esc_on_empty_search_is_cancel() {
        assert_eq!(
            from_key_event(key(KeyCode::Esc), true).unwrap(),
            Some(Action::Cancel)
        );
    }

    #[test]
    fn esc_with_search_clears() {
        assert_eq!(
            from_key_event(key(KeyCode::Esc), false).unwrap(),
            Some(Action::ClearSearch)
        );
    }

    #[test]
    fn ctrl_c_signals_exit() {
        let ev = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(from_key_event(ev, true), Err(ExitSignal)));
    }

    #[test]
    fn plain_letter_appends() {
        assert_eq!(
            from_key_event(key(KeyCode::Char('a')), true).unwrap(),
            Some(Action::Append('a'))
        );
    }
}
