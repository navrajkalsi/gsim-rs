use clap::Parser;
use ratatui::{
    Terminal,
    crossterm::{event, execute, terminal},
    prelude::CrosstermBackend,
};
use std::sync::mpsc::Receiver;
use winit::event_loop::EventLoopProxy;

use crate::{Command, Signal, app::App, config::Config, parser::Point};

pub struct Tui {
    signal: Receiver<Signal>,
    max_travels: Point,
    event_proxy: EventLoopProxy<Command>,
}

impl Tui {
    pub fn new(
        signal: Receiver<Signal>,
        max_travels: Point,
        event_proxy: EventLoopProxy<Command>,
    ) -> Self {
        Self {
            signal,
            max_travels,
            event_proxy,
        }
    }

    // this function cannot terminate the main thread, just by returnning Err.
    // it must send Command::Stop with optional error to signal the main thread to exit the program.
    pub fn run(self) {
        let config = Config::parse();

        let app = match App::build(
            config,
            self.event_proxy.clone(),
            self.signal,
            self.max_travels,
        ) {
            Ok(app) => app,
            Err(err) => {
                return self
                    .event_proxy
                    .send_event(Command::Stop(Some(err.into())))
                    .unwrap();
            }
        };

        let mut stdout = std::io::stdout();
        if let Err(err) = terminal::enable_raw_mode() {
            return self
                .event_proxy
                .send_event(Command::Stop(Some(err.into())))
                .unwrap();
        };

        if let Err(err) = execute!(
            stdout,
            terminal::EnterAlternateScreen,
            event::EnableMouseCapture
        ) {
            return self
                .event_proxy
                .send_event(Command::Stop(Some(err.into())))
                .unwrap();
        };

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(err) => {
                return self
                    .event_proxy
                    .send_event(Command::Stop(Some(err.into())))
                    .unwrap();
            }
        };

        let res = app.run(&mut terminal);

        if let Err(err) = terminal::disable_raw_mode() {
            return self
                .event_proxy
                .send_event(Command::Stop(Some(err.into())))
                .unwrap();
        }
        if let Err(err) = execute!(
            terminal.backend_mut(),
            terminal::LeaveAlternateScreen,
            event::DisableMouseCapture
        ) {
            return self
                .event_proxy
                .send_event(Command::Stop(Some(err.into())))
                .unwrap();
        }
        if let Err(err) = terminal.show_cursor() {
            return self
                .event_proxy
                .send_event(Command::Stop(Some(err.into())))
                .unwrap();
        }

        // do not message main thread to terminate if it signalled the tui thread to terminate first
        match res {
            Ok(app) => {
                if let Some(Signal::Stop) = app.last_signal {
                    ()
                } else {
                    self.event_proxy.send_event(Command::Stop(None)).unwrap();
                }
            }
            Err(err) => self
                .event_proxy
                .send_event(Command::Stop(Some(err.into())))
                .unwrap(),
        }
    }
}
