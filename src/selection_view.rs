use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Padding, Row, StatefulWidget, Table, TableState, Widget},
};
use regex::RegexBuilder;

use crate::colors::Colors;
use crate::emoji::Emoji;

pub struct SelectionView<'c> {
    emojis: &'static [Emoji],
    state: TableState,
    colors: &'c Colors,
}

impl<'c> SelectionView<'c> {
    pub fn new(emojis: &'static [Emoji], colors: &'c Colors) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));

        Self {
            emojis,
            state,
            colors,
        }
    }

    pub fn filtered_view(&mut self, search_text: &str) -> FilteredView<'_, '_> {
        let pattern = RegexBuilder::new(search_text)
            .case_insensitive(true)
            .build()
            .expect("invalid characters in search text");
        let emojis: Vec<&Emoji> = self
            .emojis
            .iter()
            .filter(|emoji| search_text.is_empty() || emoji.contains(&pattern))
            .collect();
        if self.state.selected().unwrap() >= emojis.len() {
            // Reset the selection if the list goes shorter than the selected index.
            self.state.select(Some(0));
        }

        FilteredView {
            emojis,
            state: &mut self.state,
            colors: self.colors,
        }
    }
}

pub struct FilteredView<'s, 'c> {
    emojis: Vec<&'s Emoji>,
    state: &'s mut TableState,
    colors: &'c Colors,
}

impl FilteredView<'_, '_> {
    pub fn selected(&self) -> Option<&Emoji> {
        self.emojis.get(self.state.selected().unwrap()).copied()
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
        let emojis = self
            .emojis
            .iter()
            .map(|emoji| Row::new(vec![emoji.emoji(), emoji.code(), emoji.description()]));
        let table = Table::new(
            emojis,
            [
                Constraint::Percentage(3),
                Constraint::Percentage(12),
                Constraint::Percentage(85),
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
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(self.colors.selected),
        )
        .highlight_symbol("❯ ")
        .column_spacing(2);
        StatefulWidget::render(table, area, buf, self.state);
    }
}
