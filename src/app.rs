use std::fmt::Display;

use ratatui::{
    Terminal,
    crossterm::event::{self, Event, KeyCode},
    prelude::Backend,
};

use crate::{
    config::Config,
    describe::{Describe, Description},
    error::GSimError,
    lexer::Lexer,
    machine::{Machine, Unit},
    parser::{Parser, Point},
    source::{Line, Source},
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
    /// Source loaded parser, ready for iteration.
    pub parser: Parser,
    /// Machine ready to accept state alterations.
    machine: Machine,
    /// Index of current block being executed for preview.
    pub current: usize,
    /// `None` if the program is running.
    pub interrupt: Option<Interrupt>,
    /// Text description for latest block.
    pub desc: Vec<String>,
}

impl App {
    /// Constructs an [`App`] from a [`Config`] and loads the [`Source`].
    ///
    /// The [`App::view`] is set to [`View::Text`]
    /// and [`App::single`] block execution is set to `false`.
    ///
    /// Returns [`GSimError`] on failure.
    pub fn build(config: Config) -> Result<Self, GSimError> {
        let src = Source::from_file(&config.filepath)?;

        Ok(Self {
            error: None,
            view: View::default(),
            single: false,
            parser: Parser::new(Lexer::new(src.clone())),
            machine: Machine::build(Point::new(1000.0, 500.0, -500.0), Unit::default())?,
            current: 0,
            interrupt: Some(Interrupt::Start),
            desc: Vec::new(),
        })
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), GSimError>
    where
        GSimError: From<B::Error>,
    {
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
        self.parser.reload();
    }

    /// Execute a single block from the Parser.
    fn execute(&mut self) {
        if self.interrupt.is_some() {
            return;
        }

        let mut block = match self.parser.next() {
            Some(res) => match res {
                Ok(block) => block,
                Err(err) => {
                    self.error = Some(err.into());
                    return;
                }
            },
            None => {
                self.interrupt = Some(Interrupt::End);
                return;
            }
        };

        self.desc.clear();

        for gcode in block.gcodes() {
            self.desc.push(gcode.to_string());
        }

        if let Some(mcode) = block.mcode().take() {
            self.desc.push(mcode.to_string());
        }

        for code in block.codes() {
            self.desc.push(code.prefix().to_string());
        }

        self.current += 1;
    }
}

// n should be used in single block.
// enter should be used when not in single block and detection of stop signal
