mod clipboard;
mod event_to_action;
mod terminal;

use clap::{Parser, ValueEnum};
use crossterm::event::{read, Event};
use gimoji_core::{App, Colors, Outcome, EMOJIS};
use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    process::exit,
};
#[cfg(unix)]
use std::{fs::Permissions, os::unix::prelude::PermissionsExt};

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

    /// Output the selected emoji to standard out. Note that this switches the UI to render via stderr.
    #[arg(short, long)]
    stdout: bool,
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
            for emoji in EMOJIS {
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
    let selected = match select_emoji(color_scheme.into(), args.stdout)? {
        Some(s) => s,
        None => return Ok(()),
    };

    if let Some(path) = commit_file_path {
        // Just prepend the emoji to the file.
        let mut file = File::create(path)?;
        let prefix = format!("{selected} ");
        file.write_all(prefix.as_bytes())?;
        if let Some(content) = commit_file_content {
            file.write_all(content.as_bytes())?;
        }
    } else if args.stdout {
        println!("{selected}");
    } else {
        println!("Copied {selected} to the clipboard");
        gimoji_core::Clipboard::copy(&mut clipboard::ArboardImpl, &selected)?;
    }

    Ok(())
}

fn select_emoji(colors: Colors, use_stderr: bool) -> Result<Option<String>, Box<dyn Error>> {
    let mut terminal = Terminal::setup(use_stderr)?;
    let mut app = App::new(EMOJIS, &colors);

    loop {
        terminal.draw(|f| app.render(f))?;

        let Event::Key(event) = read()? else {
            continue;
        };
        let action = match event_to_action::from_key_event(event, app.search_text().is_empty()) {
            Ok(Some(a)) => a,
            Ok(None) => continue,
            Err(_) => {
                let _ = terminal.cleanup();
                exit(130);
            }
        };

        match app.handle(action) {
            Outcome::Continue => {}
            Outcome::Picked(s) => return Ok(Some(s)),
            Outcome::Cancelled => return Ok(None),
        }
    }
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
