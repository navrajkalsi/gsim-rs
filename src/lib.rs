//! # Parser
//!
//! This library combines `lexer` and `parser` functionality to parse **G-Code**.

// TODO Split into lexer & parser explicitly

#![allow(unused)]

use std::{cmp::Ordering, fmt::Display, process::exit};
pub mod parser;

const MAX_TRAVELS: Point = Point {
    x: 40.0,
    y: 20.0,
    z: 20.0,
};

const MIN_TRAVELS: Point = Point {
    x: 5.0,
    y: 5.0,
    z: 5.0,
};

/// Represents a **3D coordinate** where every axis is required.
#[derive(PartialEq, Default, Debug)]
pub struct Point {
    x: f64,
    y: f64,
    z: f64,
}

/// Represents a **3D coordinate** where an axis can be **None**.
#[derive(Debug)]
pub struct PartialPoint {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
}

impl Point {
    pub fn new(x: f64, y: f64, z: f64) -> Point {
        Point { x, y, z }
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        println!("self: {self}, other: {other}");
        if self.x < other.x || self.y < other.y || self.z < other.z {
            Some(Ordering::Less)
        } else if self.x > other.x || self.y > other.y || self.z > other.z {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}

/// Represents a **G-Code block**.
#[derive(Debug)]
#[repr(i8)]
pub enum GBlock {
    Empty = -1,

    /// The block only contains coordinates.
    Point(PartialPoint) = -2,

    /// G00
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(PartialPoint) = 0,

    /// G01
    /// Linear Interpolate to new coordinates using provided feed rate.
    FeedMove {
        point: PartialPoint,
        f: Option<f64>,
    } = 1,

    /// G02
    /// Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CWArcMove {
        point: PartialPoint,
        method: CircleMethod,
        f: Option<f64>,
    } = 2,

    /// G03
    /// Counter-Clockwise Circular Interpolate to new coordinates using provided feed rate.
    CCWArcMove {
        point: PartialPoint,
        method: CircleMethod,
        f: Option<f64>,
    } = 3,

    /// G04
    /// Dwell (sec) blocking further code execution.
    Dwell(f64) = 4,

    /// G17
    /// Select plane parallel to X and Y axes (**default for mills**).
    XYPlane() = 17,

    /// G18
    /// Select plane parallel to X and Z axes.
    XZPlane() = 18,

    /// G19
    /// Select plane parallel to Y and Z axes.
    YZPlane() = 19,

    /// G20
    /// Use **imperial** units.
    ImperialMode = 20,

    /// G21
    /// Use **metric** units
    MetricMode = 21,

    /// G28
    /// Return to Machine Zero point.
    ReturnMachineZero(PartialPoint) = 28,

    /// G29
    /// Return from reference point.
    ReturnReference(PartialPoint) = 29,

    /// G40
    /// Cancel cutter compensation (G41/G42).
    CancelCutterComp = 40,

    /// G41
    /// 2D left cutter compensation.
    LeftCutterComp(u32) = 41,

    /// G42
    /// 2D right cutter compensation.
    RigthCutterComp(u32) = 42,

    /// G43
    /// Tool length compensation by addition.
    ToolLenCompAdd(u32) = 43,

    /// G44
    /// Tool length compensation by subtraction.
    ToolLenCompSubtract(u32) = 44,

    /// G49
    /// Cancel tool length compensation (G43, G44).
    CancelLenComp = 49,

    /// G52
    /// Work coordinate system shift.
    WorkCoordShift(PartialPoint) = 52,

    /// G53
    /// Machine coordinate system.
    MachineCoord(PartialPoint) = 53,

    /// G54-G59
    /// Work coordinate system select.
    WorkCoord(u8) = 54,

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
    InititalReturn = 98,

    /// G99
    /// Retract plane return in canned cycles.
    RetractReturn = 99,
}

impl GBlock {
    fn discriminant(&self) -> i8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        unsafe { *<*const _>::from(self).cast::<i8>() }
    }
}

/// Represents a **M-Code block**.
pub enum MBlock {
    /// M00
    /// Program stop.
    Stop,

    /// M01
    /// Optional stop.
    OptionalStop,

    /// M02/M30
    /// Program end.
    End,

    /// M03
    /// Spindle forward.
    SpindleFwd(Option<u32>),

    /// M04
    /// Spindle reverse.
    SpindleRev(Option<u32>),

    /// M05
    /// Spindle stop.
    SpindleStop,

