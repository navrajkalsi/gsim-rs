//! # Parser
//!
//! The Parser depends on the output of the Lexer,
//! and is responsible for converting a sequence of tokens to a sequence of [`Code`]s.
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

#![allow(unused)]

use super::{lexer::*, *};
use std::{cmp::PartialEq, collections::HashMap, fmt::Debug};

// ALL THE CONST ARRAYS ARE TESTED AT THE END TO PARSE CORRECTLY.

/// Every **G-code** supported.
/// An *array of binary tuples* where index 0 is a G-code *suffix*,
/// and index 1 is the *group* the G-code belongs to.
const GCODES: &[(Int, Group)] = &[
    (0, 1),   // rapid move
    (1, 1),   // feed move
    (2, 1),   // clockwise arc
    (3, 1),   // counter-clockwise arc
    (4, 0),   // dwell
    (17, 2),  // xy plane
    (18, 2),  // xz plane
    (19, 2),  // yz plane
    (20, 6),  // imperial mode
    (21, 6),  // metric mode
    (40, 7),  // cancel cutter comp
    (41, 7),  // left cutter comp
    (42, 7),  // right cutter comp
    (43, 8),  // len comp add
    (44, 8),  // len comp subtract
    (49, 8),  // cancel len comp
    (53, 0),  // machine coord system
    (54, 12), // workpiece coord system
    (80, 9),  // cancel canned cycles
    (90, 3),  // absolute mode
    (91, 3),  // incremental mode
    (94, 5),  // feed per minute mode
    (95, 5),  // feed per rev mode
    (98, 10), // initial return
    (99, 10), // retract return
];

/// Every **M-code** supported.
/// An *array of suffixes* for valid M-codes.
const MCODES: &[Int] = &[
    0,  // program stop
    1,  // optional stop
    3,  // spindle fwd
    4,  // spindle rev
    5,  // spindle stop
    6,  // tool change
    8,  // coolant on
    9,  // coolant off
    30, // program end
];

/// All prefixs that must be suffixed only with **integer type**.
const INTCODES: &[Prefix] = b"DGHMNOPST";

/// All prefixs that must be suffixed only with **floating type**.
const FLOATCODES: &[Prefix] = b"FIJKQRXYZ";

// Characters NOT in `INTCODES` or `FLOATCODES`, are INVALID PREFIXES for this parser.

/// A *tuple struct* that represents a **3D Point** in space.
///
/// The fields represent X, Y, and Z axis respectively.
#[derive(Default, Debug, PartialEq)]
pub struct Point(pub Float, pub Float, pub Float);

/// Same as [`Point`] but the fields can be `None`.
#[derive(Default, Debug, PartialEq)]
pub struct PartialPoint(pub Option<Float>, pub Option<Float>, pub Option<Float>);

impl PartialPoint {
    /// Constructs a [`PartialPoint`] by using a *mutable reference* to a **validated** [`Block`].
    ///
    /// Since `block` is validated by [`validate_block`], therefore:
    /// - All coordinate suffix types will be [`Float`]s.
    ///
    /// Returns a `PartialPoint` that may have all its fields as `None`.
    fn from_block(block: &mut Block) -> Self {
        PartialPoint::custom_from_block((b'X', b'Y', b'Z'), block)
    }

    /// Same as [`PartialPoint::from_block`], but can be used to parse custom prefix characters.
    fn custom_from_block((x, y, z): (Prefix, Prefix, Prefix), block: &mut Block) -> Self {
        PartialPoint(
            block.float_codes.remove(&x),
            block.float_codes.remove(&y),
            block.float_codes.remove(&z),
        )
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
    FixedRadius(Float),
}

/// Represents a *complete independent code*, that is,
/// each variant will contain itself and any other code it is required to have.
///
/// Since a [`Code`] variant may be a result of parsing **one or more [`Token`]s**,
/// it may or may not represent an entire [`Block`] of code.
///
/// Therefore, it is not necessary that a *line/block* of code be parsed into just one [`Code`]
/// or only one variant of it (*a mix of the variants is also valid in a line/block*).
#[derive(Debug, PartialEq)]
pub enum Code {
    G(GCode),
    M(MCode),
    /// Change feed rate.
    F(Float),
    /// Line number.
    N(Int),
    /// Program number.
    O(Int),
    /// Change spindle speed.
    S(Int),
    /// Preload a tool.
    T(Int),

