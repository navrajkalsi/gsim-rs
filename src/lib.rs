//! # Parser
//!
//! This library combines `lexer` and `parser` functionality to parse **G-Code**.
//! Aims to parse every **Mill** code listed on **[Haas'](https://example.com)** website.

// TODO Split into lexer & parser explicitly

#![allow(unused)]

/// Encapsulates a **3D coordinate**.
pub struct Point {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
}

/// Encapsulates a **G-Code block**.
pub enum GBlock {
    /// G00 Command
    /// Linear Interpolate to new coordinates using rapid rate.
    RapidMove(Point),

    /// G01 Command
    /// Linear Interpolate to new coordinates using provided feed rate.
    FeedMove { point: Point, f: Option<f64> },

    /// G02/G03 Command
    /// Circular Interpolate to new coordinates using provided feed rate.
    ArcMove {
        clockwise: bool,
        point: Point,
        method: CircleMethod,
        f: Option<f64>,
    },

    /// G04 Command
    /// Dwell (sec) blocking further code execution.
    Dwell(f64),

    /// G09 Command
    /// Exact Stop for improving accuracy by checking for completion.
    ExactStop(),

    /// G10 Command
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

    /// G12/G13 Command
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

    /// G17 Command
    /// Select plane parallel to both X and Y axes (**default for mills**).
    XYPlane(),

    /// G18 Command
    /// Select plane parallel to both X and Z axes.
    XZPlane(),

    /// G19 Command
    /// Select plane parallel to both Y and Z axes.
    YZPlane(),

    /// G20 Command
    /// Use **imperial** units.
    ImperialMode(),

    /// G21 Command
    /// Use **metric** units
    MetricMode(),
}

/// Circular Interpolation helper
/// Both relative point and radius must not appear in the same block.
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K**.
    RelativePoint(Point),
    /// Explicit radius specified with **R**.
    FixedRadius(f64),
}
