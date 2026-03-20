//! # GSim Configuration
//!
//! This module is responsible for parsing the **command line arguments**,
//! and preparing them for the program.

/// **Parsed** command line arguments.
pub struct Config {
    pub file: String,
    pub debug: bool,
}

impl Config {
    pub fn build() -> Result<Self, ConfigError> {
        let mut args = std::env::args();
        args.next().expect("At least one element must be present.");

        let mut nonflags = args.by_ref().filter(|arg| !arg.starts_with('-'));
        let file = nonflags.next();

        if file.is_none() {
            return Err(ConfigError::NoFile);
        } else if nonflags.next().is_some() {
            return Err(ConfigError::AmbiguousFile);
        }

        let mut debug = None;

        for mut arg in args {
            arg.remove(0);
            if arg.starts_with('-') {
                return Err(ConfigError::LongArg);
            }

            for c in arg.chars() {
                match c {
                    'd' => {
                        if debug.is_some() {
                            return Err(ConfigError::DuplicateFlag(c));
                        } else {
                            debug = Some(c);
                        }
                    }
                    _ => return Err(ConfigError::UnexpectedFlag(c)),
                }
            }
        }

        Ok(Config {
            file: file.unwrap(),
            debug: debug.is_some(),
        })
    }
}

/// Possible errors that can happen during parsing command line arguments.
pub enum ConfigError {
    /// No G-Code file path provided.
    NoFile,
    /// More than one required file paths detected.
    AmbiguousFile,
    /// Long arg with prefix '--' detected.
    LongArg,
    ///
    DuplicateFlag(char),
    UnexpectedFlag(char),
}
