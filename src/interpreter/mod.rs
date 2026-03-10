//! # Interpreter

use crate::parser::lexer::LexerError;
use crate::parser::parser::*;
use crate::{machine::*, parser::lexer::tokenize};
use std::{
    fmt::{Debug, Display},
    fs, io,
};

pub struct Interpreter {
    machine: Machine,
    blocks: Vec<String>,
}

impl Interpreter {
    /// Constructs an [`Interpreter`] from a provided [`Machine`] and `filepath` containing the
    /// code.
    ///
    /// The file at `filepath` should contain the [`Code`] to execute on the `Machine`.
    ///
    /// Returns a new `Interpreter` instance on success with file contents loaded, or
    /// [`InterpreterError`] on failure.
    ///
    /// # Errors:
    /// - [`InterpreterError::FileError`] -- An error occured when accessing the file at `filepath`.
    pub fn build(machine: Machine, filepath: &str) -> Result<Self, InterpreterError> {
        let blocks: Vec<String> = fs::read_to_string(filepath)?
            .lines()
            .map(|line| line.trim_end_matches(';').trim().to_owned())
            .collect();

        // `lines` will now be a vector of strings, with no ';' and leading or trailing whitespaces

        Ok(Interpreter { machine, blocks })
    }

    pub fn run(&mut self) -> Result<(), InterpreterError> {
        let mut current = 0;

        'lineloop: while current < self.blocks.len() {
            let block = &self.blocks[current];

            let codes = parse(tokenize(block)?)?;

            println!("Block: {block}\nCodes: {codes:?}\n");

            for code in codes {
                match code {
                    Code::G(gcode) => self.execute_gcode(gcode)?,

                    Code::M(mcode) => match mcode {
                        MCode::Stop => self.wait()?,

                        MCode::OptionalStop => self.wait()?,

                        MCode::SpindleFwd(s) => {
                            self.machine.spindle_on(CircularDirection::Clockwise, s)?
                        }

                        MCode::SpindleRev(s) => self
                            .machine
                            .spindle_on(CircularDirection::CounterClockwise, s)?,

                        MCode::SpindleStop => self.machine.spindle_off(),

                        MCode::ToolChange(t) => self.machine.tool_change(t)?,

                        MCode::CoolantOn => self.machine.set_coolant(true),

                        MCode::CoolantOff => self.machine.set_coolant(false),

                        MCode::End => {
                            println!("Program end detected");
                            break 'lineloop;
                        }
                    },

                    Code::F(f) => self.machine.set_feed(f),

                    Code::N(_) => (),

                    Code::O(_) => (),

                    Code::S(s) => self.machine.set_speed(s),

                    Code::T(t) => self.machine.set_next_tool(t),

                    Code::X(x) => {
                        self.machine
                            .move_machine(PartialPoint::new(Some(x), None, None))?
                    }

                    Code::Y(y) => {
                        self.machine
                            .move_machine(PartialPoint::new(None, Some(y), None))?
                    }

                    Code::Z(z) => {
                        self.machine
                            .move_machine(PartialPoint::new(None, None, Some(z)))?
                    }
                }
            }

            current += 1;
        }

        Ok(())
    }

    /// `Stop` M-Code helper.
    /// Waits for the user to press 'Enter' to continue.
    fn wait(&self) -> Result<(), io::Error> {
        println!("Program stopped.\nPress Enter to continue...");

        match io::stdin().read_line(&mut String::new()) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn execute_gcode(&mut self, gcode: GCode) -> Result<(), InterpreterError> {
        match gcode {
            GCode::RapidMove(pos) => self.machine.rapid_move(pos)?,

            GCode::FeedMove { p_point, f } => self.machine.feed_move(p_point, f)?,

            GCode::CWArcMove { p_point, method, f } => todo!(),
            GCode::CCWArcMove { p_point, method, f } => todo!(),

            GCode::Dwell(p) => {
                println!("Dwelling for {p} seconds.");
                let duration = std::time::Duration::from_millis((p * 1000.0) as u64);
                std::thread::sleep(duration);
            }

            GCode::XYPlane => self.machine.set_plane(Plane::XY),

            GCode::XZPlane => self.machine.set_plane(Plane::XZ),

            GCode::YZPlane => self.machine.set_plane(Plane::YZ),

            GCode::ImperialMode => self.machine.set_code_units(Unit::Imperial),

            GCode::MetricMode => self.machine.set_code_units(Unit::Metric),

            GCode::CancelCutterComp => todo!(),
            GCode::LeftCutterComp(_) => todo!(),
            GCode::RightCutterComp(_) => todo!(),
            GCode::ToolLenCompAdd(_) => todo!(),
            GCode::ToolLenCompSubtract(_) => todo!(),
            GCode::CancelLenComp => todo!(),

            GCode::MachineCoord(pos) => self.machine.move_machine_pos(pos)?,

            // always make the machine center as g54 offset
            GCode::WorkCoord => self.machine.set_work_offset(Point::new(
                self.machine.max_travels().x() / 2.0,
                self.machine.max_travels().y() / 2.0,
                0.0,
            )),

            GCode::CancelCanned => todo!(),

            GCode::AbsoluteMode => self.machine.set_positioning(Positioning::Absolute),

            GCode::IncrementalMode => self.machine.set_positioning(Positioning::Incremental),

            GCode::FeedMinute => todo!(),
            GCode::FeedRev => todo!(),
            GCode::InitialReturn => todo!(),
            GCode::RetractReturn => todo!(),
        };

        Ok(())
    }

    // have 3 levels of debug.
    // each level presents state different levels of details for states of the machine.
    // these will be same for each type of simulator:
    // text
    // 2d
    // 3d
}

/// Possible errors that can happen during Interpreting.
#[derive(Debug)]
pub enum InterpreterError {
    FileError(io::Error),
    LexerError(LexerError),
    ParserError(ParserError),
    MachineError(MachineError),
}

impl From<io::Error> for InterpreterError {
    fn from(e: io::Error) -> Self {
        Self::FileError(e)
    }
}

impl From<LexerError> for InterpreterError {
    fn from(e: LexerError) -> Self {
        Self::LexerError(e)
    }
}

impl From<ParserError> for InterpreterError {
    fn from(e: ParserError) -> Self {
        Self::ParserError(e)
    }
}

impl From<MachineError> for InterpreterError {
    fn from(e: MachineError) -> Self {
        Self::MachineError(e)
    }
}

impl Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            match self {
                Self::FileError(e) => format!(
                    "File Access Error:\nThe following error occured when accessing the 'G-Code' file:\n{e}."
                ),
                Self::LexerError(e) => format!(
                    "Lexer Error:\nThe following error occured when tokenizing the 'G-Code':\n{e}."
                ),
                Self::ParserError(e) => format!(
                    "Parser Error:\nThe following error occured when parsing the 'G-Code':\n{e}."
                ),
                Self::MachineError(e) => format!(
                    "Machine Error:\nThe following error occured when trying to change the Machine state:\n{e}."
                ),
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE: &'static str = "uniquegcodefile";

    #[test]
    fn construct_interpreter() {
        let c = "G00 X0. Y0.;\n\nG43 H1;\n";
        let m = Machine::build(Point::new(1000.0, 500.0, 500.0), Unit::default()).unwrap();

        fs::write(TEST_FILE, c).unwrap();

        let mut i = Interpreter::build(m, TEST_FILE).unwrap();

        assert_eq!(
            i.blocks,
            vec![
                String::from("G00 X0. Y0."),
                String::new(),
                String::from("G43 H1")
            ]
        );

        i.run();

        fs::remove_file(TEST_FILE).unwrap();
    }
}
