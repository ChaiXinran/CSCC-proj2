//! V9-A frontend smoke tests — parser, AST, and bytecode emission.
//!
//! These tests verify that V9-A syntax parses successfully and compiles to the
//! expected opcodes. Runtime failures (V9-B territory) are checked by verifying
//! that the produced runtime error message mentions "V9-B pending".

use agentjs::{
    ast::{Expression, ForBinding, FunctionLiteral, Program, Statement, VariableKind},
    bytecode::Instruction,
    BackendKind, Engine, ExecutionOptions, RuntimeConfig,
};

fn parse(source: &str) -> Program {
    let tokens = agentjs::lexer::Lexer::new(source)
        .tokenize()
        .expect("lexing succeeds");
    agentjs::parser::Parser::new(tokens)
        .parse_program()
        .expect("parsing succeeds")
}

fn compile(source: &str) -> agentjs::bytecode::Chunk {
    let prog = parse(source);
    agentjs::bytecode::Compiler::new()
        .compile_program(&prog)
        .expect("compilation succeeds")
}

fn run_native(source: &str) -> Result<String, String> {
    let engine = Engine::with_backend(BackendKind::Native, RuntimeConfig::default());
    engine
        .execute(source, ExecutionOptions::default())
        .map(|r| r.value)
        .map_err(|e| format!("{e:?}"))
}

// ---------------------------------------------------------------------------
// Generator syntax
// ---------------------------------------------------------------------------

#[test]
fn parses_generator_function_declaration() {
    let prog = parse("function* gen() { yield 1; }");
    let Statement::FunctionDeclaration {
        name,
        is_generator,
        is_async,
        ..
    } = &prog.body[0]
    else {
        panic!("expected FunctionDeclaration");
    };
    assert_eq!(name, "gen");
    assert!(*is_generator);
    assert!(!is_async);
}

#[test]
fn parses_generator_function_expression() {
    let prog = parse("var g = function*() { yield 42; };");
    let Statement::VariableDeclaration { declarations, .. } = &prog.body[0] else {
        panic!();
    };
    let init = declarations[0].initializer.as_ref().unwrap();
    let Expression::Function(FunctionLiteral { is_generator, .. }) = init else {
        panic!("expected function expression");
    };
    assert!(*is_generator);
}

#[test]
fn parses_yield_expression_with_argument() {
    let prog = parse("function* g() { yield 1 + 2; }");
    let Statement::FunctionDeclaration { body, .. } = &prog.body[0] else {
        panic!();
    };
    let Statement::Expression(expr) = &body.statements[0] else {
        panic!("expected expression statement");
    };
    assert!(matches!(
        expr,
        Expression::Yield {
            argument: Some(_),
            delegate: false
        }
    ));
}

#[test]
fn parses_yield_star() {
    let prog = parse("function* g() { yield* [1, 2]; }");
    let Statement::FunctionDeclaration { body, .. } = &prog.body[0] else {
        panic!();
    };
    let Statement::Expression(expr) = &body.statements[0] else {
        panic!();
    };
    assert!(matches!(
        expr,
        Expression::Yield {
            delegate: true,
            ..
        }
    ));
}

#[test]
fn parses_yield_without_argument() {
    let prog = parse("function* g() { yield; }");
    let Statement::FunctionDeclaration { body, .. } = &prog.body[0] else {
        panic!();
    };
    let Statement::Expression(expr) = &body.statements[0] else {
        panic!();
    };
    assert!(matches!(
        expr,
        Expression::Yield {
            argument: None,
            delegate: false
        }
    ));
}

// ---------------------------------------------------------------------------
// Async syntax
// ---------------------------------------------------------------------------

#[test]
fn parses_async_function_declaration() {
    let prog = parse("async function foo() { await bar(); }");
    let Statement::FunctionDeclaration {
        name,
        is_async,
        is_generator,
        ..
    } = &prog.body[0]
    else {
        panic!("expected FunctionDeclaration");
    };
    assert_eq!(name, "foo");
    assert!(*is_async);
    assert!(!is_generator);
}

#[test]
fn parses_async_function_expression() {
    let prog = parse("var f = async function() { await p; };");
    let Statement::VariableDeclaration { declarations, .. } = &prog.body[0] else {
        panic!();
    };
    let init = declarations[0].initializer.as_ref().unwrap();
    let Expression::Function(FunctionLiteral { is_async, .. }) = init else {
        panic!("expected function expression");
    };
    assert!(*is_async);
}

