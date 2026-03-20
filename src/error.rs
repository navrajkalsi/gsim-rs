//! # GSim Error
//!
//! This module is responsible for organizing different types of errors,
//! produced by different modules.

/// Reset output formatting.
const RESET: &'static str = "\x1b[0m";
/// Format output text in BOLD RED.
const RED: &'static str = "\x1b[1;91m";
/// Format output text in BOLD YELLOW.
const YELLOW: &'static str = "\x1b[1;93m";

/// General Cumulative Error type, supporting each individual module errors.
pub enum GSimError {
    /// Wraps an [`std::io::Error`] produced when reading or accessing the G-Code source file.
    Source(std::io::Error),
}

impl std::fmt::Display for GSimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(e) => write!(
                f,
                "{RED}File Access Error:{RESET} The following error occurred when accessing the G-Code file:\n\t{YELLOW}{e}{RESET}"
            ),
        }
    }
}

impl From<std::io::Error> for GSimError {
    fn from(e: std::io::Error) -> Self {
        Self::Source(e)
    }
}
