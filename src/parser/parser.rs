//! # Parser
//!
//! The Parser depends on the output of the Lexer, and is responsible for converting a sequence of
//! tokens to a sequence of [`GCode`]s or [`MCode`]s or a combination of both.
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

use super::lexer::{self, *};
use std::{cmp::PartialEq, fmt::Debug, ptr::eq};

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
    T(u8),
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

    /// G01
    /// Linear Interpolate to new coordinates using provided feed rate.
    FeedMove { point: PartialPoint, f: Option<f64> } = 1,
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
            Self::RapidMove(_) | Self::FeedMove { point: _, f: _ } => 1,
        }
    }

    /// Same as [`GCode::group`], but is *not a method* and rather uses an input `suffix` argument to
    /// try to return a group number.
    ///
    /// **Only [`GCode`]s are grouped codes.**
    ///
    /// Returns the `u8` group on success or [`ParserError::InvalidGCode`] on failure.
    pub fn group_from_suffix(suffix: isize) -> Result<u8, ParserError> {
        match suffix {
            0 => Ok(1), // rapid move
            1 => Ok(1), // feed move
            _ => Err(ParserError::InvalidGCode),
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
    /// This prefix does not support the type of suffix provided.
    WrongSuffixType,
    /// The code prefix provided is invalid/unimplemented
    UnknownPrefix,
    /// Same G-code found atleast twice.
    DuplicateGCode,
    /// Prefix and suffix make an invalid G-code.
    InvalidGCode,
    /// G-codes detected from the same group.
    DuplicateGCodeGroup,
    /// Multiple codes of same prefix in the same line.
    /// Only multiple G-codes are allowed in one line.
    DuplicatePrefix,
}

/// Parses a sequence of *tokens*.
///
/// Accepts ownership to a *vector of [`Token`]s*, which can be empty.
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

    let (gcodes, mut tokens) = validate_block(tokens)?;

    for gcode in gcodes {
        match gcode {
            0 => {
                println!("rapid");
            }
            1 => {
                println!("feed");
            }
            _ => {
                return Err(ParserError::InvalidGCode);
            }
        }
    }

    Ok(codes)
}

