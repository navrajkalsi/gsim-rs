//! # Parser
//!
//! This library combines `lexer` and `parser` functionality to parse **G-Code**.

// TODO Split into lexer & parser explicitly

#![allow(unused)]

/// Represents a **3D coordinate**.
pub struct Point {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
}

/// Represents a **G-Code block**.
pub enum GBlock {
    /// G00
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(Point),

    /// G01
    /// Linear Interpolate to new coordinates using provided feed rate.
    FeedMove { point: Point, f: Option<f64> },

    /// G02/G03
    /// Circular Interpolate to new coordinates using provided feed rate.
    ArcMove {
        clockwise: bool,
        point: Point,
        method: CircleMethod,
        f: Option<f64>,
    },

    /// G04
    /// Dwell (sec) blocking further code execution.
    Dwell(f64),

    /// G17
    /// Select plane parallel to both X and Y axes (**default for mills**).
    XYPlane(),

    /// G18
    /// Select plane parallel to both X and Z axes.
    XZPlane(),

    /// G19
    /// Select plane parallel to both Y and Z axes.
    YZPlane(),

    /// G20
    /// Use **imperial** units.
    ImperialMode(),

    /// G21
    /// Use **metric** units
    MetricMode(),

    /// G28
    /// Return to Machine Zero point.
    ReturnMachineZero(Point),

    /// G29
    /// Return from reference point.
    ReturnReference(Point),

    /// G40
    /// Cancel cutter compensation (G41/G42).
    CancelCutterComp(),

    /// G41/G42
    /// 2D cutter compensation, left or right.
    CutterComp { side: Side, d: u32 },

    /// G43/G44
    /// Tool length compensation, add or subtract.
    ToolLenComp { sign: Sign, h: u32 },

    /// G49
    /// Cancel tool length compensation (G43, G44).
    CancelLenComp(),

    /// G52
    /// Work coordinate system shift.
    WorkCoordShift(Point),

    /// G53
    /// Machine coordinate system.
    MachineCoord(Point),

    /// G54-G59
    /// Work coordinate system select.
    WorkCoord(u8),

    /// G80
    /// Cancel canned cycles.
    CancelCanned(),

    /// G90
    /// Absolute positioning.
    AbsoluteMode(),

    /// G91
    /// Incremental positioning.
    IncrementalMode(),

    /// G94
    /// Feed per minute mode.
    FeedMinute(),

    /// G95
    /// Feed per revolution mode.
    FeedRev(),

    /// G98
    /// Initial point return in canned cycles.
    InititalReturn(),

    /// G99
    /// Retract plane return in canned cycles.
    RetractReturn(),
}

/// Represents a **M-Code block**.
pub enum MBlock {
    /// M00
    /// Program stop.
    Stop(),

    /// M01
    /// Optional stop.
    OptionalStop(),

    /// M02/M30
    /// Program end.
    End(),

    /// M03
    /// Spindle forward.
    SpindleFwd(Option<u32>),

    /// M04
    /// Spindle reverse.
    SpindleRev(Option<u32>),

    /// M05
    /// Spindle stop.
    SpindleStop(),

    /// M06
    /// Tool change.
    ToolChange(Option<u8>),

    /// M08/M09
    /// Coolant on/off.
    Coolant(bool),
}

/// Circular Interpolation helper
/// Both relative point and radius must not appear in the same block.
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K**.
    RelativePoint(Point),
    /// Explicit radius specified with **R**.
    FixedRadius(f64),
}

/// Represents a side
pub enum Side {
    Left,
    Right,
}

/// Represents possible algerbric signs
pub enum Sign {
    Positive,
    Negative,
}
