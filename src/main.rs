mod config;
mod error;
mod source;

use crate::{config::Config, error::GSimError, source::Source};
use clap::Parser;

fn main() {
    // log error and exit
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

// helper function to facilitate error logging in main
fn run() -> Result<(), GSimError> {
    let config = Config::parse();

    if config.debug() {
        println!("config:\n{config:?}");
    }

    let src = Source::from_file(config.filepath())?;

    Ok(())
}
