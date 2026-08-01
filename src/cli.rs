//! # GSim Command Line Interface
//!
//! Command line arguments parser.
//! Extracts the first argument as **G-code source** file path,
//! with optional [`Config`](crate::config) file path.
//!
//! By default, looks for `gsim.json` config file in the current directory.

use clap::Parser;

/// Command line arguments.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path of the input G-code file. If not provided, stdin is targeted instead.
    pub source: Option<String>,

    /// Path of the config file.
    #[arg(short, default_value_t = String::from("./gsim.json"))]
    pub config: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify() {
        Cli::command().debug_assert();
    }
}
