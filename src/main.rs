use std::fs;

use crate::parser::{lexer::tokenize, parser::parse};

mod interpreter;
mod machine;
mod parser;

fn main() {
    let contents = fs::read_to_string("program.nc").unwrap();

    for line in contents.lines() {
        let line = line.trim().trim_end_matches(';');

        println!("\nParsing: {line}");

        match tokenize(line) {
            Ok(vector) => {
                println!("Tokens Ok: {vector:?}");

                match parse(vector) {
                    Ok(codes) => println!("Codes Ok: {codes:?}"),
                    Err(e) => println!("Codes Error: {e:?}"),
                }
            }
            Err(e) => println!("Tokens Error: {e:?}"),
        }
    }
}
