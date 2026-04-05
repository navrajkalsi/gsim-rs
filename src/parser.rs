//! # Parser
//!
//! This module depends on the output of the [`Lexer`],
//! and is responsible for converting a sequence of G-code [`Block`]s (represented as [`Lexer`]),
//! to a sequence of [`CodeBlock`]s (represented as [`Parser`].
//!
//! This parser is **stateless** and does not deal with any state logic across blocks.
//!
//! This module makes the following translations to Lexer structures:
//! - [`Token`] -> [`Code`]
//! - [`Block`] -> [`CodeBlock`]
//! - [`Lexer`] -> [`Parser`]
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

use std::{
    cmp::PartialEq,
    fmt::{Debug, Display},
};

use crate::describe::{Describe, Description};

use super::{
    error::{RED, RESET},
    lexer::{
        Block, *, {Float, Group, Int, Prefix},
    },
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

    /// Compares absolute values of each axis of `self` with another [`Point`].
    ///
    /// Returns `false` if all fields of `self` are less than corresponding fields of `other`,
    /// otherwise returns `true` which means at least one field of `self` exceeds that of `other`.
    pub fn over_abs(&self, other: &Self) -> bool {
        self.0.abs() > other.0.abs() || self.1.abs() > other.1.abs() || self.2.abs() > other.2.abs()
    }

    /// Compares absolute values of each axis of `self` with another [`Point`].
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

/// Same as [`Point`] but the fields are [`Option`]al.
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

impl Display for PartialPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.are_none() {
            return Ok(());
        }

        write!(f, "(")?;

        if let Some(x) = self.x() {
            write!(f, "X: {x}")?
        }

        if let Some(y) = self.y() {
            if self.x().is_some() {
                write!(f, ", Y: {y}")?
            } else {
                write!(f, "Y: {y}")?
            }
        }

        if let Some(z) = self.z() {
            if self.x().is_some() || self.y().is_some() {
                write!(f, ", Z: {z}")?
            } else {
                write!(f, "Z: {z}")?
            }
        }

        write!(f, ")")
    }
}

/// Circular Interpolation helper.
///
/// Both relative point and radius must not appear in the same block.
#[derive(Clone, Debug, PartialEq)]
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K**.
    RelativePoint(PartialPoint),
    /// Explicit radius specified with **R**.
    FixedRadius(Float),
}

impl Display for CircleMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RelativePoint(pos) => write!(f, "Relative Center at: {pos}"),
            Self::FixedRadius(r) => write!(f, "Radius: {r}"),
        }
    }
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

/// Represents a parsed & validated [`Token`].
///
/// This type ensures that each [`Prefix`] is valid and is grouped with a valid [`Suffix`] type.
#[derive(Debug, PartialEq)]
pub enum Code {
    D(Int),
    G(Int),
    H(Int),
    M(Int),
    N(Int),
    O(Int),
    P(Int),
    S(Int),
    T(Int),

    F(Float),
    I(Float),
    J(Float),
    K(Float),
    Q(Float),
    R(Float),
    X(Float),
    Y(Float),
    Z(Float),
}

impl Code {
    /// Tries to construct a [`Code`] from a [`Token`].
    ///
    /// Returns a `Code` if the `token.prefix` is valid
    /// and the `token.suffix` type is valid for the said prefix.
    /// Returns a [`ParserError`] on failure.
    fn parse(token: &Token) -> Result<Self, ParserError> {
        let code = match token.prefix {
            b'D' => Self::D(try_int(token)?),
            b'G' => Self::G(try_int(token)?),
            b'H' => Self::H(try_int(token)?),
            b'M' => Self::M(try_int(token)?),
            b'N' => Self::N(try_int(token)?),
            b'O' => Self::O(try_int(token)?),
            b'P' => Self::P(try_int(token)?),
            b'S' => Self::S(try_int(token)?),
            b'T' => Self::T(try_int(token)?),

            b'F' => Self::F(try_float(token)?),
            b'I' => Self::I(try_float(token)?),
            b'J' => Self::J(try_float(token)?),
            b'K' => Self::K(try_float(token)?),
            b'Q' => Self::Q(try_float(token)?),
            b'R' => Self::R(try_float(token)?),
            b'X' => Self::X(try_float(token)?),
            b'Y' => Self::Y(try_float(token)?),
            b'Z' => Self::Z(try_float(token)?),

            _ => return Err(ParserError::UnknownPrefix(token.prefix)),
        };

        Ok(code)
    }

    /// Returns the **ASCII** [`Prefix`] of `self`.
    pub fn prefix(&self) -> Prefix {
        match self {
            Code::D(_) => b'D',
            Code::G(_) => b'G',
            Code::H(_) => b'H',
            Code::M(_) => b'M',
            Code::N(_) => b'N',
            Code::O(_) => b'O',
            Code::P(_) => b'P',
            Code::S(_) => b'S',
            Code::T(_) => b'T',
            Code::F(_) => b'F',
            Code::I(_) => b'I',
            Code::J(_) => b'J',
            Code::K(_) => b'K',
            Code::Q(_) => b'Q',
            Code::R(_) => b'R',
            Code::X(_) => b'X',
            Code::Y(_) => b'Y',
            Code::Z(_) => b'Z',
        }
    }
}

