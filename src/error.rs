//! # GSim Error
//!
//! This module is responsible for organizing different types of errors,
//! produced by different modules.

/// General Cumulative Error type, supporting each individual module errors.
pub enum GSimError {
    Source(std::io::Error),
}

impl std::fmt::Display for GSimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(e) => write!(
                f,
                "File Access Error:\nThe following error occured when accessing the G-Code file:\n{e}"
            ),
        }
    }
}

impl From<std::io::Error> for GSimError {
    fn from(e: std::io::Error) -> Self {
        Self::Source(e)
    }
}
