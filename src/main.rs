mod emojis;
mod search_entry;
mod terminal;

use clap::{command, Parser};
use crossterm::event::{read, Event, KeyCode};
use std::error::Error;
use tui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
};

use emojis::{Emoji, Emojis};
use search_entry::SearchEntry;
use terminal::Terminal;

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
        emojis::update_cache()?;

        return Ok(());
    }

    let selected = select_emoji()?;
    println!("{}", selected);

    Ok(())
}

fn select_emoji() -> Result<String, Box<dyn Error>> {
    let emojis = Emojis::load()?.gitmojis;

    let mut terminal = Terminal::setup()?;
    let mut search_entry = SearchEntry::default();

    let mut state = TableState::default();
    state.select(Some(0));

    let selected = loop {
        let search_text = search_entry.text();
        let emojis: Vec<&Emoji> = emojis
            .iter()
            .filter(|emoji| search_text.is_empty() || emoji.contains(search_text))
            .collect();
        if state.selected().unwrap() >= emojis.len() {
            // Reset the selection if the list goes shorter than the selected index.
            state.select(Some(0));
        }
        terminal.draw(|f| {
            let chunks = Layout::default()
                .constraints([Constraint::Percentage(5), Constraint::Percentage(95)].as_ref())
                .margin(1)
                .split(f.size());

            // The search entry goes at the top.
            f.render_widget(&search_entry, chunks[0]);

            // The emoji list.
            let emojis = emojis
                .iter()
                .map(|emoji| Row::new(vec![emoji.emoji(), emoji.code(), emoji.description()]));
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
                    Some(emoji) => break emoji.emoji().to_string(),
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
