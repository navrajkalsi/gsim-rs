//! # GSim Configuration
//!
//! This module is responsible for parsing the **command line arguments**,
//! and preparing them for the program.

use clap::Parser;

/// Command line arguments.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {
    /// Path of the input G-Code file.
    file: String,
    /// Print verbose output.
    #[arg(short, long)]
    verbose: bool,
}

impl Config {
    /// Returns the **file path** of the input G-Code file as a string slice.
    pub fn filepath(&self) -> &str {
        self.file.as_str()
    }

    /// Returns the current **verbose** setting.
    pub fn verbose(&self) -> bool {
        self.verbose
    }
}
