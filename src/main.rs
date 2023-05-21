use crossterm::{
    event::{read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io};
use tui::{
    backend::CrosstermBackend,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Terminal,
};

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let items = ["Item 1", "Item 2", "Item 3"];
    let mut state = ListState::default();
    state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let items: Vec<_> = items.iter().map(|s| ListItem::new(*s)).collect();
            let list = List::new(items)
                .block(Block::default().title("List").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
                .highlight_symbol(">>");
            let size = f.size();
            f.render_stateful_widget(list, size, &mut state);
        })?;

        match read()? {
            Event::Key(event) => match event.code {
                KeyCode::Char('q') => break,
                KeyCode::Down => {
                    let i = state.selected().unwrap();
                    let i = if i >= items.len() - 1 { 0 } else { i + 1 };
                    state.select(Some(i));
                }
                KeyCode::Up => {
                    let i = state.selected().unwrap();
                    let i = if i == 0 { items.len() - 1 } else { i - 1 };
                    state.select(Some(i));
                }
                _ => {}
            },
            _ => (),
        }
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
