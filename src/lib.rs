mod config;
mod error;
mod interpreter;
pub mod lexer;
mod machine;
pub mod parser;
pub mod source;

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

    if config.debug() {
        println!("config:\n{config:?}");
    }

    let src = Source::from_file(config.filepath())?;

    if config.debug() {
        println!("source:\n{src:?}");
    }

    let lexer = Lexer::tokenize(src)?;

    if config.debug() {
        println!("lexer:\n{lexer:?}");
    }

    let parser = crate::parser::Parser::parse(lexer)?;

    if config.debug() {
        println!("parser:\n{parser:?}");
    }

    let machine = Machine::build(Point::new(1000.0, 500.0, -500.0), Unit::default())?;

    if config.debug() {
        println!("machine:\n{machine:?}");
    }

    Interpreter::execute(parser, machine)?;

    Ok(())
}
