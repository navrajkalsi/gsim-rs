//! # Parser
//!
//! This module depends on the output of the [`Lexer`],
//! and is responsible for converting a sequence of G-Code [`Block`]s (represented as [`Lexer`]),
//! to a sequence of [`CodeBlock`]s (represented as [`Parser`].
//!
//! This module makes the following translations to Lexer structures:
//! -- [`Token`] -> [`Code`]
//! -- [`Block`] -> [`CodeBlock`]
//! -- [`Lexer`] -> [`Parser`]
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

use std::{cmp::PartialEq, collections::HashMap, fmt::Debug};

use super::lexer::{
    *, {Float, Group, Int, Prefix},
};

/// Possible planes for a 3-axis machine.
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub enum Plane {
    #[default]
    XY,
    XZ,
    YZ,
}

/// Represents a **3D Point** in space.
///
/// The fields represent X, Y, and Z axis respectively.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Point(Float, Float, Float);

impl Point {
    /// Constructor for a [`Point`].
    ///
    /// The fields represent X, Y, and Z axis respectively and are necessary.
    pub const fn new(x: Float, y: Float, z: Float) -> Self {
        Self(x, y, z)
    }

    /// Returns a tuple of [`Float`] values for each axis.
    pub fn get(&self) -> (Float, Float, Float) {
        (self.0, self.1, self.2)
    }

    /// Returns current position of the 'X' axis.
    pub fn x(&self) -> Float {
        self.0
    }

    /// Returns current position of the 'Y' axis.
    pub fn y(&self) -> Float {
        self.1
    }

    /// Returns current position of the 'Z' axis.
    pub fn z(&self) -> Float {
        self.2
    }

    /// Set every axis.
    pub fn set(&mut self, x: Float, y: Float, z: Float) {
        self.0 = x;
        self.1 = y;
        self.2 = z;
    }

    /// Optionally set one or multiple axes.
    /// `None` arguments will retain the older value.
    pub fn set_optional(&mut self, x: Option<Float>, y: Option<Float>, z: Option<Float>) {
        self.0 = x.unwrap_or(self.0);
        self.1 = y.unwrap_or(self.1);
        self.2 = z.unwrap_or(self.2);
    }

    /// Checks if any axis of `self` is negative or not.
    pub fn any_negative(&self) -> bool {
        self.0 < 0.0 || self.1 < 0.0 || self.2 < 0.0
    }

    /// Returns a new [`Point`] of ratios for all 3 axes of `self`: (`x`:`y`, `x`:`z`, `y`:`z`)
    pub const fn ratio(&self) -> Self {
        Self::new(self.0 / self.1, self.0 / self.2, self.1 / self.2)
    }

    /// Compare absolute values of each axis of `self` with another [`Point`].
    ///
    /// Returns `false` if all fields of `self` are less than corresponding fields of `other`,
    /// otherwise returns `true` which means at least one field of `self` exceeds that of `other`.
    pub fn over_abs(&self, other: &Self) -> bool {
        self.0.abs() > other.0.abs() || self.1.abs() > other.1.abs() || self.2.abs() > other.2.abs()
    }

    /// Compare absolute values of each axis of `self` with another [`Point`].
    ///
    /// Returns `false` if all fields of `self` are greater than corresponding fields of `other`,
    /// otherwise returns `true` which means at least one field of `other` exceeds that of `self`.
    pub fn under_abs(&self, other: &Self) -> bool {
        self.0.abs() < other.0.abs() || self.1.abs() < other.1.abs() || self.2.abs() < other.2.abs()
    }

    /// Treats all the axes values in *Metric* system, and converts them to *Imperial* system.
    pub fn to_imperial(&mut self) {
        self.0 /= 25.4;
        self.1 /= 25.4;
        self.2 /= 25.4;
    }

    /// Treats all the axes values in *Imperial* system, and converts them to *Metric* system.
    pub fn to_metric(&mut self) {
        self.0 *= 25.4;
        self.1 *= 25.4;
        self.2 *= 25.4;
    }

    /// Calculates distance between `self` and another [`Point`] on a certain plane.
    pub fn dist(&self, other: &Self, plane: Plane) -> Float {
        match plane {
            Plane::XY => ((self.x() - other.x()).powi(2) + (self.y() - other.y()).powi(2)).sqrt(),
            Plane::XZ => ((self.x() - other.x()).powi(2) + (self.z() - other.z()).powi(2)).sqrt(),
            Plane::YZ => ((self.y() - other.y()).powi(2) + (self.z() - other.z()).powi(2)).sqrt(),
        }
    }
}

