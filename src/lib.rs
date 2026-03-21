mod config;
mod error;
pub mod lexer;
pub mod source;

use crate::{config::Config, error::GSimError, source::Source};
use clap::Parser;

// helper function to facilitate error logging in main
pub fn run() -> Result<(), GSimError> {
    let config = Config::parse();

    if config.debug() {
        println!("config:\n{config:?}");
    }

    let src = Source::from_file(config.filepath())?;

    Ok(())
}
