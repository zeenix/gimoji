use tui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::emojis::Emoji;

pub struct SelectionView {
    emojis: Vec<Emoji>,
    state: TableState,
}

impl SelectionView {
    pub fn new(emojis: Vec<Emoji>) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));

        Self { emojis, state }
    }

    pub fn filtered_view(&mut self, search_text: &str) -> FilteredView<'_> {
        let emojis: Vec<&Emoji> = self
            .emojis
            .iter()
            .filter(|emoji| search_text.is_empty() || emoji.contains(search_text))
            .collect();
        if self.state.selected().unwrap() >= emojis.len() {
            // Reset the selection if the list goes shorter than the selected index.
            self.state.select(Some(0));
        }

        FilteredView {
            emojis,
            state: &mut self.state,
        }
    }
}

pub struct FilteredView<'s> {
    emojis: Vec<&'s Emoji>,
    state: &'s mut TableState,
}

impl FilteredView<'_> {
    pub fn selected(&self) -> Option<&Emoji> {
        self.emojis.get(self.state.selected().unwrap()).map(|e| *e)
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

impl Widget for &mut FilteredView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let emojis = self
            .emojis
            .iter()
            .map(|emoji| Row::new(vec![emoji.emoji(), emoji.code(), emoji.description()]));
        let table = Table::new(emojis)
            .block(
                Block::default()
                    .title("Select an emoji")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Green),
            )
            .highlight_symbol("❯ ")
            .widths(&[
                Constraint::Percentage(3),
                Constraint::Percentage(12),
                Constraint::Percentage(85),
            ])
            .column_spacing(2);
        StatefulWidget::render(table, area, buf, self.state);
    }
}
