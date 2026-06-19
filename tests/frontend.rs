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

/// The self-contained V2 control-flow scripts from `docs/native-v2-scope.md`.
/// The front end only needs to accept them; their values are exercised once the
/// compiler and VM gain V2 support.
#[test]
fn parses_v2_control_flow_scripts() {
    let sources = [
        "var x = 0; if (true) { x = 1; } else { x = 2; } x;",
        "var x = false ? 1 : true ? 2 : 3; x;",
        "var i = 0; while (i < 5) { i = i + 1; } i;",
        "var i = 0; while (true) { i = 1; break; } i;",
        "var i = 0; while (i < 3) { i = i + 1; if (i === 2) continue; } i;",
        "throw new Test262Error(\"expected\");",
        "var t = typeof missingName;",
        "var a, b = 1; b;",
    ];

    for source in sources {
        parse_ok(source);
    }
}

/// V2 negative cases that the front end must reject before reaching the compiler.
#[test]
fn rejects_v2_syntax_errors() {
    // `break`/`continue` outside any loop.
    assert!(matches!(parse("break;"), Err(NativeError::Parse(_))));
    assert!(matches!(parse("continue;"), Err(NativeError::Parse(_))));
    // A line terminator between `throw` and its expression.
    assert!(matches!(parse("throw\n1;"), Err(NativeError::Parse(_))));
    // `else` with no matching `if`.
    assert!(matches!(parse("else 1;"), Err(NativeError::Parse(_))));
}
