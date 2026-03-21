use gsim_rs::{
    lexer::{
        Lexer,
        Suffix::{Float, Int},
        Token,
    },
    source::Source,
};

#[test]
fn tokenize_source() {
    let src = Source::from_string("G01 X.0;\n%\n(COMMENT)\n/DELETEDBLOCK");

    let mut lex = Lexer::tokenize(src).unwrap();

    let mut block = lex.next().unwrap();

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