/// Same as [`Point`] but the fields can be `None`.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct PartialPoint(Option<Float>, Option<Float>, Option<Float>);

impl PartialPoint {
    /// Constructs a [`PartialPoint`] using [`Option<Float>`] for each axis.
    ///
    /// The fields represent X, Y, and Z axis respectively and are *optional*.
    pub fn new(x: Option<Float>, y: Option<Float>, z: Option<Float>) -> Self {
        PartialPoint(x, y, z)
    }

    /// Returns a tuple of [`Option<Float>`] values for each axis.
    pub fn get(&self) -> (Option<Float>, Option<Float>, Option<Float>) {
        (self.0, self.1, self.2)
    }

    /// Returns current position of the 'X' axis.
    pub fn x(&self) -> Option<Float> {
        self.0
    }

    /// Returns current position of the 'Y' axis.
    pub fn y(&self) -> Option<Float> {
        self.1
    }

    /// Returns current position of the 'Z' axis.
    pub fn z(&self) -> Option<Float> {
        self.2
    }

    /// Check if all the axis are `None` variants.
    pub fn are_none(&self) -> bool {
        self.0.is_none() && self.1.is_none() && self.2.is_none()
    }

    /// Check if all the axis are `Some` variants.
    pub fn are_some(&self) -> bool {
        self.0.is_some() && self.1.is_some() && self.2.is_some()
    }

    /// Treats all the axes values in *Metric* system, and converts them to *Imperial* system.
    pub fn to_imperial(&mut self) {
        self.0 = self.0.map(|x| x / 25.4);
        self.1 = self.1.map(|y| y / 25.4);
        self.2 = self.2.map(|z| z / 25.4);
    }

    /// Treats all the axes values in *Imperial* system, and converts them to *Metric* system.
    pub fn to_metric(&mut self) {
        self.0 = self.0.map(|x| x * 25.4);
        self.1 = self.1.map(|y| y * 25.4);
        self.2 = self.2.map(|z| z * 25.4);
    }
}

/// Circular Interpolation helper.
/// Both relative point and radius must not appear in the same block.
#[derive(Clone, Debug, PartialEq)]
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K**.
    RelativePoint(PartialPoint),
    /// Explicit radius specified with **R**.
    FixedRadius(Float),
}

/// Possible [`Prefix`]es that need to be suffixed with an [`Int`].
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum IntPrefix {
    D,
    G,
    H,
    M,
    N,
    O,
    P,
    S,
    T,
}

/// Possible [`Prefix`]es that need to be suffixed with a [`Float`].
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum FloatPrefix {
    F,
    I,
    J,
    K,
    Q,
    R,
    X,
    Y,
    Z,
}

/// Represents a parsed & validated [`Token`].
///
/// This type ensures that each [`Prefix`] is valid and is grouped with a valid [`Suffix`] type.
pub enum Code {
    /// A Code with a prefix that is required to be suffixed with an [`Int`].
    Int(IntPrefix, Int),
    /// A Code with a prefix that is required to be suffixed with a [`Float`].
    Float(FloatPrefix, Float),
}

/// Tries to retrieve an [`Int`] from a [`Suffix`].
///
/// Returns [`ParserError::WrongSuffixType`] if the type is not `Int`.
fn try_int(token: &Token) -> Result<Int, ParserError> {
    token
        .suffix
        .int()
        .ok_or(ParserError::WrongSuffixType(token.prefix))
}

/// Tries to retrieve a [`Float`] from a [`Suffix`].
///
/// Returns [`ParserError::WrongSuffixType`] if the type is not `Float`.
fn try_float(token: &Token) -> Result<Float, ParserError> {
    token
        .suffix
        .float()
        .ok_or(ParserError::WrongSuffixType(token.prefix))
}

