use std::{error::Error, fs::read_to_string, io, path::PathBuf};

#[derive(serde::Deserialize, Debug)]
pub struct Emojis {
    pub gitmojis: Vec<Emoji>,
}

impl Emojis {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let path = cache_dir()?.join(EMOJI_CACHE_FILE);
        let emojis_json = match read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => update_cache()?,
            Err(_) => return Err("Failed to read emoji cache".into()),
        };
        let emojis: Emojis = serde_json::from_str(&emojis_json)?;

        Ok(emojis)
    }
}

pub fn update_cache() -> Result<String, Box<dyn Error>> {
    println!("Updating emoji cache...");
    let emojis_json = reqwest::blocking::get(EMOJI_URL)?.text()?;
    let path = cache_dir()?.join(EMOJI_CACHE_FILE);
    std::fs::write(path, &emojis_json)?;

    Ok(emojis_json)
}

fn cache_dir() -> Result<PathBuf, Box<dyn Error>> {
    let path = dirs::cache_dir().unwrap().join(CACHE_DIR);
    std::fs::create_dir_all(&path)?;

    Ok(path)
}

const CACHE_DIR: &str = "gimoji";
const EMOJI_CACHE_FILE: &str = "emojis.json";
const EMOJI_URL: &str = "https://gitmoji.dev/api/gitmojis";

#[derive(serde::Deserialize, Debug)]
pub struct Emoji {
    code: String,
    description: String,
    emoji: String,
    entity: String,
    name: String,
}

impl Emoji {
    pub fn contains(&self, needle: &str) -> bool {
        self.code.contains(needle)
            || self.description.contains(needle)
            || self.emoji.contains(needle)
            || self.entity.contains(needle)
            || self.name.contains(needle)
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