/// Represents a collection of **unique** [`Code`]s **without 'G' or 'M'** [`Prefix`]es.
///
/// This type ensures that each `Prefix` is only present once in a [`CodeBlock`].
#[derive(Debug, Default)]
pub struct Codes {
    d: Option<Int>,
    f: Option<Float>,
    h: Option<Int>,
    i: Option<Float>,
    j: Option<Float>,
    k: Option<Float>,
    n: Option<Int>,
    o: Option<Int>,
    p: Option<Int>,
    q: Option<Float>,
    r: Option<Float>,
    s: Option<Int>,
    t: Option<Int>,
    x: Option<Float>,
    y: Option<Float>,
    z: Option<Float>,
}

impl Codes {
    /// Constructs a new [`Codes`], ready to store non 'G' or non 'M' prefixed, unique codes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tries to add a new [`Code`] to `self`. The code **must not** be [`Code::G`] or [`Code::M`] variant.
    ///
    /// Returns [`ParserError::DuplicatePrefix`] if a code with same [`Prefix`] is already present.
    ///
    /// # Panics
    /// Panics if called with [`Code::G`] or [`Code::M`] variants.
    pub fn push(&mut self, code: Code) -> Result<(), ParserError> {
        match code {
            Code::D(d) if self.d.is_none() => self.d = Some(d),
            Code::H(h) if self.h.is_none() => self.h = Some(h),
            Code::N(n) if self.n.is_none() => self.n = Some(n),
            Code::O(o) if self.o.is_none() => self.o = Some(o),
            Code::P(p) if self.p.is_none() => self.p = Some(p),
            Code::S(s) if self.s.is_none() => self.s = Some(s),
            Code::T(t) if self.t.is_none() => self.t = Some(t),

            Code::F(f) if self.f.is_none() => self.f = Some(f),
            Code::I(i) if self.i.is_none() => self.i = Some(i),
            Code::J(j) if self.j.is_none() => self.j = Some(j),
            Code::K(k) if self.k.is_none() => self.k = Some(k),
            Code::Q(q) if self.q.is_none() => self.q = Some(q),
            Code::R(r) if self.r.is_none() => self.r = Some(r),
            Code::X(x) if self.x.is_none() => self.x = Some(x),
            Code::Y(y) if self.y.is_none() => self.y = Some(y),
            Code::Z(z) if self.z.is_none() => self.z = Some(z),

            Code::G(_) => panic!("G prefixed code was pushed to codes. Logic Error!"),
            Code::M(_) => panic!("M prefixed code was pushed to codes. Logic Error!"),

            _ => return Err(ParserError::DuplicatePrefix(code.prefix())),
        };

        Ok(())
    }

    /// Constructs a new [`PartialPoint`] by **consuming** `x`, `y`, & `z` field values from `self`.
    pub fn take_partial_point(&mut self) -> PartialPoint {
        PartialPoint::new(self.x.take(), self.y.take(), self.z.take())
    }

    /// Constructs a new [`PartialPoint`] by **consuming** `i`, `j`, & `k` field values from `self`.
    fn take_partial_point_ijk(&mut self) -> PartialPoint {
        PartialPoint::new(self.i.take(), self.j.take(), self.k.take())
    }

    /// Removes the `f` field from `self` and returns it.
    fn take_feed(&mut self) -> Option<Float> {
        self.f.take()
    }

    /// Tries to parse a [`Codes`] for circular interpolation codes,
    /// by removing any information regarding circular interpolation from it.
    ///
    /// Returns a tuple containing:
    /// - [`PartialPoint`] -- Destination coordinates.
    /// - [`CircleMethod`] -- Method to use for the circle.
    /// - [`Option<Float>`] -- Feedrate, if provided.
    pub fn take_circular(
        &mut self,
    ) -> Result<(PartialPoint, CircleMethod, Option<Float>), ParserError> {
        let pos = self.take_partial_point();
        let feed = self.take_feed();

        // both circle methods mean invalid input
        if self.r.is_some() && (self.i.is_some() || self.j.is_some() || self.k.is_some()) {
            return Err(ParserError::AmbiguousCircleMethod);
        }

        // branch based on if 'R' prefix exists or not
        let method = if let Some(r) = self.r.take() {
            CircleMethod::FixedRadius(r)
        } else {
            CircleMethod::RelativePoint(self.take_partial_point_ijk())
        };

        // destination coords are required for arcs.
        if pos.are_none() {
            return Err(ParserError::InvalidCircle(None));
        }

        match &method {
            // relative center must be on a single plane only, that is,
            // at most 2 axis can be specified, and at least one axis should be present
            CircleMethod::RelativePoint(rel_point) => {
                if rel_point.are_some() || rel_point.are_none() {
                    return Err(ParserError::InvalidCircle(Some(method)));
                }
            }
            // R must not be 0.
            CircleMethod::FixedRadius(rad) => {
                if rad.abs() < 1e-10 {
                    return Err(ParserError::InvalidCircle(Some(method)));
                }
            }
        }

        Ok((pos, method, feed))
    }
}

impl Iterator for Codes {
    type Item = Code;

