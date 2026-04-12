pub mod app;
pub mod config;
pub mod describe;
mod error;
pub mod gui;
mod interpreter;
pub mod lexer;
mod machine;
pub mod parser;
pub mod source;
pub mod tui;
mod ui;

use crate::{
    app::{App, View},
    error::GSimError,
    gui::run_gui,
    tui::run_tui,
};

///  Communicates changes from the [`Ratatui`](ratatui) loop,
/// to the [`Winit`](winit) event loop.
pub enum Signal {
    Start,
    Render { view: View },
    Stop(Option<GSimError>),
}

/// Struct for communicating progress from the [`Winit`](winit) event loop,
/// to the [`Ratatui`](ratatui) loop.
pub struct Proceed();

pub fn run() -> Result<(), GSimError> {
    let (job_send, job_recv) = std::sync::mpsc::channel();
    let (proceed_send, proceed_recv) = std::sync::mpsc::channel();

    let tui = std::thread::Builder::new()
        .name("RataTUI".to_string())
        .spawn(move || run_tui(job_send, proceed_recv));

    match job_recv.recv().unwrap() {
        // app started, start event loop
        Signal::Start => (),
        // could not setup terminal
        Signal::Stop(Some(err)) => return Err(err),
        _ => unreachable!("TUI thread provided unexpected Signal. Logic Error!"),
    };

    // any errors from the tui thread will be returned through this call
    let res = run_gui(job_recv, proceed_send);

    tui.join().unwrap();

    res
}
