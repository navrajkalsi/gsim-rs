//! # Interpreter
//!
//! This module executes [`CodeBlock`]s (represented as [`Parser`])
//! on a [`Machine`] by accessing its public API.

use std::{fmt::Display, io};

use crate::{
    describe::{Describe, Description},
    error::{RED, RESET},
    lexer::Prefix,
    machine::{
        CircularDirection, Direction, FeedMode, Machine, MachineError, Motion, Plane, Positioning,
        ReturnLevel, Unit,
    },
    parser::{Code, CodeBlock, Codes, GCode, MCode, Parser, ParserError, Point},
};

/// Represents an instance of [`Interpreter`](crate::interpreter).
pub struct Interpreter {
    parser: Parser,
    machine: Machine,
}

/// Represents textual representation of an entire [`CodeBlock`].
/// This is used for the **text** view in [`App`](crate::app::App).
pub struct BlockText {
    pub gcodes: Vec<String>,
    pub mcode: Option<String>,
    pub codes: Vec<String>,
}

impl Interpreter {
    /// Constructs an [`Interpreter`] from a provided [`Parser`] and [`Machine`],
    /// ready to execute the code on the machine.
    pub fn new(parser: Parser, machine: Machine) -> Self {
        Self { parser, machine }
    }

    /// Executes each [`CodeBlock`] of the [`Parser`] sequentially on the [`Machine`].
    ///
    /// Returns [`InterpreterError`] on failure, which itself is mostly a wrapper on [`MachineError`].
    pub fn execute(&mut self) -> Result<(), InterpreterError> {
        while let Some(_) = self.execute_single()? {}

        Ok(())
    }

