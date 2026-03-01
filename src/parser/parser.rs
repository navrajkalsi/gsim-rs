//! # Parser
//!
//! The Parser depends on the output of the Lexer, and is responsible for converting a sequence of
//! tokens to a sequence of [`GCode`]s or [`MCode`]s or a combination of both.
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

use super::lexer::{self, *};
use std::{cmp::PartialEq, fmt::Debug};

// ALL THE CONST ARRAYS ARE TESTED AT THE END.

/// Every **G-code** supported.
/// An *array of binary tuples* where index 0 is a G-code *suffix*,
/// and index 1 is the *group* the G-code belongs to.
const GCODES: &[(i32, u8)] = &[
    (0, 1),  // rapid move
    (1, 1),  // feed move
    (2, 1),  // clockwise arc
    (3, 1),  // counter-clockwise arc
    (4, 0),  // dwell
    (17, 2), // xy plane
    (18, 2), // xz plane
    (19, 2), // yz plane
    (20, 6), // imperial mode
    (21, 6), // metric mode
];

/// Every **M-code** supported.
/// An *array of suffixes* for valid M-codes.
const MCODES: &[i32] = &[
    0, // program stop
];

/// All prefix that must be suffixed only with **integer type**.
const INTCODES: &[u8] = &[b'D', b'G', b'H', b'M', b'N', b'O', b'P', b'S', b'T'];

/// All prefix that must be suffixed only with **floating type**.
const FLOATCODES: &[u8] = &[b'F', b'I', b'J', b'K', b'Q', b'R', b'X', b'Y', b'Z'];

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

impl PartialPoint {
    /// Constructs a [`PartialPoint`] by using a *mutable reference* to a vector of [`Token`]s.
    ///
    /// This function assumes that *tokens* has been verified by [`validate_block`], and therefore:
    /// - All coordinate suffix types will be [`Suffix::Float`].
    ///
    /// Returns a `PartialPoint` that may have all its fields as `None`.
    fn from_tokens(tokens: &mut Vec<Token>) -> Self {
        PartialPoint::custom_from_tokens(b'X', b'Y', b'Z', tokens)
    }

    /// Same as `from_tokens()`, but can be used to parse custom prefixes.
    fn custom_from_tokens(x: u8, y: u8, z: u8, tokens: &mut Vec<Token>) -> Self {
        let mut point = Self::default();

        // remove coords with and add suffix to point
        tokens.retain(|token| match token.prefix {
            prefix if prefix == x => {
                point.0 = token.suffix.float(); // this will be float, None not possible
                false
            }
            prefix if prefix == y => {
                point.1 = token.suffix.float();
                false
            }
            prefix if prefix == z => {
                point.2 = token.suffix.float();
                false
            }
            _ => true,
        });

        point
    }

    /// Check if all the axis are `None` variants.
    pub fn is_none(&self) -> bool {
        self.0.is_none() && self.1.is_none() && self.2.is_none()
    }

    /// Check if all the axis are `Some` variants.
    pub fn is_some(&self) -> bool {
        self.0.is_some() && self.1.is_some() && self.2.is_some()
    }
}

/// Circular Interpolation helper.
/// Both relative point and radius must not appear in the same block.
#[derive(Debug, PartialEq)]
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K**.
    RelativePoint(PartialPoint),
    /// Explicit radius specified with **R**.
    FixedRadius(f64),
}

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
    pub fn suffix(&self) -> i32 {
        match self {
            Self::G(gcode) => gcode.suffix(),
            Self::M(mcode) => mcode.suffix(),
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
#[repr(i32)]
pub enum GCode {
    /// G00
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(PartialPoint) = 0,

    /// G01
    /// Linear Interpolate to new coordinates using provided feed rate.
    FeedMove {
        p_point: PartialPoint,
        f: Option<f64>,
    } = 1,

    /// G02
    /// Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CWArcMove {
        p_point: PartialPoint,
        method: CircleMethod,
        f: Option<f64>,
    } = 2,

    /// G03
    /// Counter-Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CCWArcMove {
        p_point: PartialPoint,
        method: CircleMethod,
        f: Option<f64>,
    } = 3,

    /// G04
    /// Dwell (sec) blocking further code execution.
    Dwell(f64) = 4,

    /// G17
    /// Select plane parallel to X and Y axes (**default for mills**).
    XYPlane = 17,

    /// G18
    /// Select plane parallel to X and Z axes.
    XZPlane = 18,

    /// G19
    /// Select plane parallel to Y and Z axes.
    YZPlane = 19,

    /// G20
    /// Use **imperial** units.
    ImperialMode = 20,

    /// G21
    /// Use **metric** units
    MetricMode = 21,
}

