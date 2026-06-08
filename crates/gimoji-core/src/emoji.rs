#[derive(Debug)]
pub struct Emoji {
    code: &'static str,
    description: &'static str,
    emoji: &'static str,
    entity: &'static str,
    name: &'static str,
}

impl Emoji {
    /// Case-insensitive ASCII substring match against any of the emoji's
    /// searchable fields. `needle_lower` must already be lowercased by the
    /// caller (typically once per filter pass, not once per emoji).
    pub fn contains(&self, needle_lower: &str) -> bool {
        contains_ignore_ascii_case(self.code, needle_lower)
            || contains_ignore_ascii_case(self.description, needle_lower)
            || self.emoji.contains(needle_lower)
            || contains_ignore_ascii_case(self.entity, needle_lower)
            || contains_ignore_ascii_case(self.name, needle_lower)
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn emoji(&self) -> &'static str {
        self.emoji
    }
}

fn contains_ignore_ascii_case(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if needle_lower.len() > haystack.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle_lower.len())
        .any(|w| w.eq_ignore_ascii_case(needle_lower.as_bytes()))
}

include!(concat!(env!("OUT_DIR"), "/emojis.rs"));

#[cfg(test)]
mod tests {
    use super::contains_ignore_ascii_case;

    #[test]
    fn ascii_case_insensitive_match() {
        assert!(contains_ignore_ascii_case("Sparkles", "spark"));
        assert!(contains_ignore_ascii_case("Sparkles", "KLE"));
        assert!(!contains_ignore_ascii_case("Sparkles", "fire"));
    }

    #[test]
    fn empty_needle_matches() {
        assert!(contains_ignore_ascii_case("anything", ""));
    }

    #[test]
    fn needle_longer_than_haystack_does_not_match() {
        assert!(!contains_ignore_ascii_case("hi", "hello"));
    }
}
