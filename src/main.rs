mod emojis;
mod search_entry;
mod selection_view;
mod terminal;

use clap::{command, Parser};
use crossterm::event::{read, Event, KeyCode};
use selection_view::SelectionView;
use std::{
    error::Error,
    fs::{File, Permissions},
    io::Write,
    os::unix::prelude::PermissionsExt,
};
use tui::layout::{Constraint, Layout};

use emojis::Emojis;
use search_entry::SearchEntry;
use terminal::Terminal;

/// Select emoji for git commit message.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Initialize gimoji as a commit message hook.
    #[arg(short, long)]
    init: bool,

    /// Update local emoji cache.
    #[arg(short, long)]
    update_cache: bool,

    /// Run as git commit hook.
    #[arg(long, value_delimiter = ' ', num_args = 1..3)]
    hook: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.init {
        install_hook()?;

        return Ok(());
    } else if args.update_cache {
        emojis::update_cache()?;

        return Ok(());
    }

    let commit_file_path = if args.hook.len() > 0 {
        if args.hook.len() > 1 {
            // For now, we only modify a completely new commit.
            return Ok(());
        }

        Some(&args.hook[0])
    } else {
        None
    };

    let selected = select_emoji()?;

    if let Some(path) = commit_file_path {
        // Just write the emoji to the file.
        let mut file = File::create(path)?;
        let prefix = format!("{} ", selected);
        file.write_all(prefix.as_bytes())?;
    } else {
        println!("{}", selected);
    }

    Ok(())
}

fn select_emoji() -> Result<String, Box<dyn Error>> {
    let emojis = Emojis::load()?.gitmojis;

    let mut terminal = Terminal::setup()?;
    let mut search_entry = SearchEntry::default();
    let mut selection_view = SelectionView::new(emojis);

    let selected = loop {
        let search_text = search_entry.text();
        let mut filtered_view = selection_view.filtered_view(&search_text);

        terminal.draw(|f| {
            let chunks = Layout::default()
                .constraints([Constraint::Percentage(10), Constraint::Percentage(90)].as_ref())
                .margin(1)
                .split(f.size());

            // The search entry goes at the top.
            f.render_widget(&search_entry, chunks[0]);

            // The emoji list.
            f.render_widget(&mut filtered_view, chunks[1]);
        })?;

        match read()? {
            Event::Key(event) => match event.code {
                KeyCode::Enter => match filtered_view.selected() {
                    Some(emoji) => break emoji.emoji().to_string(),
                    None => (),
                },
                KeyCode::Down => filtered_view.move_down(),
                KeyCode::Up => filtered_view.move_up(),
                KeyCode::Char(c) => search_entry.append(c),
                KeyCode::Backspace => {
                    search_entry.delete_last();

                    ()
                }
                _ => {}
            },
            _ => (),
        }
    };

    Ok(selected)
}

fn install_hook() -> Result<(), Box<dyn Error>> {
    let mut file = File::create(HOOK_PATH)?;
    file.write_all(HOOK_CONTENT.as_bytes())?;
    file.set_permissions(Permissions::from_mode(0o744))?;

    Ok(())
}

const HOOK_PATH: &str = ".git/hooks/prepare-commit-msg";
const HOOK_CONTENT: &str = r#"
#!/usr/bin/env bash
# gimoji as a commit hook
exec < /dev/tty
gimoji --hook $1 $2
"#;
