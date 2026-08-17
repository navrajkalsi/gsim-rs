//! # Points
//!
//! Houses a family of structures that can be used to represent coordinates in some kind, namely:
//! - [`Point`] : Position in 3D, with every axis coordinate.
//! - [`PartialPoint`] : Position in 3D, with **optional** axis coordinate for each axis.
//! - [`PlanarPoint`] : Position in 2D, on a specific [`Plane`].

use crate::machine::Plane;
use serde::Deserialize;
use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
};

/// A 3D point in space.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point {
    /// Constructs a new [`Point`] from X,Y, and Z axis values.
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Constructs a new [`Point`] with all axis values equal to `0.0`.
    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Constructs a new [`Point`] from an array of 3 `f32`s, each corresponding to X, Y, and Z.
    pub fn from_array(array: [f32; 3]) -> Self {
        Self::new(array[0], array[1], array[2])
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

    /// Returns coordinates of the point as an array.
    pub fn as_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl Add for Point {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f32> for Point {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl Div<f32> for Point {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
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

    /// Checks if all the axis are `None` variants.
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

/// A 2D Point on a specific plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanarPoint {
    /// Selected plane for the point.
    pub plane: Plane,

    /// First axis coordinate for the selected plane.
    pub first: f32,

    /// Second axis coordinate for the selected plane.
    pub second: f32,
}

impl PlanarPoint {
    /// Constructor for a [`PlanarPoint`].
    ///
    /// For a given [`Plane`], *first* represents the value for first axis letter and *second*
    /// represents the second axis letter.
    /// For example:
    /// ```ignore
    /// PlanarPoint::new(Plane::XY, 1.0, 2.0); // Constructs point with X = 1.0 & Y = 2.0.
    /// PlanarPoint::new(Plane::XZ, 3.0, 4.0); // Constructs point with X = 3.0 & Z = 4.0.
    /// PlanarPoint::new(Plane::YZ, 5.0, 6.0); // Constructs point with Y = 5.0 & Z = 6.0.
    /// ```
    pub fn new(plane: Plane, first: f32, second: f32) -> Self {
        Self {
            plane,
            first,
            second,
        }
    }

    /// Constructs a new [`PlanarPoint`] from a [`Point`] and [`Plane`].
    ///
    /// Discards the coordinates for the axis that is not part of the provided [`Plane`].
    pub fn from_point(point: Point, plane: Plane) -> Self {
        match plane {
            Plane::XY => Self::new(plane, point.x, point.y),
            Plane::XZ => Self::new(plane, point.x, point.z),
            Plane::YZ => Self::new(plane, point.y, point.z),
        }
    }

    /// Calculates distance between two [`PlanarPoint`]s that **MUST** be on the same [`Plane`].
    ///
    /// # Panics
    /// Panics if the `other` [`PlanarPoint`] is on a different [`Plane`].
    pub fn dist(&self, other: &Self) -> f32 {
        assert_eq!(self.plane, other.plane, "points must be on the same plane");

        ((self.first - other.first).powi(2) + (self.second - other.second).powi(2)).sqrt()
    }
}

impl Sub for PlanarPoint {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.plane, self.first - rhs.first, self.second - rhs.second)
    }
}

impl Add for PlanarPoint {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.plane, self.first + rhs.first, self.second + rhs.second)
    }
}