    /// **Optionally** returns the next [`Code`].
    /// Returns [`None`] when the data has exhausted.
    ///
    /// This function will **never return** the [`Code::G`] or [`Code::M`] variants of [`Code`].
    fn next(&mut self) -> Option<Self::Item> {
        #![allow(clippy::redundant_closure)]
        if self.d.is_some() {
            self.d.take().map(|d| Code::D(d))
        } else if self.f.is_some() {
            self.f.take().map(|f| Code::F(f))
        } else if self.h.is_some() {
            self.h.take().map(|h| Code::H(h))
        } else if self.i.is_some() {
            self.i.take().map(|i| Code::I(i))
        } else if self.j.is_some() {
            self.j.take().map(|j| Code::J(j))
        } else if self.k.is_some() {
            self.k.take().map(|k| Code::K(k))
        } else if self.n.is_some() {
            self.n.take().map(|n| Code::N(n))
        } else if self.o.is_some() {
            self.o.take().map(|o| Code::O(o))
        } else if self.p.is_some() {
            self.p.take().map(|p| Code::P(p))
        } else if self.q.is_some() {
            self.q.take().map(|q| Code::Q(q))
        } else if self.r.is_some() {
            self.r.take().map(|r| Code::R(r))
        } else if self.s.is_some() {
            self.s.take().map(|s| Code::S(s))
        } else if self.t.is_some() {
            self.t.take().map(|t| Code::T(t))
        } else if self.x.is_some() {
            self.x.take().map(|x| Code::X(x))
        } else if self.y.is_some() {
            self.y.take().map(|y| Code::Y(y))
        } else if self.z.is_some() {
            self.z.take().map(|z| Code::Z(z))
        } else {
            None
        }
    }
}

/// Represents a *G-code*.
///
/// A G-code is used in toolpaths to move axes of a machine in a controlled way.
/// Each variant contains all the other variable values it needs to be valid.
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
    /// Provides the numeric value, suffix of a [`GCode`],
    /// by returning a primitive discriminant of the enumeration.
    ///
    /// The returned number would be the same one that was tokenized
    /// by the [`Lexer`] as the [`Suffix`].
    ///
    /// # SAFETY
    /// It is certain that [`GCode`] enum specifies a primitive representation,
    /// therefore the discriminant may be accessed via *unsafe pointer casting*.
    pub fn suffix(&self) -> Int {
        unsafe { *(self as *const Self as *const usize) }
    }

    /// Tries to construct a [`GCode`] by parsing a [`Code::G`].
    ///
    /// Accepts a [`Code::G`] variant of [`Code`],
    /// and a **mutable reference** to [`Codes`] that were found in the same [`Block`].
    ///
    /// The values used in parsing the `GCode` **will be removed** from `codes` as required.
    ///
    /// Returns a [`ParserError`] on failure.
    ///
    /// # Panics
    /// Panics if called with any variant of [`Code`] that is not [`Code::G`].
    fn parse(code: Code, codes: &mut Codes) -> Result<Self, ParserError> {
        if let Code::G(g) = code {
            let gcode = match g {
                // all fields may be none
                0 => Self::RapidMove(codes.take_partial_point()),

                1 => Self::FeedMove {
                    pos: codes.take_partial_point(),
                    feed: codes.take_feed(),
                },

                2 | 3 => {
                    let (pos, method, feed) = codes.take_circular()?;

                    if g == 2 {
                        Self::CWArcMove { pos, method, feed }
                    } else {
                        Self::CCWArcMove { pos, method, feed }
                    }
                }

                4 => {
                    // P can be used for milliseconds
                    if let Some(p) = codes.p.take() {
                        Self::Dwell((p as f64) / 1000.0)
                    }
                    // X can be used for seconds
                    else if let Some(x) = codes.x.take() {
                        Self::Dwell(x)
                    } else {
                        return Err(ParserError::MissingCodeForGCode(b'P'));
                    }
                }

                17 => Self::XYPlane,

                18 => Self::XZPlane,

                19 => Self::YZPlane,

                20 => Self::ImperialMode,

                21 => Self::MetricMode,

                40 => Self::CancelCutterComp,

                41 | 42 => {
                    if let Some(d) = codes.d.take() {
                        if g == 41 {
                            Self::LeftCutterComp(d)
                        } else {
                            Self::RightCutterComp(d)
                        }
                    } else {
                        return Err(ParserError::MissingCodeForGCode(b'D'));
                    }
                }

                43 | 44 => {
                    if let Some(h) = codes.h.take() {
                        if g == 43 {
                            Self::ToolLenCompAdd(h)
                        } else {
                            Self::ToolLenCompSubtract(h)
                        }
                    } else {
                        return Err(ParserError::MissingCodeForGCode(b'H'));
                    }
                }

                49 => Self::CancelLenComp,

                53 => {
                    let pos = codes.take_partial_point();

                    if pos.are_none() {
                        // need atleast one axis to move
                        return Err(ParserError::MissingCodeForGCode(b'X'));
                    } else {
                        Self::MachineCoord(pos)
                    }
                }

                54 => Self::WorkCoord,

                80 => Self::CancelCanned,

                90 => Self::AbsoluteMode,

                91 => Self::IncrementalMode,

                94 => Self::FeedMinute,

                95 => Self::FeedRev,

                98 => Self::InitialReturn,

                99 => Self::RetractReturn,

                _ => return Err(ParserError::InvalidGCode(g)),
            };

            Ok(gcode)
        } else {
            panic!("Non 'G' prefixed code was tried to be parsed as GCode. Logic Error!");
        }
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

impl Display for GCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "G{:0>2} - ", self.suffix())?;

        match self {
            Self::RapidMove(pos) => {
                if pos.are_none() {
                    write!(f, "Rapid Move")
                } else {
                    write!(f, "Rapid Move to: {pos}")
                }
            }

            Self::FeedMove { pos, feed } => match feed {
                Some(feed) => {
                    if pos.are_none() {
                        write!(f, "Feed Move, with feed: {feed}")
                    } else {
                        write!(f, "Feed Move, with feed: {feed}, to: {pos}")
                    }
                }
                None => {
                    if pos.are_none() {
                        write!(f, "Feed Move")
                    } else {
                        write!(f, "Feed Move to: {pos}")
                    }
                }
            },

            Self::CWArcMove { pos, method, feed } => match feed {
                // pos.are_none() will always be false for arc moves
                Some(feed) => {
                    write!(
                        f,
                        "Clockwise Move using:\n{method}, with feed: {feed}, to: {pos}"
                    )
                }
                None => {
                    write!(f, "Clockwise Move using:\n{method}, to: {pos}")
                }
            },

            Self::CCWArcMove { pos, method, feed } => match feed {
                // pos.are_none() will always be false for arc moves
                Some(feed) => {
                    write!(
                        f,
                        "Counter-Clockwise Move using:\n{method}, with feed: {feed}, to: {pos}"
                    )
                }
                None => {
                    write!(f, "Counter-Clockwise Move using:\n{method}, to: {pos}")
                }
            },

            Self::Dwell(p) => write!(f, "Dwell for {p} seconds"),

            Self::XYPlane => write!(f, "Select XY Plane"),

            Self::XZPlane => write!(f, "Select XZ Plane"),

            Self::YZPlane => write!(f, "Select YZ Plane"),

            Self::ImperialMode => write!(f, "Activate Imperial Mode"),

            Self::MetricMode => write!(f, "Activate Metric Mode"),

            Self::CancelCutterComp => write!(f, "Cancel Cutter Compensation"),

            Self::LeftCutterComp(d) => {
                write!(f, "Activate Left Cutter Compensation with D{d} offset")
            }

            Self::RightCutterComp(d) => {
                write!(f, "Activate Right Cutter Compensation with D{d} offset")
            }

            Self::ToolLenCompAdd(h) => write!(f, "Add Tool Length with H{h} offset"),

            Self::ToolLenCompSubtract(h) => write!(f, "Subtract Tool Length with H{h} offset"),

            Self::CancelLenComp => write!(f, "Cancel Tool Length Compensation"),

            Self::MachineCoord(pos) =>
            // machine pos will not be none for all coords
            {
                write!(f, "Machine Position Move to: {pos}")
            }

            Self::WorkCoord => write!(f, "Activate Work Coordinate offset"),

            Self::CancelCanned => write!(f, "Cancel Canned cycle"),

            Self::AbsoluteMode => write!(f, "Activate Absolute Positioning"),

            Self::IncrementalMode => write!(f, "Activate Incremental Positioning"),

            Self::FeedMinute => write!(f, "Activate Inverse Minute Feed mode"),

            Self::FeedRev => write!(f, "Activate Inverse Revolution Feed mode"),

            Self::InitialReturn => write!(f, "Activate Initial Level return in canned cycles"),

            Self::RetractReturn => write!(f, "Activate Retract Level return in canned cycles"),
        }
    }
}

