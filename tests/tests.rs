use gsim_rs::{
    lexer::{
        Lexer,
        Suffix::{Float, Int},
        Token,
    },
    parser::{GCode, Parser, PartialPoint},
    source::Source,
};

#[test]
fn tokenize_source() {
    let src = Source::from_string("G01 X.0;\n%\n(COMMENT)\n/DELETEDBLOCK");

    let mut lex = Lexer::new(src);

    let mut block = lex.next().unwrap().unwrap();

    assert_eq!(
        block.next().unwrap(),
        Token {
            prefix: b'G',
            suffix: Int(1)
        }
    );

    assert_eq!(
        block.next().unwrap(),
        Token {
            prefix: b'X',
            suffix: Float(0.0)
        }
    );

    assert!(block.next().is_none());
}

#[test]
fn parse_lexer() {
    let src = Source::from_string("G01 X.0;\n%\n(COMMENT)\n/DELETEDBLOCK");

    let lex = Lexer::new(src);

    let mut parser = Parser::new(lex);

    let mut codeblock = parser.next().unwrap().unwrap();

    assert_eq!(
        codeblock.gcodes().next().unwrap(),
        GCode::FeedMove {
            pos: PartialPoint::new(Some(0.0), None, None),
            feed: None
        }
    );

    assert!(codeblock.gcodes().next().is_none());
    assert!(codeblock.mcode().is_none());
    assert!(codeblock.codes().next().is_none());
}
