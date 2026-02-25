//! # Lexer
//!
//! The Lexer is responsible for converting a raw **ASCII G-code line**,
//! into usable [`Token`]s that can then be parsed.
//!
//! Reference used: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)

/// A token represents a single **G-code field**.
///
/// A field begins with a single alphabet (*the prefix*),
/// followed by a numeric *suffix* (an integer or floating point).
///
/// Any whitespace is ignored.
///
/// # Examples
/// - G01
/// - M06
/// - X1.2345
#[derive(PartialEq, Debug)]
pub struct Token {
    /// 'G' in G01
    pub prefix: u8,
    /// '1.2345' in X1.2345
    pub suffix: Suffix,
}

/// Represents a numeric suffix for a [`Token`].
///
/// A G-code field can contain an integer or a floating point.
#[derive(PartialEq, Debug)]
pub enum Suffix {
    Int(isize),
    Float(f64),
}

/// Possible errors that can happen during tokenization.
#[derive(PartialEq, Debug)]
pub enum LexerError {
    /// A non-ASCII char is detected.
    IllegalChar,
    /// An invalid ASCII G-code char is detected.
    NonUsableChar,
    /// Semicolon (`;`) is detected.
    EOBFound,
    /// No numeric suffix found.
    NoSuffix,
    /// Error while parsing numeric suffix.
    ParseSuffix,
}

/// Tokenizes a single G-code block.
///
/// Accepts a string slice containing **only ASCII** characters and without a semicolon (`;`),
/// known as `End of Block (EOB)`.
///
/// Returns a vector of [`Token`]s, which may be *empty*, on success or an [`LexerError`] on failure.
///
/// # Errors
/// - [`LexerError::IllegalChar`] -- The block contains a non-ASCII character.
/// - [`LexerError::NonUsableChar`] -- The block contains an invalid G-code character which is an ASCII
/// character.
/// - [`LexerError::EOBFound`] -- The block contains a semicolon.
/// - [`LexerError::NoSuffix`] -- The block contains a word that has no numeric suffix.
/// - [`LexerError::ParseSuffix`] -- The numeric suffix of the block cannot be parse into a type.
///
/// # Examples
/// - Most common usage:
/// ```
/// # use gsim_rs::parser::lexer::*;
/// # fn main() -> Result<(), LexerError> {
/// assert_eq!(tokenize("G00 X.0 Y.0 Z-5.")?, vec![
///     Token {prefix: b'G', suffix: Suffix::Int(0)},
///     Token {prefix: b'X', suffix: Suffix::Float(0.0)},
///     Token {prefix: b'Y', suffix: Suffix::Float(0.0)},
///     Token {prefix: b'Z', suffix: Suffix::Float(-5.0)},
/// ]);
/// # Ok(())
/// # }
/// ```
///
/// - Rare usage, using lowercase and no whitespace:
/// ```
/// # use gsim_rs::parser::lexer::*;
/// # fn main() -> Result<(), LexerError> {
/// assert_eq!(tokenize("g00x.0y.0z-5.")?, vec![
///     Token {prefix: b'G', suffix: Suffix::Int(0)},
///     Token {prefix: b'X', suffix: Suffix::Float(0.0)},
///     Token {prefix: b'Y', suffix: Suffix::Float(0.0)},
///     Token {prefix: b'Z', suffix: Suffix::Float(-5.0)},
/// ]);
/// # Ok(())
/// # }
/// ```
pub fn tokenize(block: &str) -> Result<Vec<Token>, LexerError> {
    let mut tokens = vec![];

    let mut prefix = None;
    let mut suffix_str: Option<String> = None; // owned string of numeric value, but in chars

    for byte in block.trim().as_bytes() {
        if !byte.is_ascii() {
            return Err(LexerError::IllegalChar);
        } else if byte.is_ascii_control() {
            return Err(LexerError::NonUsableChar);
        } else if *byte == b';' {
            return Err(LexerError::EOBFound);
        }

        // start new word
        if prefix.is_none() {
            if *byte == b' ' {
                continue; // try to start new on next()
            } else if byte.is_ascii_alphabetic() {
                prefix = Some(byte.to_ascii_uppercase()); // read numeric on next()
                continue;
            } else {
                return Err(LexerError::NonUsableChar);
            }
        }

        // suffix complete
        if *byte == b' ' || byte.is_ascii_alphabetic() {
            let suffix = parse_suffix(suffix_str)?;

            // add to vec and read next
            tokens.push(Token {
                prefix: prefix.expect("Control should reach here only if prefix is Some."),
                suffix,
            });

            // reset for the next word and use the alphabet, if found
            prefix = if byte.is_ascii_alphabetic() {
                Some(byte.to_ascii_uppercase())
            } else {
                None
            };
            suffix_str = None;

            // read suffix
        } else if byte.is_ascii_digit() || *byte == b'.' || *byte == b'-' {
            if suffix_str.is_none() {
                suffix_str = Some(String::from(*byte as char));
            } else if *byte == b'-' {
                // cannot have - in the middle of suffix
                return Err(LexerError::NonUsableChar);
            } else {
                suffix_str
                    .as_mut()
                    .expect("None variant for suffix has already been handled.")
                    .push(*byte as char);
            }

            // invalid suffix
        } else {
            return Err(LexerError::NonUsableChar);
        }
    }

    // parse last suffix, if present
    if prefix.is_some() {
        let suffix = parse_suffix(suffix_str)?;

        // add to vec and read next
        tokens.push(Token {
            prefix: prefix.expect("Control should reach here only if prefix is Some."),
            suffix,
        });
    }

    Ok(tokens)
}

