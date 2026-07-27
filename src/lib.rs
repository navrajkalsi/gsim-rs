mod cli;
pub mod config;
pub mod geometry;
mod gui;
pub mod interpreter;
pub mod lexer;
pub mod machine;
pub mod parser;
mod renderer;
pub mod source;
mod tui;

use crate::{
    cli::Cli,
    config::{Body, Config, Setup},
    gui::Gui,
    interpreter::{BlockSummary, Interpreter, InterpreterError},
    lexer::Lexer,
    machine::{Machine, MotionSummary},
    parser::Parser,
    source::Source,
    tui::Tui,
};
use clap::Parser as _;
use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

/// Allowed variance when comparing floating points.
const FLOAT_VARIANCE: f32 = 1e-5;

/// Single block execution at program start.
pub const SINGLE: bool = false;
/// Stock visibility at program start.
pub const STOCK: bool = true;
/// Toolpath visibility at program start.
pub const TOOLPATH: bool = true;
/// Tool visibility at program start.
pub const TOOL: bool = true;

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

/// Represents the types of program cycle interruptions.
/// These interruptions need user input to be removed and resume cycle.
#[derive(Debug, Clone, Copy)]
pub enum Interrupt {
    /// Confirm program start or restart.
    Start,
    /// M00 program stop detected.
    Stop,
    /// M01 optional program stop detected.
    OptionalStop,
    /// M30 program end detected.
    End,
}

impl Display for Interrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Interrupt::Start => "START INTERRUPT",
            Interrupt::Stop => "STOP INTERRUPT",
            Interrupt::OptionalStop => "OPTIONAL STOP INTERRUPT",
            Interrupt::End => "END INTERRUPT",
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
    SetToolVisibility(bool),
    SetToolpathVisibility(bool),
    SetStockVisibility(bool),
    Clear,
    Stop(Option<anyhow::Error>),
}

#[derive(Debug, Clone)]
pub enum Signal {
    Run {
        summary: Arc<BlockSummary>,
        machine: Machine,
        current: usize,
    },
    Interrupt {
        interrupt: Interrupt,
        machine: Machine,
        current: usize,
    },
    Error {
        error: InterpreterError,
        machine: Machine,
        current: usize,
    },
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

    let cli = Cli::parse();
    let config = Config::from_file(cli.config.as_str())?;
    let source = match &cli.source {
        Some(path) => Source::from_file(path),
        None => Source::from_stdin(),
    }?;
    let machine = Machine::new(config.units, config.zero_pos, config.start_pos);
    let interpreter = Interpreter::new(Parser::new(Lexer::new(source.clone())), machine.clone());
    let signal = Arc::new(Mutex::new(Signal::Interrupt {
        interrupt: Interrupt::Start,
        machine,
        current: 0,
    }));

    assert_eq!(config.setup, Setup::Milling, "lathe is not implemented yet");
    assert!(
        matches!(config.stock, Body::Cuboid { .. }),
        "cylindrical stock is not implemented yet"
    );

    let gui = Gui::new(config.clone(), signal.clone(), interpreter);
    let tui = Tui::new(gui.create_proxy(), source, signal.clone());

    let child = std::thread::Builder::new()
        .name("TUI".to_string())
        .spawn(move || tui.run())?;

    // any errors from the tui thread will be returned through this call
    let res = gui.run();

    child.join().unwrap();

    res
}
