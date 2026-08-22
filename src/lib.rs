mod cli;
pub mod config;
mod cursor;
mod defaults;
pub mod geometry;
mod gui;
pub mod interpreter;
pub mod lexer;
pub mod machine;
pub mod parser;
pub mod points;
mod renderer;
mod signal;
pub mod source;
mod speed;
mod tui;

use crate::{
    cli::Cli,
    config::Config,
    gui::Gui,
    interpreter::{Interpreter, Interrupt},
    lexer::Lexer,
    machine::Machine,
    parser::Parser,
    signal::{CycleSignal, UserSignal},
    source::Source,
    tui::Tui,
};
use clap::Parser as _;
use std::sync::{Arc, Mutex, mpsc};

/// Allowed variance when comparing floating points.
const FLOAT_VARIANCE: f32 = 1e-5;

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
/// using an [`Arc<Mutex<CycleSignal>>`], [`mpsc::channel<UserSignal>`], and an [`EventLoopProxy`](winit::event_loop::EventLoopProxy).
pub fn run() -> anyhow::Result<()> {
    display_banner();

    let cli = Cli::parse();

    let config = match cli.config {
        Some(path) => Config::from_file(path.as_str())?,
        None => Config::default(),
    };
    let source = match &cli.source {
        Some(path) => Source::from_file(path),
        None => Source::from_stdin(),
    }?;
    let machine = Machine::new(config.units, config.zero_pos, config.start_pos);
    let interpreter = Interpreter::new(Parser::new(Lexer::new(source.clone())), machine);

    // start cycle with interrupt
    let cycle = Arc::new(Mutex::new(CycleSignal::Pause {
        interrupt: Interrupt::Start,
        machine,
        index: 0,
    }));

    // setup usersignal channel
    let (user_sender, user_receiver) = mpsc::channel::<UserSignal>();

    let gui = Gui::build(config, interpreter, cycle.clone(), user_sender)?;
    let tui = Tui::new(gui.create_proxy(), source, cycle.clone(), user_receiver);

    let child = std::thread::Builder::new()
        .name("TUI".to_string())
        .spawn(move || tui.run())?;

    let gui_res = gui.run();
    let tui_res = child.join().unwrap();

    // prioritize tui error
    tui_res.and(gui_res)
}
