//! Utility to update emoji database from gitmoji upstream.
//!
//! This tool fetches the latest gitmoji database and merges it with our current
//! emoji database, preserving custom emojis while giving priority to upstream changes.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use clap::Parser;
use serde::{Deserialize, Serialize};

const UPSTREAM_URL: &str = "https://raw.githubusercontent.com/carloscuesta/gitmoji/refs/heads/master/packages/gitmojis/src/gitmojis.json";
const EMOJIS_FILE: &str = "../emojis.json";

#[derive(Parser)]
#[command(name = "update-emojis")]
#[command(about = "Update emoji database from gitmoji upstream")]
struct Args {
    /// Output GitHub Actions format
    #[arg(long)]
    github_actions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Emoji {
    pub emoji: String,
    pub entity: String,
    pub code: String,
    pub description: String,
    pub name: String,
    pub semver: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmojiDatabase {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub gitmojis: Vec<Emoji>,
}

/// Fetches the upstream gitmoji database.
async fn fetch_upstream_database() -> Result<EmojiDatabase, Box<dyn std::error::Error>> {
    println!("📡 Fetching upstream database...");

    let response = reqwest::get(UPSTREAM_URL).await?;
    let status = response.status();

    if !status.is_success() {
        return Err(format!("HTTP {}: Failed to fetch upstream database", status).into());
    }

    let upstream: EmojiDatabase = response.json().await?;

    if upstream.gitmojis.is_empty() {
        return Err("Upstream database is empty".into());
    }

    println!("📡 Upstream database: {} emojis", upstream.gitmojis.len());
    Ok(upstream)
}

/// Reads the current emoji database from file.
fn read_current_database() -> Result<EmojiDatabase, Box<dyn std::error::Error>> {
    println!("📖 Reading current database...");

    if !Path::new(EMOJIS_FILE).exists() {
        return Err(format!("emojis.json not found at {}", EMOJIS_FILE).into());
    }

    let content = fs::read_to_string(EMOJIS_FILE)?;
    let current: EmojiDatabase = serde_json::from_str(&content)?;

    println!("📊 Current database: {} emojis", current.gitmojis.len());
    Ok(current)
}

/// Merges emoji databases with upstream taking priority.
fn merge_databases(current: EmojiDatabase, upstream: EmojiDatabase) -> (EmojiDatabase, MergeStats) {
    // Create sets for comparison.
    let current_codes: HashSet<String> = current.gitmojis.iter().map(|e| e.code.clone()).collect();

    let upstream_codes: HashSet<String> =
        upstream.gitmojis.iter().map(|e| e.code.clone()).collect();

    // Find new emojis from upstream.
    let mut new_upstream_emojis = Vec::new();
    for emoji in &upstream.gitmojis {
        if !current_codes.contains(&emoji.code) {
            new_upstream_emojis.push(emoji.clone());
        }
    }

    // Start with upstream emojis (they take priority).
    let mut merged_emojis = upstream.gitmojis.clone();

    // Add emojis from current database that don't exist upstream.
    let mut custom_emojis = Vec::new();
    for emoji in &current.gitmojis {
        if !upstream_codes.contains(&emoji.code) {
            merged_emojis.push(emoji.clone());
            custom_emojis.push(emoji.clone());
        }
    }

    let merged = EmojiDatabase {
        schema: current.schema,
        gitmojis: merged_emojis,
    };

    let stats = MergeStats {
        total: merged.gitmojis.len(),
        upstream: upstream.gitmojis.len(),
        custom: custom_emojis.len(),
        custom_emojis,
        new_upstream: new_upstream_emojis.len(),
        new_upstream_emojis,
    };

    println!("✨ Merged database: {} emojis", stats.total);

    if stats.new_upstream > 0 {
        println!("🆕 New emojis from upstream: {}", stats.new_upstream);
        for emoji in &stats.new_upstream_emojis {
            println!(
                "   • {} {} - {}",
                emoji.emoji, emoji.code, emoji.description
            );
        }
    }

    if stats.custom > 0 {
        println!("🎨 Custom emojis preserved: {}", stats.custom);
        for emoji in &stats.custom_emojis {
            println!(
                "   • {} {} - {}",
                emoji.emoji, emoji.code, emoji.description
            );
        }
    }

    (merged, stats)
}

/// Statistics about the merge operation.
struct MergeStats {
    total: usize,
    upstream: usize,
    custom: usize,
    custom_emojis: Vec<Emoji>,
    new_upstream: usize,
    new_upstream_emojis: Vec<Emoji>,
}

/// Writes the merged database to file if changes are detected.
fn update_database(
    current_content: &str,
    merged: &EmojiDatabase,
) -> Result<bool, Box<dyn std::error::Error>> {
    let new_content = serde_json::to_string_pretty(merged)? + "\n";
    let has_changes = current_content != new_content;

    if has_changes {
        fs::write(EMOJIS_FILE, &new_content)?;
        println!("✅ Database updated successfully!");
    } else {
        println!("ℹ️  No changes detected - database is already up to date!");
    }

    Ok(has_changes)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("🚀 Starting emoji database update...\n");

    // Read current database.
    let current = read_current_database()?;
    let current_content = fs::read_to_string(EMOJIS_FILE)?;

    // Fetch upstream database.
    let upstream = fetch_upstream_database().await?;

    println!();

    // Merge databases.
    let (merged, stats) = merge_databases(current, upstream);

    println!();

    // Update file if changes detected.
    let has_changes = update_database(&current_content, &merged)?;

    if has_changes {
        println!("📊 Summary:");
        println!("   • Total emojis: {}", stats.total);
        println!("   • From upstream: {}", stats.upstream);
        println!("   • New from upstream: {}", stats.new_upstream);
        println!("   • Custom preserved: {}", stats.custom);
    }

    // Output for GitHub Actions only if requested.
    if !args.github_actions {
        return Ok(());
    }

    let github_output = match env::var("GITHUB_OUTPUT") {
        Ok(path) => path,
        Err(_) => {
            eprintln!("Warning: GITHUB_OUTPUT environment variable not set");
            return Ok(());
        }
    };

    // Use modern GitHub Actions environment files.
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&github_output)?;

    writeln!(file, "has_changes={}", has_changes)?;
    writeln!(file, "total_emojis={}", stats.total)?;
    writeln!(file, "upstream_emojis={}", stats.upstream)?;
    writeln!(file, "custom_emojis={}", stats.custom)?;
    writeln!(file, "new_upstream_emojis={}", stats.new_upstream)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_preserves_custom_emojis() {
        let current = EmojiDatabase {
            schema: "test".to_string(),
            gitmojis: vec![
                Emoji {
                    emoji: "🎨".to_string(),
                    entity: "test".to_string(),
                    code: ":art:".to_string(),
                    description: "Old description".to_string(),
                    name: "art".to_string(),
                    semver: None,
                },
                Emoji {
                    emoji: "🤖".to_string(),
                    entity: "test".to_string(),
                    code: ":robot:".to_string(),
                    description: "Custom emoji".to_string(),
                    name: "robot".to_string(),
                    semver: None,
                },
            ],
        };

        let upstream = EmojiDatabase {
            schema: "upstream".to_string(),
            gitmojis: vec![Emoji {
                emoji: "🎨".to_string(),
                entity: "test".to_string(),
                code: ":art:".to_string(),
                description: "New description".to_string(),
                name: "art".to_string(),
                semver: None,
            }],
        };

        let (merged, stats) = merge_databases(current, upstream);

        assert_eq!(merged.gitmojis.len(), 2);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.upstream, 1);
        assert_eq!(stats.custom, 1);
        assert_eq!(stats.custom_emojis.len(), 1);
        assert_eq!(stats.custom_emojis[0].code, ":robot:");
        assert_eq!(stats.new_upstream, 0);

        // Upstream should take priority.
        let art_emoji = merged.gitmojis.iter().find(|e| e.code == ":art:").unwrap();
        assert_eq!(art_emoji.description, "New description");

        // Custom emoji should be preserved.
        let robot_emoji = merged
            .gitmojis
            .iter()
            .find(|e| e.code == ":robot:")
            .unwrap();
        assert_eq!(robot_emoji.description, "Custom emoji");
    }
}
