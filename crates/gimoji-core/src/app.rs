use std::time::Duration;

use ratatui::{
    layout::{Position, Rect},
    Frame,
};

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
    colors: &'c Colors,
    toast: Option<Toast>,
    emoji_source: EmojiSource,
    last_rendered_rows: Vec<Rect>,
    last_row_offset: usize,
    last_visible_emojis: Vec<VisibleEmoji>,
    last_emoji_band: Option<Rect>,
}

/// One row of the picker as positioned during the last render.
/// Overlay-rendering frontends consume this list to paint the emoji glyphs
/// at the right cell coordinates; frontends that emit glyphs into the
/// buffer ignore it.
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
    /// separate layer (e.g. the web frontend's overlay pass) can paint the
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
            last_row_offset: 0,
            last_visible_emojis: Vec::new(),
            last_emoji_band: None,
        }
    }

    pub fn search_text(&self) -> &str {
        self.search.text()
    }

    /// Swap the palette in place, e.g. when the OS colour scheme flips.
    ///
    /// Everything the user built up — search text, selection, scroll
    /// position and any visible toast — survives; only the colours change.
    pub fn set_colors(&mut self, colors: &'c Colors) {
        self.colors = colors;
        self.search.set_colors(colors);
        self.selection.set_colors(colors);
        if let Some(toast) = &mut self.toast {
            toast.set_colors(colors);
        }
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

    /// Show a confirmation toast for a freshly picked emoji. The prefix is
    /// rendered into the buffer; the emoji glyph goes into the buffer too,
    /// unless the picker was built for overlay rendering — see
    /// [`Self::toast_overlay_emoji`].
    pub fn show_toast(&mut self, prefix: impl Into<String>, emoji: impl Into<String>) {
        self.toast = Some(Toast::new(prefix, emoji, self.emoji_source, self.colors));
    }

    pub fn has_toast(&self) -> bool {
        self.toast.is_some()
    }

    /// Cell position and glyph of the toast's emoji slot, if any. Used by
    /// the web frontend to extend its overlay pass with the toast emoji.
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
        self.render_in_area(frame, frame.area());
    }

    /// Render the picker into an explicit sub-area of `frame` instead of
    /// the frame's full area. The web frontend uses this to centre a
    /// size-capped picker inside a larger cell grid; native callers
    /// generally want [`Self::render`].
    pub fn render_in_area(&mut self, frame: &mut Frame<'_>, area: Rect) {
        use ratatui::layout::{Constraint, Layout};
        let chunks = Layout::default()
            .constraints([Constraint::Min(5), Constraint::Percentage(100)].as_ref())
            .margin(1)
            .split(area);

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
        let row_offset;
        {
            let mut view = self.selection.filtered_view(self.search.text());
            let visible_count = view.visible_count();
            frame.render_widget(&mut view, chunks[1]);
            row_offset = view.offset();
            let row_count = visible_count
                .saturating_sub(row_offset)
                .min(visible_h as usize);
            visible.reserve_exact(row_count);
            for i in 0..row_count {
                let y = inner_top + i as u16;
                let emoji = match view.get(row_offset + i) {
                    Some(e) => e.emoji(),
                    None => continue,
                };
                visible.push((y, emoji));
            }
        }

        self.last_rendered_rows.clear();
        self.last_row_offset = row_offset;
        self.last_visible_emojis.clear();
        // Extend the band by one cell into the block's top padding row so
        // the external overlay catches any upward pixel bleed from the
        // first row's glyph (some colour-emoji glyphs render slightly above
        // their em-box top). The padding row carries no buffer content, so
        // wiping it is harmless.
        let band_y = inner_top.saturating_sub(1);
        let band_h = visible_h.saturating_add(inner_top - band_y);
        self.last_emoji_band = Some(Rect {
            x: emoji_x,
            y: band_y,
            width: EMOJI_COLUMN_WIDTH,
            height: band_h,
        });
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
            toast.render(area, frame.buffer_mut());
        }
    }

    /// Index into the current filtered list of the row covering the cell at
    /// (`x`, `y`) in the last rendered frame, or `None` when no row does.
    ///
    /// The index includes the list's scroll offset, so it addresses the same
    /// list [`Action::PickAt`] indexes into — pointer-driven frontends can
    /// feed the result straight back in.
    pub fn hit_test(&self, x: u16, y: u16) -> Option<usize> {
        let visible_index = self
            .last_rendered_rows
            .iter()
            .position(|row| row.contains(Position { x, y }))?;

        Some(self.last_row_offset + visible_index)
    }

    pub fn visible_emojis(&self) -> &[VisibleEmoji] {
        &self.last_visible_emojis
    }

    /// Cell rectangle covering the full vertical band of the emoji column in
    /// the picker's selection list. Overlay-rendering frontends wipe this band
    /// each frame before re-painting glyphs, so any rows that fell out of
    /// the visible list (e.g. due to filtering) leave no stale glyphs behind
    /// — overlay mode keeps these cells blank in the buffer, so ratatui's
    /// diff loop never asks the backend to repaint them.
    pub fn emoji_overlay_band(&self) -> Option<Rect> {
        self.last_emoji_band
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
    fn arrow_keys_with_no_matches_return_continue() {
        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        for c in "zzzzzzzz_no_match_zzzzzz".chars() {
            app.handle(Action::Append(c));
        }
        assert_eq!(app.handle(Action::MoveDown), Outcome::Continue);
        assert_eq!(app.handle(Action::MoveUp), Outcome::Continue);
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
    fn set_colors_keeps_the_search_selection_and_toast() {
        let (emojis, dark) = fixture();
        let light = Colors::light();
        let mut app = App::new(emojis, &dark);
        for c in "fix".chars() {
            app.handle(Action::Append(c));
        }
        app.handle(Action::MoveDown);
        app.show_toast("Copied", "🐛");
        let before = app.handle(Action::PickFocused);

        app.set_colors(&light);

        assert_eq!(app.search_text(), "fix");
        assert_eq!(app.handle(Action::PickFocused), before);
        assert!(app.has_toast());
    }

    #[test]
    fn hit_test_returns_none_before_first_render() {
        let (emojis, colors) = fixture();
        let app = App::new(emojis, &colors);
        assert!(app.hit_test(0, 0).is_none());
    }

    #[test]
    fn hit_test_maps_a_scrolled_row_to_its_filtered_index() {
        use ratatui::{backend::TestBackend, Terminal};

        let (emojis, colors) = fixture();
        let mut app = App::new(emojis, &colors);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        // Fewer rows fit than we move down by, so the table scrolls and the
        // first visible row is no longer the first emoji.
        for _ in 0..15 {
            app.handle(Action::MoveDown);
        }
        terminal.draw(|frame| app.render(frame)).unwrap();

        let first = *app
            .visible_emojis()
            .first()
            .expect("the picker rendered at least one row");
        let index = app
            .hit_test(first.cell.x, first.cell.y)
            .expect("the first visible row is hit-testable");
        assert_eq!(
            app.handle(Action::PickAt(index)),
            Outcome::Picked(first.emoji.to_string())
        );
        assert_ne!(index, 0, "the table should have scrolled off the first row");
        // The search box sits above the list, so nothing is hit there.
        assert!(app.hit_test(0, 0).is_none());
    }
}