/// Represents a collection of **unique** [`GCode`]s, belonging to unique [`Group`]s.
///
/// This type ensures that:
/// -- Each GCode is not present more than once.
/// -- Multiple GCodes from the same Group do not exist at once.
#[derive(Debug, Default)]
pub struct GCodes {
    codes: Vec<GCode>,
    /// Suffixes already present in the `codes` vector.
    suffixes: Vec<Int>,
    /// Groups already present in the `codes` vector.
    groups: Vec<Group>,
}

impl GCodes {
    /// Constructs a new [`GCodes`], ready to store unique [`GCode`]s with unique [`Group`]s.
    fn new() -> Self {
        Self::default()
    }

    /// Tries to add a new [`GCode`] to `self`.
    ///
    /// Returns [`ParserError`] when the the same suffix or same group is already present,
    /// indicating failure.
    fn push(&mut self, gcode: GCode) -> Result<(), ParserError> {
        let suffix = gcode.suffix();
        let group = gcode.group();

        if self.suffixes.contains(&suffix) {
            return Err(ParserError::DuplicateGCode(suffix));
        } else {
            self.suffixes.push(suffix);
        }

        if self.groups.contains(&group) {
            return Err(ParserError::DuplicateGCodeGroup(group));
        } else {
            self.groups.push(group);
        }

        self.codes.push(gcode);

        Ok(())
    }
}

impl Iterator for GCodes {
    type Item = GCode;

    /// **Optionally** returns the next [`GCode`].
    /// Returns [`None`] when the data has exhausted.
    fn next(&mut self) -> Option<Self::Item> {
        self.groups.pop();
        self.suffixes.pop();
        self.codes.pop()
    }
}

