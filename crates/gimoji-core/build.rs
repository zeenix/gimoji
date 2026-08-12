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
#[databake(path = gimoji_core::emoji)]
pub struct Emoji<'e> {
    pub code: &'e str,
    pub description: &'e str,
    pub emoji: &'e str,
    pub entity: &'e str,
    pub name: &'e str,
}

#[derive(serde::Deserialize, Debug)]
pub struct Emojis<'e> {
    #[serde(borrow)]
    gitmojis: Vec<Emoji<'e>>,
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=emojis.json");
    println!("cargo:rerun-if-changed={ROOT_EMOJI_FILE}");
    let path = PathBuf::from(EMOJI_FILE);
    let emojis_json = read_to_string(path)?;

    // The repo root carries a second copy of emojis.json: its raw GitHub URL
    // is a public contract — published commitlint-plugin-gimoji versions
    // fetch it from main. The root copy is absent from the packaged crate,
    // so this check only runs on repo checkouts.
    let root_path = PathBuf::from(ROOT_EMOJI_FILE);
    if root_path.exists() && read_to_string(root_path)? != emojis_json {
        return Err(format!(
            "{ROOT_EMOJI_FILE} differs from {EMOJI_FILE}; the root copy is fetched by \
             commitlint-plugin-gimoji and must stay identical. Copy one over the other."
        )
        .into());
    }
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
const ROOT_EMOJI_FILE: &str = "../../emojis.json";
