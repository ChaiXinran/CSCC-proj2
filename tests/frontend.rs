//! Front-end (lexer + parser) acceptance tests for the Native V1 milestone.
//!
//! Per `docs/interface-spec.md`, while the compiler is unfinished the front end
//! is validated by checking that source produces a well-formed `Program` (or the
//! correct error category), without depending on bytecode, the VM, or Boa.

use agentjs::{
    NativeError,
    contracts::{NativeFrontend, Program, SourceParser},
};

fn parse(source: &str) -> Result<Program, NativeError> {
    NativeFrontend.parse_source(source)
}

fn parse_ok(source: &str) -> Program {
    parse(source).unwrap_or_else(|error| panic!("front end should parse {source:?}: {error}"))
}

/// The six Test262 expression files that Native V1 must ultimately pass. The
/// front end only needs to accept them and yield a non-empty `Program`; runtime
/// semantics are checked once the compiler and VM land.
#[test]
fn parses_target_test262_expression_files() {
    let files = [
        include_str!("../test262/test/language/expressions/multiplication/line-terminator.js"),
        include_str!("../test262/test/language/expressions/division/line-terminator.js"),
        include_str!("../test262/test/language/expressions/division/no-magic-asi.js"),
        include_str!("../test262/test/language/expressions/modulus/line-terminator.js"),
        include_str!("../test262/test/language/expressions/unary-plus/11.4.6-2-1.js"),
        include_str!("../test262/test/language/expressions/unary-minus/11.4.7-4-1.js"),
    ];

    for source in files {
        let program = parse_ok(source);
        assert!(
            !program.body.is_empty(),
            "expected statements from a target file"
        );
    }
}

/// The self-contained end-to-end expressions from `docs/native-v1-scope.md`.
/// Here we only assert they parse; their values are exercised downstream.
#[test]
fn parses_v1_end_to_end_expressions() {
    let sources = [
        "1 + 2 * 3;",
        "(1 + 2) * 3;",
        "var x = 18; x / 2 / 3;",
        "+\"\";",
        "-\"\";",
        "1 === 1;",
        "NaN === NaN;",
        "false && missingName;",
        "true || missingName;",
        "\"agent\" + 262;",
    ];

    for source in sources {
        parse_ok(source);
    }
}

#[test]
fn unterminated_string_is_a_lex_error() {
    assert!(matches!(parse("\"unterminated"), Err(NativeError::Lex(_))));
}

#[test]
fn missing_operand_is_a_parse_error() {
    assert!(matches!(parse("1 +"), Err(NativeError::Parse(_))));
}

#[test]
fn reference_to_unknown_name_parses_and_defers_to_runtime() {
    // `missingName` is syntactically valid; the ReferenceError belongs to the VM.
    let program = parse_ok("missingName;");
    assert_eq!(program.body.len(), 1);
}
