use ratatui::{
    Terminal,
    crossterm::event::{self, Event, KeyCode},
    prelude::Backend,
};

use crate::{
    config::Config,
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

/// Represents current state of the program.
pub struct App {
    error: Option<GSimError>,
    /// Current selected view.
    pub view: View,
    /// Single step through code blocks.
    pub single: bool,
    /// Source loaded parser, ready for iteration.
    parser: Parser,
    /// Machine ready to accept state alterations.
    machine: Machine,
    /// Copy of source for previewing.
    pub preview: Vec<Line>,
    /// Index of current block being executed for preview.
    pub current: usize,
    /// `None` if the program is running.
    interrupt: Option<Interrupt>,
    /// Text description for latest block.
    desc: Vec<String>,
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
            preview: src.map(|line| line.to_owned()).collect(),
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

            if !self.single && self.interrupt.is_none() {
                if self.parser.next().is_none() {
                    self.interrupt = Some(Interrupt::End);
                } else {
                    self.current += 1;
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
                        // toggle single block
                        self.single = !self.single;
                        continue;
                    }
                    KeyCode::Char('n') => {
                        self.current += 1;
                    }
                    KeyCode::Enter if self.interrupt.is_some() => {
                        if let Interrupt::End = self.interrupt.as_ref().unwrap() {
                            self.reload();
                        } else {
                            self.interrupt = None;
                        }
                    }
                    _ => {}
                }
            } else {
                continue;
            }
        }
    }

    fn reload(&mut self) {
        self.current = 0;
        self.interrupt = Some(Interrupt::Start);
        self.parser.reload();
    }
}

// n should be used in single block.
// enter should be used when not in single block and detection of stop signal
