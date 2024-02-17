extern crate self as gimoji;

mod colors;
mod emoji;
mod search_entry;
mod selection_view;
mod terminal;

use arboard::Clipboard;
use clap::{command, Parser};
use colors::Colors;
use crossterm::event::{read, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use selection_view::SelectionView;
use std::{
    error::Error,
    fs::File,
    io::{BufRead, BufReader, Write},
};
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
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.init {
        install_hook()?;

        return Ok(());
    } else if args.update_cache {
        println!(
            "Emojis are now a part of the gimoji binary. This option is now a NO-OP and kept \
             for backwards compatibility only and will be removed in a future release."
        );

        return Ok(());
    }

    let (commit_file_path, commit_file_content) = if !args.hook.is_empty() {
        let path = &args.hook[0];
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut content = String::new();
        reader.read_line(&mut content)?;
        let content = if !content.is_empty() {
            // FIXME: There has to be a faster way to detect an emoji.
            for emoji in emoji::EMOJIS {
                if content.contains(emoji.emoji()) || content.contains(emoji.code()) {
                    // The commit shortlog already contains an emoji.
                    return Ok(());
                }
            }

            Some(content)
        } else {
            None
        };

        (Some(path), content)
    } else {
        (None, None)
    };

    let selected = match select_emoji()? {
        Some(s) => s,
        None => return Ok(()),
    };

    if let Some(path) = commit_file_path {
        // Just prepend the emoji to the file.
        let mut file = File::create(path)?;
        let prefix = format!("{} ", selected);
        file.write_all(prefix.as_bytes())?;
        if let Some(content) = commit_file_content {
            file.write_all(content.as_bytes())?;
        }
    } else {
        println!("Copied {selected} to the clipboard");
        copy_to_clipboard(selected)?;
    }

    Ok(())
}

fn select_emoji() -> Result<Option<String>, Box<dyn Error>> {
    let emojis = &emoji::EMOJIS;

    let colors = match terminal_light::luma() {
        Ok(luma) if luma > 0.6 => Colors::light(),
        _ => Colors::dark(),
    };

    let mut terminal = Terminal::setup()?;
    let mut search_entry = SearchEntry::new(&colors);
    let mut selection_view = SelectionView::new(emojis, &colors);

    let selected = loop {
        let search_text = search_entry.text();
        let mut filtered_view = selection_view.filtered_view(search_text);

        terminal.draw(|f| {
            let chunks = Layout::default()
                .constraints([Constraint::Min(5), Constraint::Percentage(100)].as_ref())
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
                KeyCode::Char(c) => {
                    if c == 'c' && event.modifiers.contains(KeyModifiers::CONTROL) {
                        break None;
                    } else {
                        search_entry.append(c)
                    }
                }
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

/// Copy the text to the clipboard.
///
/// This function exits the process and never returns because on some platforms (X11, Wayland)
/// clipboard data is only available for as long as the process that "owns" it is alive, in which
/// case this function will spawn a background task to host the clipboard data.
///
/// Note that it is possible to make it work without exiting the process, but it would require an
/// `unsafe { fork() }`. However, in this program this is simply not needed.
fn copy_to_clipboard(s: String) -> Result<(), Box<dyn Error>> {
    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "illumos",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris"
    ))]
    {
        use arboard::SetExtLinux;
        nix::unistd::daemon(false, false)?;
        Clipboard::new()?.set().wait().text(s)?;
    }

    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "illumos",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris"
    )))]
    {
        Clipboard::new()?.set().text(s)?;
    }

    std::process::exit(0)
}

const HOOK_PATH: &str = ".git/hooks/prepare-commit-msg";
const HOOK_CONTENT: &str = r#"
#!/usr/bin/env bash
# gimoji as a commit hook
exec < /dev/tty
gimoji --hook $1 $2
"#;
