use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    error::Error,
    io::{self, Stdout},
    ops::{Deref, DerefMut},
};
use tui::backend::CrosstermBackend;

pub struct Terminal(tui::Terminal<CrosstermBackend<Stdout>>);

impl Terminal {
    pub fn setup() -> Result<Self, Box<dyn Error>> {
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);

        tui::Terminal::new(backend).map(Self).map_err(Into::into)
    }

    pub fn cleanup(&mut self) -> Result<(), Box<dyn Error>> {
        // restore terminal
        disable_raw_mode()?;
        execute!(self.0.backend_mut(), LeaveAlternateScreen,)?;
        self.0.show_cursor()?;

        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // FIXME: Don't panic on error but instead log.
        self.cleanup().unwrap();
    }
}

impl Deref for Terminal {
    type Target = tui::Terminal<CrosstermBackend<Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Terminal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
