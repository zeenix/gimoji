use regex::Regex;

#[derive(serde::Deserialize, Debug)]
pub struct Emoji {
    code: String,
    description: String,
    emoji: String,
    entity: String,
    name: String,
}

impl Emoji {
    pub fn contains(&self, pattern: &Regex) -> bool {
        pattern.is_match(&self.code)
            || pattern.is_match(&self.description)
            || pattern.is_match(&self.emoji)
            || pattern.is_match(&self.entity)
            || pattern.is_match(&self.name)
    }

    pub fn code(&self) -> &str {
        self.code.as_ref()
    }

    pub fn description(&self) -> &str {
        self.description.as_ref()
    }

    pub fn emoji(&self) -> &str {
        self.emoji.as_ref()
    }
}
