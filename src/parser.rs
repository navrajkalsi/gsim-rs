//! # Parser
//!
//! Consumes a [`Lexer`], converting a sequence of G-code [`Block`]s (represented as [`Lexer`]),
//! to a sequence of [`CodeBlock`]s (represented as [`Parser`].
//!
//! This parser is **stateless** and does not deal with any state logic across blocks.
//!
//! This module makes the following translations to Lexer structures:
//! - [`Token`] -> [`Code`]
//! - [`Block`] -> [`CodeBlock`]
//! - [`Lexer`] -> [`Parser`]
//!
//! ## Reference
//! [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

use crate::{
    FLOAT_VARIANCE,
    config::Point,
    lexer::{Block, *},
};
use std::{
    cmp::PartialEq,
    fmt::{Debug, Display},
    ops::{Add, Sub},
};

/// Possible planes for a 3-axis machine.
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub enum Plane {
    #[default]
    XY,
    XZ,
    YZ,
}

impl Point {
    /// Constructor for a [`Point`] from X,Y, and Z axis values.
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Treats all the axes values in **metric** system, and converts them to **imperial** system.
    pub fn to_imperial(&mut self) {
        self.x /= 25.4;
        self.y /= 25.4;
        self.z /= 25.4;
    }

    /// Treats all the axes values in **imperial** system, and converts them to **metric** system.
    pub fn to_metric(&mut self) {
        self.x *= 25.4;
        self.y *= 25.4;
        self.z *= 25.4;
    }

    /// Calculates distance between `self` and another [`Point`] on a certain plane.
    pub fn dist(&self, other: &Self, plane: Plane) -> f32 {
        match plane {
            Plane::XY => ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt(),
            Plane::XZ => ((self.x - other.x).powi(2) + (self.z - other.z).powi(2)).sqrt(),
            Plane::YZ => ((self.y - other.y).powi(2) + (self.z - other.z).powi(2)).sqrt(),
        }
    }

    /// Multiplies the provided `factor` to each axis value and returns a new [`Point`] with these
    /// new values.
    pub fn mul_float(&self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }

    /// Divides each axis value with the provided `divisor` and returns a new [`Point`] with these
    /// new values.
    pub fn div_float(&self, divisor: f32) -> Self {
        Self::new(self.x / divisor, self.y / divisor, self.z / divisor)
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Add for Point {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

/// Same as [`Point`] but the fields are [`Option`]al.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartialPoint {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
}

impl PartialPoint {
    /// Constructs a [`PartialPoint`] using [`Option<f32>`] for each axis.
    pub fn new(x: Option<f32>, y: Option<f32>, z: Option<f32>) -> Self {
        PartialPoint { x, y, z }
    }

    /// Check if all the axis are `None` variants.
    pub fn are_none(&self) -> bool {
        self.x.is_none() && self.y.is_none() && self.z.is_none()
    }

    /// Check if all the axis are `Some` variants.
    pub fn are_some(&self) -> bool {
        self.x.is_some() && self.y.is_some() && self.z.is_some()
    }

    /// Treats all the axes values in **metric** system, and converts them to **imperial** system.
    pub fn to_imperial(&mut self) {
        self.x = self.x.map(|x| x / 25.4);
        self.y = self.y.map(|y| y / 25.4);
        self.z = self.z.map(|z| z / 25.4);
    }

    /// Treats all the axes values in **imperial** system, and converts them to **metric** system.
    pub fn to_metric(&mut self) {
        self.x = self.x.map(|x| x * 25.4);
        self.y = self.y.map(|y| y * 25.4);
        self.z = self.z.map(|z| z * 25.4);
    }
}

impl Display for PartialPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut axes = vec![];

        if let Some(x) = self.x {
            axes.push(format!("X: {x}"));
        }

        if let Some(y) = self.y {
            axes.push(format!("Y: {y}"));
        }

        if let Some(z) = self.z {
            axes.push(format!("Z: {z}"));
        }

        if axes.is_empty() {
            return Ok(());
        }

        write!(f, "({})", axes.join(", "))
    }
}

/// Circular Interpolation helper.
///
/// Ensures that both relative point and radius do not appear in the same block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K** from current position.
    RelativePoint(PartialPoint),
    /// Explicit radius specified with **R**.
    FixedRadius(f32),
}