/// Represents a *M-code*.
///
/// A M-code is used to control machine specific features, mostly as an on-off switch.
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
    /// The returned number would be the same one that was tokeniezed
    /// by the [`Lexer`] as the [`Suffix`].
    ///
    /// # SAFETY
    /// It is certain that [`MCode`] enum specifies a primitive representation,
    /// therefore the discriminant may be accessed via *unsafe pointer casting*.
    pub fn suffix(&self) -> Int {
        unsafe { *(self as *const Self as *const usize) }
    }

    /// Tries to construct a [`MCode`] by parsing a [`Code::M`].
    ///
    /// Accepts a [`Code::M`] variant of [`Code`],
    /// and a **mutable reference** to [`Codes`] that were found in the same [`Block`].
    ///
    /// The values used in parsing the `MCode` **will be removed** from `codes` as required.
    ///
    /// Returns a [`ParserError`] on failure.
    ///
    /// # Panics
    /// Panics if called with any variant of [`Code`] that is not [`Code::M`].
    fn parse(code: Code, codes: &mut Codes) -> Result<Self, ParserError> {
        if let Code::M(m) = code {
            let mcode = match m {
                0 => Self::Stop,
                1 => Self::OptionalStop,
                3 => Self::SpindleFwd(codes.s.take()),
                4 => Self::SpindleRev(codes.s.take()),
                5 => Self::SpindleStop,
                6 => Self::ToolChange(codes.t.take()),
                8 => Self::CoolantOn,
                9 => Self::CoolantOff,
                30 => Self::End,

                _ => return Err(ParserError::InvalidMCode(m)),
            };

            Ok(mcode)
        } else {
            panic!("Non 'M' prefixed code was tried to be parsed as MCode. Logic Error!");
        }
    }
}

impl Display for MCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "M{:0>2} - ", self.suffix())?;

        match self {
            Self::Stop => write!(f, "Program Stop"),
            Self::OptionalStop => write!(f, "Optional Stop"),
            Self::SpindleFwd(s) => {
                if let Some(s) = s {
                    write!(f, "Clockwise Spindle On, at {s} RPMs")
                } else {
                    write!(f, "Clockwise Spindle On")
                }
            }
            Self::SpindleRev(s) => {
                if let Some(s) = s {
                    write!(f, "Counter-Clockwise Spindle On, at {s} RPMs")
                } else {
                    write!(f, "Counter-Clockwise Spindle On")
                }
            }
            Self::SpindleStop => write!(f, "Spindle Off"),
            Self::ToolChange(t) => {
                if let Some(t) = t {
                    write!(f, "Tool Change to tool number: {t}")
                } else {
                    write!(f, "Tool Change to any preloaded tool")
                }
            }
            Self::CoolantOn => write!(f, "Coolant On"),
            Self::CoolantOff => write!(f, "Coolant Off"),
            Self::End => write!(f, "Program End"),
        }
    }
}

/// Represents a **parsed** [`Block`].
#[derive(Debug)]
pub struct CodeBlock {
    /// Sequence of unique GCodes, with unique Groups.
    gcodes: GCodes,
    /// At most one parsed MCode per block.
    mcode: Option<MCode>,
    /// Collection of unique Codes, suffixed by the appropriate type.
    codes: Codes,
}

impl CodeBlock {
    /// Tries to construct a [`CodeBlock`] by parsing a [`Block`].
    ///
    /// Returns a [`ParserError`] on failure.
    fn parse(block: Block) -> Result<Self, ParserError> {
        let mut gcodes = GCodes::new();
        let mut mcode = None;
        let mut codes = Codes::new();

        // parse and store only non G and non M tokens
        for token in block
            .clone()
            .filter(|token| token.prefix != b'G' && token.prefix != b'M')
        {
            let code = Code::parse(&token)?;
            codes.push(code)?;
        }

        // parse any mcode & gcode(s)
        for token in block.filter(|token| token.prefix == b'G' || token.prefix == b'M') {
            let code = Code::parse(&token)?;

            if token.prefix == b'M' {
                if mcode.is_some() {
                    return Err(ParserError::DuplicatePrefix(b'M'));
                }
                mcode = Some(MCode::parse(code, &mut codes)?);
            } else {
                gcodes.push(GCode::parse(code, &mut codes)?)?;
            }
        }

        Ok(Self {
            gcodes,
            mcode,
            codes,
        })
    }

    /// Returns a **mutable reference** to parsed [`GCodes`].
    pub fn gcodes(&mut self) -> &mut GCodes {
        &mut self.gcodes
    }

    /// Returns a `Optional` parsed [`MCode`].
    pub fn mcode(&mut self) -> Option<MCode> {
        self.mcode.take()
    }

    /// Returns a **mutable reference** to parsed [`Codes`].
    pub fn codes(&mut self) -> &mut Codes {
        &mut self.codes
    }
}

/// Represents the whole G-Code as **parsed** [`CodeBlock`]s,
/// generated by parsing each [`Block`].
///
/// Each [`CodeBlock`] is parsed **lazily** on request from the [`Lexer`].
#[derive(Debug)]
pub struct Parser(Lexer);

impl Parser {
    /// Constructs [`Parser`] from a [`Lexer`].
    ///
    /// This function **does not parse** any [`Block`]s.
    /// Parsing is done on demand with a call to [`Parser::next`].
    pub fn new(lexer: Lexer) -> Self {
        Self(lexer)
    }

    /// Reloads the [`Parser`] to start from beginning of the [`Lexer`].
    pub fn reload(&mut self) {
        self.0.reload();
    }

    /// **Optinally** returns the next [`Line`](crate::source::Line) as a string slice from the [`Source`](crate::source::Source).
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.0.get_line(index)
    }
}

impl Iterator for Parser {
    type Item = Result<CodeBlock, ParserError>;