/// This function is responsible for performing all the validation on a list of [`Token`]s that are
/// required for it to be parsed correctly.
///
/// Consumes the input *vector of [`Token`]s*.
/// On success, returns a *tuple* made up of two vectors:
/// - A *vector of `isize`*, which contains all the valid [`GCode`] integer suffixes.
/// - A *vector of `Token`s*, which contains all the valid `Tokens`, that are not prefixed with
/// **'G'**.
/// On failure, returns a [`ParserError`].
///
/// The purpose of this validation is to make sure that all the tokens present in the sequence
/// (*a line/block of code*), go well together & do not interfere with one another's functionality.
///
/// The [`parse`] function should not contain any validation of the block.
/// Check [*Errors*](#Errors) section below to see what validations are done by this function.
///
/// # Errors
/// - [`ParserError::WrongSuffixType`] -- The suffix type is not what the prefix expected.
/// - [`ParserError::UnknownPrefix`] -- The prefix character is invalid or not supported by parser.
/// - [`ParserError::DuplicateGCode`] -- Same G-code found more than once.
/// - [`ParserError::InvalidGCode`] -- The suffix of 'G' prefix token is not valid or supported.
/// - [`ParserError::DuplicateGCodeGroup`] -- Two or more G-codes of the same group found.
/// - [`ParserError::DuplicatePrefix`] -- Two or more codes with the same prefix (not 'G') found.
pub fn validate_block(mut tokens: Vec<Token>) -> Result<(Vec<isize>, Vec<Token>), ParserError> {
    let mut g_suffix_found = Vec::new(); // unique gcode suffixes found
    let mut g_found_indices = Vec::new(); // for removing gcodes at the end
    let mut groups_found = Vec::new(); // groups of all gcodes found
    let mut prefix_found = Vec::new(); // unique token prefixes from the block

    for (index, token) in tokens.iter().enumerate() {
        // check suffix type based on the prefix, only for KNOWN/SUPPORTED prefixes
        match token.prefix {
            // following prefix must be with ints
            b'D' | b'G' | b'H' | b'M' | b'N' | b'O' | b'S' | b'T' => {
                if !matches!(token.suffix, Suffix::Int(_)) {
                    return Err(ParserError::WrongSuffixType);
                }
            }
            // following prefix must be with floats
            b'F' | b'I' | b'J' | b'K' | b'Q' | b'R' | b'X' | b'Y' | b'Z' => {
                if !matches!(token.suffix, Suffix::Float(_)) {
                    return Err(ParserError::WrongSuffixType);
                }
            }
            // unknown prefix
            _ => return Err(ParserError::UnknownPrefix),
        }

        // suffix type has been validated

        if token.prefix == b'G' {
            let suffix = match token.suffix {
                Suffix::Int(suffix) => suffix,
                Suffix::Float(_) => {
                    unreachable!("'G' has been validated to be suffixed by an integer value only.")
                }
            };

            // multiple gcodes are valid, but must be of different suffixes
            if g_suffix_found.contains(&suffix) {
                return Err(ParserError::DuplicateGCode);
            }
            g_suffix_found.push(suffix);
            g_found_indices.push(index);

            // the same group must not have been found already
            let group = GCode::group_from_suffix(suffix)?; // can return InvalidGCode

            // check if same group already found or not
            if groups_found.contains(&group) {
                return Err(ParserError::DuplicateGCodeGroup);
            }
            groups_found.push(group);
        } else {
            // mutiple codes of prefix other than 'G' is invalid
            if prefix_found.contains(&token.prefix) {
                return Err(ParserError::DuplicatePrefix);
            }
            prefix_found.push(token.prefix);
        }
    }

    // remove all g_found_indices from the input
    for index in g_found_indices {
        tokens.remove(index);
    }

    Ok((g_suffix_found, tokens))
}

#[cfg(test)]
mod tests {
    use super::{lexer::tokenize, *};

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
        let _ = Code::T(0).suffix();
    }

    #[test]
    // Test incompatible prefix and suffix types.
    fn wrong_suffix_type() {
        assert_eq!(
            parse(tokenize("G20.0").unwrap()).unwrap_err(),
            ParserError::WrongSuffixType
        );

        assert_eq!(
            parse(tokenize("F20").unwrap()).unwrap_err(),
            ParserError::WrongSuffixType
        );
    }

    #[test]
    // Test unknown prefix
    fn unknown_prefix() {
        assert_eq!(
            parse(tokenize("A0").unwrap()).unwrap_err(),
            ParserError::UnknownPrefix
        );
    }

    #[test]
    // Repeat the same 'G' prefix code.
    fn duplicate_gcode() {
        assert_eq!(
            parse(tokenize("G00 G00").unwrap()).unwrap_err(),
            ParserError::DuplicateGCode
        );
    }

    #[test]
    // Test with a G-code having an invalid suffix.
    fn invalid_gcode() {
        // although the gcode is suffixed by an int, the code itself is invalid
        assert_eq!(
            parse(tokenize("G999").unwrap()).unwrap_err(),
            ParserError::InvalidGCode
        );
    }

    #[test]
    // Test with a G-code having an invalid suffix.
    fn duplicate_gcode_group() {
        assert_eq!(
            parse(tokenize("G00 G01").unwrap()).unwrap_err(),
            ParserError::DuplicateGCodeGroup
        );
    }

    #[test]
    // Repeat prefix codes must be rejected, other than 'G' prefix.
    fn duplicate_prefix() {
        assert_eq!(
            parse(tokenize("M5 M9").unwrap()).unwrap_err(),
            ParserError::DuplicatePrefix
        );
    }

    #[test]
    // Test rapid move.
    fn parse_test() {
        assert_eq!(
            parse(tokenize("G00 X0.0 Y0.0").unwrap()).unwrap(),
            Vec::new()
        );
    }
}