impl Code {
    /// Constructs a [`Code`] from a [`Token`].
    ///
    /// Returns a `Code` if the `token.prefix` is valid
    /// and the `token.suffix` type is valid for the said prefix.
    /// Returns a [`ParserError`] on failure.
    fn parse(token: &Token) -> Result<Self, ParserError> {
        let code = match token.prefix {
            b'D' => Self::Int(IntPrefix::D, try_int(token)?),
            b'G' => Self::Int(IntPrefix::G, try_int(token)?),
            b'H' => Self::Int(IntPrefix::H, try_int(token)?),
            b'M' => Self::Int(IntPrefix::M, try_int(token)?),
            b'N' => Self::Int(IntPrefix::N, try_int(token)?),
            b'O' => Self::Int(IntPrefix::O, try_int(token)?),
            b'P' => Self::Int(IntPrefix::P, try_int(token)?),
            b'S' => Self::Int(IntPrefix::S, try_int(token)?),
            b'T' => Self::Int(IntPrefix::T, try_int(token)?),

            b'F' => Self::Float(FloatPrefix::F, try_float(token)?),
            b'I' => Self::Float(FloatPrefix::I, try_float(token)?),
            b'J' => Self::Float(FloatPrefix::J, try_float(token)?),
            b'K' => Self::Float(FloatPrefix::K, try_float(token)?),
            b'Q' => Self::Float(FloatPrefix::Q, try_float(token)?),
            b'R' => Self::Float(FloatPrefix::R, try_float(token)?),
            b'X' => Self::Float(FloatPrefix::X, try_float(token)?),
            b'Y' => Self::Float(FloatPrefix::Y, try_float(token)?),
            b'Z' => Self::Float(FloatPrefix::Z, try_float(token)?),

            _ => return Err(ParserError::UnknownPrefix(token.prefix)),
        };

        Ok(code)
    }

    /// Returns the **ASCII** [`Prefix`] regardless of the `self` variant.
    pub fn prefix(&self) -> Prefix {
        match self {
            Self::Int(i, _) => match i {
                IntPrefix::D => b'D',
                IntPrefix::G => b'G',
                IntPrefix::H => b'H',
                IntPrefix::M => b'M',
                IntPrefix::N => b'N',
                IntPrefix::O => b'O',
                IntPrefix::P => b'P',
                IntPrefix::S => b'S',
                IntPrefix::T => b'T',
            },
            Self::Float(f, _) => match f {
                FloatPrefix::F => b'F',
                FloatPrefix::I => b'I',
                FloatPrefix::J => b'J',
                FloatPrefix::K => b'K',
                FloatPrefix::Q => b'Q',
                FloatPrefix::R => b'R',
                FloatPrefix::X => b'X',
                FloatPrefix::Y => b'Y',
                FloatPrefix::Z => b'Z',
            },
        }
    }
}

/// Represents a collection of **unique** [`Code`]s **without 'G' or 'M'** [`Prefix`]es.
///
/// This type ensures that each prefix is unique.
pub struct Codes {
    int_codes: HashMap<IntPrefix, Int>,
    float_codes: HashMap<FloatPrefix, Float>,
}

impl Codes {
    /// Constructs a new [`Codes`], ready to store non 'G' or 'M' prefixed, unique [`Code`]s.
    pub fn new() -> Self {
        Self {
            int_codes: HashMap::new(),
            float_codes: HashMap::new(),
        }
    }

    /// Adds a new [`Code`] to `self`. The code **must not** be [`Code::G`] or [`Code::M`].
    ///
    /// Returns [`ParserError::DuplicatePrefix`] if a code with same [`Prefix`] is already present.
    ///
    /// # Panics
    /// Panics if called with [`Code::G`] or [`Code::M`] variants.
    pub fn push(&mut self, code: Code) -> Result<(), ParserError> {
        debug_assert!(
            !matches!(
                code,
                Code::Int(IntPrefix::G, _) | Code::Int(IntPrefix::M, _)
            ),
            "A 'G' or 'M' prefixed code was pushed to Codes, which must not be used for G or M codes. Logic Error!"
        );

        let prefix = code.prefix();

        match code {
            Code::Int(intprefix, suffix) => {
                if self.int_codes.contains_key(&intprefix) {
                    Err(ParserError::DuplicatePrefix(prefix))
                } else {
                    self.int_codes.insert(intprefix, suffix);
                    Ok(())
                }
            }
            Code::Float(floatprefix, suffix) => {
                if self.float_codes.contains_key(&floatprefix) {
                    Err(ParserError::DuplicatePrefix(prefix))
                } else {
                    self.float_codes.insert(floatprefix, suffix);
                    Ok(())
                }
            }
        }
    }

    /// Extracts a [`PartialPoint`] by trying to remove [`FloatPrefix::X`], [`FloatPrefix::Y`] &
    /// [`FloatPrefix::Z`] from `self`.
    ///
    /// If any of the axis is not found, the returned `PartialPoint` contains `None` in its place.
    pub fn partial_point(&mut self) -> PartialPoint {
        self.custom_partial_point(&FloatPrefix::X, &FloatPrefix::Y, &FloatPrefix::Z)
    }

