use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use std::{
    error::Error,
    io::{self, Write},
    ops::{Deref, DerefMut},
};

pub struct Terminal(ratatui::Terminal<CrosstermBackend<Box<dyn Write>>>);

impl Terminal {
    pub fn setup(use_stderr: bool) -> Result<Self, Box<dyn Error>> {
        // setup terminal
        enable_raw_mode()?;
        let mut output: Box<dyn Write> = if use_stderr {
            Box::new(io::stderr())
        } else {
            Box::new(io::stdout())
        };
        execute!(output, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(output);

        ratatui::Terminal::new(backend)
            .map(|x| Self(x))
            .map_err(Into::into)
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
    type Target = ratatui::Terminal<CrosstermBackend<Box<dyn Write>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Terminal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