    X(Float),

    Y(Float),

    Z(Float),
}

impl Code {
    /// Retrieves a numeric suffix of a [`Code`].
    ///
    /// For [`Code::G`] and [`Code::M`] this is done by:
    /// returning a primitive discriminant of the enumeration inside the variants.
    /// Rest of the variants directly contain their suffixes.
    ///
    /// Returns a [`Suffix`] which will be the same one that was [`tokenize`]d by the [`lexer`].
    pub fn suffix(&self) -> Suffix {
        match self {
            Self::G(gcode) => Suffix::Int(gcode.suffix()),

            Self::M(mcode) => Suffix::Int(mcode.suffix()),

            Self::F(f) => Suffix::Float(*f),

            Self::N(n) => Suffix::Int(*n),

            Self::O(o) => Suffix::Int(*o),

            Self::S(s) => Suffix::Int(*s),

            Self::T(t) => Suffix::Int(*t),

            Self::X(x) => Suffix::Float(*x),

            Self::Y(y) => Suffix::Float(*y),

            Self::Z(z) => Suffix::Float(*z),
        }
    }
}

/// Represents a *G-code*.
///
/// A G-code is used in toolpaths to move axes of a machine in a controlled way.
///
/// Each variant contains all the other variable values it needs to be a valid.
#[derive(Debug, PartialEq)]
#[repr(usize)]
pub enum GCode {
    /// G00
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(PartialPoint) = 0,

    /// G01
    /// Linear Interpolate to new coordinates using provided feed rate.
    FeedMove {
        p_point: PartialPoint,
        f: Option<Float>,
    } = 1,

    /// G02
    /// Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CWArcMove {
        p_point: PartialPoint,
        method: CircleMethod,
        f: Option<Float>,
    } = 2,

