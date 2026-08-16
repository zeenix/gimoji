use crate::colors::Colors;
use ratatui::{
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

pub struct SearchEntry<'c> {
    text: String,
    colors: &'c Colors,
}

impl<'c> SearchEntry<'c> {
    pub fn new(colors: &'c Colors) -> Self {
        Self {
            text: String::from(""),
            colors,
        }
    }

    pub fn set_colors(&mut self, colors: &'c Colors) {
        self.colors = colors;
    }

    pub fn text(&self) -> &str {
        self.text.as_ref()
    }

    /// Replace the whole text at once.
    ///
    /// Frontends that mirror a host-owned editor (e.g. the web build's
    /// offscreen `<input>`, which is what raises a mobile on-screen
    /// keyboard) can't express every edit as append/delete: autocorrect,
    /// paste and IME composition all rewrite arbitrary spans.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn append(&mut self, c: char) {
        self.text.push(c);
    }

    pub fn delete_last(&mut self) {
        self.text.pop();
    }

    pub fn delete_all(&mut self) {
        self.text.clear();
    }
}

impl Widget for &SearchEntry<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        // Explicit fg colour so the search text and placeholder are
        // visible on backends that default unset `fg` to white (e.g.
        // the web canvas backend) — `Style::default()` alone leaves both
        // invisible on a light page background.
        let base = Style::default().fg(self.colors.unselected);
        let (text, style) = if self.text.is_empty() {
            (DEFAULT_TEXT, base.add_modifier(Modifier::DIM))
        } else {
            (&*self.text, base)
        };
        let paragraph = Paragraph::new(Span::styled(text, style)).block(
            Block::default()
                .title(TITLE)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.colors.border))
                .padding(Padding {
                    left: 1,
                    right: 1,
                    top: 1,
                    bottom: 1,
                }),
        );

        paragraph.render(area, buf)
    }
}

const TITLE: &str = "Search an emoji";
const DEFAULT_TEXT: &str = "Use arrow keys or type to search";
