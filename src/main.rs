use crate::{config::Config, error::GSimError, source::Source};
mod config;
mod error;
mod source;

fn main() {
    // log error and exit
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

// helper function to facilitate error logging in main
fn run() -> Result<(), GSimError> {
    let config = Config::build().unwrap();

    let src = Source::from_file(config.file.as_str())?;

    Ok(())
}
