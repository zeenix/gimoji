use std::{
    env::var_os,
    error::Error,
    fs::{read_to_string, File},
    io::Write,
    path::PathBuf,
};

use databake::Bake;
use serde::Deserialize;

#[derive(Deserialize, Debug, Bake)]
#[databake(path = gimoji::emoji)]
pub struct Emoji<'e> {
    code: &'e str,
    description: &'e str,
    emoji: &'e str,
    entity: &'e str,
    name: &'e str,
}

#[derive(serde::Deserialize, Debug)]
pub struct Emojis<'e> {
    #[serde(borrow)]
    gitmojis: Vec<Emoji<'e>>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(EMOJI_FILE);
    let emojis_json = read_to_string(path)?;
    let emojis: Emojis = serde_json::from_str(&emojis_json)?;
    let baked = (&emojis.gitmojis[..]).bake(&Default::default()).to_string();

    let out = format!("pub const EMOJIS: &[crate::emoji::Emoji] = {baked};\n");

    let out_dir = var_os("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(out_dir).join("emojis.rs");
    let mut dest_file = File::create(dest_path)?;
    dest_file.write_all(out.as_bytes())?;

    Ok(())
}

const EMOJI_FILE: &str = "emojis.json";
