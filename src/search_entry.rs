use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Widget},
};

#[derive(Default)]
pub struct SearchEntry {
    text: String,
}

impl SearchEntry {
    pub fn text(&self) -> &str {
        self.text.as_ref()
    }

    pub fn append(&mut self, c: char) {
        self.text.push(c);
    }

    pub fn delete_last(&mut self) {
        self.text.pop();
    }
}

impl Widget for &SearchEntry {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let (text, style) = if self.text.is_empty() {
            (DEFAULT_TEXT, Style::default().add_modifier(Modifier::DIM))
        } else {
            (&*self.text, Style::default())
        };
        let paragraph = Paragraph::new(Span::styled(text, style)).block(
            Block::default()
                .title(TITLE)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        );

        paragraph.render(area, buf)
    }
}

const TITLE: &str = "Search an emoji";
const DEFAULT_TEXT: &str = "Use arrow keys or type to search";