    /// **Optionally** parses the next [`Block`] from the stored [`Lexer`].
    /// Returns [`Some`] variant with:
    /// - [`CodeBlock`] if the parsing was successful.
    /// - [`ParserError`] on parsing failure.
    ///
    /// Returns [`None`] when no more [`Block`]s are available from the [`Lexer`].
    fn next(&mut self) -> Option<Self::Item> {
        let res = match self.0.next()? {
            Ok(block) => CodeBlock::parse(block),
            Err(e) => Err(ParserError::from(e)),
        };

        Some(res)
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
    /// The code block contains codes for both variants of circle methods.
    AmbiguousCircleMethod,
    /// Conditions for a particular circle method were not met, or the end coords are missing.
    InvalidCircle(Option<CircleMethod>),
    /// Prefix and suffix make an invalid M-code.
    InvalidMCode(Int),
    /// Missing token required for a MCode variant.
    MissingCodeForMCode(Prefix),
    /// Prefix was found after parsing G & M Codes, but cannot be parsed on its own.
    UnexpectedPrefix(Prefix),
    Lexer(LexerError),
}

impl From<LexerError> for ParserError {
    fn from(e: LexerError) -> Self {
        Self::Lexer(e)
    }
}

impl Describe for ParserError {
    fn describe(&self) -> Description {
        let (title, desc) = match self {
            Self::WrongSuffixType(prefix) => (
                "Wrong Suffix Type Detected",
                format!(
                    "The following prefix has a wrong suffix type: '{}'.",
                    *prefix as char
                ),
            ),

            Self::UnknownPrefix(prefix) => (
                "Unsupported Prefix Detected",
                format!(
                    "The following prefix is not supported: '{}'.",
                    *prefix as char
                ),
            ),

            Self::DuplicateGCode(suffix) => (
                "Duplicate G-Code Detected",
                format!("The following code was repeated: 'G{suffix}'."),
            ),

            Self::InvalidGCode(suffix) => (
                "Invalid G-Code Detected",
                format!("The following G-Code is not supported: 'G{suffix}'."),
            ),

            Self::DuplicateGCodeGroup(group) => (
                "Duplicate G-Code Group Detected",
                format!(
                    "The following group contains more than one G-Codes that belong to it: '{group}'."
                ),
            ),

            Self::DuplicatePrefix(prefix) => (
                "Duplicate Prefix Detected",
                format!(
                    "The following prefix code appears more than once: '{}'.",
                    *prefix as char
                ),
            ),

            Self::InvalidParamForGCode(suffix) => (
                "Invalid Parameter Detected",
                format!("The following G-Code requirements were not met: 'G{suffix}'."),
            ),

            Self::MissingCodeForGCode(prefix) => (
                "Required Code not found for G-Code",
                format!(
                    "The following prefix code was not found: '{}'.",
                    *prefix as char
                ),
            ),

            Self::AmbiguousCircleMethod => (                "Ambiguous Circle Method Detected", "The code block contains codes from each of the two circular methods, which is invalid.".to_string()
            ),

            Self::InvalidCircle(opt) => ("Invalid Circle Detected", match opt {
                Some(method) => match method {
                    CircleMethod::RelativePoint(_) => "The relative center of the requested arc must lie on one single plane.".to_string(),
                    CircleMethod::FixedRadius(_) => "The radius of the requested arc is detected to be zero, which is invalid.".to_string(),
                },
                None => "No end coordinates were detected for the requested arc.".to_string(),
            }),

            Self::InvalidMCode(suffix) => (                "Invalid M-Code Detected", format!("The following M-Code is not supported by this parser: 'M{suffix}'.")),

            Self::MissingCodeForMCode(prefix) => (                "Required Code not found for M-Code", format!("The following prefix code was not found: '{}'.",
                *prefix as char)
            ),

            Self::UnexpectedPrefix(prefix) => (                "Unexpected Prefix Detected", format!("The following prefix was not consumed by the parser, but cannot be parsed on its own: '{}'.",
                *prefix as char)
            ),

            Self::Lexer(e) => return e.describe(),
        };

        Description::new(title, desc)
    }
}

impl Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::WrongSuffixType(prefix) => write!(
                f,
                "Wrong Suffix Type Detected:{RESET}\n\t\tThe following prefix has a wrong suffix type: '{RED}{}{RESET}'.",
                *prefix as char
            ),

            Self::UnknownPrefix(prefix) => write!(
                f,
                "Unsupported Prefix Detected:{RESET}\n\t\tThe following prefix is not supported: '{RED}{}{RESET}'.",
                *prefix as char
            ),

            Self::DuplicateGCode(suffix) => write!(
                f,
                "Duplicate G-Code Detected:{RESET}\n\t\tThe following code was repeated: '{RED}G{suffix}{RESET}'."
            ),

            Self::InvalidGCode(suffix) => write!(
                f,
                "Invalid G-Code Detected:{RESET}\n\t\tThe following G-Code is not supported: '{RED}G{suffix}{RESET}'."
            ),

            Self::DuplicateGCodeGroup(group) => write!(
                f,
                "Duplicate G-Code Group Detected:{RESET}\n\t\tThe following group contains more than one G-Codes that belong to it: '{RED}{group}{RESET}'."
            ),

            Self::DuplicatePrefix(prefix) => write!(
                f,
                "Duplicate Prefix Detected:{RESET}\n\t\tThe following prefix code appears more than once: '{RED}{}{RESET}'.",
                *prefix as char
            ),

            Self::InvalidParamForGCode(suffix) => write!(
                f,
                "Invalid Parameter Detected:{RESET}\n\t\tThe following G-Code requirements were not met: '{RED}G{suffix}{RESET}'."
            ),