    /// G03
    /// Counter-Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CCWArcMove {
        p_point: PartialPoint,
        method: CircleMethod,
        f: Option<Float>,
    } = 3,

    /// G04
    /// Dwell (sec) blocking further code execution.
    Dwell(Float) = 4,

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

    /// G40
    /// Cancel cutter compensation (G41/G42).
    CancelCutterComp = 40,

    /// G41
    /// 2D left cutter compensation.
    LeftCutterComp(Int) = 41,

    /// G42
    /// 2D right cutter compensation.
    RightCutterComp(Int) = 42,

    /// G43
    /// Tool length compensation by addition.
    ToolLenCompAdd(Int) = 43,

    /// G44
    /// Tool length compensation by subtraction.
    ToolLenCompSubtract(Int) = 44,

    /// G49
    /// Cancel tool length compensation (G43, G44).
    CancelLenComp = 49,

    /// G53
    /// Machine coordinate system.
    MachineCoord(PartialPoint) = 53,

    /// G54
    /// Work coordinate system select.
    WorkCoord = 54,

    /// G80
    /// Cancel canned cycles.
    CancelCanned = 80,

    /// G90
    /// Absolute positioning.
    AbsoluteMode = 90,

    /// G91
    /// Incremental positioning.
    IncrementalMode = 91,

    /// G94
    /// Feed per minute mode.
    FeedMinute = 94,

    /// G95
    /// Feed per revolution mode.
    FeedRev = 95,

    /// G98
    /// Initial point return in canned cycles.
    InitialReturn = 98,

    /// G99
    /// Retract plane return in canned cycles.
    RetractReturn = 99,
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
    pub fn suffix(&self) -> Int {
        unsafe { *(self as *const Self as *const usize) }
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
    /// If the suffix is not found in `GCODES` array,
    /// then the [`GCode`] creation must not have been possible in the first place.
    /// This would indicate a major logic error.
    pub fn group(&self) -> Group {
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
    /// Returns the [`Group`] on success or [`ParserError::InvalidGCode`] on failure.
    fn group_from_suffix(suffix: Int) -> Result<Group, ParserError> {
        for gcode in GCODES {
            if gcode.0 == suffix {
                return Ok(gcode.1);
            }
        }

        Err(ParserError::InvalidGCode(suffix))
    }

    /// Specifically for parsing 'G' prefix codes.
    ///
    /// Accepts the [`Int`] *suffix* of the 'G' prefix code to parse and
    /// a *mutable reference to the [`Block`]* that contains the said [`GCode`].
    /// Since the function accepts a [`Block`], therefore:
    /// - No duplicate tokens are present.
    /// - The suffix types are as expected ([`Int`] for GCodes).
    /// - All int suffixes represent a valid [`GCode`].
    ///
    /// Returns a [`GCode`] with all the specific fields filled from `block`
    /// values on success, and [`ParserError`] on failure.
    ///
    /// The values used in parsing the GCode **will be removed** from `block.int_codes` or
    /// `block.float_codes` as required.
    ///
    /// # Errors
    /// - [`ParserError::InvalidGCode`] -- The code suffix is unknown.
    /// - [`ParserError::InvalidParamForGCode`] -- The required tokens for a GCode variant are
    /// invalid.
    /// - [`ParserError::MissingCodeForGCode`] -- The variant of GCode needs another token for
    /// parsing, that is not present in the block.
    fn parse_from_suffix(suffix: Int, block: &mut Block) -> Result<Self, ParserError> {
        match suffix {
            0 => {
                let p_point = PartialPoint::from_block(block);

                Ok(Self::RapidMove(p_point)) // all fields may be None
            }

            1 => {
                let p_point = PartialPoint::from_block(block);
                let f = block.get_feed();

                Ok(Self::FeedMove { p_point, f })
            }

            2 | 3 => {
                let p_point = PartialPoint::from_block(block);
                let f = block.get_feed();

                // branch based on if 'R' prefix exists or not
                let method = if let Some(r) = block.float_codes.remove(&b'R') {
                    CircleMethod::FixedRadius(r)
                } else {
                    CircleMethod::RelativePoint(PartialPoint::custom_from_block(
                        (b'I', b'J', b'K'),
                        block,
                    ))
                };

                // destination coords are required for arcs.
                if p_point.is_none() {
                    return Err(ParserError::InvalidParamForGCode(suffix));
                }

                // relative center must be on a single plane only, that is,
                // at most 2 axis can be specified, and at least one axis should be present
                if let CircleMethod::RelativePoint(rel_point) = &method {
                    if rel_point.is_some() || rel_point.is_none() {
                        return Err(ParserError::InvalidParamForGCode(suffix));
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
                if let Some(p) = block.int_codes.remove(&b'P') {
                    Ok(Self::Dwell((p as f64) / 1000.0))
                } else if let Some(x) = block.float_codes.remove(&b'X') {
                    // X can be used for seconds
                    Ok(Self::Dwell(x))
                } else {
                    Err(ParserError::MissingCodeForGCode(b'P'))
                }
            }

            17 => Ok(Self::XYPlane),

            18 => Ok(Self::XZPlane),

            19 => Ok(Self::YZPlane),

            20 => Ok(Self::ImperialMode),

            21 => Ok(Self::MetricMode),

            40 => Ok(Self::CancelCutterComp),

            41 | 42 => {
                if let Some(d) = block.int_codes.remove(&b'D') {
                    if suffix == 41 {
                        Ok(Self::LeftCutterComp(d))
                    } else {
                        Ok(Self::RightCutterComp(d))
                    }
                } else {
                    Err(ParserError::MissingCodeForGCode(b'D'))
                }
            }

            43 | 44 => {
                if let Some(h) = block.int_codes.remove(&b'H') {
                    if suffix == 43 {
                        Ok(Self::ToolLenCompAdd(h))
                    } else {
                        Ok(Self::ToolLenCompSubtract(h))
                    }
                } else {
                    Err(ParserError::MissingCodeForGCode(b'H'))
                }
            }

            49 => Ok(Self::CancelLenComp),

            53 => {
                let p_point = PartialPoint::from_block(block);

                if p_point.is_none() {
                    // need atleast one axis to move
                    Err(ParserError::MissingCodeForGCode(b'X'))
                } else {
                    Ok(Self::MachineCoord(p_point))
                }
            }

            54 => Ok(Self::WorkCoord),

            80 => Ok(Self::CancelCanned),

            90 => Ok(Self::AbsoluteMode),

            91 => Ok(Self::IncrementalMode),

            94 => Ok(Self::FeedMinute),

            95 => Ok(Self::FeedRev),

            98 => Ok(Self::InitialReturn),

            99 => Ok(Self::RetractReturn),

            _ => Err(ParserError::InvalidGCode(suffix)),
        }
    }
}

/// Represents a *M-code*.
///
/// A M-code is used to control machine specific features, mostly as an on-off switch.
///
/// Each variant contains all the other variable values it needs to be a valid.
#[derive(Debug, PartialEq)]
#[repr(usize)]
pub enum MCode {
    /// M00
    /// Program stop.
    Stop = 0,

    /// M01
    /// Optional stop.
    OptionalStop = 1,

    /// M03
    /// Spindle forward.
    SpindleFwd(Option<Int>) = 3,

    /// M04
    /// Spindle reverse.
    SpindleRev(Option<Int>) = 4,

    /// M05
    /// Spindle stop.
    SpindleStop = 5,

    /// M06
    /// Tool change.
    ToolChange(Option<Int>) = 6,

    /// M08
    /// Coolant on.
    CoolantOn = 8,

    /// M09
    /// Coolant off.
    CoolantOff = 9,

    /// M30
    /// Program end.
    End = 30,
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
    pub fn suffix(&self) -> Int {
        unsafe { *(self as *const Self as *const usize) }
    }

    /// Specifically for parsing 'M' prefix codes.
    ///
    /// Accepts the [`Int`] *suffix* of the 'M' prefix code to parse and
    /// a *mutable reference to the [`Block`]* that contains the said [`MCode`].
    /// Since the function accepts a [`Block`], therefore:
    /// - No duplicate tokens are present.
    /// - The suffix types are as expected ([`Int`] for MCodes).
    /// - All int suffixes represent a valid [`MCode`].
    ///
    /// Returns a [`MCode`] with all the specific fields filled from `block`
    /// values on success, and [`ParserError`] on failure.
    ///
    /// The values used in parsing the MCode **will be removed** from `block.int_codes` or
    /// `block.float_codes` as required.
    ///
    /// # Errors
    /// - [`ParserError::InvalidMCode`] -- The code suffix is unknown.
    fn parse_from_suffix(suffix: Int, block: &mut Block) -> Result<Self, ParserError> {
        match suffix {
            0 => Ok(Self::Stop),

            1 => Ok(Self::OptionalStop),

            3 | 4 => {
                let speed = block.int_codes.remove(&b'S');

                if suffix == 3 {
                    Ok(Self::SpindleFwd(speed))
                } else {
                    Ok(Self::SpindleRev(speed))
                }
            }

            5 => Ok(Self::SpindleStop),

            6 => Ok(Self::ToolChange(block.int_codes.remove(&b'T'))),

            8 => Ok(Self::CoolantOn),

            9 => Ok(Self::CoolantOff),

            30 => Ok(Self::End),

            _ => Err(ParserError::InvalidMCode(suffix)),
        }
    }
}

/// Possible errors that can happen during parsing.
#[derive(PartialEq)]
pub enum ParserError {
    /// This prefix does not support the type of suffix provided.
    WrongSuffixType(Prefix),
    /// The code prefix provided is invalid/unimplemented
    UnknownPrefix(Prefix),
    /// Same G-code found atleast twice.
    DuplicateGCode(Int),
    /// Prefix and suffix make an invalid G-code.
    InvalidGCode(Int),
    /// G-codes detected from the same group.
    DuplicateGCodeGroup(Group),
    /// Multiple codes of same prefix in the same line.
    /// Only multiple G-codes are allowed in one line.
    DuplicatePrefix(Prefix),
    /// The tokens passed along with a 'G' prefix token
    /// do not meet the requirements of the said GCode variant.
    InvalidParamForGCode(Int),
    /// Missing token required for a GCode variant.
    MissingCodeForGCode(Prefix),
    /// Prefix and suffix make an invalid M-code.
    InvalidMCode(Int),
    /// Missing token required for a MCode variant.
    MissingCodeForMCode(Prefix),
}

impl Debug for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            match self {
                Self::WrongSuffixType(prefix) =>
                    format!("Wrong Suffix Type detected for prefix code: '{prefix}'"),

                Self::UnknownPrefix(prefix) => format!(
                    "Unsupported Prefix Detected:\nPrefix '{prefix}' is not supported by this parser."
                ),

                Self::DuplicateGCode(suffix) => format!(
                    "Duplicate G-Code Detected:\nThe following code was repeated: 'G{suffix}'"
                ),

                Self::InvalidGCode(suffix) => format!(
                    "Invalid G-Code Detected:\nThe following G-Code is not supported by this parser: 'G{suffix}'"
                ),

                Self::DuplicateGCodeGroup(group) => format!(
                    "Duplicate G-Code Group Detected:\nThe following group contains more than one G-Codes belong to it: '{group}'"
                ),

                Self::DuplicatePrefix(prefix) => format!(
                    "Duplicate Prefix Detected:\nThe following prefix code appears more than once: '{prefix}'"
                ),

                Self::InvalidParamForGCode(suffix) => format!(
                    "Invalid Parameter Detected:\nThe following G-Code requirements were not met: 'G{suffix}'"
                ),

                Self::MissingCodeForGCode(prefix) => format!(
                    "Required Code not found for G-Code:\nThe following prefix code was not found: '{prefix}'"
                ),

                Self::InvalidMCode(suffix) => format!(
                    "Invalid M-Code Detected:\nThe following M-Code is not supported by this parser: 'M{suffix}'"
                ),

                Self::MissingCodeForMCode(prefix) => format!(
                    "Required Code not found for M-Code:\nThe following prefix code was not found: '{prefix}'"
                ),
            }
        )
    }
}

