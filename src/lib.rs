pub mod config;
mod error;
mod interpreter;
pub mod lexer;
mod machine;
pub mod parser;
pub mod source;
mod verbose;

use crate::{
    config::Config,
    error::GSimError,
    interpreter::Interpreter,
    lexer::Lexer,
    machine::{Machine, Unit},
    parser::Point,
    source::Source,
};
use clap::Parser;

// helper function to facilitate error logging in main
pub fn run() -> Result<(), GSimError> {
    let config = Config::parse();

    let src = Source::from_config(config)?;

    let lexer = Lexer::new(src);

    let parser = crate::parser::Parser::new(lexer);

    let machine = Machine::build(Point::new(1000.0, 500.0, -500.0), Unit::default())?;

    let mut interpreter = Interpreter::new(parser, machine);

    interpreter.execute()?;

    Ok(())
}