impl Display for CircleMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RelativePoint(pos) => write!(f, "Relative Center at: {pos}"),
            Self::FixedRadius(r) => write!(f, "Radius: {r}"),
        }
    }
}

/// Tries to retrieve an `u32` from a [`Suffix`].
///
/// Returns [`ParserError::WrongSuffixType`] if the type is not [`Suffix::Int`].
fn try_int(token: &Token) -> Result<u32, ParserError> {
    token
        .suffix
        .int()
        .ok_or(ParserError::WrongSuffixType(token.prefix))
}

/// Tries to retrieve a `f32` from a [`Suffix`].
///
/// Returns [`ParserError::WrongSuffixType`] if the type is not [`Suffix::Float`].
fn try_float(token: &Token) -> Result<f32, ParserError> {
    token
        .suffix
        .float()
        .ok_or(ParserError::WrongSuffixType(token.prefix))
}

/// Represents a parsed & validated [`Token`].
///
/// This type ensures that each prefix is valid and is grouped with a valid [`Suffix`] type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Code {
    D(u32),
    G(u32),
    H(u32),
    M(u32),
    N(u32),
    O(u32),
    P(u32),
    S(u32),
    T(u32),

    F(f32),
    I(f32),
    J(f32),
    K(f32),
    Q(f32),
    R(f32),
    X(f32),
    Y(f32),
    Z(f32),
}

impl Code {
    /// Tries to construct a [`Code`] from a [`Token`].
    ///
    /// Returns a [`Code`] if the [`Token::prefix`] is valid
    /// and the [`Token::suffix`] type is valid for the said prefix.
    ///
    /// # Errors
    /// Returns [`ParserError::UnknownPrefix`]
    /// if a code with unknown prefix is found.
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

    /// Returns the **ASCII** prefix of `self` as `u8`.
    pub fn prefix(&self) -> u8 {
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

impl Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Code::N(n) => write!(f, "Line number: {n}"),
            Code::O(o) => write!(f, "Program number: {o}"),
            Code::S(s) => write!(f, "Set new spindle speed: {s}"),
            Code::T(t) => write!(f, "Preload tool number: {t}"),
            Code::F(feed) => write!(f, "Set new feedrate: {feed}"),
            _ => write!(f, ""),
        }
    }
}

/// Represents a collection of **unique** [`Code`]s **without 'G' or 'M'** prefixes.
///
/// This type ensures that each prefix is only present once in a [`CodeBlock`].
#[derive(Debug, Default)]
pub struct Codes {
    d: Option<u32>,
    f: Option<f32>,
    h: Option<u32>,
    i: Option<f32>,
    j: Option<f32>,
    k: Option<f32>,
    n: Option<u32>,
    o: Option<u32>,
    p: Option<u32>,
    q: Option<f32>,
    r: Option<f32>,
    s: Option<u32>,
    t: Option<u32>,
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
}

impl Codes {
    /// Constructs a new [`Codes`], ready to store non 'G' or non 'M' prefixed, unique codes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tries to add a new [`Code`] to `self`. The code **must not** be [`Code::G`] or [`Code::M`] variant.
    ///
    /// # Errors
    /// Returns [`ParserError::DuplicatePrefix`]
    /// if a code with same prefix is already present.
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

            Code::G(_) => unreachable!("G prefixed code was pushed to codes. Logic Error!"),
            Code::M(_) => unreachable!("M prefixed code was pushed to codes. Logic Error!"),

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
    fn take_feed(&mut self) -> Option<f32> {
        self.f.take()
    }

