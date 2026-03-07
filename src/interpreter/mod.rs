//! # Interpreter

use crate::machine::*;
use std::str::Lines;

pub struct Interpreter {
    machine: Machine,
    lines: Lines<'static>,
}

impl Interpreter {
    pub fn new(machine: Machine, lines: Lines<'static>) -> Self {
        Self { machine, lines }
    }
}
