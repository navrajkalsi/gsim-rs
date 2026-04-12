use std::{
    fmt::Display,
    sync::mpsc::{Receiver, Sender},
};

use ratatui::{
    Terminal,
    crossterm::event::{self, Event, KeyCode},
    prelude::Backend,
};

use crate::{
    Signal,
    config::Config,
    describe::{Describe, Description},
    error::GSimError,
    interpreter::{BlockSummary, Interpreter},
    lexer::Lexer,
    machine::{Machine, Unit},
    parser::{Parser, Point},
    source::Source,
    ui::ui,
};

/// Represents the types of view possible on the left section.
/// Right section always previews the raw code.
#[derive(Default)]
pub enum View {
    /// Only print text description of each block.
    #[default]
    Text,
    /// Simlutate `X` & `Y` axes of the [`Machine`], from **top view**.
    Plane,
    /// Simuate all three axes, from **isometric view**.
    Isometric,
}

/// Represents the types program cycle interruptions that need user input to resume cycle.
pub enum Interrupt {
    /// Confirm program start or restart.
    Start,
    /// M00 program stop detected.
    Stop,
    /// M01 optional program stop detected.
    OptionalStop,
    /// M30 Program end detected.
    End,
}

/// Possible errors that can happen during [`App`] event reading.
#[derive(Debug)]
pub enum AppError {
    IO(std::io::Error),
}

impl Describe for AppError {
    fn describe(&self) -> Description {
        match self {
            AppError::IO(error) => Description::new("Event Read Error Detected", error.to_string()),
        }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::IO(error) => write!(f, "{}", error.to_string()),
        }
    }
}

/// Represents current state of the program.
pub struct App {
    pub error: Option<GSimError>,
    /// Current selected view.
    pub view: View,
    /// Single step through code blocks.
    pub single: bool,
    /// Parsed source loaded interpreter, ready for iteration.
    pub interpreter: Interpreter,
    /// Index of current block being executed for preview.
    pub current: usize,
    /// `None` if the program is running.
    pub interrupt: Option<Interrupt>,
    /// Summaries for all executed blocks.
    /// These stay in memory for the whole life of the program,
    /// making looping for the second times more efficient.
    pub summary: Vec<BlockSummary>,
    /// Send rendering jobs to the [`Winit`](winit) thread.
    pub job: Sender<Signal>,
    /// Proceed and send another job to the [`Winit`](winit) thread.
    pub proceed: Receiver<bool>,
}

impl App {
    /// Constructs an [`App`] and loads the [`Source`].
    ///
    /// Consumes a [`Config`], [`Sender`] for [`Job`]s,
    /// and [`Receiver`] for [`Proceed`]s.
    ///
    /// The [`App::view`] is set to [`View::Text`]
    /// and [`App::single`] block execution is set to `false`.
    ///
    /// Returns [`GSimError`] on failure.
    pub fn build(
        config: Config,
        job: Sender<Signal>,
        proceed: Receiver<bool>,
    ) -> Result<Self, GSimError> {
        let src = Source::from_file(&config.filepath)?;

        Ok(Self {
            error: None,
            view: View::default(),
            single: false,
            interpreter: Interpreter::new(
                Parser::new(Lexer::new(src)),
                Machine::build(Point::new(1000.0, 750.0, -500.0), Unit::default())?,
            ),
            current: 0,
            interrupt: Some(Interrupt::Start),
            summary: Vec::new(),
            job,
            proceed,
        })
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), GSimError>
    where
        GSimError: From<B::Error>,
    {
        // to allow use of ? operator,
        // the parent sends `Signal::Stop`
        self.job.send(Signal::Start).unwrap();

        loop {
            terminal.draw(|f| ui(f, &self))?;

            if !self.single && self.interrupt.is_none() && self.error.is_none() {
                self.execute();
            } else if self.error.is_some()
                && let Event::Key(key) = event::read()?
                && key.kind != event::KeyEventKind::Release
            {
                if key.code == KeyCode::Enter {
                    return Err(self.error.take().unwrap());
                } else {
                    continue;
                }
            } else if let Event::Key(key) = event::read()?
                && key.kind != event::KeyEventKind::Release
            {
                // Skip events that are not KeyEventKind::Press
                match key.code {
                    KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Char('v') => {
                        match self.view {
                            View::Text => self.view = View::Plane,
                            View::Plane => self.view = View::Isometric,
                            View::Isometric => self.view = View::Text,
                        };
                        continue;
                    }
                    KeyCode::Char('s') => {
                        self.single = !self.single;
                        continue;
                    }
                    KeyCode::Char('n') if self.interrupt.is_none() => self.execute(),
                    KeyCode::Enter => match self.interrupt {
                        Some(Interrupt::End) => self.reload(),
                        Some(Interrupt::Start) => {
                            self.interrupt = None;
                            self.execute();
                        }
                        Some(_) => self.interrupt = None,
                        None => {}
                    },
                    _ => {}
                }
            } else {
                continue;
            }

            // self.error = Some(GSimError::Interpreter(
            //     crate::interpreter::InterpreterError::ExcessCode(b'b'),
            // ));
        }
    }

    fn reload(&mut self) {
        self.current = 0;
        self.interrupt = Some(Interrupt::Start);
        self.interpreter.reload();
    }

    /// Execute a single block from the Parser.
    fn execute(&mut self) {
        if self.interrupt.is_some() {
            return;
        }

        // no need to execute again, just display the stored results
        if self.summary.get(self.current).is_some() {
            return;
        }

        let res = match self.interpreter.execute() {
            Ok(res) => res,
            Err(err) => {
                self.error = Some(err.into());
                return;
            }
        };

        match res {
            Some(s) => self.summary.push(s),
            None => {
                self.interrupt = Some(Interrupt::End);
                return;
            }
        };

        self.current += 1;
    }
}

// n should be used in single block.
// enter should be used when not in single block and detection of stop signal
