use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{
        Block, Borders, HighlightSpacing, Padding, Row, StatefulWidget, Table, TableState, Widget,
    },
};

use crate::colors::Colors;
use crate::emoji::Emoji;

/// Where the picker's emoji glyph for each row comes from.
#[derive(Debug, Clone, Copy)]
pub enum EmojiSource {
    /// Glyph is emitted into the ratatui buffer so the host (terminal +
    /// crossterm) renders it from its own font. Used by the native build.
    InCanvas,
    /// Glyph is painted by a separate overlay (e.g. the browser DOM); the
    /// buffer keeps a fixed-width blank gap so column layout stays aligned
    /// regardless of how `unicode-width` counts the emoji.
    Overlay,
}

pub struct SelectionView<'c> {
    emojis: &'static [Emoji],
    state: TableState,
    colors: &'c Colors,
    source: EmojiSource,
}

impl<'c> SelectionView<'c> {
    pub fn new(emojis: &'static [Emoji], colors: &'c Colors, source: EmojiSource) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));

        Self {
            emojis,
            state,
            colors,
            source,
        }
    }

    pub fn filtered_view(&mut self, search_text: &str) -> FilteredView<'_, '_> {
        let needle = search_text.to_ascii_lowercase();
        let emojis: Vec<&Emoji> = self
            .emojis
            .iter()
            .filter(|emoji| needle.is_empty() || emoji.contains(&needle))
            .collect();

        self.state
            .select(adjust_selected(self.state.selected(), emojis.len()));

        FilteredView {
            emojis,
            state: &mut self.state,
            colors: self.colors,
            source: self.source,
        }
    }
}

fn adjust_selected(selected: Option<usize>, list_len: usize) -> Option<usize> {
    match (selected, list_len) {
        (_, 0) => None,
        (None, _) => Some(0),
        // Reset the selection if the list goes shorter than the selected index.
        (Some(selected), _) if selected >= list_len => Some(0),
        (Some(_), _) => selected,
    }
}

pub struct FilteredView<'s, 'c> {
    emojis: Vec<&'s Emoji>,
    state: &'s mut TableState,
    colors: &'c Colors,
    source: EmojiSource,
}

impl<'s> FilteredView<'s, '_> {
    pub fn selected(&self) -> Option<&Emoji> {
        self.emojis.get(self.state.selected()?).copied()
    }

    pub fn get(&self, index: usize) -> Option<&'s Emoji> {
        self.emojis.get(index).copied()
    }

    pub fn visible_count(&self) -> usize {
        self.emojis.len()
    }

    pub fn offset(&self) -> usize {
        self.state.offset()
    }

    pub fn move_up(&mut self) {
        let i = self.state.selected().unwrap();
        let i = if i == 0 { self.emojis.len() - 1 } else { i - 1 };
        self.state.select(Some(i));
    }

    pub fn move_down(&mut self) {
        let i = self.state.selected().unwrap();
        let i = if i == self.emojis.len() - 1 { 0 } else { i + 1 };
        self.state.select(Some(i));
    }
}

impl Widget for &mut FilteredView<'_, '_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // When an external layer paints the emoji glyphs the buffer must
        // stay free of ZWJ / variation-selector sequences whose
        // `unicode-width` count doesn't match their rendered width, or the
        // following columns slide right on those rows. Emit a blank
        // placeholder that always consumes exactly `EMOJI_COLUMN_WIDTH`
        // cells per row.
        let source = self.source;
        let emojis = self.emojis.iter().map(|emoji| {
            let cell0 = match source {
                EmojiSource::InCanvas => emoji.emoji(),
                EmojiSource::Overlay => "",
            };
            Row::new(vec![cell0, emoji.code(), emoji.description()])
        });
        let table = Table::new(
            emojis,
            [
                Constraint::Length(EMOJI_COLUMN_WIDTH),
                Constraint::Length(20),
                Constraint::Fill(1),
            ],
        )
        .block(
            Block::default()
                .title("Select an emoji")
                .borders(Borders::ALL)
                .padding(Padding {
                    left: 1,
                    right: 1,
                    top: 1,
                    bottom: 0,
                }),
        )
        .style(Style::default().fg(self.colors.unselected))
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(self.colors.selected),
        )
        .highlight_symbol("❯ ")
        // Always reserve the highlight-symbol gutter so column positions
        // don't shift when the selection moves — the web overlay relies on
        // a stable per-row x offset for the emoji column.
        .highlight_spacing(HighlightSpacing::Always)
        .column_spacing(2);
        StatefulWidget::render(table, area, buf, self.state);
    }
}

/// Cell width of the leading emoji column in each row. Native renders the
/// emoji glyph through the terminal font; the web overlay positions a DOM
/// span over this region and lets the browser render it.
pub const EMOJI_COLUMN_WIDTH: u16 = 3;
/// Cell width of the gutter reserved for the row highlight symbol ("❯ ").
pub const HIGHLIGHT_GUTTER_WIDTH: u16 = 2;
