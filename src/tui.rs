use clap::Parser;
use ratatui::{
    Terminal,
    crossterm::{event, execute, terminal},
    prelude::CrosstermBackend,
};
use std::sync::mpsc::{Receiver, Sender};

use crate::{Proceed, Signal, app::App, config::Config};

// this function cannot terminate the main thread, just by returnning Err.
// it must send Signal::End with optional error to signal the main thread to exit the program.
pub fn run_tui(job: Sender<Signal>, proceed: Receiver<Proceed>) {
    let config = Config::parse();

    let mut app = match App::build(config, job.clone(), proceed) {
        Ok(app) => app,
        Err(err) => return job.send(Signal::Stop(Some(err))).unwrap(),
    };

    let mut stdout = std::io::stdout();
    if let Err(err) = terminal::enable_raw_mode() {
        return job.send(Signal::Stop(Some(err.into()))).unwrap();
    };

    if let Err(err) = execute!(
        stdout,
        terminal::EnterAlternateScreen,
        event::EnableMouseCapture
    ) {
        return job.send(Signal::Stop(Some(err.into()))).unwrap();
    };

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(err) => {
            return job.send(Signal::Stop(Some(err.into()))).unwrap();
        }
    };

    let res = app.run(&mut terminal);

    if let Err(err) = terminal::disable_raw_mode() {
        return job.send(Signal::Stop(Some(err.into()))).unwrap();
    }
    if let Err(err) = execute!(
        terminal.backend_mut(),
        terminal::LeaveAlternateScreen,
        event::DisableMouseCapture
    ) {
        return job.send(Signal::Stop(Some(err.into()))).unwrap();
    }
    if let Err(err) = terminal.show_cursor() {
        return job.send(Signal::Stop(Some(err.into()))).unwrap();
    }

    match res {
        Ok(_) => job.send(Signal::Stop(None)).unwrap(),
        Err(err) => job.send(Signal::Stop(Some(err))).unwrap(),
    }
}