    /// Extracts a [`PartialPoint`] by trying to remove **custom** [`FloatPrefix`]es from `self`.
    ///
    /// If any of the `FloatPrefix` is not found,
    /// the returned `PartialPoint` contains `None` in its place.
    pub fn custom_partial_point(
        &mut self,
        first: &FloatPrefix,
        second: &FloatPrefix,
        third: &FloatPrefix,
    ) -> PartialPoint {
        PartialPoint::new(
            self.float_codes.remove(first),
            self.float_codes.remove(second),
            self.float_codes.remove(third),
        )
    }
}

/// Represents a `G` prefixed [`Code`].
///
/// A G-code is used in toolpaths to move axes of a machine in a controlled way.
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
        pos: PartialPoint,
        feed: Option<Float>,
    } = 1,

    /// G02
    /// Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CWArcMove {
        pos: PartialPoint,
        method: CircleMethod,
        feed: Option<Float>,
    } = 2,

    /// G03
    /// Counter-Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CCWArcMove {
        pos: PartialPoint,
        method: CircleMethod,
        feed: Option<Float>,
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
    fn parse(gcode: Code) -> Result<Self, ParserError> {
        // let gcode = match code {
        //     Code::G(0) =>
        //
        //     _ => panic!("Non 'G' prefixed token passed to 'GCode::parse()'. Logic Error!"),
        // }
        Ok(GCode::XYPlane)
    }

    /// Provides the numeric value, suffix of a [`GCode`],
    /// by returning a primitive discriminant of the enumeration.
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
    pub fn group(&self) -> Group {
        match self {
            // non-modal codes
            Self::Dwell(_) | Self::MachineCoord(_) => 0,

            // interpolations moves group
            Self::RapidMove(_)
            | Self::FeedMove { .. }
            | Self::CWArcMove { .. }
            | Self::CCWArcMove { .. } => 1,

            // plane selection group
            Self::XYPlane | Self::XZPlane | Self::YZPlane => 2,

            // positioning selection group
            Self::AbsoluteMode | Self::IncrementalMode => 3,

            // feed type selection group
            Self::FeedMinute | Self::FeedRev => 5,

            // unit type selection group
            Self::ImperialMode | Self::MetricMode => 6,

            // cutter comp group
            Self::CancelCutterComp | Self::LeftCutterComp(_) | Self::RightCutterComp(_) => 7,

            // len comp group
            Self::ToolLenCompAdd(_) | Self::ToolLenCompSubtract(_) | Self::CancelLenComp => 8,

            // canned cycles group
            Self::CancelCanned => 9,

            // canned cycle return level group
            Self::InitialReturn | Self::RetractReturn => 10,

            // work offset group
            Self::WorkCoord => 12,
        }
    }
}

pub struct GCodes {
    gcodes: Vec<GCode>,
    groups: Vec<Group>,
}

impl GCodes {
    fn new() -> Self {
        Self {
            gcodes: Vec::new(),
            groups: Vec::new(),
        }
    }

    fn push(&mut self, gcode: GCode) -> Result<(), ParserError> {
        let group = gcode.group();

        if self.groups.contains(&group) {
            if self.gcodes.contains(&gcode) {
                Err(ParserError::DuplicateGCode(gcode.suffix()))
            } else {
                Err(ParserError::DuplicateGCodeGroup(group))
            }
        } else {
            self.gcodes.push(gcode);
            self.groups.push(group);
            Ok(())
        }
    }
}

// have a gcode and gcodes struct
// only add to gcodes struct when the groups do not interfere

pub struct CodeBlock {
    gcodes: GCodes,
    mcode: Option<MCode>,
    codes: Codes,
}

impl CodeBlock {
    pub fn parse(tokens: Block) -> Result<Self, ParserError> {
        let mut gcodes = Vec::new();
        let mut mcode = None;
        let mut codes = Codes::new();

        while let Some(token) = tokens.next() {
            let code = Code::parse(&token)?;

            // now we know that the code is valid and suffixed by a valid type
            // now separate out g and m codes
            let prefix = code.prefix();

            if prefix == b'G' {
                gcodes.push(code);
            } else if prefix == b'M' {
                if mcode.is_some() {
                    return Err(ParserError::DuplicatePrefix(prefix));
                }
                mcode = Some(MCode::parse(code)?);
            } else {
                // add unique code or error
                codes.push(code)?;
            }
        }

        Ok(Self {
            gcodes: (),
            mcode,
            codes,
        })
    }
}

/// Possible errors that can happen during parsing.
#[derive(PartialEq, Debug)]
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
    /// Prefix was found after parsing G & M Codes, but cannot be parsed on its own.
    UnexpectedPrefix(Prefix),
}
