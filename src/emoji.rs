use regex::Regex;

#[derive(Debug)]
pub struct Emoji {
    code: &'static str,
    description: &'static str,
    emoji: &'static str,
    entity: &'static str,
    name: &'static str,
}

impl Emoji {
    pub fn contains(&self, pattern: &Regex) -> bool {
        pattern.is_match(self.code)
            || pattern.is_match(self.description)
            || pattern.is_match(self.emoji)
            || pattern.is_match(self.entity)
            || pattern.is_match(self.name)
    }

    pub fn code(&self) -> &str {
        self.code
    }

    pub fn description(&self) -> &str {
        self.description
    }

    pub fn emoji(&self) -> &str {
        self.emoji
    }
}

include!(concat!(env!("OUT_DIR"), "/emojis.rs"));
