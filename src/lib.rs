mod cli;
pub mod config;
pub mod geometry;
mod gui;
pub mod interpreter;
pub mod lexer;
pub mod machine;
pub mod parser;
pub mod points;
mod renderer;
pub mod source;
mod tui;

use crate::{
    cli::Cli,
    config::Config,
    gui::Gui,
    interpreter::{BlockSummary, Interpreter, InterpreterError},
    lexer::Lexer,
    machine::Machine,
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

const MAX_SPEED: u8 = 10;
const MIN_SPEED: u8 = 1;
const SPEED: u8 = (MAX_SPEED + MIN_SPEED) / 2;

#[derive(Debug, Clone, Copy)]
pub struct Speed(u8);

impl Default for Speed {
    fn default() -> Self {
        Self(SPEED)
    }
}

impl Display for Speed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self.0 {
            MAX_SPEED => String::from("Max"),
            MIN_SPEED => String::from("Min"),
            SPEED => String::from("Default"),
            num => (num as i8 - SPEED as i8).to_string(),
        };

        write!(f, "{string}")
    }
}

impl Speed {
    pub fn numeric(&self) -> u8 {
        self.0
    }

    pub fn inc(&mut self) -> bool {
        if self.0 < MAX_SPEED {
            self.0 += 1;
            true
        } else {
            false
        }
    }

    pub fn dec(&mut self) -> bool {
        if self.0 > MIN_SPEED {
            self.0 -= 1;
            true
        } else {
            false
        }
    }
}

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
    SetView(View),
    SetSingle(bool),
    SetToolVisibility(bool),
    SetToolpathVisibility(bool),
    SetStockVisibility(bool),
    SetSpeed(Speed),
    ClearInterrupt,
    Next,
    Stop,
}

#[derive(Debug, Clone)]
pub enum Signal {
    Run {
        summary: Arc<BlockSummary>,
        machine: Machine,
        current: usize,
    },
    Pause {
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
/// using an [`Arc<Mutex<Signal>>`] and an [`EventLoopProxy`](winit::event_loop::EventLoopProxy).
pub fn run() -> anyhow::Result<()> {
    display_banner();

    let cli = Cli::parse();
    let config = Config::from_file(cli.config.as_str())?;
    let source = match &cli.source {
        Some(path) => Source::from_file(path),
        None => Source::from_stdin(),
    }?;
    let machine = Machine::new(config.units, config.zero_pos, config.start_pos);
    let interpreter = Interpreter::new(Parser::new(Lexer::new(source.clone())), machine);
    let signal = Arc::new(Mutex::new(Signal::Pause {
        interrupt: Interrupt::Start,
        machine,
        current: 0,
    }));

    let gui = Gui::new(config, signal.clone(), interpreter);
    let tui = Tui::new(gui.create_proxy(), source, signal.clone());

    let child = std::thread::Builder::new()
        .name("TUI".to_string())
        .spawn(move || tui.run())?;

    let gui_res = gui.run();
    let tui_res = child.join().unwrap();

    // prioritize tui error
    tui_res.and(gui_res)
}
