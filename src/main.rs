use std::error::Error;

use clap::Parser;

use gsim_rs::app::App;
use gsim_rs::config::Config;

use ratatui::{
    Terminal,
    crossterm::{event, execute, terminal},
    prelude::CrosstermBackend,
};

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse();

    let mut app = match App::build(config) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let mut stdout = std::io::stdout();
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        event::EnableMouseCapture
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = app.run(&mut terminal);

    terminal::disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        terminal::LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{err}");
        std::process::exit(1);
    }

    Ok(())
}
