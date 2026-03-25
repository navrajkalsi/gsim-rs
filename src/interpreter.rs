//! # Intrpreter
//!
//! This module executes [`CodeBlock`](crate::parser::CodeBlock)s (represented as [`Parser`])
//! on a [`Machine`] by accessing its public API.

use std::{fmt::Display, io};

use crate::{
    error::RESET,
    machine::{
        CircularDirection, Direction, FeedMode, Machine, MachineError, Plane, Positioning,
        ReturnLevel, Unit,
    },
    parser::{Code, GCode, MCode, Parser, Point},
};

/// Represents an instance of [`Interpreter`](crate::interpreter).
// empty for future use
pub struct Interpreter();

impl Interpreter {
    /// Executes each [`CodeBlock`] of the provided [`Parser`]
    /// sequentially on the provided [`Machine`].
    ///
    /// Returns [`InterpreterError`] on failure, which itself is mostly a wrapper on [`MachineError`].
    pub fn execute(parser: Parser, mut machine: Machine) -> Result<(), InterpreterError> {
        'mainloop: for mut block in parser {
            for gcode in block.gcodes() {
                match gcode {
                    GCode::RapidMove(pos) => machine.rapid_move(pos)?,

                    GCode::FeedMove { pos, feed } => machine.feed_move(pos, feed)?,

                    GCode::CWArcMove { pos, method, feed } => {
                        _ = machine.arc_move(pos, method, CircularDirection::Clockwise, feed)?
                    }

                    GCode::CCWArcMove { pos, method, feed } => {
                        _ = machine.arc_move(
                            pos,
                            method,
                            CircularDirection::CounterClockwise,
                            feed,
                        )?
                    }

                    GCode::Dwell(p) => {
                        println!("Dwelling for {p} seconds.");
                        let duration = std::time::Duration::from_millis((p * 1000.0) as u64);
                        std::thread::sleep(duration);
                    }

                    GCode::XYPlane => machine.set_plane(Plane::XY),

                    GCode::XZPlane => machine.set_plane(Plane::XZ),

                    GCode::YZPlane => machine.set_plane(Plane::YZ),

                    GCode::ImperialMode => machine.set_code_units(Unit::Imperial),

                    GCode::MetricMode => machine.set_code_units(Unit::Metric),

                    GCode::CancelCutterComp => machine.cancel_dia_offset(),

                    GCode::LeftCutterComp(d) => machine.set_dia_offset(d, Direction::Left)?,

                    GCode::RightCutterComp(d) => machine.set_dia_offset(d, Direction::Right)?,

                    GCode::ToolLenCompAdd(h) => machine.set_height_offset(h, Direction::Up)?,

                    GCode::ToolLenCompSubtract(h) => {
                        machine.set_height_offset(h, Direction::Down)?
                    }

                    GCode::CancelLenComp => machine.cancel_height_offset(),

                    GCode::MachineCoord(pos) => machine.move_machine_pos(pos)?,

                    // always make the machine center as g54 offset
                    GCode::WorkCoord => machine.set_work_offset(Point::new(
                        machine.max_travels().x() / 2.0,
                        machine.max_travels().y() / 2.0,
                        0.0,
                    )),

                    GCode::CancelCanned => machine.cancel_canned(),

                    GCode::AbsoluteMode => machine.set_positioning(Positioning::Absolute),

                    GCode::IncrementalMode => machine.set_positioning(Positioning::Incremental),

                    GCode::FeedMinute => machine.set_feed_mode(FeedMode::PerMinute),

                    GCode::FeedRev => machine.set_feed_mode(FeedMode::PerRev),

                    GCode::InitialReturn => machine.set_return_level(ReturnLevel::Initial),

                    GCode::RetractReturn => machine.set_return_level(ReturnLevel::Retract),
                }
            }

            if let Some(mcode) = block.mcode() {
                match mcode {
                    MCode::Stop => Self::wait()?,

                    MCode::OptionalStop => Self::wait()?,

                    MCode::SpindleFwd(s) => machine.spindle_on(CircularDirection::Clockwise, s)?,

                    MCode::SpindleRev(s) => {
                        machine.spindle_on(CircularDirection::CounterClockwise, s)?
                    }

                    MCode::SpindleStop => machine.spindle_off(),

                    MCode::ToolChange(t) => machine.tool_change(t)?,

                    MCode::CoolantOn => machine.set_coolant(true),

                    MCode::CoolantOff => machine.set_coolant(false),

                    MCode::End => {
                        println!("Program end detected");
                        machine.reset();
                        break 'mainloop;
                    }
                }
            }

            for code in block.codes() {
                match code {
                    Code::D(_) => todo!(),

                    Code::G(_) => unreachable!("The parser will not emit G code with other codes."),

                    Code::H(_) => todo!(),

                    Code::M(_) => unreachable!("The parser will not emit M code with other codes."),

                    Code::N(_) => (),

                    Code::O(_) => (),

                    Code::P(_) => todo!(),

                    Code::S(s) => machine.set_speed(s),

                    Code::T(t) => machine.set_next_tool(t),

                    Code::F(f) => machine.set_feed(f),

                    Code::I(_) => todo!(),

                    Code::J(_) => todo!(),

                    Code::K(_) => todo!(),

                    Code::Q(_) => todo!(),

                    Code::R(_) => todo!(),

                    Code::X(_) => todo!(),

                    Code::Y(_) => todo!(),

                    Code::Z(_) => todo!(),
                }
            }
        }

        Ok(())
    }

    /// `Stop` M-Code helper.
    /// Waits for the user to press 'Enter' to continue.
    fn wait() -> Result<(), io::Error> {
        println!("Program stopped.\nPress Enter to continue...");

        match io::stdin().read_line(&mut String::new()) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Possible errors that can happen during executing the code.
#[derive(Debug)]
pub enum InterpreterError {
    File(io::Error),
    Machine(MachineError),
}

impl From<io::Error> for InterpreterError {
    fn from(e: io::Error) -> Self {
        Self::File(e)
    }
}

impl From<MachineError> for InterpreterError {
    fn from(e: MachineError) -> Self {
        Self::Machine(e)
    }
}

impl Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::File(_) => write!(
                f,
                "File Access Error:{RESET}\n\t\tError encountered while trying to read input from user."
            ),
            // no need to format new error,
            // just print machine error as interpreter error which is formatted
            Self::Machine(e) => write!(f, "{e}"),
        }
    }
}
