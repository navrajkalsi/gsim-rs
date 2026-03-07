use crate::{interpreter::Interpreter, machine::Machine, parser::parser::Point};

mod interpreter;
mod machine;
mod parser;

fn main() {
    let m = match Machine::build(Point::new(1000.0, 500.0, 500.0)) {
        Ok(m) => m,
        Err(e) => panic!("{e}"),
    };

    let mut i = match Interpreter::build(m, "gcode.nc") {
        Ok(i) => i,
        Err(e) => panic!("{e}"),
    };

    if let Err(e) = i.run() {
        println!("{e}");
    }
}
