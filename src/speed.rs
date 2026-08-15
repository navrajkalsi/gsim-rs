//! # Speed
//!
//! Provides simulation speed runtime controls using the [`Speed`] type.
//!
//! The simulation speed is controlled by adjusting the number of frames
//! that end up being drawn/rendered to the window.
//! [`Speed`] tracks the number of frames to skip and performs bounds checks on the said count.
//!
//! Skipping more frames leads to fewer draw calls,
//! increasing the number of frames processed in a specific time interval.

use std::fmt::Display;

/// Maximum number of frames to skip before forcing a draw.
const MAX: u8 = 10;
/// Minimum number of frames to skip before forcing a draw.
const MIN: u8 = 0;
/// Default average of the maximum and minimum frame skip count.
const DEFAULT: u8 = (MAX + MIN) / 2;

/// Tracks the number of frames to skip between draw calls,
/// giving the affect of simulation speed manipulation.
#[derive(Debug, Clone, Copy)]
pub struct Speed(u8);

impl Speed {
    /// Returns the number of frames to skip between two render calls.
    pub fn frames_to_skip(&self) -> u8 {
        self.0
    }

    /// Tries to increment the internal counter, respecting the predefined limits.
    ///
    /// Returns `false` if the counter was not incremented.
    pub fn inc(&mut self) -> bool {
        if self.0 < MAX {
            self.0 += 1;
            true
        } else {
            false
        }
    }

    /// Tries to decrement the internal counter, respecting the predefined limits.
    ///
    /// Returns `false` if the counter was not decremented.
    pub fn dec(&mut self) -> bool {
        if self.0 > MIN {
            self.0 -= 1;
            true
        } else {
            false
        }
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self(DEFAULT)
    }
}

impl Display for Speed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self.0 {
            MAX => String::from("Max"),
            MIN => String::from("Min"),
            DEFAULT => String::from("Default"),
            num => {
                let diff = num as i8 - DEFAULT as i8;
                format!("{diff:+}")
            }
        };

        write!(f, "{string}")
    }
}
