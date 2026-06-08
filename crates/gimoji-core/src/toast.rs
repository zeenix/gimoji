use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::selection_view::{EmojiSource, EMOJI_COLUMN_WIDTH};

const VISIBLE_FOR: Duration = Duration::from_millis(1500);

pub struct Toast {
    prefix: String,
    emoji: String,
    source: EmojiSource,
    elapsed: Duration,
    last_emoji_cell: Option<Rect>,
}

impl Toast {
    pub fn new(prefix: impl Into<String>, emoji: impl Into<String>, source: EmojiSource) -> Self {
        Self {
            prefix: prefix.into(),
            emoji: emoji.into(),
            source,
            elapsed: Duration::ZERO,
            last_emoji_cell: None,
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        self.elapsed = self.elapsed.saturating_add(dt);
    }

    pub fn is_expired(&self) -> bool {
        self.elapsed >= VISIBLE_FOR
    }

    /// Cell position of the emoji glyph for an external overlay to paint.
    /// `None` for `EmojiSource::InCanvas` (the buffer already holds the
    /// glyph), or before the first render of an overlay-mode toast.
    pub fn emoji_cell(&self) -> Option<(Rect, &str)> {
        self.last_emoji_cell.map(|r| (r, self.emoji.as_str()))
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // The emoji slot in the rendered text is a fixed-width blank when
        // an external layer paints the glyph. That keeps the box width
        // independent of the emoji's `unicode-width` count (which is 4 for
        // ZWJ sequences vs. their 2-cell rendered width) and matches the
        // picker's emoji column.
        let (text, emoji_offset) = match self.source {
            EmojiSource::InCanvas => (format!("{} {}", self.prefix, self.emoji), None),
            EmojiSource::Overlay => {
                let blank: String = " ".repeat(EMOJI_COLUMN_WIDTH as usize);
                let text = format!("{} {}", self.prefix, blank);
                let offset = self.prefix.chars().count() as u16 + 1;
                (text, Some(offset))
            }
        };

        let text_len = text.chars().count() as u16;
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

        // Paragraph::Center inside a Borders::ALL block: the visible text
        // starts at x + 1 (border) + (inner_w - 2 - text_len) / 2 inside
        // the inner area, on the middle row (y + 1).
        self.last_emoji_cell = emoji_offset.map(|off| {
            let inner_left = rect.x + 1;
            let inner_text_w = rect.width.saturating_sub(2);
            let text_x = inner_left + inner_text_w.saturating_sub(text_len) / 2;
            Rect {
                x: text_x + off,
                y: rect.y + 1,
                width: EMOJI_COLUMN_WIDTH,
                height: 1,
            }
        });

        let block = Block::default().borders(Borders::ALL);
        let para = Paragraph::new(Line::from(text))
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
        let mut t = Toast::new("hi", "", EmojiSource::InCanvas);
        assert!(!t.is_expired());
        t.tick(VISIBLE_FOR / 2);
        assert!(!t.is_expired());
        t.tick(VISIBLE_FOR);
        assert!(t.is_expired());
    }
}
