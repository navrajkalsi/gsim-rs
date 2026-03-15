//! # G-Code Parser
//!
//! A proper parser for **Geometric Code (G-Code)**, consisting of two parts:
//! - [`Lexer`](mod@lexer) -- Responsible for transforming raw text to a sequence of `Token`s.
//! - [`Parser`](mod@parser) -- Uses the output of the `Lexer` and parses it into structures of
//!   *Rust code*.

pub mod lexer;
pub mod parser;

/// Prefix **ASCII** character for codes.
pub type Prefix = u8;
/// Suffix type which pairs with prefixes expecting **an integer** type.
pub type Int = usize;
/// Suffix type which pairs with prefixes expecting **a floating** type.
pub type Float = f64;
/// Type specifying **a code group**. Only for 'G' prefix codes.
pub type Group = u8;
