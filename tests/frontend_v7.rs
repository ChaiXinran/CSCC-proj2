//! A-group acceptance tests for the Native V7 front end.
//!
//! Validates parser safety under adversarial inputs: deep nesting, large
//! literals, chained prefix operators, and malformed source. All tests must
//! terminate without stack overflow or process abort — parse failures must
//! return `ParseError`, not panic.
//!
//! Depth notes: MAX_PARSE_DEPTH is 50. The failure tests use 100 levels (well
//! over the limit), and the success tests use 20 levels (well under).
//!
//! Owned by: A Group (Frontend and Source Safety)
//! Owned files: `src/lexer/`, `src/parser/`, `src/ast/`

use agentjs::{
    NativeError,
    contracts::{NativeFrontend, Program, SourceParser},
    lexer::Lexer,
    parser::Parser,
};

fn parse(source: &str) -> Result<Program, NativeError> {
    NativeFrontend.parse_source(source)
}

fn parse_ok(source: &str) -> Program {
    parse(source).unwrap_or_else(|err| panic!("expected parse success: {err}"))
}

fn assert_parse_error(source: &str) {
    assert!(
        parse(source).is_err(),
        "expected parse failure for input (len={})",
        source.len()
    );
}

fn assert_parse_error_not_panic(source: &str) {
    let result = std::panic::catch_unwind(|| parse(source));
    match result {
        Ok(Err(_)) => {} // expected: parse error returned cleanly
        Ok(Ok(_)) => panic!("expected parse failure but got success"),
        Err(_) => panic!("parser panicked instead of returning a ParseError"),
    }
}

// ── Depth limits: parenthesized expressions ──────────────────────────────────

/// 100 levels of `(…)` is well above MAX_PARSE_DEPTH (50) and must return a
/// SyntaxError without overflowing the Rust call stack.
#[test]
fn deeply_nested_parens_returns_error_not_panic() {
    let depth = 100usize;
    let source = "(".repeat(depth) + "1" + &")".repeat(depth);
    assert_parse_error_not_panic(&source);
    assert_parse_error(&source);
}

/// 20 levels of `(…)` is well within MAX_PARSE_DEPTH and must parse
/// successfully.
#[test]
fn moderate_nesting_parses_ok() {
    let depth = 20usize;
    let source = "(".repeat(depth) + "1" + &")".repeat(depth);
    parse_ok(&source);
}

/// One level above MAX_PARSE_DEPTH (51) must fail; one below (49) must
/// succeed. Verifies the boundary is exact.
#[test]
fn nesting_at_limit_boundary() {
    // 49 levels is safely within the limit.
    let safe = "(".repeat(49) + "1" + &")".repeat(49);
    assert!(
        parse(&safe).is_ok(),
        "49 levels of nesting should be within MAX_PARSE_DEPTH"
    );

    // 51 levels should trip the limit.
    let over = "(".repeat(51) + "1" + &")".repeat(51);
    assert_parse_error_not_panic(&over);
    assert_parse_error(&over);
}

// ── Depth limits: prefix unary chains ────────────────────────────────────────

/// 100 `!` operators in a chain exceed MAX_PARSE_DEPTH (50) and must return a
/// SyntaxError without crashing.
#[test]
fn deeply_chained_not_returns_error_not_panic() {
    let depth = 100usize;
    let source = "!".repeat(depth) + "true";
    assert_parse_error_not_panic(&source);
    assert_parse_error(&source);
}

/// 100 `-` operators in a chain exceed MAX_PARSE_DEPTH and must return a
/// SyntaxError. Each `-` is separated by a space so the lexer tokenizes them
/// individually (consecutive `--` would be parsed as a prefix update operator).
#[test]
fn deeply_chained_negate_returns_error_not_panic() {
    let depth = 100usize;
    let source = "- ".repeat(depth) + "1";
    assert_parse_error_not_panic(&source);
    assert_parse_error(&source);
}

/// 20 `!` operators — within MAX_PARSE_DEPTH — must parse successfully.
#[test]
fn moderate_unary_chain_parses_ok() {
    let depth = 20usize;
    let source = "!".repeat(depth) + "false";
    parse_ok(&source);
}

/// 20 `-` operators — within MAX_PARSE_DEPTH — must parse successfully.
/// Each `-` is separated by a space so the lexer tokenizes them individually.
#[test]
fn moderate_negate_chain_parses_ok() {
    let depth = 20usize;
    let source = "- ".repeat(depth) + "1";
    parse_ok(&source);
}

// ── Depth limits: nested blocks ───────────────────────────────────────────────

/// 100 nested `{ { … } }` blocks exceed MAX_PARSE_DEPTH and must return a
/// SyntaxError without crashing.
#[test]
fn deeply_nested_blocks_returns_error_not_panic() {
    let depth = 100usize;
    let source = "{".repeat(depth) + &"}".repeat(depth);
    assert_parse_error_not_panic(&source);
    assert_parse_error(&source);
}

/// 20 nested blocks are within MAX_PARSE_DEPTH and must parse successfully.
#[test]
fn moderate_block_nesting_parses_ok() {
    let depth = 20usize;
    let source = "{".repeat(depth) + &"}".repeat(depth);
    parse_ok(&source);
}

// ── Large literals ────────────────────────────────────────────────────────────

/// A large array literal with 10 000 elements does not crash the parser.
#[test]
fn large_array_literal_parses_ok() {
    let elems: Vec<String> = (0..10_000).map(|i| i.to_string()).collect();
    let source = format!("[{}]", elems.join(","));
    parse_ok(&source);
}

