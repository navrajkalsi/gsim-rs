use crate::{
    config::Config,
    error::GSimError,
    interpreter::Interpreter,
    lexer::Lexer,
    machine::{Machine, Unit},
    parser::{Parser, Point},
    source::{Line, Source},
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

/// Represents current state of the program.
pub struct App {
    error: Option<GSimError>,
    /// Current selected view.
    view: View,
    /// Single step through code blocks.
    single: bool,
    /// Source loaded parser, ready for iteration.
    parser: Parser,
    /// Machine ready to accept state alterations.
    machine: Machine,
    /// Copy of source for previewing.
    preview: Vec<Line>,
    /// Index of current block being executed for preview.
    current: usize,
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
        })
    }

    pub fn run(&mut self) -> Result<(), GSimError> {
        Ok(())
    }
}
