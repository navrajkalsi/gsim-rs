//! # GSim Error
//!
//! This module is responsible for organizing different types of errors,
//! produced by different modules.

use crate::{Source, lexer::LexerError};

/// Reset output formatting.
pub const RESET: &str = "\x1b[0m";
/// Format output text in BOLD RED.
pub const RED: &str = "\x1b[1;31m";
/// Format output text in BOLD YELLOW.
pub const YELLOW: &str = "\x1b[1;33m";
/// Format output text with an underline.
pub const UNDERLINE: &str = "\x1b[4m";
pub const RESET_UNDERLINE: &str = "\x1b[24m";

/// General Cumulative Error type, supporting each individual module errors.
pub enum GSimError {
    /// Wraps an [`std::io::Error`] produced when reading or accessing the G-Code source file.
    Source(std::io::Error),
    /// Wraps a [`LexerError`] produced when tokenizing a [`Source`].
    Lexer(LexerError),
}

impl std::fmt::Display for GSimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(e) => write!(
                f,
                "{RED}File Access Error:{RESET} The following error occurred when accessing the G-Code file:\n\t{YELLOW}{e}{RESET}"
            ),
            Self::Lexer(e) => write!(
                f,
                "{RED}Lexer Error:{RESET} The following error occurred when tokenizing the G-Code:\n\t{YELLOW}{e}{RESET}"
            ),
        }
    }
}

impl From<std::io::Error> for GSimError {
    fn from(e: std::io::Error) -> Self {
        Self::Source(e)
    }
}

impl From<LexerError> for GSimError {
    fn from(e: LexerError) -> Self {
        Self::Lexer(e)
    }
}