/// A large object literal with 5 000 properties does not crash the parser.
#[test]
fn large_object_literal_parses_ok() {
    let props: Vec<String> = (0..5_000).map(|i| format!("k{i}: {i}")).collect();
    let source = format!("({{ {} }})", props.join(","));
    parse_ok(&source);
}

/// A string literal containing 100 000 ASCII characters tokenizes and parses
/// without OOM or stack overflow.
#[test]
fn large_string_literal_parses_ok() {
    let content = "a".repeat(100_000);
    let source = format!("\"{content}\"");
    parse_ok(&source);
}

/// A source file with 10 000 consecutive `var x = N;` statements parses
/// without stack overflow — statement lists are iterated, not recursed.
#[test]
fn many_sequential_statements_parse_ok() {
    let source: String = (0..10_000).map(|i| format!("var x{i} = {i};\n")).collect();
    parse_ok(&source);
}

// ── Syntax errors return ParseError, not panic ────────────────────────────────

/// Unmatched opening parenthesis.
#[test]
fn unmatched_open_paren_returns_error() {
    assert_parse_error_not_panic("(1 + 2");
    assert_parse_error("(1 + 2");
}

/// Unexpected closing brace.
#[test]
fn unexpected_closing_brace_returns_error() {
    assert_parse_error_not_panic("}");
    assert_parse_error("}");
}

/// Bare `var` keyword with no declarator.
#[test]
fn bare_var_keyword_returns_error() {
    assert_parse_error_not_panic("var");
    assert_parse_error("var");
}

/// Anonymous function declaration (name required at statement level).
#[test]
fn anonymous_function_declaration_returns_error() {
    assert_parse_error_not_panic("function() {}");
    assert_parse_error("function() {}");
}

/// `return` outside a function body.
#[test]
fn return_outside_function_returns_error() {
    assert_parse_error_not_panic("return 1;");
    assert_parse_error("return 1;");
}

/// `break` outside a loop or switch.
#[test]
fn break_outside_loop_returns_error() {
    assert_parse_error_not_panic("break;");
    assert_parse_error("break;");
}

/// `continue` outside a loop.
#[test]
fn continue_outside_loop_returns_error() {
    assert_parse_error_not_panic("continue;");
    assert_parse_error("continue;");
}

/// Invalid assignment target.
#[test]
fn invalid_assignment_target_returns_error() {
    assert_parse_error_not_panic("1 = 2;");
    assert_parse_error("1 = 2;");
}

/// Empty input is a valid program (zero statements).
#[test]
fn empty_source_parses_ok() {
    let program = parse_ok("");
    assert!(program.body.is_empty());
}

/// Whitespace-only input is a valid program.
#[test]
fn whitespace_only_parses_ok() {
    let program = parse_ok("   \n\t\r\n   ");
    assert!(program.body.is_empty());
}

// ── Parse result independence from NativeContext ──────────────────────────────

/// The same source parsed twice produces identical ASTs. This verifies that
/// the `Program` returned by the parser contains no context-local runtime IDs
/// (ObjectId, EnvironmentId, etc.) — a precondition for safe parse caching.
#[test]
fn parse_result_is_context_independent() {
    let sources = [
        "var x = 1 + 2;",
        "function f(a, b) { return a + b; }",
        "for (var i = 0; i < 10; i++) { }",
        "[1, 2, 3].forEach(function(x) { return x * 2; });",
    ];
    for source in sources {
        let program_a = parse_ok(source);
        let program_b = parse_ok(source);
        assert_eq!(
            program_a, program_b,
            "parse result must be deterministic for: {source}"
        );
    }
}

/// Tokens produced from the same source by two independent `Lexer` instances
/// are identical, confirming the lexer holds no mutable shared state.
#[test]
fn lexer_is_stateless() {
    let source = "var answer = 6 * 7;";
    let tokens_a = Lexer::new(source).tokenize().expect("tokenize a");
    let tokens_b = Lexer::new(source).tokenize().expect("tokenize b");
    assert_eq!(
        tokens_a, tokens_b,
        "lexer must produce identical tokens from identical source"
    );
}

/// Parsing the same source through two independent `Parser` instances (fed
/// from separate `Lexer` runs) yields equal `Program` values.
#[test]
fn parser_is_stateless() {
    let source = "var x = (1 + 2) * 3;";
    let toks_a = Lexer::new(source).tokenize().expect("tokenize a");
    let toks_b = Lexer::new(source).tokenize().expect("tokenize b");
    let prog_a = Parser::with_source(toks_a, source)
        .parse_program()
        .expect("parse a");
    let prog_b = Parser::with_source(toks_b, source)
        .parse_program()
        .expect("parse b");
    assert_eq!(prog_a, prog_b);
}

// ── Mixed stress: combined large + valid ──────────────────────────────────────

/// A function containing a large array initializer inside a for-loop body
/// exercises the parser under realistic "big but valid" workloads without
/// triggering the nesting depth limit.
#[test]
fn realistic_large_program_parses_ok() {
    let elements: Vec<String> = (0..1_000).map(|i| i.to_string()).collect();
    let source = format!(
        "function process() {{
            var data = [{}];
            var sum = 0;
            for (var i = 0; i < data.length; i++) {{
                sum = sum + data[i];
            }}
            return sum;
        }}",
        elements.join(", ")
    );
    parse_ok(&source);
}