    /// M06
    /// Tool change.
    ToolChange(Option<u8>),

    /// M08/M09
    /// Coolant on/off.
    Coolant(bool),
}

/// Circular Interpolation helper.
/// Both relative point and radius must not appear in the same block.
#[derive(Debug)]
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K**.
    RelativePoint(PartialPoint),
    /// Explicit radius specified with **R**.
    FixedRadius(f64),
}

/// Represents a side.
#[derive(Debug)]
pub enum Side {
    Left,
    Right,
}

/// Represents possible algerbric signs.
#[derive(Debug)]
pub enum Sign {
    Positive,
    Negative,
}

/// Represents a plane that is parallel to both the specified axes.
#[derive(Default, Debug)]
pub enum Plane {
    #[default]
    XY,
    XZ,
    YZ,
}

impl Display for Plane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Plane::XY => "XY",
                Plane::YZ => "YZ",
                Plane::XZ => "XZ",
            }
        )
    }
}

/// Represents a **Machine**.
pub struct Machine {
    max_travels: Point,
    pos: Point,
    work_coord: Point,
    active_gcodes: Vec<u8>,
    plane: Plane,
    rapid: bool,
    feed_rate: Option<f64>,
    absolute: bool,
    imperial: bool,
    tool: Option<u8>,
    coolant: bool,
    spindle_on: bool,
    spindle_speed: Option<u32>,
}

impl Machine {
    /// Generates a new default machine instance and accepts the max dimensions of the machine.
    ///
    /// # Defaults
    /// The defaults are as follows:
    /// - **Home Position**: X0. Y0. Z0.
    /// - **Start Position**: Home Position
    /// - **Movement Type**: Rapid
    /// - **Feed Rate**: Null
    /// - **Positioning**: Absolute
    /// - **Unit System**: Imperial
    /// - **Current Tool**: Null
    /// - **Coolant**: Off
    /// - **Spindle**: Off
    /// - **Spindle Speed**: Null
    ///
    /// # Errors
    /// If the travels provided are above the **MAX_TRAVLELS** constant an error is returned
    pub fn build(max_travels: Point) -> Result<Self, &'static str> {
        if max_travels > MAX_TRAVELS {
            Err("Machine travels provided are too large.")
        } else if max_travels < MIN_TRAVELS {
            Err("Machine travels have to a resonable positive number.")
        } else {
            Ok(Machine {
                max_travels,
                pos: Point::default(),
                work_coord: Point::default(),
                active_gcodes: Vec::new(),
                plane: Plane::default(),
                rapid: true,
                feed_rate: None,
                absolute: true,
                imperial: true,
                tool: None,
                coolant: false,
                spindle_on: false,
                spindle_speed: None,
            })
        }
    }
}

impl Display for Machine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Machine specs & state:\nStroke Limits: {}\nCurrent Position: {}\nCurrent Plane: {}",
            self.max_travels, self.pos, self.plane
        )
    }
}

pub enum Alarm {
    OvertravelXNeg(&'static str),
    OvertravelXPos(&'static str),
    OvertravelYNeg(&'static str),
    OvertravelYPos(&'static str),
    OvertravelZNeg(&'static str),
    OvertravelZPos(&'static str),
}

#[derive(Debug)]
pub enum ParseError {
    EndBlockDetected,
    IllegalChar,
    NonUsableChar,
}

/// Parse a single G-Code line.
/// Assumes that the lines have been split around ';' and the function is receiving one of those
/// lines.
///
/// # Errors
/// - EndBlockDetected: ';' should not be present.
/// - IllegalChar: Non-Ascii char is detected.
/// - NonUsableChar: An ascii char, that is not applicable to this usecase, is detected.
///
pub fn parse_block(block: &str) -> Result<GBlock, ParseError> {
    println!("Received block: {block}");
    if !block.is_ascii() {
        return Err(ParseError::IllegalChar);
    }

    if block.contains(';') {
        return Err(ParseError::EndBlockDetected);
    }

    let mut res = GBlock::Empty;
    // this buffer stores an ascii letter and the following numbers
    // it is reset when a space or another letter is detected
    //
    let mut buffer: [u8; 5] = [0, 0, 0, 0, 0];

    println!("{}", res.discriminant());

    for char in block.as_bytes() {
        if char.is_ascii_control() {
            return Err(ParseError::NonUsableChar);
        }

        let mut h = vec![];
        h.push(0);

        println!("{char}");
    }

    exit(0);
    Ok(res)
}