    /// Tries to parse a [`Codes`] for circular interpolation codes,
    /// by removing any information regarding circular interpolation from it.
    ///
    /// Returns a tuple containing:
    /// - [`PartialPoint`] -- Destination coordinates.
    /// - [`CircleMethod`] -- Method to use for the circle.
    /// - [`Option<f32>`] -- Feedrate, if provided.
    ///
    /// # Errors
    /// Returns [`ParserError::AmbiguousCircleMethod`] or [`ParserError::InvalidCircle`] on
    /// failure.
    pub fn take_circular(
        &mut self,
    ) -> Result<(PartialPoint, CircleMethod, Option<f32>), ParserError> {
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
                if rad.abs() < FLOAT_VARIANCE {
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
    /// This function will **never** return the [`Code::G`] or [`Code::M`] variants of [`Code`].
    fn next(&mut self) -> Option<Self::Item> {
        if self.d.is_some() {
            self.d.take().map(Code::D)
        } else if self.f.is_some() {
            self.f.take().map(Code::F)
        } else if self.h.is_some() {
            self.h.take().map(Code::H)
        } else if self.i.is_some() {
            self.i.take().map(Code::I)
        } else if self.j.is_some() {
            self.j.take().map(Code::J)
        } else if self.k.is_some() {
            self.k.take().map(Code::K)
        } else if self.n.is_some() {
            self.n.take().map(Code::N)
        } else if self.o.is_some() {
            self.o.take().map(Code::O)
        } else if self.p.is_some() {
            self.p.take().map(Code::P)
        } else if self.q.is_some() {
            self.q.take().map(Code::Q)
        } else if self.r.is_some() {
            self.r.take().map(Code::R)
        } else if self.s.is_some() {
            self.s.take().map(Code::S)
        } else if self.t.is_some() {
            self.t.take().map(Code::T)
        } else if self.x.is_some() {
            self.x.take().map(Code::X)
        } else if self.y.is_some() {
            self.y.take().map(Code::Y)
        } else if self.z.is_some() {
            self.z.take().map(Code::Z)
        } else {
            None
        }
    }
}

/// Represents a *G-code*.
///
/// A G-code is used in toolpaths to move axes of a machine in a controlled way.
/// Each variant contains all the other variable values it needs to be valid.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(usize)]
pub enum GCode {
    /// G00
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(PartialPoint) = 0,

    /// G01
    /// Linear Interpolate to new coordinates using provided feed rate.
    FeedMove {
        pos: PartialPoint,
        feed: Option<f32>,
    } = 1,

    /// G02
    /// Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CWArcMove {
        pos: PartialPoint,
        method: CircleMethod,
        feed: Option<f32>,
    } = 2,

    /// G03
    /// Counter-Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CCWArcMove {
        pos: PartialPoint,
        method: CircleMethod,
        feed: Option<f32>,
    } = 3,

    /// G04
    /// Dwell, for seconds, blocking further code execution.
    Dwell(f32) = 4,

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
    LeftCutterComp(u32) = 41,

    /// G42
    /// 2D right cutter compensation.
    RightCutterComp(u32) = 42,

    /// G43
    /// Tool length compensation by addition.
    ToolLenCompAdd(u32) = 43,

    /// G44
    /// Tool length compensation by subtraction.
    ToolLenCompSubtract(u32) = 44,

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
    pub fn suffix(&self) -> u32 {
        unsafe { *(self as *const Self as *const u32) }
    }

