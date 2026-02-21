//! # Parser
//!
//! This library combines `lexer` and `parser` functionality to parse **G-Code**.
//! Aims to parse every **Mill** code on **Haas'** website.

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
}

/// Circular Interpolation helper
/// Both relative point and radius must not appear in the same block.
pub enum CircleMethod {
    /// Relative coordinate of circle center with **I, J & K**.
    RelativePoint(Point),
    /// Explicit radius specified with **R**.
    FixedRadius(f64),
}
