mod cli;
pub mod config;
pub mod geometry;
mod gui;
pub mod interpreter;
pub mod lexer;
pub mod machine;
pub mod parser;
pub mod source;
mod tui;

use crate::{
    cli::Cli,
    config::{Config, Setup, Stock},
    gui::Gui,
    machine::MotionSummary,
    tui::Tui,
};
use clap::Parser;
use std::fmt::Display;

/// Allowed variance when comparing floating points.
const FLOAT_VARIANCE: f32 = 1e-5;

/// Single block execution at program start.
pub const SINGLE: bool = false;
/// Tool visibility at program start.
pub const TOOL: bool = true;
/// XY plane grid visibility at program start.
pub const GRID: bool = true;
/// Origin visibility at program start.
pub const ORIGIN: bool = true;
/// Machine boudnary visibility at program start.
pub const BOUNDARY: bool = false;

/// Represents the possible views that can be used in the [`Gui`] and controlled using [`Tui`].
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, bytemuck::Zeroable)]
pub enum View {
    /// Simuate all three axes, from **isometric view**.
    #[default]
    Isometric,
    /// Simlutate `X` & `Y` axes, from **top view**.
    Top,
}

// required for use in gui uniforms
unsafe impl bytemuck::Pod for View {}

impl Display for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            View::Isometric => "ISOMETRIC",
            View::Top => "TOP",
        };

        write!(f, "{string}")
    }
}

/// Communicates changes from the [`Ratatui`](ratatui) loop,
/// to the [`Winit`](winit) event loop.
#[derive(Debug)]
pub enum Command {
    Render(MotionSummary),
    SetView(View),
    SetSingle(bool),
    SetTool(bool),
    SetGrid(bool),
    SetOrigin(bool),
    SetBoundary(bool),
    Clear,
    Stop(Option<anyhow::Error>),
}

/// Communicates when the [`Winit`](winit) event loop is ready to process
/// another [`Command`] from [`Ratatui`](ratatui) loop.
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    Proceed,
    Stop,
}

fn display_banner() {
    println!(
        "\x1b[1;37m
 ██████╗ ███████╗██╗███╗   ███╗      ██████╗ ███████╗
██╔════╝ ██╔════╝██║████╗ ████║      ██╔══██╗██╔════╝
██║  ███╗███████╗██║██╔████╔██║█████╗██████╔╝███████╗
██║   ██║╚════██║██║██║╚██╔╝██║╚════╝██╔══██╗╚════██║
╚██████╔╝███████║██║██║ ╚═╝ ██║      ██║  ██║███████║
 ╚═════╝ ╚══════╝╚═╝╚═╝     ╚═╝      ╚═╝  ╚═╝╚══════╝
\x1b[0m"
    );
}

/// Main entry point for the program.
///
/// Sets up [`Gui`] in the **main thread**, and [`Tui`] in a **new thread**.
/// Sets up bidirectional communication between both the threads,
/// using a [`Channel`](std::sync::mpsc::channel) and an [`EventLoopProxy`](winit::event_loop::EventLoopProxy).
pub fn run() -> anyhow::Result<()> {
    display_banner();

    let (sender, receiver) = std::sync::mpsc::channel();
    let cli = Cli::parse();
    let config = Config::from_file(cli.config.as_str())?;

    assert_eq!(config.setup, Setup::Milling, "lathe is not implemented yet");
    assert!(
        matches!(config.stock, Stock::Cuboid { .. }),
        "cylindrical stock is not implemented yet"
    );

    let gui = Gui::build(sender, config.clone())?;
    let tui = Tui::build(receiver, cli, config, gui.create_proxy())?;

    let tui = std::thread::Builder::new()
        .name("TUI".to_string())
        .spawn(move || tui.run())?;

    // any errors from the tui thread will be returned through this call
    let res = gui.run();

    tui.join().unwrap();

    res
}