    /// Tries to construct a [`GCode`] by parsing a [`Code::G`].
    ///
    /// Accepts a [`Code::G`] variant of [`Code`],
    /// and a **mutable reference** to [`Codes`] that were found in the same [`Block`].
    ///
    /// The values used in parsing the [`GCode`] **will be removed** from `codes` as required.
    ///
    /// # Errors
    /// Returns a [`ParserError::InvalidParamForGCode`] or [`ParserError::InvalidGCode`] on failure.
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
                        Self::Dwell((p as f32) / 1000.0)
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
            unreachable!("Non 'G' prefixed code was tried to be parsed as GCode. Logic Error!");
        }
    }

    /// Returns what *group* a [`GCode`] belongs to.
    ///
    /// G-codes can be modal and are divided into *groups*.
    ///
    /// At any given time **only one G-code** from each group can be supplied and be activated.
    /// A line/block of code with more than one G-codes of the same group is **invalid**.
    ///
    /// ## Reference
    /// [Haas](https://www.haascnc.com/service/service-content/guide-procedures/what-are-g-codes.html#gsc.tab=0)
    pub fn group(&self) -> u8 {
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
        let mut textual = Vec::with_capacity(4);
        textual.push(format!("G{:0>2} - ", self.suffix()));

        match self {
            Self::RapidMove(pos) => {
                textual.push("Rapid Move".into());
                if !pos.are_none() {
                    textual.push(format!(" to: {pos}"));
                }
            }

            Self::FeedMove { pos, feed } => {
                textual.push("Feed Move".into());

                if !pos.are_none() {
                    textual.push(format!(" to: {pos}"));
                }

                if let Some(feed) = feed {
                    textual.push(format!(" with feed: {feed}"));
                }
            }

            Self::CWArcMove { pos, method, feed } => {
                // pos.are_none() will always be false for arc moves
                textual.push(format!("Clockwise Move with {method} to: {pos}"));

                if let Some(feed) = feed {
                    textual.push(format!(" with feed: {feed}"));
                }
            }

            Self::CCWArcMove { pos, method, feed } => {
                // pos.are_none() will always be false for arc moves
                textual.push(format!("Counter-Clockwise Move with {method} to: {pos}"));

                if let Some(feed) = feed {
                    textual.push(format!(" with feed: {feed}"));
                }
            }

            Self::Dwell(p) => textual.push(format!("Dwell for {p} seconds")),

            Self::XYPlane => textual.push("Select XY Plane".into()),

            Self::XZPlane => textual.push("Select XZ Plane".into()),

            Self::YZPlane => textual.push("Select YZ Plane".into()),

            Self::ImperialMode => textual.push("Activate Imperial Mode".into()),

            Self::MetricMode => textual.push("Activate Metric Mode".into()),

            Self::CancelCutterComp => textual.push("Cancel Cutter Compensation".into()),

            Self::LeftCutterComp(d) => textual.push(format!(
                "Activate Left Cutter Compensation with D{d} offset"
            )),

            Self::RightCutterComp(d) => textual.push(format!(
                "Activate Right Cutter Compensation with D{d} offset"
            )),

            Self::ToolLenCompAdd(h) => textual.push(format!("Add Tool Length with H{h} offset")),

            Self::ToolLenCompSubtract(h) => {
                textual.push(format!("Subtract Tool Length with H{h} offset"))
            }

            Self::CancelLenComp => textual.push("Cancel Tool Length Compensation".into()),

            Self::MachineCoord(pos) => textual.push(format!("Machine Position Move to: {pos}")), // machine pos will not be none for all coords

            Self::WorkCoord => textual.push("Activate Work Coordinate offset".into()),

            Self::CancelCanned => textual.push("Cancel Canned cycle".into()),

            Self::AbsoluteMode => textual.push("Activate Absolute Positioning".into()),

            Self::IncrementalMode => textual.push("Activate Incremental Positioning".into()),

            Self::FeedMinute => textual.push("Activate Inverse Minute Feed mode".into()),

            Self::FeedRev => textual.push("Activate Inverse Revolution Feed mode".into()),

            Self::InitialReturn => {
                textual.push("Activate Initial Level return in canned cycles".into())
            }

            Self::RetractReturn => {
                textual.push("Activate Retract Level return in canned cycles".into())
            }
        };

        write!(f, "{}", textual.join(""))
    }
}

/// Represents a collection of **unique** [`GCode`]s, belonging to unique groups.
///
/// This type ensures that:
/// -- Each GCode is not present more than once.
/// -- Multiple GCodes from the same Group do not exist in a single block.
#[derive(Debug, Default)]
pub struct GCodes {
    codes: Vec<GCode>,
    /// Suffixes already present in the `codes` vector.
    suffixes: Vec<u32>,
    /// Groups already present in the `codes` vector.
    groups: Vec<u8>,
}

impl GCodes {
    /// Constructs a new [`GCodes`], ready to store unique [`GCode`]s with unique groups.
    fn new() -> Self {
        Self::default()
    }

    /// Tries to add a new [`GCode`] to `self`.
    ///
    /// # Errors
    /// Returns [`ParserError::DuplicateGCode`] or [`ParserError::DuplicateGCodeGroup`] on failure.
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
    SpindleFwd(Option<u32>) = 3,

    /// M04
    /// Spindle reverse.
    SpindleRev(Option<u32>) = 4,

    /// M05
    /// Spindle stop.
    SpindleStop = 5,