            Self::MissingCodeForGCode(prefix) => write!(
                f,
                "Required Code not found for G-Code:{RESET}\n\t\tThe following prefix code was not found: '{RED}{}{RESET}'.",
                *prefix as char
            ),

            Self::AmbiguousCircleMethod => write!(
                f,
                "Ambiguous Circle Method Detected:{RESET}\n\t\tThe code block contains codes from each of the two circular methods, which is invalid."
            ),

            Self::InvalidCircle(opt) => match opt {
                Some(method) => match method {
                    CircleMethod::RelativePoint(_) => write!(
                        f,
                        "Invalid Circle Detected:{RESET}\n\t\tThe relative center of the requested arc must lie on one single plane."
                    ),
                    CircleMethod::FixedRadius(_) => write!(
                        f,
                        "Invalid Circle Detected:{RESET}\n\t\tThe radius of the requested arc is detected to be zero, which is invalid."
                    ),
                },
                None => write!(
                    f,
                    "Invalid Circle Detected:{RESET}\n\t\tNo end coordinates were detected for the requested arc."
                ),
            },

            Self::InvalidMCode(suffix) => write!(
                f,
                "Invalid M-Code Detected:{RESET}\n\t\tThe following M-Code is not supported by this parser: '{RED}M{suffix}{RESET}'."
            ),

            Self::MissingCodeForMCode(prefix) => write!(
                f,
                "Required Code not found for M-Code:{RESET}\n\t\tThe following prefix code was not found: '{RED}{}{RESET}'.",
                *prefix as char
            ),

            Self::UnexpectedPrefix(prefix) => write!(
                f,
                "Unexpected Prefix Detected:{RESET}\n\t\tThe following prefix was not consumed by the parser, but cannot be parsed on its own: '{RED}{}{RESET}'.",
                *prefix as char
            ),

