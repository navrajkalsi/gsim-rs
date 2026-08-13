//! Signal
//!
//! Aids in communicating state changes from [`Gui`](crate::gui) window to the [`Tui`](crate::tui),
//! that serves as an interface for representing the current overall state.

use crate::{
    Interrupt, Speed, View,
    interpreter::{BlockSummary, InterpreterError},
    machine::Machine,
};
use std::sync::Arc;

/// State changes in the [`Gui`](crate::gui), to be reflected in the [`Tui`](crate::tui).
#[derive(Debug, Clone)]
pub enum Signal {
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

    /// Simulation stop triggered.
    Stop,
}
