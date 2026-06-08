use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

const VISIBLE_FOR: Duration = Duration::from_millis(1500);

pub struct Toast {
    text: String,
    elapsed: Duration,
}

impl Toast {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            elapsed: Duration::ZERO,
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        self.elapsed = self.elapsed.saturating_add(dt);
    }

    pub fn is_expired(&self) -> bool {
        self.elapsed >= VISIBLE_FOR
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let text_len = self.text.chars().count() as u16;
        let inner_w = (text_len + 4).min(area.width.saturating_sub(2));
        let inner_h = 3u16;
        let x = area.x + area.width.saturating_sub(inner_w) / 2;
        let y = area.y + area.height.saturating_sub(inner_h) / 2;
        let rect = Rect {
            x,
            y,
            width: inner_w,
            height: inner_h,
        };

        let block = Block::default().borders(Borders::ALL);
        let para = Paragraph::new(Line::from(self.text.as_str()))
            .style(Style::default().add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(block);
        Widget::render(para, rect, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_expires_after_visible_for() {
        let mut t = Toast::new("hi");
        assert!(!t.is_expired());
        t.tick(VISIBLE_FOR / 2);
        assert!(!t.is_expired());
        t.tick(VISIBLE_FOR);
        assert!(t.is_expired());
    }
}
