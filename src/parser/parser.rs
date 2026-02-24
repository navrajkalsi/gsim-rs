//! # Parser
//!
//! The Parser depends on the output of the Lexer, and is responsible for converting a sequence of
//! tokens to a sequence of [`GCode`]s or [`MCode`]s or a combination of both.
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

#![allow(unused)]

use crate::parser::lexer::Token;

/// A *tuple struct* that represents a **3D Point** in space.
///
/// The fields represent X, Y, and Z axis respectively.
///
/// # Example
/// - Storing **max travels** for each axis of a machine where every axis **must** have a value.
/// ```
/// # use gsim_rs::parser::parser::*;
/// let max_travels = Point(40.0, 20.0, 20.0);
/// ```
#[derive(Debug, PartialEq)]
pub struct Point(pub f64, pub f64, pub f64);

/// Same as [`Point`] but the fields can be `None`.
///
/// # Example
/// - Representing a block when [`GCode`] may or may not contain coordinates for each axis.
/// ```
/// # use gsim_rs::parser::parser::*;
/// PartialPoint(
///     Some(1.0),
///     Some(-5.0),
///     None,
/// ); // Represents block: X2. Y-5.
/// ```
#[derive(Debug, PartialEq)]
pub struct PartialPoint(pub Option<f64>, pub Option<f64>, pub Option<f64>);

/// Represents a *G-code* block.
#[derive(Debug, PartialEq)]
#[repr(i8)]
pub enum GCode {
    /// Tokens has a `len` of 0.
    Empty = -2,

    /// The block only contains coordinates.
    Point(PartialPoint) = -1,

    /// G00
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(PartialPoint) = 0,
}

impl GCode {
    /// Returns primitive discriminant of a [`GCode`] variant.
    ///
    /// # SAFETY
    /// It is certain that the enum specifies a primitive representation, therefore the
    /// discriminant is being accessed via *unsafe pointer casting*.
    fn discriminant(&self) -> i8 {
        unsafe { *(self as *const Self as *const i8) }
    }

    /// Returns the *suffix* of a [`GCode`] word by getting its discriminant.
    pub fn suffix(&self) -> i8 {
        self.discriminant()
    }
}

pub fn parse(tokens: Vec<Token>) -> Vec<GCode> {
    let mut codes = Vec::new();

    if tokens.is_empty() {
        codes.push(GCode::Empty);
        return codes;
    }

    for token in tokens {
        if token.prefix == b'G' {
            println!("g detected");
            println!("suffix: {:?}", token.suffix);
        } else {
            println!("something else");
        }
    }

    codes
}

#[cfg(test)]
mod tests {
    use crate::parser::lexer::tokenize;

    use super::*;

    #[test]
    // Test to get the suffix of a code by accessing its discriminant.
    fn get_code_suffix() {
        assert_eq!(GCode::RapidMove(PartialPoint(None, None, None)).suffix(), 0);
        assert_eq!(GCode::Empty.suffix(), -2);
    }

    #[test]
    // Test empty block.
    fn parse_emtpy() {
        assert_eq!(parse(tokenize("").unwrap()), vec![GCode::Empty]);
    }

    #[test]
    // Test rapid move.
    fn parse_rapid() {
        assert_eq!(
            parse(tokenize("G00 X0.0 Y0.0").unwrap()),
            Vec::<GCode>::new()
        );
    }
}
