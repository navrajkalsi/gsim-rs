//! # Parser
//!
//! The Parser depends on the output of the Lexer, and is responsible for converting a sequence of
//! tokens to a sequence of [`GCode`]s or [`MCode`]s or a combination of both.
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

#![allow(unused)]

use super::lexer::{self, *};
use std::{cmp::PartialEq, fmt::Debug};

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
#[derive(Default, Debug, PartialEq)]
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
#[derive(Default, Debug, PartialEq)]
pub struct PartialPoint(pub Option<f64>, pub Option<f64>, pub Option<f64>);

/// Represents a *complete independent code*, that is,
/// each variant will contain itself and any other code it is required to have.
///
/// Since a [`Code`] variant may be a result of parsing **one or more [`Token`]s**,
/// it may or may not represent an entire *line/block* of code.
///
/// Therefore, it is not necessary that a *line/block* of code be parsed into just one [`Code`]
/// or only one variant of it (*a mix of the variants is also valid in a line/block*).
#[derive(Debug, PartialEq)]
pub enum Code {
    G(GCode),
    M(MCode),
    /// Preload a tool
    Tool(u8),
}

impl Code {
    /// Provides a numeric value of a [`GCode`] or [`MCode`]
    /// by returning a primitive discriminant of the said enumeration.
    ///
    /// The returned number would be the same one that was [`tokenize`]d
    /// by the [`lexer`] as the [`Suffix`].
    ///
    /// # SAFETY
    /// *Not to be used for any other [`Code`] variants.*
    ///
    /// It is certain that the [`GCode`] & [`MCode`] enums specify a primitive representation,
    /// therefore the discriminant may be accessed via *unsafe pointer casting*.
    ///
    /// # PANICS
    ///
    /// The function panics if called on any other variant.
    pub fn suffix(&self) -> u8 {
        match self {
            Self::G(gcode) => unsafe { *(gcode as *const GCode as *const u8) },
            Self::M(mcode) => unsafe { *(mcode as *const MCode as *const u8) },
            _ => panic!("suffix() must only be called on variants: G & M"),
        }
    }
}

/// Represents a *G-code*.
///
/// A G-code is used in toolpaths to move axes of a machine.
///
/// Each variant contains all the other variable values it needs to be a valid.
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum GCode {
    /// G00
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(PartialPoint) = 0,
}

impl GCode {
    /// Returns what *group* a [`GCode`] belongs to.
    ///
    /// G-codes can be modal and are divided into *groups*.
    ///
    /// At any given time **only one G-code** from each group can be supplied and be activated.
    /// A line/block of code with more than one G-codes of the same group is **invalid**.
    ///
    /// Reference:
    /// [Haas](https://www.haascnc.com/service/service-content/guide-procedures/what-are-g-codes.html#gsc.tab=0)
    pub fn group(&self) -> u8 {
        match self {
            Self::RapidMove(_) => 01,
        }
    }
}

impl Token {
    /// Optionally returns what *group* a [`Suffix`] of a [`Token`] with **G prefix** belongs to.
    ///
    /// Same as *group()* for [`GCode`], but for tokens.
    ///
    /// Intended for use on *valid 'G' prefix* tokens.
    /// `None` is returned when either:
    /// - The prefix is *not 'G'*.
    /// - The suffix variant is [`Suffix::Float`].
    /// - The suffix is an *unknown integer*.
    pub fn group(&self) -> Option<u8> {
        if self.prefix == b'G' {
            match self.suffix {
                Suffix::Int(0) => Some(01),                // rapid move
                Suffix::Int(_) | Suffix::Float(_) => None, // ignore rest of ints and all floats
            }
        } else {
            None // only gcodes are grouped
        }
    }
}

/// Represents a *M-code*.
///
/// A M-code is used to control machine specific features, mostly as an on-off switch.
///
/// Each variant contains all the other variable values it needs to be a valid.
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum MCode {
    /// M00
    /// Program stop.
    Stop = 0,
}

/// Possible errors that can happen during parsing.
#[derive(PartialEq, Debug)]
pub enum ParserError {
    /// Multiple M-codes on the same line.
    MultipleMCodes,
    /// Same G-code found atleast twice.
    DuplicateGCode,
    /// G-codes detected from the same group.
    SameGroupGCode,
    /// The suffix of G-code is detected to be a floating point.
    FloatGCode,
    /// The given G-code is not valid, i.e., the suffix is unknown.
    InvalidGCode,
}

