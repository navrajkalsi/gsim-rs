//! # GSim Error
//!
//! This module is responsible for organizing different types of errors,
//! produced by different modules.

use crate::app::AppError;
use crate::describe::Describe;
use crate::interpreter::InterpreterError;
use crate::machine::MachineError;
use crate::source::SourceError;

use super::lexer::LexerError;
use super::parser::ParserError;

/// Reset output formatting.
pub const RESET: &str = "\x1b[0m";
/// Format output text in BOLD RED.
pub const RED: &str = "\x1b[1;31m";
/// Format output text in BOLD YELLOW.
pub const YELLOW: &str = "\x1b[1;33m";

/// General Cumulative Error type, supporting each individual module errors.
pub enum GSimError {
    /// Wraps an [`SourceError`] produced when reading or accessing the G-Code source file.
    Source(SourceError),
    /// Wraps a [`LexerError`] produced when tokenizing a [`Source`](crate::source::Source).
    Lexer(LexerError),
    /// Wraps a [`ParserError`] produced when parsing a [`Lexer`](crate::lexer::Lexer).
    Parser(ParserError),
    /// Wraps a [`MachineError`] produced when creating a [`Machine`](crate::machine::Machine).
    Machine(MachineError),
    /// Wraps an [`InterpreterError`] produced when changing the state of a [`Machine`](crate::machine::Machine).
    Interpreter(InterpreterError),
    /// Wraps an [`AppError`] produced when reading an event during [`App`](crate::app::App) execution.
    App(AppError),
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
            Self::Parser(e) => write!(
                f,
                "{RED}Parser Error:{RESET} The following error occurred when parsing the G-Code:\n\t{YELLOW}{e}{RESET}"
            ),
            Self::Machine(e) => write!(
                f,
                "{RED}Machine Error:{RESET} The following error occurred during machine creation:\n\t{YELLOW}{e}{RESET}"
            ),
            Self::Interpreter(e) => write!(
                f,
                "{RED}Interpreter Error:{RESET} The following error occurred changing the state of the machine:\n\t{YELLOW}{e}{RESET}"
            ),
            Self::App(e) => write!(
                f,
                "{RED}Event Access Error:{RESET} The following error occurred when reading for input events:\n\t{YELLOW}{e}{RESET}"
            ),
        }
    }
}

impl Describe for GSimError {
    fn describe(&self) -> crate::describe::Description {
        match self {
            GSimError::Source(e) => e.describe(),
            GSimError::Lexer(e) => e.describe(),
            GSimError::Parser(e) => e.describe(),
            GSimError::Machine(e) => e.describe(),
            GSimError::Interpreter(e) => e.describe(),
            GSimError::App(e) => e.describe(),
        }
    }
}

impl From<SourceError> for GSimError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}

impl From<LexerError> for GSimError {
    fn from(e: LexerError) -> Self {
        Self::Lexer(e)
    }
}

impl From<ParserError> for GSimError {
    fn from(e: ParserError) -> Self {
        Self::Parser(e)
    }
}

impl From<MachineError> for GSimError {
    fn from(e: MachineError) -> Self {
        Self::Machine(e)
    }
}

impl From<InterpreterError> for GSimError {
    fn from(e: InterpreterError) -> Self {
        Self::Interpreter(e)
    }
}

impl From<std::io::Error> for GSimError {
    fn from(e: std::io::Error) -> Self {
        Self::App(AppError::IO(e))
    }
}
