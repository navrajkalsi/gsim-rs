use clap::Parser;
use ratatui::{
    Terminal,
    crossterm::{event, execute, terminal},
    prelude::CrosstermBackend,
};
use std::sync::mpsc::Receiver;
use winit::event_loop::EventLoopProxy;

use crate::{Signal, app::App, config::Config};

// this function cannot terminate the main thread, just by returnning Err.
// it must send Signal::Stop with optional error to signal the main thread to exit the program.
pub fn run_tui(proxy: EventLoopProxy<Signal>, proceed: Receiver<bool>) {
    let config = Config::parse();

    let app = match App::build(config, proxy.clone(), proceed) {
        Ok(app) => app,
        Err(err) => return proxy.send_event(Signal::Stop(Some(err.into()))).unwrap(),
    };

    let mut stdout = std::io::stdout();
    if let Err(err) = terminal::enable_raw_mode() {
        return proxy.send_event(Signal::Stop(Some(err.into()))).unwrap();
    };

    if let Err(err) = execute!(
        stdout,
        terminal::EnterAlternateScreen,
        event::EnableMouseCapture
    ) {
        return proxy.send_event(Signal::Stop(Some(err.into()))).unwrap();
    };

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(err) => {
            return proxy.send_event(Signal::Stop(Some(err.into()))).unwrap();
        }
    };

    let res = app.run(&mut terminal);

    if let Err(err) = terminal::disable_raw_mode() {
        return proxy.send_event(Signal::Stop(Some(err.into()))).unwrap();
    }
    if let Err(err) = execute!(
        terminal.backend_mut(),
        terminal::LeaveAlternateScreen,
        event::DisableMouseCapture
    ) {
        return proxy.send_event(Signal::Stop(Some(err.into()))).unwrap();
    }
    if let Err(err) = terminal.show_cursor() {
        return proxy.send_event(Signal::Stop(Some(err.into()))).unwrap();
    }

    // do not message main thread to terminate if it signalled the tui thread to terminate first
    match res {
        Ok(app) => {
            if let Some(false) = app.last_proceed {
                ()
            } else {
                proxy.send_event(Signal::Stop(None)).unwrap();
            }
        }
        Err(err) => proxy.send_event(Signal::Stop(Some(err.into()))).unwrap(),
    }
}