impl GCode {
    /// Provides the numeric value, suffix of a [`GCode`],
    /// by returning a primitive discriminant of the enumeration.
    ///
    /// The returned number would be the same one that was [`tokenize`]d
    /// by the [`lexer`] as the [`Suffix`].
    ///
    /// # SAFETY
    /// It is certain that [`GCode`] enum specifies a primitive representation,
    /// therefore the discriminant may be accessed via *unsafe pointer casting*.
    pub fn suffix(&self) -> i32 {
        unsafe { *(self as *const Self as *const i32) }
    }

    /// Returns what *group* a [`GCode`] belongs to.
    ///
    /// G-codes can be modal and are divided into *groups*.
    ///
    /// At any given time **only one G-code** from each group can be supplied and be activated.
    /// A line/block of code with more than one G-codes of the same group is **invalid**.
    ///
    /// Reference:
    /// [Haas](https://www.haascnc.com/service/service-content/guide-procedures/what-are-g-codes.html#gsc.tab=0)
    ///
    /// # PANICS
    /// If the suffix is not found in `GCODES` array, then the [`GCode`] creation must not have been
    /// possible in the first place.
    /// This would indicate a major logic error.
    pub fn group(&self) -> u8 {
        let suffix = self.suffix();

        for gcode in GCODES {
            if gcode.0 == suffix {
                return gcode.1;
            }
        }

        unreachable!("All GCode variants must be in the GCODES array.");
    }

    /// Same as [`GCode::group`], but is *not a method* and rather
    /// uses an input `suffix` argument to try to return a group number.
    ///
    /// **Only [`GCode`]s are grouped codes.**
    ///
    /// Returns the `u8` group on success or [`ParserError::InvalidGCode`] on failure.
    fn group_from_suffix(suffix: i32) -> Result<u8, ParserError> {
        for gcode in GCODES {
            if gcode.0 == suffix {
                return Ok(gcode.1);
            }
        }

        return Err(ParserError::InvalidGCode);
    }