/// Parses a sequence of *tokens*.
///
/// Accepts a vector of [`Token`]s, which can be empty.
///
/// Returns a vector made up of [`Code`]s on success or [`ParserError`] on failure.
/// The returned vector *may be empty*, only if the passed argument is also an empty vector.
///
/// # Errors
/// - Errors generated by [`validate_block`] are returned *as is*.
///
pub fn parse(mut tokens: Vec<Token>) -> Result<Vec<Code>, ParserError> {
    let mut codes = Vec::new();

    if tokens.is_empty() {
        return Ok(codes);
    }

    validate_block(&tokens)?;

    Ok(codes)
}

/// This function is responsible for performing all the validation on a list of [`Token`]s that are
/// required for it to be parsed correctly.
///
/// The purpose of this validation is to make sure that all the tokens present in the sequence
/// (*a line/block of code*), go well together & do not interfere with one another's functionality.
///
/// The [`parse`] function should not contain any validation of the block.
/// Check [*Errors*](#Errors) section below to see what validations are done by this function.
///
/// # Errors
/// - [`ParserError::MultipleMCodes`] -- Two or more [`MCode`]s found.
/// - [`ParserError::FloatGCode`] -- [`GCode`] is suffixed by a floating point number, which is
/// *invalid*.
/// - [`ParserError::InvalidGCode`] -- [`GCode`] is suffixed by an unknown integer value.
pub fn validate_block(tokens: &[Token]) -> Result<(), ParserError> {
    let mut m_found = false; // the block contains a M-code
    let mut g_found = Vec::new(); // unique G-codes read from the block
    let mut g_groups = Vec::new(); // group of G-codes in `g_found` vector

    for token in tokens {
        if token.prefix == b'G' {
            // check suffix variant
            if let Suffix::Float(_) = token.suffix {
                return Err(ParserError::FloatGCode);
            }

            // if G-code already found, return error else add to the vector
            if g_found.contains(&token) {
                return Err(ParserError::DuplicateGCode);
            }
            g_found.push(token);

            // if same group G-code already found, return error else add to the vector
            match token.group() {
                Some(group) => {
                    if g_groups.contains(&group) {
                        return Err(ParserError::SameGroupGCode);
                    }
                    g_groups.push(group);
                }
                None => return Err(ParserError::InvalidGCode), // none is only possible if the
                                                               // suffix is int variant and unknown
            }
        } else if token.prefix == b'M' {
            if m_found {
                return Err(ParserError::MultipleMCodes);
            }
            m_found = true;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{super::lexer::tokenize, *};

    #[test]
    // Test to get the suffix of a code by accessing its discriminant.
    fn get_code_suffix() {
        assert_eq!(
            Code::G(GCode::RapidMove(PartialPoint(None, None, None))).suffix(),
            0
        );
        assert_eq!(Code::M(MCode::Stop).suffix(), 0);
    }

    #[test]
    #[should_panic(expected = "G & M")]
    // Test to get the suffix of an invalid variant.
    fn get_code_suffix_invalid() {
        let _ = Code::Tool(0).suffix();
    }

    #[test]
    // Test G-code with floating point suffix.
    fn floating_gcode() {
        assert_eq!(
            parse(tokenize("G20.0").unwrap()).unwrap_err(),
            ParserError::FloatGCode
        );
    }

    #[test]
    // Test duplicate G-codes.
    fn duplicate_gcode() {
        assert_eq!(
            parse(tokenize("G0 G0 X0. Y0.").unwrap()).unwrap_err(),
            ParserError::DuplicateGCode
        );
    }

    #[test]
    // Multiple M codes must be rejected.
    fn multiple_mcodes() {
        assert_eq!(
            parse(tokenize("G00 M5 M9").unwrap()).unwrap_err(),
            ParserError::MultipleMCodes
        );
    }

    // #[test]
    // // Test empty block.
    // fn parse_emtpy() {
    //     assert_eq!(parse(tokenize("").unwrap()).unwrap(), vec![GCode::Empty]);
    // }
    //
    // #[test]
    // // Test rapid move.
    // fn parse_rapid() {
    //     assert_eq!(
    //         parse(tokenize("G00 X0.0 Y0.0").unwrap()).unwrap(),
    //         Vec::<GCode>::new()
    //     );
    // }
}
