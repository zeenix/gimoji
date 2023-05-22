mod emojis;
mod search_entry;
mod selection_view;
mod terminal;

use clap::{command, Parser};
use crossterm::event::{read, Event, KeyCode};
use selection_view::SelectionView;
use std::error::Error;
use tui::layout::{Constraint, Layout};

use emojis::Emojis;
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
    let mut selection_view = SelectionView::new(emojis);

    let selected = loop {
        let search_text = search_entry.text();
        let mut filtered_view = selection_view.filtered_view(&search_text);

        terminal.draw(|f| {
            let chunks = Layout::default()
                .constraints([Constraint::Percentage(5), Constraint::Percentage(95)].as_ref())
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
