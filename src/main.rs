extern crate self as gimoji;

mod colors;
mod emoji;
mod search_entry;
mod selection_view;
mod terminal;

use arboard::Clipboard;
use clap::{command, Parser, ValueEnum};
use colors::Colors;
use crossterm::event::{read, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use selection_view::SelectionView;
use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    process::exit,
};
#[cfg(unix)]
use std::{fs::Permissions, os::unix::prelude::PermissionsExt};

use search_entry::SearchEntry;
use terminal::Terminal;

/// Select emoji for git commit message.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Initialize gimoji as a commit message (`prepare-commit-msg`) hook.
    #[arg(short, long)]
    init: bool,

    /// Run as git commit hook.
    #[arg(long, value_delimiter = ' ', num_args = 1..3)]
    hook: Vec<String>,

    /// The color scheme to use (`GIMOJI_COLOR_SCHEME` environment variable takes precedence).
    ///
    /// If not specified, the color scheme is autodetected.
    #[arg(short, long)]
    color_scheme: Option<ColorScheme>,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum ColorScheme {
    Light,
    Dark,
}

impl From<ColorScheme> for Colors {
    fn from(c: ColorScheme) -> Self {
        match c {
            ColorScheme::Dark => Colors::dark(),
            ColorScheme::Light => Colors::light(),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.init {
        install_hook()?;

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

            // Load the rest of the file.
            reader.read_to_string(&mut content)?;
            Some(content)
        } else {
            None
        };

        (Some(path), content)
    } else {
        (None, None)
    };

    let color_scheme = get_color_scheme(&args);
    let selected = match select_emoji(color_scheme.into())? {
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

fn select_emoji(colors: Colors) -> Result<Option<String>, Box<dyn Error>> {
    let emojis = &emoji::EMOJIS;

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
                .split(f.area());

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
                        let _ = terminal.cleanup();
                        exit(130);
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
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(HOOK_PATH)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            eprintln!(
                "Failed to create `{HOOK_PATH}` as it already exists. \
                Please either remove it and re-run `gimoji -i`, or \
                add the following command line to it:\n{HOOK_CMD}",
            );
            exit(-1);
        }
        Err(e) => return Err(e.into()),
    };
    file.write_all(HOOK_HEADER.as_bytes())?;
    file.write_all(HOOK_CMD.as_bytes())?;
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

    exit(0)
}

// Color scheme selection. Precedence: env, arg, detection, default.
fn get_color_scheme(args: &Args) -> ColorScheme {
    std::env::var("GIMOJI_COLOR_SCHEME")
        .ok()
        .and_then(|s| match s.as_str() {
            "light" => Some(ColorScheme::Light),
            "dark" => Some(ColorScheme::Dark),
            _ => None,
        })
        .or(args.color_scheme)
        .unwrap_or_else(|| {
            terminal_light::luma()
                .map(|l| {
                    if l > 0.6 {
                        ColorScheme::Light
                    } else {
                        ColorScheme::Dark
                    }
                })
                .unwrap_or_else(|e| {
                    eprintln!("WARNING: Failed to detect terminal luma: {e}. Assuming dark.");

                    ColorScheme::Dark
                })
        })
}

const HOOK_PATH: &str = ".git/hooks/prepare-commit-msg";
const HOOK_HEADER: &str = "#!/usr/bin/env bash\n# gimoji as a commit hook\n";
const HOOK_CMD: &str = "gimoji --hook \"$1\" \"$2\"";
