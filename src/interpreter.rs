//! # Interpreter
//!
//! Executes [`CodeBlock`]s (represented as [`Parser`])
//! on a [`Machine`], by accessing its public API.

#[allow(unused_imports)]
use crate::{
    config::{Point, Unit},
    machine::{
        CircularDirection, Direction, FeedMode, Machine, MachineError, Motion, MotionSummary,
        Positioning, ReturnLevel,
    },
    parser::{Code, CodeBlock, Codes, GCode, MCode, Parser, ParserError, Plane},
};

/// Represents a summary consumed [`CodeBlock`].
/// Contains all the information required by the [`Tui`](crate::tui::Tui)
/// to render the new [`Machine`] state.
#[derive(Debug, Clone)]
pub struct BlockSummary {
    /// Parsed [`GCode`]s from the block.
    pub gcodes: Vec<GCode>,
    /// Parsed [`MCode`] from the block.
    pub mcode: Option<MCode>,
    /// Parsed [`Code`]s from the block.
    pub codes: Vec<Code>,
    /// Captures any [`Motion`] and position changes.
    pub motion: Option<MotionSummary>,
}

/// Represents an instance of [`Interpreter`](crate::interpreter).
pub struct Interpreter {
    parser: Parser,
    machine: Machine,
}

impl Interpreter {
    /// Constructs an [`Interpreter`] from a provided [`Parser`] and [`Machine`],
    /// ready to execute the code on the machine on demand.
    pub fn new(parser: Parser, machine: Machine) -> Self {
        Self { parser, machine }
    }

    /// Executes the [`Parser::next`] [`CodeBlock`] of the [`Parser`] on the [`Machine`].
    ///
    /// Returns the summary of changes during execution as [`BlockSummary`],
    /// or [`None`] on exhaustion of [`CodeBlock`]s.
    ///
    /// Returns [`InterpreterError`] on failure.
    pub fn execute(&mut self) -> Result<Option<BlockSummary>, InterpreterError> {
        let parser = &mut self.parser;
        let machine = &mut self.machine;
        let mut motion = None;

        let mut block = match parser.next() {
            Some(res) => res?,
            None => return Ok(None),
        };

        let mcode = block.mcode();

        let mut gcodes = vec![];
        for gcode in block.gcodes() {
            gcodes.push(gcode);
            match gcode {
                GCode::RapidMove(pos) => motion = Some(machine.rapid_move(pos)?),

                GCode::FeedMove { pos, feed } => motion = Some(machine.feed_move(pos, feed)?),

                GCode::CWArcMove { pos, method, feed } => {
                    motion =
                        Some(machine.arc_move(pos, method, CircularDirection::Clockwise, feed)?)
                }

                GCode::CCWArcMove { pos, method, feed } => {
                    motion = Some(machine.arc_move(
                        pos,
                        method,
                        CircularDirection::CounterClockwise,
                        feed,
                    )?)
                }

                GCode::Dwell(_) => {} // does not actually dwell, just prints description in tui

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

                GCode::MachineCoord(pos) => motion = Some(machine.move_machine_pos(pos)),

                GCode::WorkCoord => (),

                GCode::CancelCanned => machine.cancel_canned(),

                GCode::AbsoluteMode => machine.set_positioning(Positioning::Absolute),

                GCode::IncrementalMode => machine.set_positioning(Positioning::Incremental),

                GCode::FeedMinute => machine.set_feed_mode(FeedMode::PerMinute),

                GCode::FeedRev => machine.set_feed_mode(FeedMode::PerRev),

                GCode::InitialReturn => machine.set_return_level(ReturnLevel::Initial),

                GCode::RetractReturn => machine.set_return_level(ReturnLevel::Retract),
            }
        }

        if let Some(mcode) = &mcode {
            match mcode {
                MCode::Stop | MCode::OptionalStop => {}

                MCode::SpindleFwd(s) => machine.spindle_on(CircularDirection::Clockwise, *s)?,

                MCode::SpindleRev(s) => {
                    machine.spindle_on(CircularDirection::CounterClockwise, *s)?
                }

                MCode::SpindleStop => machine.spindle_off(),

                MCode::ToolChange(t) => machine.tool_change(*t)?,

                MCode::CoolantOn => machine.set_coolant(true),

                MCode::CoolantOff => machine.set_coolant(false),

                MCode::End => machine.reset(),
            }
        }

        let mut excess_codes = Codes::new(); // storing any coord codes for parsing them altogether
        let mut excess = false; // flag for deciding later if to parse or not
        let mut codes = vec![];

        for code in block.codes() {
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
                | Code::Z(_) => {
                    excess_codes.push(code).unwrap();
                    excess = true;
                    continue; // do not add these codes to the summary
                    // as they are meant to be consumed by the excess right after this
                }
            };

            // display is only implemented for variants that will not cause any errors
            // and which do not fall through to excess codes
            codes.push(code);
        }

        if excess {
            // from excess codes, only interpolation is possible
            //
            // it is worth noting that an single block cannot be interpreted twice,
            // that is, a block will not have two interpolations,
            // because everyblock needs x, y or z, and there are no duplicates.
            //
            // these excess moves will be labelled as gcodes,
            // because these are basically gcodes lines with the 'G' code omitted as those are
            // modal.
            motion = Some(match machine.motion() {
                Motion::Rapid => {
                    let pos = excess_codes.take_partial_point();
                    gcodes.push(GCode::RapidMove(pos));
                    machine.rapid_move(pos)?
                }

                // feed would be set from the for loop, if provided
                Motion::Feed => {
                    let pos = excess_codes.take_partial_point();
                    gcodes.push(GCode::FeedMove { pos, feed: None });
                    machine.feed_move(pos, None)?
                }

                Motion::Arc(dir) => {
                    let (pos, method, feed) = excess_codes.take_circular()?;
                    match dir {
                        CircularDirection::Clockwise => {
                            gcodes.push(GCode::CWArcMove { pos, method, feed })
                        }
                        CircularDirection::CounterClockwise => {
                            gcodes.push(GCode::CCWArcMove { pos, method, feed })
                        }
                    };
                    machine.arc_move(pos, method, *dir, feed)?
                }
            });

            // single move should consume all the excess codes
            if let Some(code) = excess_codes.next() {
                return Err(InterpreterError::ExcessCode(code.prefix()));
            }
        }

        Ok(Some(BlockSummary {
            gcodes,
            mcode,
            codes,
            motion,
        }))
    }

    /// Reloads the [`Interpreter`] to start from beginning of the [`Parser`].
    pub fn reload(&mut self) {
        self.parser.reload();
        self.machine.reset();
    }

    /// **Optionally** returns the next line.
    /// as a string slice from the [`Source`](crate::source::Source).
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.parser.get_line(index)
    }

    /// Returns a reference to the [`Machine`] owned by the [`Interpreter`].
    pub fn machine(&self) -> &Machine {
        &self.machine
    }
}

/// Possible errors that can happen during executing the code.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum InterpreterError {
    /// Changing [`Machine`] state failed.
    #[error("machine rejected the last block")]
    Machine(#[from] MachineError),
    /// Parsing the next [`CodeBlock`] failed.
    #[error("parsing block failed")]
    Parser(#[from] ParserError),
    /// At least one code from a code block exists that was not consumed.
    #[error("unconsumed prefix: '{}'", *.0 as char)]
    ExcessCode(u8),
}