            Self::Lexer(e) => write!(f, "{e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::source::Source;

    use super::*;

    // helper for tests
    // returns a parsed vector of gcodes
    fn tokenize_parse(tokens: &str) -> Result<Vec<GCode>, ParserError> {
        let mut parser = Parser::new(Lexer::new(Source::from_string(tokens)));
        parser.next().unwrap().map(|block| block.gcodes.collect())
    }

    // helper for tests
    // returns a parsed mcode
    fn tokenize_parse_m(tokens: &str) -> Result<MCode, ParserError> {
        let mut parser = Parser::new(Lexer::new(Source::from_string(tokens)));
        parser.next().unwrap().map(|block| block.mcode.unwrap())
    }

    #[test]
    // Test to get the suffix of a code by accessing its discriminant.
    fn get_code_suffix() {
        assert_eq!(GCode::RapidMove(PartialPoint(None, None, None)).suffix(), 0);

        assert_eq!(MCode::Stop.suffix(), 0);
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
    fn parse_rapid_move() {
        assert_eq!(
            tokenize_parse("G0 X0. Y0.").unwrap(),
            vec![GCode::RapidMove(PartialPoint(Some(0.0), Some(0.0), None))]
        );
    }

    #[test]
    fn parse_feed_move() {
        assert_eq!(
            tokenize_parse("G1 X0. Y0. F20.").unwrap(),
            vec![GCode::FeedMove {
                pos: PartialPoint(Some(0.0), Some(0.0), None),
                feed: Some(20.0)
            }]
        );
    }

    #[test]
    fn parse_cw_arc() {
        assert_eq!(
            tokenize_parse("G2 X0. I1. J2. F20.").unwrap(),
            vec![GCode::CWArcMove {
                pos: PartialPoint(Some(0.0), None, None),
                method: CircleMethod::RelativePoint(PartialPoint(Some(1.0), Some(2.0), None)),
                feed: Some(20.0)
            }]
        );

        assert_eq!(
            tokenize_parse("G2 Y0. R20. F20.").unwrap(),
            vec![GCode::CWArcMove {
                pos: PartialPoint(None, Some(0.0), None),
                method: CircleMethod::FixedRadius(20.0),
                feed: Some(20.0)
            }]
        );
    }

    #[test]
    fn parse_ccw_arc() {
        assert_eq!(
            tokenize_parse("G3 X0. I1. J2. F20.").unwrap(),
            vec![GCode::CCWArcMove {
                pos: PartialPoint(Some(0.0), None, None),
                method: CircleMethod::RelativePoint(PartialPoint(Some(1.0), Some(2.0), None)),
                feed: Some(20.0)
            }]
        );

        assert_eq!(
            tokenize_parse("G3 Y0. R20. F20.").unwrap(),
            vec![GCode::CCWArcMove {
                pos: PartialPoint(None, Some(0.0), None),
                method: CircleMethod::FixedRadius(20.0),
                feed: Some(20.0)
            }]
        );
    }

    #[test]
    fn parse_dwell() {
        assert_eq!(tokenize_parse("G4 X10.").unwrap(), vec![GCode::Dwell(10.0)]);

        assert_eq!(tokenize_parse("G4 P1000").unwrap(), vec![GCode::Dwell(1.0)]);

        assert_eq!(
            tokenize_parse("G4").unwrap_err(),
            ParserError::MissingCodeForGCode(b'P')
        );
    }

    #[test]
    fn parse_planes() {
        assert_eq!(tokenize_parse("G17").unwrap(), vec![GCode::XYPlane]);

        assert_eq!(tokenize_parse("G18").unwrap(), vec![GCode::XZPlane]);

        assert_eq!(tokenize_parse("G19").unwrap(), vec![GCode::YZPlane]);
    }

    #[test]
    fn parse_unit_modes() {
        assert_eq!(tokenize_parse("G20").unwrap(), vec![GCode::ImperialMode]);

        assert_eq!(tokenize_parse("G21").unwrap(), vec![GCode::MetricMode]);
    }

    #[test]
    fn parse_cutter_comp() {
        assert_eq!(
            tokenize_parse("G40").unwrap(),
            vec![GCode::CancelCutterComp]
        );

        assert_eq!(
            tokenize_parse("G41 D1").unwrap(),
            vec![GCode::LeftCutterComp(1)]
        );

        assert_eq!(
            tokenize_parse("G42 D1").unwrap(),
            vec![GCode::RightCutterComp(1)]
        );
    }

    #[test]
    fn parse_len_comp() {
        assert_eq!(
            tokenize_parse("G43 H1").unwrap(),
            vec![GCode::ToolLenCompAdd(1)]
        );

        assert_eq!(
            tokenize_parse("G44 H1").unwrap(),
            vec![GCode::ToolLenCompSubtract(1)]
        );

        assert_eq!(tokenize_parse("G49").unwrap(), vec![GCode::CancelLenComp]);
    }

    #[test]
    fn parse_machine_coord() {
        assert_eq!(
            tokenize_parse("G53").unwrap_err(),
            ParserError::MissingCodeForGCode(b'X')
        );

        assert_eq!(
            tokenize_parse("G53 X0. Z0.").unwrap(),
            vec![GCode::MachineCoord(PartialPoint(
                Some(0.0),
                None,
                Some(0.0)
            ))]
        );
    }

    #[test]
    fn parse_workpiece_coord() {
        assert_eq!(tokenize_parse("G54").unwrap(), vec![GCode::WorkCoord]);
    }

    #[test]
    fn parse_canned_cycles() {
        assert_eq!(tokenize_parse("G80").unwrap(), vec![GCode::CancelCanned]);
    }

    #[test]
    fn parse_positioning_modes() {
        assert_eq!(tokenize_parse("G90").unwrap(), vec![GCode::AbsoluteMode]);

        assert_eq!(tokenize_parse("G91").unwrap(), vec![GCode::IncrementalMode]);
    }

    #[test]
    fn parse_feed_modes() {
        assert_eq!(tokenize_parse("G94").unwrap(), vec![GCode::FeedMinute]);

        assert_eq!(tokenize_parse("G95").unwrap(), vec![GCode::FeedRev]);
    }

    #[test]
    fn parse_return_canned() {
        assert_eq!(tokenize_parse("G98").unwrap(), vec![GCode::InitialReturn]);

        assert_eq!(tokenize_parse("G99").unwrap(), vec![GCode::RetractReturn]);
    }

    #[test]
    fn parse_stop() {
        assert_eq!(tokenize_parse_m("M00").unwrap(), MCode::Stop);
    }

    #[test]
    fn parse_optional_stop() {
        assert_eq!(tokenize_parse_m("M01").unwrap(), MCode::OptionalStop);
    }

    #[test]
    fn parse_spindle() {
        assert_eq!(
            tokenize_parse_m("M03 S1000").unwrap(),
            MCode::SpindleFwd(Some(1000))
        );
        assert_eq!(tokenize_parse_m("M03").unwrap(), MCode::SpindleFwd(None));
        assert_eq!(
            tokenize_parse_m("M04 S1000").unwrap(),
            MCode::SpindleRev(Some(1000))
        );
        assert_eq!(tokenize_parse_m("M04").unwrap(), MCode::SpindleRev(None));
        assert_eq!(tokenize_parse_m("M05").unwrap(), MCode::SpindleStop);
    }

    #[test]
    fn parse_tool_change() {
        assert_eq!(
            tokenize_parse_m("M06 T1").unwrap(),
            MCode::ToolChange(Some(1))
        );
        assert_eq!(tokenize_parse_m("M06").unwrap(), MCode::ToolChange(None));
    }

    #[test]
    fn parse_coolant() {
        assert_eq!(tokenize_parse_m("M08").unwrap(), MCode::CoolantOn);
        assert_eq!(tokenize_parse_m("M09").unwrap(), MCode::CoolantOff);
    }

    #[test]
    fn parse_program_end() {
        assert_eq!(tokenize_parse_m("M30").unwrap(), MCode::End);
    }

    #[test]
    /// Test all setters for [`Point`]
    fn point_set() {
        let mut p = Point::new(0.0, 0.0, 0.0);

        p.set(1.0, 2.0, 3.0);
        assert_eq!(p.get(), (1.0, 2.0, 3.0));

        p.set_optional(Some(-1.0), None, Some(-3.0));
        assert_eq!(p.get(), (-1.0, 2.0, -3.0));
    }

    #[test]
    fn point_negative() {
        assert!(Point::new(-10.0, 0.0, 20.0).any_negative());
    }

    #[test]
    fn point_comparisons() {
        let p = Point::new(100.0, 200.0, 300.0);
        let mid = Point::new(200.0, 200.0, 200.0);

        // atleast one field is over and under `mid`
        assert!(p.over_abs(&mid));
        assert!(p.under_abs(&mid));
    }
}
