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
    pub filepath: String,
}
