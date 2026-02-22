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

    /// G09
    /// Exact Stop for improving accuracy by checking for completion.
    ExactStop(),

    /// G10
    /// Set offsets from within the program.
    SetOffset {
        /// Offset category
        l: i32,
        /// Specific offset
        p: i32,
        // Offset value
        r: f64,
        zero: Point,
    },

    /// G12/G13
    /// Mill circular shapes.
    CirclePocket {
        clockwise: bool,
        /// Tool diameter
        d: i32,
        /// Radius of first circle
        i: f64,
        /// Radius of finished circle
        k: f64,
        /// Repeat count
        l: i32,
        /// Radius depth
        q: f64,
        /// Axial depth
        z: f64,
    },

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

    /// G47
    /// Text engraving.
    Engrave {
        /// Smoothness setting
        d: Level,
        /// Plunge feed rate
        e: f64,
        f: f64,
        /// Angle of rotation
        i: f64,
        /// Height of text
        j: f64,
        /// Max corner rounding
        k: f64,
        /// Engraving type
        p: u32,
        r: f64,
        /// Starting point on the selected plane, third axis will be the depth
        start: Point,
    },

    /// G49
    /// Cancel tool length compensation (G43, G44).
    CancelLenComp(),

    /// G50
    /// Cancel scaling (G51).
    CancelScaling(),

    /// G51
    /// Scaling.
    Scaling {
        center: Point,
        /// Scaling factor
        p: f32,
    },

    //// G52
    /// Work coordinate system shift.
    WorkCoordShift(Point),

    /// G53
    /// Machine coordinate system.
    MachineCoord(Point),

    /// G54-G59
    /// Work coordinate system select.
    WorkCoord(u8),

    /// G60
    /// Uni-Directional positioning.
    UniDirectional(),

    /// G61
    /// Activate exact stop mode.
    ExactStopMode(),

    /// G64
    /// Cancel exact stop mode.
    ExactStopModeCancel(),
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

/// Represents possible levels for a variable
pub enum Level {
    Low,
    Medium,
    High,
}
