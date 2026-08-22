//! # Signal
//!
//! Aids in communicating state changes from [`Gui`](crate::gui) window to the [`Tui`](crate::tui),
//! which serves as an interface for representing the current overall state.
//!
//! - [`CycleSignal`]: Changes that arise as a direct result of G-code execution.
//!   Delivered via [`Arc<Mutex<_>>`], since G-code execution rate is far more than Tui's draw and
//!   poll rate. Only the latest value matters, so overwriting any non-consumed values is safe.
//! - [`UserSignal`]: Changes which are result of user input.
//!   Delivered via [`mpsc::channel`](std::sync::mpsc::channel), since user input frequency is very
//!   low and any update must **never be lost**. Unread updates are just **queued** chronologically.

use crate::{
    Interrupt,
    geometry::view::View,
    interpreter::{BlockSummary, InterpreterError},
    machine::Machine,
    speed::Speed,
};
use std::sync::Arc;

/// State changes in the [`Gui`](crate::gui) as a result of **G-code execution**, to be reflected in the [`Tui`](crate::tui).
///
/// To be delivered via [`Arc<Mutex<_>>`]. See [`crate::signal`] for more details.
#[derive(Debug, Clone)]
pub enum CycleSignal {
    /// Executing a new G-code block.
    Run {
        /// Summary of the executed block.
        summary: Arc<BlockSummary>,
        /// Machine state on block execution.
        machine: Machine,
        /// Index of the executed block.
        index: usize,
    },

    /// G-code execution halted.
    ///
    /// Delivered on detection of `M00`, `M01`, or `M30`, or on program end.
    Pause {
        /// Cause of the halt.
        interrupt: Interrupt,
        /// Machine state during the halt.
        machine: Machine,
        /// Index of the block that caused the halt.
        index: usize,
    },

    /// G-code block execution failed.
    ///
    /// Represents any errors related to G-code **lexing**, **parsing** and **interpretation**.
    /// **Does not** represent any simulation errors.
    Error {
        /// Error while generating a summary for the current G-code block.
        error: InterpreterError,
        /// Machine state during the error.
        machine: Machine,
        /// Index of the block that produced the error.
        index: usize,
    },

    /// Simulation stop triggered.
    Stop,
}

/// State changes in the [`Gui`](crate::gui) as a result of **user input**, to be reflected in the [`Tui`](crate::tui).
///
/// To be delivered via [`mpsc::channel`](std::sync::mpsc::channel). See [`crate::signal`] for more details.
#[derive(Debug, Clone)]
pub enum UserSignal {
    /// Simulation view has changed due to user input/interaction.
    ///
    /// - `Some`: Reset to a predefined [`View`].
    /// - `None`: Free orbiting.
    SetView(Option<View>),

    /// Simulation view fit has changed due to user input/interaction.
    ///
    /// - `true`: Fully contained inside the simulation window, with most zoom applied.
    /// - `false`: Simulation may be zoomed out and/or offscreen, either partially or fully.
    SetFit(bool),

    /// Single block execution setting changed.
    SetSingle(bool),

    /// Tool visibility changed.
    SetToolVisibility(bool),

    /// Toolpath visibility changed.
    SetToolpathVisibility(bool),

    /// Stock visibility changed.
    SetStockVisibility(bool),

    /// Simulation speed changed.
    SetSpeed(Speed),
}