    /// Executes the [`Parser::next`] [`CodeBlock`] of the [`Parser`] on the [`Machine`].
    ///
    /// Returns [`None`] on exhaustion of [`CodeBlock`]s or on [`MCode::End`].
    /// Returns [`InterpreterError`] on failure, which itself is mostly a wrapper on [`MachineError`].
    pub fn execute_single(&mut self) -> Result<Option<BlockText>, InterpreterError> {
        let parser = &mut self.parser;
        let machine = &mut self.machine;

        let mut block = match parser.next() {
            Some(res) => res?,
            None => return Ok(None),
        };

        let mut gcode_lines = vec![];
        for gcode in block.gcodes() {
            gcode_lines.push(gcode.to_string());
            match gcode {
                GCode::RapidMove(pos) => machine.rapid_move(pos)?,

                GCode::FeedMove { pos, feed } => machine.feed_move(pos, feed)?,

                GCode::CWArcMove { pos, method, feed } => {
                    _ = machine.arc_move(pos, method, CircularDirection::Clockwise, feed)?
                }

                GCode::CCWArcMove { pos, method, feed } => {
                    machine.arc_move(pos, method, CircularDirection::CounterClockwise, feed)?
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

                GCode::ToolLenCompSubtract(h) => machine.set_height_offset(h, Direction::Down)?,

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

        let mut mcode_line = None;
        if let Some(mcode) = block.mcode() {
            mcode_line = Some(mcode.to_string());
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
                    return Ok(None);
                }
            }
        }

        let mut code_lines = vec![];
        for code in block.codes() {
            // for storing any coord codes and parsing them altogether
            let mut excess = Codes::new();

            // display is only implemented for variants that will not cause any errors
            // and which do not fall through to excess codes
            let code_line = code.to_string();
            if !code_line.is_empty() {
                code_lines.push(code.to_string());
            }

            match code {
                Code::G(_) => unreachable!("The parser will not emit G code with other codes."),
                Code::M(_) => unreachable!("The parser will not emit M code with other codes."),

                Code::D(_) => return Err(InterpreterError::ExcessCode(b'D')),
                Code::H(_) => return Err(InterpreterError::ExcessCode(b'H')),
                Code::P(_) => return Err(InterpreterError::ExcessCode(b'P')),
                Code::Q(_) => return Err(InterpreterError::ExcessCode(b'Q')),

                // ignore line & program numbers
                Code::N(_) | Code::O(_) => (),

                Code::S(s) => machine.set_speed(s),

                Code::T(t) => machine.set_next_tool(t),

                Code::F(f) => machine.set_feed(f),

                Code::I(_)
                | Code::J(_)
                | Code::K(_)
                | Code::R(_)
                | Code::X(_)
                | Code::Y(_)
                | Code::Z(_) => excess.push(code).unwrap(),
            };

            // from excess codes, only interpolation is possible
            //
            // it is worth noting that an single block cannot be interpreted twice,
            // that is, a block will not have two interpolations,
            // because everyblock needs x, y or z, and there are no duplicates.
            //
            // these excess moves will be labelled as gcode lines,
            // because these are basically gcodes lines with the 'G' code omitted as those are
            // modal.
            match machine.motion() {
                Motion::Rapid => {
                    let pos = excess.take_partial_point();
                    gcode_lines.push(GCode::RapidMove(pos.clone()).to_string());
                    machine.rapid_move(pos)?;
                }

                // feed would be set from the for loop, if provided
                Motion::Feed => {
                    let pos = excess.take_partial_point();
                    gcode_lines.push(
                        GCode::FeedMove {
                            pos: pos.clone(),
                            feed: None,
                        }
                        .to_string(),
                    );
                    machine.feed_move(pos, None)?;
                }

                Motion::Arc(dir) => {
                    let (pos, method, feed) = excess.take_circular()?;
                    match dir {
                        CircularDirection::Clockwise => gcode_lines.push(
                            GCode::CWArcMove {
                                pos: pos.clone(),
                                method: method.clone(),
                                feed,
                            }
                            .to_string(),
                        ),
                        CircularDirection::CounterClockwise => gcode_lines.push(
                            GCode::CCWArcMove {
                                pos: pos.clone(),
                                method: method.clone(),
                                feed,
                            }
                            .to_string(),
                        ),
                    };
                    machine.arc_move(pos, method, *dir, feed)?;
                }
            };

            // single move should consume all the excess codes
            if let Some(code) = excess.next() {
                return Err(InterpreterError::ExcessCode(code.prefix()));
            }
        }

        Ok(Some(BlockText {
            gcodes: gcode_lines,
            mcode: mcode_line,
            codes: code_lines,
        }))
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

    /// Reloads the [`Interpreter`] to start from beginning of the [`Parser`].
    pub fn reload(&mut self) {
        self.parser.reload();
    }

    /// **Optinally** returns the next [`Line`](crate::source::Line) as a string slice from the [`Source`](crate::source::Source).
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.parser.get_line(index)
    }
}

/// Possible errors that can happen during executing the code.
#[derive(Debug)]
pub enum InterpreterError {
    File(io::Error),
    Machine(MachineError),
    Parser(ParserError),
    /// At least one code from a code block exists that was not consumed.
    ExcessCode(Prefix),
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

impl From<ParserError> for InterpreterError {
    fn from(e: ParserError) -> Self {
        Self::Parser(e)
    }
}

impl Describe for InterpreterError {
    fn describe(&self) -> crate::describe::Description {
        let (title, desc) = match self {
            Self::File(_) => (
                "File Access Error",
                "Error encountered while trying to read input from user.".to_string(),
            ),

            Self::ExcessCode(c) => (
                "Excess Code Detected",
                format!(
                    "The code block contains the following code, which could not be consumed and may be redundant: {}.",
                    *c as char
                ),
            ),

            // no need to format new error,
            // just print machine & parser error as interpreter error which is formatted
            Self::Machine(e) => return e.describe(),
            Self::Parser(e) => return e.describe(),
        };

        Description::new(title, desc)
    }
}

impl Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::File(_) => write!(
                f,
                "File Access Error:{RESET}\n\t\tError encountered while trying to read input from user."
            ),

            Self::ExcessCode(c) => write!(
                f,
                "Excess Code Detected:{RESET}\n\t\tThe code block contains the following code, which could not be consumed and may be redundant: {RED}{}{RESET}.",
                *c as char
            ),

            // no need to format new error,
            // just print machine & parser error as interpreter error which is formatted
            Self::Machine(e) => write!(f, "{e}"),
            Self::Parser(e) => write!(f, "{e}"),
        }
    }
}
