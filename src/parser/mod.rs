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