/// Represents a *validated block of code(s)*.
///
/// Can only be generated by [`validate_block`], only when the source block is certain to have:
/// - No duplicate prefixes. Except for 'G' prefix.
/// - Unique 'G' prefix codes.
/// - Unique groups for multiple 'G' prefix codes.
/// - Valid suffix type for each prefix.
#[derive(Debug, Clone)]
struct Block {
    /// A vector containing all the valid suffixes found with prefix as 'G'.
    gcodes: Vec<Int>,

    /// An option with its `Some` variant containing the valid suffix found with
    /// prefix as 'M'. Is an `Option` since [`MCode`] is optional and only be one in a `Block`.
    mcode: Option<Int>,

    /// A hash map of **validated** prefixes and `usize` suffixes.
    int_codes: HashMap<Prefix, Int>,

    /// A hash map of **validated** prefixes and `float` suffixes.
    float_codes: HashMap<Prefix, Float>,
}

impl Block {
    /// Removes 'F' key, and *optionally* returns its *float* value.
    fn get_feed(&mut self) -> Option<Float> {
        self.float_codes.remove(&b'F')
    }
}

/// This function is responsible for performing all the validations on a sequence of [`Token`]s,
/// that are required for it to be parsed correctly.
///
/// Consumes the input *vector of [`Token`]s*.
/// Returns a validated [`Block`] on success and [`ParserError`] on failure.
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
/// - [`ParserError::InvalidMCode`] -- The suffix of 'M' prefix token is not valid or supported.
fn validate_block(mut tokens: Vec<Token>) -> Result<Block, ParserError> {
    let mut g_suffix_found = Vec::new(); // unique gcode suffixes found
    let mut m_suffix_found = None; // mcode suffix, if found
    let mut groups_found = Vec::new(); // groups of all gcodes found
    let mut prefix_found = Vec::new(); // unique token prefixes from the block
    let mut int_codes_found: HashMap<Prefix, Int> = HashMap::new();
    let mut float_codes_found: HashMap<Prefix, Float> = HashMap::new();

    for token in &tokens {
        // check suffix type based on the prefix, only for KNOWN/SUPPORTED prefixes
        if INTCODES.contains(&token.prefix) {
            if token.suffix.int().is_none() {
                return Err(ParserError::WrongSuffixType(token.prefix));
            }
        } else if FLOATCODES.contains(&token.prefix) {
            if token.suffix.float().is_none() {
                return Err(ParserError::WrongSuffixType(token.prefix));
            }
        } else {
            return Err(ParserError::UnknownPrefix(token.prefix));
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
                return Err(ParserError::InvalidGCode(suffix));
            }

            // multiple gcodes are valid, but must be of different suffixes
            if g_suffix_found.contains(&suffix) {
                return Err(ParserError::DuplicateGCode(suffix));
            }
            g_suffix_found.push(suffix);

            // the same group must not have been found already
            let group = GCode::group_from_suffix(suffix)?; // can return InvalidGCode

            // check if same group already found or not
            if groups_found.contains(&group) {
                return Err(ParserError::DuplicateGCodeGroup(group));
            }
            groups_found.push(group);
        } else if token.prefix == b'M' {
            if m_suffix_found.is_some() {
                // only one mcode is allowed per block
                return Err(ParserError::DuplicatePrefix(b'M'));
            }

            let suffix = match token.suffix {
                Suffix::Int(suffix) => suffix,
                Suffix::Float(_) => {
                    unreachable!("'M' has been validated to be suffixed by an integer value only.")
                }
            };

            // check if suffix is supported
            if MCODES.iter().position(|mcode| *mcode == suffix).is_none() {
                return Err(ParserError::InvalidMCode(suffix));
            }

            m_suffix_found = Some(suffix);
        } else {
            // mutiple codes of prefix other than 'G' is invalid
            if prefix_found.contains(&token.prefix) {
                return Err(ParserError::DuplicatePrefix(token.prefix));
            }
            prefix_found.push(token.prefix);

            if let Suffix::Int(s) = token.suffix {
                int_codes_found.insert(token.prefix, s);
            } else if let Suffix::Float(s) = token.suffix {
                float_codes_found.insert(token.prefix, s);
            }
        }
    }

    // at this point all 'G' and 'M' prefix codes would be valid, with unique groups, no duplicate
    // suffixes, and int suffix type
    // remove G and M codes from the vector
    tokens.retain(|token| token.prefix != b'G' && token.prefix != b'M');

    Ok(Block {
        gcodes: g_suffix_found,
        mcode: m_suffix_found,
        int_codes: int_codes_found,
        float_codes: float_codes_found,
    })
}

