//! # GSim Configuration
//!
//! This module is responsible for parsing the **command line arguments**,
//! and preparing them for the program.

use clap::Parser;

/// **Parsed** command line arguments.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {
    /// Path of the input G-Code file.
    file: String,
    /// Turn debugging information on.
    #[arg(short, long)]
    debug: bool,
}

impl Config {
    /// Returns the **file path** of the input G-Code file as a string slice.
    pub fn filepath(&self) -> &str {
        self.file.as_str()
    }

    /// Returns the current **debug** setting.
    pub fn debug(&self) -> bool {
        self.debug
    }
}
