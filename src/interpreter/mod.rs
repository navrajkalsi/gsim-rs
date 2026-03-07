//! # Interpreter

use crate::parser::lexer::LexerError;
use crate::parser::parser::*;
use crate::{machine::*, parser::lexer::tokenize};
use std::fmt::Display;
use std::{fmt::Debug, fs, io};

pub struct Interpreter {
    machine: Machine,
    lines: Vec<String>,
}

impl Interpreter {
    /// Constructs an [`Interpreter`] from a provided [`Machine`] and `filepath` containing the
    /// code.
    ///
    /// The file at `filepath` should contain the [`Code`] to execute on the `Machine`.
    ///
    /// Returns a new `Interpreter` instance on success with file contents loaded, or
    /// [`InterpreterError`] on failure.
    ///
    /// # Errors:
    /// - [`InterpreterError::FileError`] -- An error occured when accessing the file at `filepath`.
    pub fn build(machine: Machine, filepath: &str) -> Result<Self, InterpreterError> {
        let lines: Vec<String> = fs::read_to_string(filepath)?
            .lines()
            .map(|line| line.trim_end_matches(';').trim().to_owned())
            .collect();

        // `lines` will now be a vector of strings, with no ';' and leading or trailing whitespaces

        Ok(Interpreter { machine, lines })
    }

    fn run_block(block: &str, machine: &mut Machine) -> Result<(), InterpreterError> {
        let codes = parse(tokenize(block)?)?;
        println!("Output: {codes:?}");

        Ok(())
    }

    pub fn run(&mut self) -> Result<&mut Self, InterpreterError> {
        for line in &self.lines {
            Interpreter::run_block(line.as_str(), &mut self.machine)?;
        }

        Ok(self)
    }

    // have 3 levels of debug.
    // each level presents state different levels of details for states of the machine.
    // these will be same for each type of simulator:
    // text
    // 2d
    // 3d
}

/// Possible errors that can happen during Interpreting.
#[derive(Debug)]
pub enum InterpreterError {
    FileError(io::Error),
    LexerError(LexerError),
    ParserError(ParserError),
}

/// Convert I/O Errors to InterpreterError.
impl From<io::Error> for InterpreterError {
    fn from(e: io::Error) -> Self {
        Self::FileError(e)
    }
}

/// Convert Lexer Errors to InterpreterError.
impl From<LexerError> for InterpreterError {
    fn from(e: LexerError) -> Self {
        Self::LexerError(e)
    }
}

/// Convert Parser Errors to InterpreterError.
impl From<ParserError> for InterpreterError {
    fn from(e: ParserError) -> Self {
        Self::ParserError(e)
    }
}

impl Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            match self {
                Self::FileError(e) => format!(
                    "File Access Error:\nThe following error occured when accessing the 'G-Code' file:\n{e}."
                ),
                Self::LexerError(e) => format!(
                    "Lexer Error:\nThe following error occured when tokenizing the 'G-Code':\n{e}."
                ),
                Self::ParserError(e) => format!(
                    "Parser Error:\nThe following error occured when parsing the 'G-Code':\n{e}."
                ),
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE: &'static str = "uniquegcodefile";

    #[test]
    fn construct_interpreter() {
        let c = "G00 X0. Y0.;\n\nG43 H1;\n";
        let m = Machine::build(Point::new(1000.0, 500.0, 500.0)).unwrap();

        fs::write(TEST_FILE, c).unwrap();

        let mut ip = Interpreter::build(m, TEST_FILE).unwrap();

        assert_eq!(
            ip.lines,
            vec![
                String::from("G00 X0. Y0."),
                String::new(),
                String::from("G43 H1")
            ]
        );

        ip.run();

        fs::remove_file(TEST_FILE).unwrap();
    }
}
