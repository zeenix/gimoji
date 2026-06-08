use std::time::Duration;

use ratatui::Frame;

use crate::{
    colors::Colors, emoji::Emoji, search_entry::SearchEntry, selection_view::SelectionView,
    toast::Toast,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Append(char),
    Backspace,
    ClearSearch,
    MoveUp,
    MoveDown,
    PickFocused,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Picked(String),
    Cancelled,
}

pub struct App<'c> {
    search: SearchEntry<'c>,
    selection: SelectionView<'c>,
    #[allow(dead_code)]
    colors: &'c Colors,
    toast: Option<Toast>,
}

impl<'c> App<'c> {
    pub fn new(emojis: &'static [Emoji], colors: &'c Colors) -> Self {
        Self {
            search: SearchEntry::new(colors),
            selection: SelectionView::new(emojis, colors),
            colors,
            toast: None,
        }
    }

    pub fn search_text(&self) -> &str {
        self.search.text()
    }

    pub fn handle(&mut self, action: Action) -> Outcome {
        match action {
            Action::Append(c) => {
                self.search.append(c);
                Outcome::Continue
            }
            Action::Backspace => {
                self.search.delete_last();
                Outcome::Continue
            }
            Action::ClearSearch => {
                self.search.delete_all();
                Outcome::Continue
            }
            Action::MoveDown => {
                let mut view = self.selection.filtered_view(self.search.text());
                view.move_down();
                Outcome::Continue
            }
            Action::MoveUp => {
                let mut view = self.selection.filtered_view(self.search.text());
                view.move_up();
                Outcome::Continue
            }
            Action::PickFocused => {
                let view = self.selection.filtered_view(self.search.text());
                match view.selected() {
                    Some(emoji) => Outcome::Picked(emoji.emoji().to_string()),
                    None => Outcome::Continue,
                }
            }
            Action::Cancel => Outcome::Cancelled,
        }
    }

    pub fn show_toast(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast::new(text));
    }

    pub fn has_toast(&self) -> bool {
        self.toast.is_some()
    }

    pub fn tick(&mut self, dt: Duration) {
        if let Some(toast) = &mut self.toast {
            toast.tick(dt);
            if toast.is_expired() {
                self.toast = None;
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        use ratatui::layout::{Constraint, Layout};
        let chunks = Layout::default()
            .constraints([Constraint::Min(5), Constraint::Percentage(100)].as_ref())
            .margin(1)
            .split(frame.area());

        frame.render_widget(&self.search, chunks[0]);

        let mut view = self.selection.filtered_view(self.search.text());
        frame.render_widget(&mut view, chunks[1]);

        if let Some(toast) = &mut self.toast {
            toast.render(frame.area(), frame.buffer_mut());
        }
    }
}

pub trait Clipboard {
    type Error: std::fmt::Display;
    fn copy(&mut self, text: &str) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{colors::Colors, emoji::Emoji};

    fn fixture() -> (&'static [Emoji], Colors) {
        (crate::emoji::EMOJIS, Colors::dark())
    }

    #[test]
    fn append_pushes_a_character_into_search() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        assert_eq!(app.handle(Action::Append('h')), Outcome::Continue);
        assert_eq!(app.handle(Action::Append('i')), Outcome::Continue);
        assert_eq!(app.search_text(), "hi");
    }

    #[test]
    fn backspace_removes_last_character() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        app.handle(Action::Append('a'));
        app.handle(Action::Append('b'));
        app.handle(Action::Backspace);
        assert_eq!(app.search_text(), "a");
    }

    #[test]
    fn clear_search_empties_text() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        app.handle(Action::Append('x'));
        app.handle(Action::Append('y'));
        app.handle(Action::ClearSearch);
        assert_eq!(app.search_text(), "");
    }

    #[test]
    fn move_down_advances_selection_within_filtered_list() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        app.handle(Action::MoveDown);
        assert_eq!(app.handle(Action::MoveUp), Outcome::Continue);
    }

    #[test]
    fn pick_focused_returns_the_emoji_at_index_zero_for_empty_search() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        let outcome = app.handle(Action::PickFocused);
        let Outcome::Picked(s) = outcome else {
            panic!("expected Picked, got {outcome:?}");
        };
        assert_eq!(s, emojis[0].emoji());
    }

    #[test]
    fn pick_focused_after_move_down_picks_the_next_emoji() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        app.handle(Action::MoveDown);
        let Outcome::Picked(s) = app.handle(Action::PickFocused) else {
            panic!();
        };
        assert_eq!(s, emojis[1].emoji());
    }

    #[test]
    fn pick_focused_with_no_matches_returns_continue() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        for c in "zzzzzzzz_no_match_zzzzzz".chars() {
            app.handle(Action::Append(c));
        }
        assert_eq!(app.handle(Action::PickFocused), Outcome::Continue);
    }

    #[test]
    fn cancel_returns_cancelled() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        assert_eq!(app.handle(Action::Cancel), Outcome::Cancelled);
    }

    #[test]
    fn tick_clears_expired_toast() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        app.show_toast("Copied 🎉");
        assert!(app.has_toast());
        app.tick(Duration::from_secs(5));
        assert!(!app.has_toast());
    }
}