/// Parses a sequence of *tokens*.
///
/// Accepts ownership to a *vector of [`Token`]s*, which can be empty.
///
/// Returns a vector made up of [`Code`]s on success or [`ParserError`] on failure.
/// The returned vector *may be empty*, only if the passed argument is also an empty vector.
///
/// # Errors
/// - Errors generated by [`validate_block`] are returned *as they are*.
pub fn parse(tokens: Vec<Token>) -> Result<Vec<Code>, ParserError> {
    let mut codes = Vec::new();

    if tokens.is_empty() {
        return Ok(codes);
    }

    let mut block = validate_block(tokens)?;
    let gcodes = block.gcodes.clone();
    let mcode = block.mcode;

    // parse g prefix codes
    for suffix in gcodes {
        match GCode::parse_from_suffix(suffix, &mut block) {
            Ok(gcode) => codes.push(Code::G(gcode)),
            Err(ParserError::InvalidGCode(_)) => {
                panic!("Invalid GCode must be dealt with in validate_block().")
            }
            Err(e) => return Err(e),
        }
    }

    // parse m prefix code, if available
    if let Some(suffix) = mcode {
        match MCode::parse_from_suffix(suffix, &mut block) {
            Ok(mcode) => codes.push(Code::M(mcode)),
            Err(ParserError::InvalidMCode(_)) => {
                panic!("Invalid MCode must be dealt with in validate_block().")
            }
            Err(e) => return Err(e),
        }
    }

    Ok(codes)
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
            Suffix::Int(0)
        );

        assert_eq!(Code::M(MCode::Stop).suffix(), Suffix::Int(0));
    }

    #[test]
    // Test incompatible prefix and suffix types.
    fn wrong_suffix_type() {
        assert_eq!(
            tokenize_parse("G20.0").unwrap_err(),
            ParserError::WrongSuffixType(b'G')
        );

        assert_eq!(
            tokenize_parse("F20").unwrap_err(),
            ParserError::WrongSuffixType(b'F')
        );
    }

    #[test]
    // Test unknown prefix
    fn unknown_prefix() {
        assert_eq!(
            tokenize_parse("A0").unwrap_err(),
            ParserError::UnknownPrefix(b'A')
        );
    }

    #[test]
    // Repeat the same 'G' prefix code.
    fn duplicate_gcode() {
        assert_eq!(
            tokenize_parse("G00 G00").unwrap_err(),
            ParserError::DuplicateGCode(0)
        );
    }

    #[test]
    // Test with a G-code having an invalid suffix.
    fn invalid_gcode() {
        // although the gcode is suffixed by an int, the code itself is invalid
        assert_eq!(
            tokenize_parse("G999").unwrap_err(),
            ParserError::InvalidGCode(999)
        );
    }

    #[test]
    // Test with a G-code having an invalid suffix.
    fn duplicate_gcode_group() {
        assert_eq!(
            tokenize_parse("G00 G01").unwrap_err(),
            ParserError::DuplicateGCodeGroup(1)
        );
    }

    #[test]
    // Repeat prefix codes must be rejected, other than 'G' prefix.
    fn duplicate_prefix() {
        assert_eq!(
            tokenize_parse("M5 M9").unwrap_err(),
            ParserError::DuplicatePrefix(b'M')
        );
    }

    #[test]
    // Test all groups are correct.
    fn parse_gcode_groups() {
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
    fn parse_valid_gcodes() {
        let tokens = tokenize("X0. I0. D1 H1").unwrap();
        let block = validate_block(tokens).unwrap();

        for (suffix, group) in GCODES {
            let gcode = GCode::parse_from_suffix(*suffix, &mut block.clone())
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
            ParserError::MissingCodeForGCode(b'P')
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

    #[test]
    fn parse_cutter_comp() {
        assert_eq!(
            tokenize_parse("G40").unwrap(),
            vec![Code::G(GCode::CancelCutterComp)]
        );

        assert_eq!(
            tokenize_parse("G41 D1").unwrap(),
            vec![Code::G(GCode::LeftCutterComp(1))]
        );

        assert_eq!(
            tokenize_parse("G42 D1").unwrap(),
            vec![Code::G(GCode::RightCutterComp(1))]
        );
    }

    #[test]
    fn parse_len_comp() {
        assert_eq!(
            tokenize_parse("G43 H1").unwrap(),
            vec![Code::G(GCode::ToolLenCompAdd(1))]
        );

        assert_eq!(
            tokenize_parse("G44 H1").unwrap(),
            vec![Code::G(GCode::ToolLenCompSubtract(1))]
        );

        assert_eq!(
            tokenize_parse("G49").unwrap(),
            vec![Code::G(GCode::CancelLenComp)]
        );
    }

    #[test]
    fn parse_machine_coord() {
        assert_eq!(
            tokenize_parse("G53").unwrap_err(),
            ParserError::MissingCodeForGCode(b'X')
        );

        assert_eq!(
            tokenize_parse("G53 X0. Z0.").unwrap(),
            vec![Code::G(GCode::MachineCoord(PartialPoint(
                Some(0.0),
                None,
                Some(0.0)
            )))]
        );
    }

    #[test]
    fn parse_workpiece_coord() {
        assert_eq!(
            tokenize_parse("G54").unwrap(),
            vec![Code::G(GCode::WorkCoord)]
        );
    }

    #[test]
    fn parse_canned_cycles() {
        assert_eq!(
            tokenize_parse("G80").unwrap(),
            vec![Code::G(GCode::CancelCanned)]
        );
    }

    #[test]
    fn parse_positioning_modes() {
        assert_eq!(
            tokenize_parse("G90").unwrap(),
            vec![Code::G(GCode::AbsoluteMode)]
        );

        assert_eq!(
            tokenize_parse("G91").unwrap(),
            vec![Code::G(GCode::IncrementalMode)]
        );
    }

    #[test]
    fn parse_feed_modes() {
        assert_eq!(
            tokenize_parse("G94").unwrap(),
            vec![Code::G(GCode::FeedMinute)]
        );

        assert_eq!(
            tokenize_parse("G95").unwrap(),
            vec![Code::G(GCode::FeedRev)]
        );
    }

    #[test]
    fn parse_return_canned() {
        assert_eq!(
            tokenize_parse("G98").unwrap(),
            vec![Code::G(GCode::InitialReturn)]
        );

        assert_eq!(
            tokenize_parse("G99").unwrap(),
            vec![Code::G(GCode::RetractReturn)]
        );
    }

    #[test]
    // Test that all codes inside MCODES array parse.
    fn parse_valid_mcodes() {
        let tokens = tokenize("").unwrap();
        let block = validate_block(tokens).unwrap();

        for suffix in MCODES {
            let gcode = MCode::parse_from_suffix(*suffix, &mut block.clone())
                .expect("Every suffix must generate a valid MCode variant.");

            // test suffix method
            assert_eq!(*suffix, gcode.suffix());
        }
    }

    #[test]
    fn parse_stop() {
        assert_eq!(tokenize_parse("M00").unwrap(), vec![Code::M(MCode::Stop)]);
    }

    #[test]
    fn parse_optional_stop() {
        assert_eq!(
            tokenize_parse("M01").unwrap(),
            vec![Code::M(MCode::OptionalStop)]
        );
    }

    #[test]
    fn parse_spindle() {
        assert_eq!(
            tokenize_parse("M03 S1000").unwrap(),
            vec![Code::M(MCode::SpindleFwd(Some(1000)))]
        );
        assert_eq!(
            tokenize_parse("M03").unwrap(),
            vec![Code::M(MCode::SpindleFwd(None))]
        );
        assert_eq!(
            tokenize_parse("M04 S1000").unwrap(),
            vec![Code::M(MCode::SpindleRev(Some(1000)))]
        );
        assert_eq!(
            tokenize_parse("M04").unwrap(),
            vec![Code::M(MCode::SpindleRev(None))]
        );
        assert_eq!(
            tokenize_parse("M05").unwrap(),
            vec![Code::M(MCode::SpindleStop)]
        );
    }

    #[test]
    fn parse_tool_change() {
        assert_eq!(
            tokenize_parse("M06 T1").unwrap(),
            vec![Code::M(MCode::ToolChange(Some(1)))]
        );
        assert_eq!(
            tokenize_parse("M06").unwrap(),
            vec![Code::M(MCode::ToolChange(None))]
        );
    }

    #[test]
    fn parse_coolant() {
        assert_eq!(
            tokenize_parse("M08").unwrap(),
            vec![Code::M(MCode::CoolantOn)]
        );
        assert_eq!(
            tokenize_parse("M09").unwrap(),
            vec![Code::M(MCode::CoolantOff)]
        );
    }

    #[test]
    fn parse_program_end() {
        assert_eq!(tokenize_parse("M30").unwrap(), vec![Code::M(MCode::End)]);
    }
}