/// Suffix parsing helper.
///
/// Accepts an `Option` containing a suffix String.
///
/// Returns a [`Suffix`], with the appropriate variant, on success and [`LexerError`] on failure.
fn parse_suffix(suffix_str: Option<String>) -> Result<Suffix, LexerError> {
    if suffix_str.is_none() {
        return Err(LexerError::NoSuffix);
    }

    let suffix_str = suffix_str.expect("None variant of suffix has already been handled.");

    if suffix_str.is_empty() {
        return Err(LexerError::NoSuffix);
    }

    // parse as float
    if suffix_str.contains('.') {
        return match suffix_str.parse::<f64>() {
            Ok(s) => Ok(Suffix::Float(s)),
            Err(_) => Err(LexerError::ParseSuffix),
        };
    }

    // try to parse as int
    match suffix_str.parse::<isize>() {
        Ok(s) => Ok(Suffix::Int(s)),
        Err(_) => Err(LexerError::ParseSuffix),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Tests the most common format of G-code.
    // Tests with all alphabets in uppercase, both with and without whitespace between them.
    fn tokenize_upper() {
        // with whitespace
        let whitespace = String::from("G01 X0.0 Y.0 Z-1.");
        let tokens = tokenize(&whitespace);
        let expected = vec![
            Token {
                prefix: b'G',
                suffix: Suffix::Int(1),
            },
            Token {
                prefix: b'X',
                suffix: Suffix::Float(0.0),
            },
            Token {
                prefix: b'Y',
                suffix: Suffix::Float(0.0),
            },
            Token {
                prefix: b'Z',
                suffix: Suffix::Float(-1.0),
            },
        ];

        assert_eq!(tokens.unwrap(), expected);

        // no whitespace
        let no_whitespace = String::from("G01X0.0Y.0Z-1.");
        let tokens = tokenize(&no_whitespace);

        assert_eq!(tokens.unwrap(), expected);
    }

    #[test]
    // Tests with all alphabets in lowercase, both with and without whitespace between them.
    fn tokenize_lower() {
        // with whitespace
        let whitespace = String::from("g01 x0.0 y.0 z-1.");
        let tokens = tokenize(&whitespace);
        let expected = vec![
            Token {
                prefix: b'G',
                suffix: Suffix::Int(1),
            },
            Token {
                prefix: b'X',
                suffix: Suffix::Float(0.0),
            },
            Token {
                prefix: b'Y',
                suffix: Suffix::Float(0.0),
            },
            Token {
                prefix: b'Z',
                suffix: Suffix::Float(-1.0),
            },
        ];

        assert_eq!(tokens.unwrap(), expected);

        // no whitespace
        let no_whitespace = String::from("g01x0.0y.0z-1.");
        let tokens = tokenize(&no_whitespace);

        assert_eq!(tokens.unwrap(), expected);
    }

    #[test]
    // Tests with semicolon
    fn tokenize_semicolon() {
        assert_eq!(tokenize("G01;").unwrap_err(), LexerError::EOBFound);
    }

    #[test]
    // Test non usable ASCII character
    fn tokenize_non_usable() {
        assert_eq!(tokenize("G53 {").unwrap_err(), LexerError::NonUsableChar);
        assert_eq!(
            tokenize("G53 X1-1.").unwrap_err(),
            LexerError::NonUsableChar
        );
    }

    #[test]
    // Test non-ASCII character
    fn tokenize_non_ascii() {
        assert_eq!(tokenize("G53 नमस्ते").unwrap_err(), LexerError::IllegalChar);
    }

    #[test]
    // Test a field with no suffix
    fn tokenize_no_suffix() {
        assert_eq!(tokenize("G53 X0.Y Z-1.").unwrap_err(), LexerError::NoSuffix);
    }

    #[test]
    // Test an invalid suffix.
    fn tokenize_invalid_suffix() {
        assert_eq!(tokenize("G53 X.").unwrap_err(), LexerError::ParseSuffix);
    }
}
