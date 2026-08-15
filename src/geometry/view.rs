//! # View
//!
//! Provided default presets for viewing the simulation.

use std::{f32::consts::PI, fmt::Display};

/// Possible predefined views that can be switched between in the simulation.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// Simulate all three axes, from **isometric view**.
    #[default]
    Isometric,

    /// Simulate `X` & `Y` axes, from **top view**.
    Top,
}

impl View {
    /// Returns the required rotation angles (in **radians**),
    /// around all the three axes, to simulate a given view.
    ///
    /// The rotations **must** be applied in the following order:
    /// 1. Around `Z`.
    /// 2. Around `Y`.
    /// 3. Around `X`.
    pub fn rotations(&self) -> [f32; 3] {
        match self {
            View::Isometric => [(PI / 2.0) - 0.615473, 0.0, -PI / 4.0],
            View::Top => [0.0, 0.0, 0.0],
        }
    }
}

impl Display for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            View::Isometric => "ISOMETRIC",
            View::Top => "TOP",
        };

        write!(f, "{string}")
    }
}
