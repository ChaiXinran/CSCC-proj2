//! V10-A frontend smoke tests for BigInt, numeric separator, and Unicode tails.

use agentjs::{
    ast::{Expression, Literal, Program, Statement},
    bytecode::Compiler,
    lexer::Lexer,
    parser::Parser,
};

fn parse(source: &str) -> Program {
    let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
    Parser::with_source(tokens, source)
        .parse_program()
        .expect("parsing succeeds")
}

fn parse_fails(source: &str) {
    let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
    assert!(
        Parser::with_source(tokens, source).parse_program().is_err(),
        "{source} should fail during parsing"
    );
}

#[test]
fn bigint_literals_preserve_raw_source_text() {
    let program = parse("1_000n; 0xfn; 0b101n; 0o7n;");
    let raws: Vec<_> = program
        .body
        .iter()
        .map(|statement| match statement {
            Statement::Expression(Expression::Literal(Literal::BigInt(raw))) => raw.as_str(),
            other => panic!("expected BigInt literal expression, got {other:?}"),
        })
        .collect();

    assert_eq!(raws, ["1_000n", "0xfn", "0b101n", "0o7n"]);
}

#[test]
fn bigint_literals_do_not_compile_as_numbers() {
    let program = parse("1n;");
    let error = Compiler::new()
        .compile_program(&program)
        .expect_err("BigInt runtime semantics are not installed yet");

    assert!(error.message.contains("BigInt literal `1n`"));
}

#[test]
fn rejects_legacy_octal_like_bigint_literals() {
    for source in ["00n", "01n", "07n", "08n", "09n", "0008n", "012348n"] {
        assert!(
            Lexer::new(source).tokenize().is_err(),
            "{source} should be a SyntaxError"
        );
    }
}

#[test]
fn rejects_numeric_separator_in_leading_zero_decimal_literals() {
    for source in ["0_0", "0_1", "0_7", "0_8", "0_9", "0_0n", "0_8n", "0_9n"] {
        assert!(
            Lexer::new(source).tokenize().is_err(),
            "{source} should reject numeric separators after leading zero"
        );
    }
}

#[test]
fn accepts_other_id_start_and_continue_characters() {
    parse("var \\u2118 = 1; var \\u212E = 1; var \\u309B = 1; var \\u309C = 1;");
    parse("var \\u1885 = 1; var \\u1886 = 1;");
    parse(
        "var a\\u00B7 = 1; var a\\u0387 = 1; var a\\u1369 = 1; var a\\u1371 = 1; var a\\u19DA = 1;",
    );
}

#[test]
fn rejects_bigint_as_literal_property_name() {
    parse_fails("({ 1n: true });");
}

#[test]
fn rejects_reserved_words_spelled_with_unicode_escapes() {
    for source in [
        r"var \u0069\u0066 = 1;",
        r"var \u0074\u0072\u0075\u0065 = 1;",
        r"\u0069\u0066;",
    ] {
        parse_fails(source);
    }
}
