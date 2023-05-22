use clap::{command, Parser};
use crossterm::{
    event::{read, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, fs::read_to_string, io, path::PathBuf};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
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
    let emojis = load_emojis()?.gitmojis;

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = TableState::default();
    state.select(Some(0));
    let default_search_text = "Use arrow keys or type to search";
    let mut search_text = String::new();

    let selected = loop {
        let emojis: Vec<&Emoji> = emojis
            .iter()
            .filter(|emoji| search_text.is_empty() || emoji.contains(&search_text))
            .collect();
        if state.selected().unwrap() >= emojis.len() {
            // Reset the selection if the list goes shorter than the selected index.
            state.select(Some(0));
        }
        terminal.draw(|f| {
            // ? Choose a gitmoji: (Use arrow keys or type to search)
            let chunks = Layout::default()
                .constraints([Constraint::Percentage(5), Constraint::Percentage(95)].as_ref())
                .margin(1)
                .split(f.size());

            // The text at the top.
            let (text, style) = if search_text.is_empty() {
                (
                    default_search_text,
                    Style::default().add_modifier(Modifier::DIM),
                )
            } else {
                (&*search_text, Style::default())
            };
            let text = format!(" {}", text);
            let text = Paragraph::new(Span::styled(text, style)).block(
                Block::default()
                    .title("Search an emoji")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            );
            f.render_widget(text, chunks[0]);

            // The emoji list.
            let emojis = emojis
                .iter()
                .map(|emoji| Row::new(vec![&*emoji.emoji, &*emoji.code, &*emoji.description]));
            let table = Table::new(emojis)
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
                .highlight_symbol("❯ ")
                .widths(&[
                    Constraint::Percentage(3),
                    Constraint::Percentage(12),
                    Constraint::Percentage(85),
                ])
                .column_spacing(2);
            f.render_stateful_widget(table, chunks[1], &mut state);
        })?;

        match read()? {
            Event::Key(event) => match event.code {
                KeyCode::Enter => match emojis.get(state.selected().unwrap()) {
                    Some(emoji) => break emoji.emoji.clone(),
                    None => (),
                },
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
                KeyCode::Char(c) => search_text.push(c),
                KeyCode::Backspace => {
                    search_text.pop();

                    ()
                }
                _ => {}
            },
            _ => (),
        }
    };

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(selected)
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
    entity: String,
    name: String,
}

impl Emoji {
    fn contains(&self, needle: &str) -> bool {
        self.code.contains(needle)
            || self.description.contains(needle)
            || self.emoji.contains(needle)
            || self.entity.contains(needle)
            || self.name.contains(needle)
    }
}
