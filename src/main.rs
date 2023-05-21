use clap::{command, Parser};
use crossterm::{
    event::{read, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, fs::read_to_string, io, path::PathBuf};
use tui::{
    backend::CrosstermBackend,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Terminal,
};

/// Select emoji for git commit message.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Update local emoji cache.
    #[arg(short, long)]
    update_cache: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.update_cache {
        update_cache()?;

        return Ok(());
    }

    let selected = select_emoji()?;
    println!("{}", selected);

    Ok(())
}

fn select_emoji() -> Result<String, Box<dyn Error>> {
    let mut emojis = load_emojis()?.gitmojis;

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = ListState::default();
    state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let items: Vec<_> = emojis
                .iter()
                .map(|emoji| {
                    let s = format!("{} - {} - {}", emoji.emoji, emoji.code, emoji.description);
                    ListItem::new(s)
                })
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .title("Select an emoji")
                        .borders(Borders::ALL),
                )
                .style(Style::default().fg(Color::White))
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Green),
                )
                .highlight_symbol("❯");
            let size = f.size();
            f.render_stateful_widget(list, size, &mut state);
        })?;

        match read()? {
            Event::Key(event) => match event.code {
                KeyCode::Enter => break,
                KeyCode::Down => {
                    let i = state.selected().unwrap();
                    let i = if i >= emojis.len() - 1 { 0 } else { i + 1 };
                    state.select(Some(i));
                }
                KeyCode::Up => {
                    let i = state.selected().unwrap();
                    let i = if i == 0 { emojis.len() - 1 } else { i - 1 };
                    state.select(Some(i));
                }
                _ => {}
            },
            _ => (),
        }
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(emojis.remove(state.selected().unwrap()).emoji)
}

fn load_emojis() -> Result<Emojis, Box<dyn Error>> {
    let path = cache_dir()?.join(EMOJI_CACHE_FILE);
    let emojis_json = match read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => update_cache()?,
        Err(_) => return Err("Failed to read emoji cache".into()),
    };
    let emojis: Emojis = serde_json::from_str(&emojis_json)?;

    Ok(emojis)
}

fn update_cache() -> Result<String, Box<dyn Error>> {
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
struct Emojis {
    gitmojis: Vec<Emoji>,
}

#[derive(serde::Deserialize, Debug)]
struct Emoji {
    code: String,
    description: String,
    emoji: String,
    #[allow(unused)]
    entity: String,
    #[allow(unused)]
    name: String,
}
