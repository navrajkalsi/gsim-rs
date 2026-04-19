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
    gui::{init_gui, run_gui},
    interpreter::BlockSummary,
    parser::Point,
    tui::run_tui,
};

/// Communicates changes from the [`Ratatui`](ratatui) loop,
/// to the [`Winit`](winit) event loop.
#[derive(Debug)]
pub enum Signal {
    Start(Point),
    Render(BlockSummary),
    Stop(Option<anyhow::Error>),
}

pub fn run() -> anyhow::Result<()> {
    let (gui, tui) = std::sync::mpsc::channel();

    let (event_loop, proxy) = init_gui();

    let tui = std::thread::Builder::new()
        .name("RataTUI".to_string())
        .spawn(move || run_tui(proxy, tui))?;

    // any errors from the tui thread will be returned through this call
    let res = run_gui(event_loop, gui);

    tui.join().unwrap();

    res
}