    /// Specifically for parsing 'G' prefix codes.
    /// Assumes [`validate_block`] has been called, and therefore:
    /// - No duplicate tokens are present.
    /// - The suffix types are as expected ([`Suffix::Int`] for GCoes).
    /// - All int suffixes represent a valid [`GCode`].
    ///
    /// Accepts the *suffix* of the 'G' prefix code and a *mutable reference to a vector of
    /// [`Token`]s* that were found with the said [`GCode`].
    ///
    /// Returns a [`GCode`] with all the specific fields filled from token values on success, and
    /// [`ParserError`] on failure.
    ///
    /// The tokens used in parsing the GCode **will be consumed and removed** from the `tokens`
    /// vector.
    ///
    /// # Errors
    /// - [`ParserError::InvalidGCode`] -- The code suffix is unknown.
    /// - [`ParserError::InvalidParamForGCode`] -- The required tokens for a GCode variant are
    /// invalid.
    /// - [`ParserError::MissingParamForGCode`] -- The variant of GCode needs another token for
    /// parsing, that is not present in the block.
    pub fn parse_from_suffix(suffix: i32, tokens: &mut Vec<Token>) -> Result<Self, ParserError> {
        // parsing can be done with two points in mind:
        // - no duplicate tokens at all.
        // - the suffix types will be as expected.
        match suffix {
            0 => {
                let p_point = PartialPoint::from_tokens(tokens);

                Ok(Self::RapidMove(p_point)) // all fields may be None
            }
            1 => {
                let p_point = PartialPoint::from_tokens(tokens);
                let f = get_feed(tokens);

                Ok(Self::FeedMove { p_point, f })
            }
            2 | 3 => {
                let p_point = PartialPoint::from_tokens(tokens);
                let f = get_feed(tokens);

                // branch based on if 'R' token exists or not
                let method = if let Some(pos) = tokens.iter().position(|token| token.prefix == b'R')
                {
                    CircleMethod::FixedRadius(
                        tokens
                            .remove(pos)
                            .suffix
                            .float()
                            .expect("R must be suffixed by a float."),
                    )
                } else {
                    CircleMethod::RelativePoint(PartialPoint::custom_from_tokens(
                        b'I', b'J', b'K', tokens,
                    ))
                };

                // destination coords are required for arcs.
                if p_point.is_none() {
                    return Err(ParserError::InvalidParamForGCode);
                }

                // relative center must be on a single plane only, that is,
                // at most 2 axis can be specified, and at least one axis should be present
                if let CircleMethod::RelativePoint(rel_point) = &method {
                    if rel_point.is_some() || rel_point.is_none() {
                        return Err(ParserError::InvalidParamForGCode);
                    }
                }

                if suffix == 2 {
                    Ok(Self::CWArcMove { p_point, method, f })
                } else {
                    Ok(Self::CCWArcMove { p_point, method, f })
                }
            }
            4 => {
                // P can be used for milliseconds
                if let Some(pos) = tokens.iter().position(|token| token.prefix == b'P') {
                    let p = tokens
                        .remove(pos)
                        .suffix
                        .int()
                        .expect("P must be suffixed with an int.")
                        as f64;

                    Ok(Self::Dwell(p / 1000 as f64))
                }
                // X can be used for seconds
                else if let Some(pos) = tokens.iter().position(|token| token.prefix == b'X') {
                    Ok(Self::Dwell(
                        tokens
                            .remove(pos)
                            .suffix
                            .float()
                            .expect("X must be suffixed with a float."),
                    ))
                } else {
                    Err(ParserError::MissingParamForGCode)
                }
            }
            17 => Ok(Self::XYPlane),
            18 => Ok(Self::XZPlane),
            19 => Ok(Self::YZPlane),
            20 => Ok(Self::ImperialMode),
            21 => Ok(Self::MetricMode),
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
#[repr(i32)]
pub enum MCode {
    /// M00
    /// Program stop.
    Stop = 0,
}

impl MCode {
    /// Provides the numeric value, suffix of a [`MCode`],
    /// by returning a primitive discriminant of the enumeration.
    ///
    /// The returned number would be the same one that was [`tokenize`]d
    /// by the [`lexer`] as the [`Suffix`].
    ///
    /// # SAFETY
    /// It is certain that [`MCode`] enum specifies a primitive representation,
    /// therefore the discriminant may be accessed via *unsafe pointer casting*.
    pub fn suffix(&self) -> i32 {
        unsafe { *(self as *const Self as *const i32) }
    }
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
    /// The tokens passed along with a 'G' prefix token
    /// do not meet the requirements of the said GCode variant.
    InvalidParamForGCode,
    /// Missing token required for a GCode variant.
    MissingParamForGCode,
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
/// # Panics
/// This function assumes that all invalid GCodes have been detected by [`validate_block`],
/// therefore if any function calls return [`ParserError::InvalidGCode`], the function will panic,
/// indiciating a major design flaw.
pub fn parse(tokens: Vec<Token>) -> Result<Vec<Code>, ParserError> {
    let mut codes = Vec::new();

    if tokens.is_empty() {
        return Ok(codes);
    }

    let (gcodes, mut tokens) = validate_block(tokens)?;

    for suffix in gcodes {
        match GCode::parse_from_suffix(suffix, &mut tokens) {
            Ok(gcode) => codes.push(Code::G(gcode)),
            Err(ParserError::InvalidGCode) => {
                panic!("Invalid GCode must be dealt with in validate_block().")
            }
            Err(e) => return Err(e),
        }
    }

    Ok(codes)
}

/// This function is responsible for performing all the validation on a list of [`Token`]s that are
/// required for it to be parsed correctly.
///
/// Consumes the input *vector of [`Token`]s*.
/// On success, returns a *tuple* made up of two vectors:
/// - A *vector of `i32`*, which contains all the valid [`GCode`] integer suffixes.
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
pub fn validate_block(mut tokens: Vec<Token>) -> Result<(Vec<i32>, Vec<Token>), ParserError> {
    let mut g_suffix_found = Vec::new(); // unique gcode suffixes found
    let mut groups_found = Vec::new(); // groups of all gcodes found
    let mut prefix_found = Vec::new(); // unique token prefixes from the block

    for token in &tokens {
        // check suffix type based on the prefix, only for KNOWN/SUPPORTED prefixes
        if INTCODES.contains(&token.prefix) {
            if !matches!(token.suffix, Suffix::Int(_)) {
                return Err(ParserError::WrongSuffixType);
            }
        } else if FLOATCODES.contains(&token.prefix) {
            if !matches!(token.suffix, Suffix::Float(_)) {
                return Err(ParserError::WrongSuffixType);
            }
        } else {
            return Err(ParserError::UnknownPrefix); // unknown prefix
        }

        // suffix type has been validated

        if token.prefix == b'G' {
            let suffix = match token.suffix {
                Suffix::Int(suffix) => suffix,
                Suffix::Float(_) => {
                    unreachable!("'G' has been validated to be suffixed by an integer value only.")
                }
            };

            // check if suffix is supported
            if GCODES.iter().position(|gcode| gcode.0 == suffix).is_none() {
                return Err(ParserError::InvalidGCode);
            }

            // multiple gcodes are valid, but must be of different suffixes
            if g_suffix_found.contains(&suffix) {
                return Err(ParserError::DuplicateGCode);
            }
            g_suffix_found.push(suffix);

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

    // at this point all 'G' prefix codes would be valid, with unique groups, no duplicate
    // suffixes, and int suffix type
    // remove G-codes from the vector
    tokens.retain(|token| token.prefix != b'G');

    Ok((g_suffix_found, tokens))
}

// ## Parser helper:

/// Removes 'F' prefix token, and *optionally* returns its *float* suffix.
fn get_feed(tokens: &mut Vec<Token>) -> Option<f64> {
    if let Some(pos) = tokens.iter().position(|token| token.prefix == b'F') {
        tokens.remove(pos).suffix.float()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{lexer::tokenize, *};

    // helper for tests
    fn tokenize_parse(tokens: &str) -> Result<Vec<Code>, ParserError> {
        parse(tokenize(tokens).unwrap())
    }

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
            tokenize_parse("G20.0").unwrap_err(),
            ParserError::WrongSuffixType
        );

        assert_eq!(
            tokenize_parse("F20").unwrap_err(),
            ParserError::WrongSuffixType
        );
    }

    #[test]
    // Test unknown prefix
    fn unknown_prefix() {
        assert_eq!(
            tokenize_parse("A0").unwrap_err(),
            ParserError::UnknownPrefix
        );
    }

    #[test]
    // Repeat the same 'G' prefix code.
    fn duplicate_gcode() {
        assert_eq!(
            tokenize_parse("G00 G00").unwrap_err(),
            ParserError::DuplicateGCode
        );
    }

    #[test]
    // Test with a G-code having an invalid suffix.
    fn invalid_gcode() {
        // although the gcode is suffixed by an int, the code itself is invalid
        assert_eq!(
            tokenize_parse("G999").unwrap_err(),
            ParserError::InvalidGCode
        );
    }

    #[test]
    // Test with a G-code having an invalid suffix.
    fn duplicate_gcode_group() {
        assert_eq!(
            tokenize_parse("G00 G01").unwrap_err(),
            ParserError::DuplicateGCodeGroup
        );
    }

    #[test]
    // Repeat prefix codes must be rejected, other than 'G' prefix.
    fn duplicate_prefix() {
        assert_eq!(
            tokenize_parse("M5 M9").unwrap_err(),
            ParserError::DuplicatePrefix
        );
    }

    #[test]
    // Test all groups are correct.
    fn test_gcode_groups() {
        for (suffix, group) in GCODES {
            assert_eq!(
                *group,
                GCode::group_from_suffix(*suffix).expect("Every suffix must be valid.")
            );
        }
    }

    #[test]
    // Test that all codes inside GCODES array parse.
    // also tests the group() and suffix() methods as well.
    fn test_valid_gcodes() {
        for (suffix, group) in GCODES {
            let mut tokens = tokenize("X0. I0.").unwrap();

            let gcode = GCode::parse_from_suffix(*suffix, &mut tokens)
                .expect("Every suffix must generate a valid GCode variant.");

            // test suffix method
            assert_eq!(*suffix, gcode.suffix());

            // test group method
            assert_eq!(*group, gcode.group());
        }
    }

    #[test]
    fn parse_rapid_move() {
        assert_eq!(
            tokenize_parse("G0 X0. Y0.").unwrap(),
            vec![Code::G(GCode::RapidMove(PartialPoint(
                Some(0.0),
                Some(0.0),
                None
            )))]
        );
    }

    #[test]
    fn parse_feed_move() {
        assert_eq!(
            tokenize_parse("G1 X0. Y0. F20.").unwrap(),
            vec![Code::G(GCode::FeedMove {
                p_point: PartialPoint(Some(0.0), Some(0.0), None),
                f: Some(20.0)
            })]
        );
    }

    #[test]
    fn parse_cw_arc() {
        assert_eq!(
            tokenize_parse("G2 X0. I1. J2. F20.").unwrap(),
            vec![Code::G(GCode::CWArcMove {
                p_point: PartialPoint(Some(0.0), None, None),
                method: CircleMethod::RelativePoint(PartialPoint(Some(1.0), Some(2.0), None)),
                f: Some(20.0)
            })]
        );

        assert_eq!(
            tokenize_parse("G2 Y0. R20. F20.").unwrap(),
            vec![Code::G(GCode::CWArcMove {
                p_point: PartialPoint(None, Some(0.0), None),
                method: CircleMethod::FixedRadius(20.0),
                f: Some(20.0)
            })]
        );
    }

    #[test]
    fn parse_ccw_arc() {
        assert_eq!(
            tokenize_parse("G3 X0. I1. J2. F20.").unwrap(),
            vec![Code::G(GCode::CCWArcMove {
                p_point: PartialPoint(Some(0.0), None, None),
                method: CircleMethod::RelativePoint(PartialPoint(Some(1.0), Some(2.0), None)),
                f: Some(20.0)
            })]
        );

        assert_eq!(
            tokenize_parse("G3 Y0. R20. F20.").unwrap(),
            vec![Code::G(GCode::CCWArcMove {
                p_point: PartialPoint(None, Some(0.0), None),
                method: CircleMethod::FixedRadius(20.0),
                f: Some(20.0)
            })]
        );
    }

    #[test]
    fn parse_dwell() {
        assert_eq!(
            tokenize_parse("G4 X10.").unwrap(),
            vec![Code::G(GCode::Dwell(10.0))]
        );

        assert_eq!(
            tokenize_parse("G4 P1000").unwrap(),
            vec![Code::G(GCode::Dwell(1.0))]
        );

        assert_eq!(
            tokenize_parse("G4").unwrap_err(),
            ParserError::MissingParamForGCode
        );
    }

    #[test]
    fn parse_planes() {
        assert_eq!(
            tokenize_parse("G17").unwrap(),
            vec![Code::G(GCode::XYPlane)]
        );

        assert_eq!(
            tokenize_parse("G18").unwrap(),
            vec![Code::G(GCode::XZPlane)]
        );

        assert_eq!(
            tokenize_parse("G19").unwrap(),
            vec![Code::G(GCode::YZPlane)]
        );
    }

    #[test]
    fn parse_unit_modes() {
        assert_eq!(
            tokenize_parse("G20").unwrap(),
            vec![Code::G(GCode::ImperialMode)]
        );

        assert_eq!(
            tokenize_parse("G21").unwrap(),
            vec![Code::G(GCode::MetricMode)]
        );
    }
}
