use crate::{
    config::Config,
    error::GSimError,
    interpreter::Interpreter,
    lexer::Lexer,
    machine::{Machine, Unit},
    parser::{Parser, Point},
    source::Source,
};

/// Represents the types of view possible on the left side.
/// Right side always previews the raw code.
#[derive(Default)]
pub enum View {
    /// Verbose flag was passed. Log everything.
    /// Cannot be toggled to or from after program start.
    Verbose,
    /// Only print text description of each block.
    #[default]
    Text,
    /// Simlutate `X` & `Y` axes of the [`Machine`], from a **top view**.
    TwoDimensional,
    /// Simuate all three axes, from an **isometric view**.
    ThreeDimensional,
}

/// Represents current state of the program.
pub struct App {
    error: Option<GSimError>,
    /// Current selected view.
    view: View,
    /// Single step through code blocks.
    single: bool,
    config: Config,
    /// Raw G-Code directly from the source file.
    src: Vec<String>,
    /// Index of current block being executed.
    current: usize,
}

impl App {
    /// Construct an [`App`] from a [`Config`].
    ///
    /// If [`Config::verbose`] is `true`,
    /// the [`App::view`] is set to [`View::Verbose`] and cannot be altered again in the program.
    pub fn new(config: Config) -> Self {
        Self {
            error: None,
            view: if config.verbose {
                View::Verbose
            } else {
                View::default()
            },
            single: false,
            config,
            src: Vec::new(),
            current: 0,
        }
    }

    pub fn run(&mut self) -> Result<(), GSimError> {
        let src = Source::from_config(self.config.clone())?;

        // fill preview source vector
        self.src = src.clone().map(|line| line.as_str().to_owned()).collect();

        let lexer = Lexer::new(src);

        let parser = Parser::new(lexer);

        let machine = Machine::build(Point::new(1000.0, 500.0, -500.0), Unit::default())?;

        let mut interpreter = Interpreter::new(parser, machine);

        while let Some(()) = interpreter.execute_single()? {}

        Ok(())
    }
}