    /// M06
    /// Tool change.
    ToolChange(Option<u32>) = 6,

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
    /// The returned number would be the same one that was tokenized
    /// by the [`Lexer`] as the [`Suffix`].
    ///
    /// # SAFETY
    /// It is certain that [`MCode`] enum specifies a primitive representation,
    /// therefore the discriminant may be accessed via *unsafe pointer casting*.
    pub fn suffix(&self) -> u32 {
        unsafe { *(self as *const Self as *const u32) }
    }

    /// Tries to construct a [`MCode`] by parsing a [`Code::M`].
    ///
    /// Accepts a [`Code::M`] variant of [`Code`],
    /// and a **mutable reference** to [`Codes`] that were found in the same [`Block`].
    ///
    /// The values used in parsing the `MCode` **will be removed** from `codes` as required.
    ///
    /// # Errors
    /// Returns a [`ParserError::InvalidMCode`] on failure.
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
            unreachable!("Non 'M' prefixed code was tried to be parsed as MCode. Logic Error!");
        }
    }
}

impl Display for MCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut textual = Vec::with_capacity(2);
        textual.push(format!("M{:0>2} - ", self.suffix()));

        textual.push(match self {
            Self::Stop => "Program Stop".into(),
            Self::OptionalStop => "Optional Stop".into(),
            Self::SpindleFwd(s) => {
                if let Some(s) = s {
                    format!("Clockwise Spindle On, at {s} RPMs")
                } else {
                    "Clockwise Spindle On".into()
                }
            }
            Self::SpindleRev(s) => {
                if let Some(s) = s {
                    format!("Counter-Clockwise Spindle On, at {s} RPMs")
                } else {
                    "Counter-Clockwise Spindle On".into()
                }
            }
            Self::SpindleStop => "Spindle Off".into(),
            Self::ToolChange(t) => {
                if let Some(t) = t {
                    format!("Tool Change to tool number: {t}")
                } else {
                    "Tool Change to any preloaded tool".into()
                }
            }
            Self::CoolantOn => "Coolant On".into(),
            Self::CoolantOff => "Coolant Off".into(),
            Self::End => "Program End".into(),
        });

        write!(f, "{}", textual.join(""))
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
        let mut gcodes_unparsed = Vec::new();
        let mut mcode_unparsed = None;

        // do not parse G or M codes until every other code is parsed
        for token in block {
            let code = Code::parse(&token)?;

            match token.prefix {
                b'G' => gcodes_unparsed.push(code),
                b'M' => {
                    if mcode_unparsed.is_some() {
                        return Err(ParserError::DuplicatePrefix(b'M'));
                    }
                    mcode_unparsed = Some(code);
                }
                _ => codes.push(code)?,
            }
        }

        // parse any mcode & gcode(s)
        for code in gcodes_unparsed {
            gcodes.push(GCode::parse(code, &mut codes)?)?;
        }

        if let Some(code) = mcode_unparsed {
            mcode = Some(MCode::parse(code, &mut codes)?);
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

    /// [`Option`]ally returns the parsed [`MCode`].
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

    /// **Optionally** returns the next line as a string slice from the [`Source`](crate::source::Source).
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
#[derive(PartialEq, Debug, thiserror::Error)]
pub enum ParserError {
    /// This prefix does not support the type of suffix provided.
    #[error("wrong suffix type found after prefix: '{}'", *.0 as char)]
    WrongSuffixType(u8),
    /// The code prefix provided is invalid/unimplemented
    #[error("unsupported prefix: '{}'", *.0 as char)]
    UnknownPrefix(u8),
    /// Same G-code found atleast twice.
    #[error("duplicate GCode found: 'G{0}'")]
    DuplicateGCode(u32),
    /// Prefix and suffix make an invalid G-code.
    #[error("unsupported GCode: 'G{0}'")]
    InvalidGCode(u32),
    /// G-codes detected from the same group.
    #[error("duplicate GCode found from group: '{0}'")]
    DuplicateGCodeGroup(u8),
    /// Multiple codes of same prefix in the same line.
    /// Only multiple G-codes are allowed in one line.
    #[error("duplicate prefix: '{}'", *.0 as char)]
    DuplicatePrefix(u8),
    /// The tokens passed along with a 'G' prefix token
    /// do not meet the requirements of the said GCode variant.
    #[error("requirements for 'G{0}' not met")]
    InvalidParamForGCode(u32),
    /// Missing token required for a GCode variant.
    #[error("could not find required prefix '{}' for parsing GCode", *.0 as char)]
    MissingCodeForGCode(u8),
    /// The code block contains codes for both variants of circle methods.
    #[error("codes from both arc methods detected")]
    AmbiguousCircleMethod,
    /// Conditions for a particular circle method were not met, or the end coords are missing.
    #[error("{}", invalid_circle_msg(.0))]
    InvalidCircle(Option<CircleMethod>),
    /// Prefix and suffix make an invalid M-code.
    #[error("unsupported MCode: 'M{0}'")]
    InvalidMCode(u32),
    /// Missing token required for a MCode variant.
    #[error("could not find required prefix '{}' for parsing MCode", *.0 as char)]
    MissingCodeForMCode(u8),
    /// Prefix was found after parsing G & M Codes, but cannot be parsed on its own.
    #[error("unconsumed prefix: '{}'", *.0 as char)]
    UnexpectedPrefix(u8),
    #[error("tokenization failed")]
    Lexer(#[from] LexerError),
}

/// Genertes error message for [`ParserError::InvalidCircle`]
fn invalid_circle_msg(opt: &Option<CircleMethod>) -> &'static str {
    match opt {
        Some(method) => match method {
            CircleMethod::RelativePoint(_) => "relative center of arc not on a single plane",

            CircleMethod::FixedRadius(_) => "zero radius arc requested",
        },
        None => "end coordinates not found for the requested arc",
    }
}

#[cfg(test)]
mod tests {
    use crate::source::Source;

    use super::*;

    // helper for tests
    // returns a parsed vector of gcodes
    fn tokenize_parse(tokens: &str) -> Result<Vec<GCode>, ParserError> {
        let mut parser = Parser::new(Lexer::new(Source::from_str(tokens)));
        parser.next().unwrap().map(|block| block.gcodes.collect())
    }

    // helper for tests
    // returns a parsed mcode
    fn tokenize_parse_m(tokens: &str) -> Result<MCode, ParserError> {
        let mut parser = Parser::new(Lexer::new(Source::from_str(tokens)));
        parser.next().unwrap().map(|block| block.mcode.unwrap())
    }

    #[test]
    // Test to get the suffix of a code by accessing its discriminant.
    fn get_code_suffix() {
        assert_eq!(
            GCode::RapidMove(PartialPoint::new(None, None, None)).suffix(),
            0
        );

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
            vec![GCode::RapidMove(PartialPoint::new(
                Some(0.0),
                Some(0.0),
                None
            ))]
        );
    }

    #[test]
    fn parse_feed_move() {
        assert_eq!(
            tokenize_parse("G1 X0. Y0. F20.").unwrap(),
            vec![GCode::FeedMove {
                pos: PartialPoint::new(Some(0.0), Some(0.0), None),
                feed: Some(20.0)
            }]
        );
    }

    #[test]
    fn parse_cw_arc() {
        assert_eq!(
            tokenize_parse("G2 X0. I1. J2. F20.").unwrap(),
            vec![GCode::CWArcMove {
                pos: PartialPoint::new(Some(0.0), None, None),
                method: CircleMethod::RelativePoint(PartialPoint::new(Some(1.0), Some(2.0), None)),
                feed: Some(20.0)
            }]
        );

        assert_eq!(
            tokenize_parse("G2 Y0. R20. F20.").unwrap(),
            vec![GCode::CWArcMove {
                pos: PartialPoint::new(None, Some(0.0), None),
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
                pos: PartialPoint::new(Some(0.0), None, None),
                method: CircleMethod::RelativePoint(PartialPoint::new(Some(1.0), Some(2.0), None)),
                feed: Some(20.0)
            }]
        );

        assert_eq!(
            tokenize_parse("G3 Y0. R20. F20.").unwrap(),
            vec![GCode::CCWArcMove {
                pos: PartialPoint::new(None, Some(0.0), None),
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
            vec![GCode::MachineCoord(PartialPoint::new(
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
}
