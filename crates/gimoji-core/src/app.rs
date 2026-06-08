use std::time::Duration;

use ratatui::{layout::Rect, Frame};

use crate::{
    colors::Colors,
    emoji::Emoji,
    search_entry::SearchEntry,
    selection_view::{EmojiSource, SelectionView, EMOJI_COLUMN_WIDTH, HIGHLIGHT_GUTTER_WIDTH},
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
    PickAt(usize),
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
    emoji_source: EmojiSource,
    last_rendered_rows: Vec<Rect>,
    last_visible_emojis: Vec<VisibleEmoji>,
}

/// One row of the picker as positioned during the last render. The web
/// frontend consumes this list to place HTML emoji overlays at the right
/// cell coordinates; native ignores it.
#[derive(Debug, Clone, Copy)]
pub struct VisibleEmoji {
    /// Cell rect of the emoji glyph column (3 cells wide × 1 cell high).
    pub cell: Rect,
    /// Unicode emoji to render at that position.
    pub emoji: &'static str,
}

impl<'c> App<'c> {
    /// Build a picker that emits emoji glyphs directly into the buffer.
    /// The host (e.g. crossterm + terminal) draws them from its own font.
    pub fn new(emojis: &'static [Emoji], colors: &'c Colors) -> Self {
        Self::build(emojis, colors, EmojiSource::InCanvas)
    }

    /// Build a picker whose emoji column is left blank in the buffer so a
    /// separate layer (e.g. the web frontend's DOM overlay) can paint the
    /// glyphs. Keeps column alignment stable across rows even when the
    /// emoji is a ZWJ sequence whose `unicode-width` count doesn't match
    /// its rendered width.
    pub fn with_emoji_overlay(emojis: &'static [Emoji], colors: &'c Colors) -> Self {
        Self::build(emojis, colors, EmojiSource::Overlay)
    }

    fn build(emojis: &'static [Emoji], colors: &'c Colors, source: EmojiSource) -> Self {
        Self {
            search: SearchEntry::new(colors),
            selection: SelectionView::new(emojis, colors, source),
            colors,
            toast: None,
            emoji_source: source,
            last_rendered_rows: Vec::new(),
            last_visible_emojis: Vec::new(),
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
            Action::PickAt(i) => {
                let view = self.selection.filtered_view(self.search.text());
                match view.get(i) {
                    Some(emoji) => Outcome::Picked(emoji.emoji().to_string()),
                    None => Outcome::Continue,
                }
            }
            Action::Cancel => Outcome::Cancelled,
        }
    }

    /// Show a confirmation toast for a freshly picked emoji. The prefix
    /// is rendered into the buffer, the emoji glyph is rendered by the
    /// canvas on native and by the DOM overlay on web — see
    /// [`Self::toast_overlay_emoji`].
    pub fn show_toast(&mut self, prefix: impl Into<String>, emoji: impl Into<String>) {
        self.toast = Some(Toast::new(prefix, emoji, self.emoji_source));
    }

    pub fn has_toast(&self) -> bool {
        self.toast.is_some()
    }

    /// Cell position and glyph of the toast's emoji slot, if any. Used by
    /// the web frontend to extend its DOM overlay with the toast emoji.
    pub fn toast_overlay_emoji(&self) -> Option<(Rect, &str)> {
        self.toast.as_ref().and_then(|t| t.emoji_cell())
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

        let list_area = chunks[1];
        let inner_left = list_area.x.saturating_add(2);
        let inner_top = list_area.y.saturating_add(2);
        let inner_w = list_area.width.saturating_sub(4);
        let inner_bottom = list_area.y + list_area.height.saturating_sub(1);
        let visible_h = inner_bottom.saturating_sub(inner_top);
        // Emoji glyph column starts after the row's border+padding gutter
        // and the always-reserved highlight-symbol gutter.
        let emoji_x = inner_left.saturating_add(HIGHLIGHT_GUTTER_WIDTH);

        // Capture the visible row contents into a local while the view
        // borrows `self.selection`, then write into `self.last_*` once the
        // view is dropped. Reading `view.offset()` *after* the render is
        // important: the Table widget updates the offset during render to
        // scroll the selection into view.
        let mut visible: Vec<(u16, &'static str)> = Vec::new();
        {
            let mut view = self.selection.filtered_view(self.search.text());
            let visible_count = view.visible_count();
            frame.render_widget(&mut view, chunks[1]);
            let view_offset = view.offset();
            let row_count = visible_count
                .saturating_sub(view_offset)
                .min(visible_h as usize);
            visible.reserve_exact(row_count);
            for i in 0..row_count {
                let y = inner_top + i as u16;
                let emoji = match view.get(view_offset + i) {
                    Some(e) => e.emoji(),
                    None => continue,
                };
                visible.push((y, emoji));
            }
        }

        self.last_rendered_rows.clear();
        self.last_visible_emojis.clear();
        for (y, emoji) in visible {
            self.last_rendered_rows.push(Rect {
                x: inner_left,
                y,
                width: inner_w,
                height: 1,
            });
            self.last_visible_emojis.push(VisibleEmoji {
                cell: Rect {
                    x: emoji_x,
                    y,
                    width: EMOJI_COLUMN_WIDTH,
                    height: 1,
                },
                emoji,
            });
        }

        if let Some(toast) = &mut self.toast {
            toast.render(frame.area(), frame.buffer_mut());
        }
    }

    pub fn row_rect(&self, row_index: usize) -> Option<Rect> {
        self.last_rendered_rows.get(row_index).copied()
    }

    pub fn visible_emojis(&self) -> &[VisibleEmoji] {
        &self.last_visible_emojis
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
    fn pick_at_index_returns_the_corresponding_emoji() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        let outcome = app.handle(Action::PickAt(2));
        assert_eq!(outcome, Outcome::Picked(emojis[2].emoji().to_string()));
    }

    #[test]
    fn pick_at_out_of_bounds_index_returns_continue() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        assert_eq!(
            app.handle(Action::PickAt(emojis.len() + 100)),
            Outcome::Continue
        );
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
        app.show_toast("Copied", "🎉");
        assert!(app.has_toast());
        app.tick(Duration::from_secs(5));
        assert!(!app.has_toast());
    }

    #[test]
    fn row_rect_returns_none_before_first_render() {
        let (emojis, colors) = fixture();
        let app = App::new(emojis, &colors);
        assert!(app.row_rect(0).is_none());
    }
}
