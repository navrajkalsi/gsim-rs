pub mod app;
pub mod config;
pub mod describe;
mod error;
pub mod geometry;
pub mod gui;
mod interpreter;
pub mod lexer;
mod machine;
pub mod parser;
pub mod source;
pub mod tui;
mod ui;

use crate::{gui::Gui, interpreter::BlockSummary, parser::Point, tui::Tui};

/// Non-Zero extremes for each axis of the machine.
/// Passed to both GUI and TUI.
const MACHINE_TRAVELS: Point = Point::new(500.0, 500.0, -500.0);

/// Communicates changes from the [`Ratatui`](ratatui) loop,
/// to the [`Winit`](winit) event loop.
#[derive(Debug)]
pub enum Command {
    Start(),
    Render(BlockSummary),
    Stop(Option<anyhow::Error>),
}

/// Communicates if the [`Winit`](winit) event loop is ready to process
/// another [`Command`] from [`Ratatui`](ratatui) loop.
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    Proceed,
    Stop,
}

pub fn run() -> anyhow::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();

    let gui = Gui::new(sender, MACHINE_TRAVELS);
    let tui = Tui::new(receiver, MACHINE_TRAVELS, gui.create_proxy());

    let tui = std::thread::Builder::new()
        .name("TUI".to_string())
        .spawn(move || tui.run())?;

    // any errors from the tui thread will be returned through this call
    let res = gui.run();

    tui.join().unwrap();

    res
}
