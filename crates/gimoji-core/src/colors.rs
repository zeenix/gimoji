use ratatui::style::Color;

pub struct Colors {
    pub selected: Color,
    pub unselected: Color,
    pub border: Color,
}

impl Colors {
    pub fn light() -> Self {
        Self {
            selected: Color::Green,
            unselected: Color::DarkGray,
            border: Color::DarkGray,
        }
    }

    pub fn dark() -> Self {
        Self {
            selected: Color::Green,
            unselected: Color::White,
            border: Color::White,
        }
    }
}
