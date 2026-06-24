//! V11-A frontend smoke tests for RegExp literal static errors.

use agentjs::{
    ast::{Expression, Literal, Program, Statement},
    lexer::Lexer,
    parser::Parser,
};

fn parse(source: &str) -> Result<Program, String> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|err| err.to_string())?;
    Parser::with_source(tokens, source)
        .parse_program()
        .map_err(|err| err.to_string())
}

fn parse_ok(source: &str) -> Program {
    parse(source).unwrap_or_else(|err| panic!("expected parse success for {source:?}: {err}"))
}

fn parse_fails(source: &str) {
    assert!(parse(source).is_err(), "{source:?} should fail to parse");
}

#[test]
fn regexp_literal_preserves_pattern_and_flags() {
    let program = parse_ok(r"/\p{ASCII}+/u;");
    let Statement::Expression(Expression::Literal(Literal::RegExp { pattern, flags })) =
        &program.body[0]
    else {
        panic!("expected RegExp literal expression");
    };
    assert_eq!(pattern, r"\p{ASCII}+");
    assert_eq!(flags, "u");
}

#[test]
fn rejects_quantifiers_without_atoms() {
    for source in [r"/?/;", r"/{2}/;", r"/{2,}/;", r"/{2,3}/;"] {
        parse_fails(source);
    }
}

#[test]
fn rejects_unicode_mode_identity_and_control_escapes() {
    for source in [r"/\M/u;", r"/(?<a>\a)/u;", r"/\c0/u;"] {
        parse_fails(source);
    }
}

#[test]
fn rejects_unicode_mode_decimal_escape_residuals() {
    for source in [r"/\1/u;", r"/\8/u;", r"/\01/u;"] {
        parse_fails(source);
    }
    parse_ok(r"/(a)\1/u;");
    parse_ok(r"/\0/u;");
}

#[test]
fn rejects_unicode_mode_class_escape_ranges() {
    for source in [r"/[\d-a]/u;", r"/[\s-\d]/u;", r"/[%-\d]/u;", r"/[--\d]/u;"] {
        parse_fails(source);
    }
}

#[test]
fn rejects_quantified_lookarounds() {
    for source in [
        r"/.(?=.)?/u;",
        r"/.(?!.){2,3}/u;",
        r"/.(?<=.)?/;",
        r"/.(?<!.){2,3}/;",
    ] {
        parse_fails(source);
    }
}

#[test]
fn accepts_unicode_escape_and_property_escape_syntax() {
    parse_ok(r"/a{2}/u;");
    parse_ok(r"/\u{10ffff}/u;");
    parse_ok(r"/\p{ASCII}/u;");
    parse_ok(r"/\P{General_Category=Letter}/u;");
}
