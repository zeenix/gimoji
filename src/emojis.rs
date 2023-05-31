use std::{error::Error, fs::read_to_string, io, path::PathBuf};

use crate::emoji::Emoji;

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
    let emojis_json = ureq::get(EMOJI_URL).call()?.into_string()?;
    let path = cache_dir()?.join(EMOJI_CACHE_FILE);
    std::fs::write(path, &emojis_json)?;

    Ok(emojis_json)
}

pub fn cache_dir() -> Result<PathBuf, Box<dyn Error>> {
    let path = dirs::cache_dir().unwrap().join(CACHE_DIR);
    std::fs::create_dir_all(&path)?;

    Ok(path)
}

const CACHE_DIR: &str = "gimoji";
const EMOJI_CACHE_FILE: &str = "emojis.json";
const EMOJI_URL: &str = "https://gitmoji.dev/api/gitmojis";