#[test]
fn parses_await_expression() {
    let prog = parse("async function f() { await somePromise; }");
    let Statement::FunctionDeclaration { body, .. } = &prog.body[0] else {
        panic!();
    };
    let Statement::Expression(expr) = &body.statements[0] else {
        panic!();
    };
    assert!(matches!(expr, Expression::Await(_)));
}

#[test]
fn parses_async_generator_function_declaration() {
    let prog = parse("async function* ag() { yield await fetch(); }");
    let Statement::FunctionDeclaration {
        is_async,
        is_generator,
        ..
    } = &prog.body[0]
    else {
        panic!();
    };
    assert!(*is_async && *is_generator);
}

// ---------------------------------------------------------------------------
// for-of syntax
// ---------------------------------------------------------------------------

#[test]
fn parses_for_of_with_let() {
    let prog = parse("for (let x of arr) { }");
    let Statement::ForOf {
        left,
        is_await,
        body,
        ..
    } = &prog.body[0]
    else {
        panic!("expected ForOf");
    };
    assert!(!is_await);
    assert!(matches!(
        left,
        ForBinding::Declaration {
            kind: VariableKind::Let,
            ..
        }
    ));
    assert!(matches!(body.as_ref(), Statement::Block(_)));
}

#[test]
fn parses_for_of_with_existing_target() {
    let prog = parse("for (x of arr) { }");
    let Statement::ForOf { left, .. } = &prog.body[0] else {
        panic!("expected ForOf");
    };
    assert!(matches!(left, ForBinding::Target(Expression::Identifier(_))));
}

#[test]
fn parses_for_of_with_var() {
    let prog = parse("for (var i of items) {}");
    let Statement::ForOf {
        left,
        is_await,
        ..
    } = &prog.body[0]
    else {
        panic!("expected ForOf");
    };
    assert!(!is_await);
    assert!(matches!(
        left,
        ForBinding::Declaration {
            kind: VariableKind::Var,
            ..
        }
    ));
}

#[test]
fn parses_for_await_of() {
    let prog = parse("async function f() { for await (const x of gen()) {} }");
    let Statement::FunctionDeclaration { body, .. } = &prog.body[0] else {
        panic!();
    };
    let Statement::ForOf { is_await, .. } = &body.statements[0] else {
        panic!("expected ForOf");
    };
    assert!(*is_await);
}

// ---------------------------------------------------------------------------
// Bytecode emission
// ---------------------------------------------------------------------------

#[test]
fn for_of_emits_get_iterator_and_iterator_next() {
    let chunk = compile("for (let x of arr) { x; }");
    let has_get_iter = chunk
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::GetIterator));
    let has_iter_next = chunk
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::IteratorNext));
    assert!(has_get_iter, "expected GetIterator opcode");
    assert!(has_iter_next, "expected IteratorNext opcode");
}

#[test]
fn generator_function_declaration_emits_declare_function() {
    let chunk = compile("function* gen() { yield 1; }");
    let has_declare = chunk
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::DeclareFunction { .. }));
    assert!(has_declare, "expected DeclareFunction opcode");
}

#[test]
fn async_function_declaration_emits_declare_function() {
    let chunk = compile("async function f() { }");
    let has_declare = chunk
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::DeclareFunction { .. }));
    assert!(has_declare, "expected DeclareFunction opcode");
}

// ---------------------------------------------------------------------------
// Runtime exposure (V9-B territory — verify stub error messages)
// ---------------------------------------------------------------------------

#[test]
fn for_of_over_array_runs_successfully() {
    // Basic array for-of now executes end-to-end through the iterator protocol.
    let result = run_native("var s = 0; for (let x of [1, 2, 3]) { s = s + x; } s");
    assert_eq!(result.expect("for-of should succeed"), "6");
}

#[test]
fn generator_call_fails_at_runtime_with_pending_message() {
    let result = run_native("function* g() { yield 1; } g();");
    let err = result.expect_err("should fail at runtime");
    assert!(
        err.contains("V9-B") || err.contains("not yet implemented"),
        "unexpected error: {err}"
    );
}

#[test]
fn async_function_call_with_await_fails_at_runtime_with_pending_message() {
    // An async function body that contains `await` triggers the V9-B stub.
    let result = run_native("async function f() { await 1; } f();");
    let err = result.expect_err("should fail at runtime");
    assert!(
        err.contains("V9-B") || err.contains("not yet implemented"),
        "unexpected error: {err}"
    );
}
