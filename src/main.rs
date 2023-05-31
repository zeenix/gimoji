extern crate self as gimoji;

mod emoji;
mod search_entry;
mod selection_view;
mod terminal;

use arboard::Clipboard;
use clap::{command, Parser};
use crossterm::event::{read, Event, KeyCode};
use ratatui::layout::{Constraint, Layout};
use selection_view::SelectionView;
use std::{error::Error, fs::File, io::Write};
#[cfg(unix)]
use std::{fs::Permissions, os::unix::prelude::PermissionsExt};

use search_entry::SearchEntry;
use terminal::Terminal;

/// Select emoji for git commit message.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Initialize gimoji as a commit message hook.
    #[arg(short, long)]
    init: bool,

    /// Update local emoji cache (deprecated and NO-OP).
    #[arg(short, long)]
    update_cache: bool,

    /// Run as git commit hook.
    #[arg(long, value_delimiter = ' ', num_args = 1..3)]
    hook: Vec<String>,

    /// Print the local emoji cache path (deprecated and NO-OP).
    #[arg(short, long, default_value_t = false)]
    cache_path: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.init {
        install_hook()?;

        return Ok(());
    } else if args.update_cache || args.cache_path {
        println!(
            "Emojis are now a part of the gimoji binary. This option is now a NO-OP and kept \
             for backwards compatibility only and will be removed in a future release."
        );

        return Ok(());
    }

    let commit_file_path = if !args.hook.is_empty() {
        if args.hook.len() > 1 {
            // For now, we only modify a completely new commit.
            return Ok(());
        }

        Some(&args.hook[0])
    } else {
        None
    };

    let selected = match select_emoji()? {
        Some(s) => s,
        None => return Ok(()),
    };

    if let Some(path) = commit_file_path {
        // Just write the emoji to the file.
        let mut file = File::create(path)?;
        let prefix = format!("{} ", selected);
        file.write_all(prefix.as_bytes())?;
    } else {
        Clipboard::new()?.set_text(&selected)?;
        println!("Copied {} to the clipboard", selected);
    }

    Ok(())
}

fn select_emoji() -> Result<Option<String>, Box<dyn Error>> {
    let emojis = &emoji::EMOJIS;

    let mut terminal = Terminal::setup()?;
    let mut search_entry = SearchEntry::default();
    let mut selection_view = SelectionView::new(emojis);

    let selected = loop {
        let search_text = search_entry.text();
        let mut filtered_view = selection_view.filtered_view(search_text);

        terminal.draw(|f| {
            let chunks = Layout::default()
                .constraints([Constraint::Min(4), Constraint::Percentage(100)].as_ref())
                .margin(1)
                .split(f.size());

            // The search entry goes at the top.
            f.render_widget(&search_entry, chunks[0]);

            // The emoji list.
            f.render_widget(&mut filtered_view, chunks[1]);
        })?;

        if let Event::Key(event) = read()? {
            match event.code {
                KeyCode::Enter => {
                    if let Some(emoji) = filtered_view.selected() {
                        break Some(emoji.emoji().to_string());
                    }
                }
                KeyCode::Esc => {
                    if search_text.is_empty() {
                        break None;
                    } else {
                        search_entry.delete_all();
                    }
                }
                KeyCode::Down => filtered_view.move_down(),
                KeyCode::Up => filtered_view.move_up(),
                KeyCode::Char(c) => search_entry.append(c),
                KeyCode::Backspace => {
                    search_entry.delete_last();
                }
                _ => {}
            }
        }
    };

    Ok(selected)
}

fn install_hook() -> Result<(), Box<dyn Error>> {
    let mut file = File::create(HOOK_PATH)?;
    file.write_all(HOOK_CONTENT.as_bytes())?;
    #[cfg(unix)]
    file.set_permissions(Permissions::from_mode(0o744))?;

    Ok(())
}

const HOOK_PATH: &str = ".git/hooks/prepare-commit-msg";
const HOOK_CONTENT: &str = r#"
#!/usr/bin/env bash
# gimoji as a commit hook
gimoji --hook $1 $2
"#;
