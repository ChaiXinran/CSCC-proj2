//! V9-A frontend smoke tests — parser, AST, and bytecode emission.
//!
//! These tests verify that iteration and async syntax parses, compiles, and
//! reaches the native runtime contracts expected by the VM.

use agentjs::{
    BackendKind, Engine, ExecutionOptions, RuntimeConfig,
    ast::{Expression, ForBinding, FunctionLiteral, Program, Statement, VariableKind},
    bytecode::Instruction,
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
fn parses_dynamic_import_expression() {
    let prog = parse("var p = import('./x.js', { with: { type: 'json' } });");
    let Statement::VariableDeclaration { declarations, .. } = &prog.body[0] else {
        panic!("expected variable declaration");
    };
    let Expression::DynamicImport { specifier, options } =
        declarations[0].initializer.as_ref().expect("initializer")
    else {
        panic!("expected dynamic import expression");
    };
    assert!(matches!(specifier.as_ref(), Expression::Literal(_)));
    assert!(options.is_some());
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
    assert!(matches!(expr, Expression::Yield { delegate: true, .. }));
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
    assert!(matches!(
        left,
        ForBinding::Target(Expression::Identifier(_))
    ));
}

#[test]
fn parses_for_of_with_var() {
    let prog = parse("for (var i of items) {}");
    let Statement::ForOf { left, is_await, .. } = &prog.body[0] else {
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
fn for_of_uses_custom_symbol_iterator() {
    let result = run_native(
        "var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           var i = 0; \
           return { next: function() { i = i + 1; return { value: i, done: i > 3 }; } }; \
         }; \
         var s = 0; \
         for (var x of iterable) { s = s + x; } \
         s;",
    );
    assert_eq!(result.expect("custom iterator should run"), "6");
}

#[test]
fn for_of_next_getter_error_is_catchable_at_iterator_acquisition() {
    let result = run_native(
        "var gets = 0; var caught = ''; var iterable = {}; \
         iterable[Symbol.iterator] = function() { return { \
           get next() { gets = gets + 1; throw 'next-error'; } \
         }; }; \
         try { for (var value of iterable) {} } catch (error) { caught = error; } \
         gets + ':' + caught;",
    );
    assert_eq!(
        result.expect("next getter throw should be catchable"),
        "1:next-error"
    );
}

#[test]
fn for_of_break_closes_iterator_but_natural_exhaustion_does_not() {
    let result = run_native(
        "var closes = 0; \
         function make() { var i = 0; return { \
           next: function() { i++; return { value: i, done: i > 2 }; }, \
           return: function() { closes++; return {}; }, \
           [Symbol.iterator]: function() { return this; } \
         }; } \
         for (var first of make()) { break; } \
         for (var second of make()) {} \
         closes;",
    );
    assert_eq!(result.expect("break should close iterator"), "1");
}

#[test]
fn generator_next_runs_until_yield_and_completion() {
    let result = run_native(
        "function* g() { yield 1; return 2; } \
         var it = g(); \
         var a = it.next(); \
         var b = it.next(); \
         '' + a.value + '/' + a.done + '/' + b.value + '/' + b.done;",
    );
    assert_eq!(result.expect("generator should run"), "1/false/2/true");
}

#[test]
fn generator_yield_star_delegates_array_values() {
    let result = run_native(
        "function* g() { yield* [1, 2]; return 3; } \
         var it = g(); \
         var a = it.next(); var b = it.next(); var c = it.next(); \
         '' + a.value + '/' + b.value + '/' + c.value + '/' + c.done;",
    );
    assert_eq!(result.expect("yield* should delegate"), "1/2/3/true");
}

#[test]
fn generator_yield_star_uses_custom_symbol_iterator_incrementally() {
    let result = run_native(
        "var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           var i = 0; \
           return { next: function() { i = i + 1; return { value: i, done: i > 2 }; } }; \
         }; \
         function* g() { yield* iterable; return 9; } \
         var it = g(); \
         var a = it.next(); var b = it.next(); var c = it.next(); \
         '' + a.value + '/' + b.value + '/' + c.value + '/' + c.done;",
    );
    assert_eq!(result.expect("custom yield* should run"), "1/2/9/true");
}

#[test]
fn generator_return_closes_yield_star_delegate() {
    let result = run_native(
        "var calls = 0; \
         var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { \
             next: function() { return { value: 1, done: false }; }, \
             return: function() { calls = calls + 1; return { done: true }; } \
           }; \
         }; \
         function* g() { yield* iterable; return 9; } \
         var it = g(); \
         var first = it.next(); \
         var closed = it.return(42); \
         '' + first.value + '/' + calls + '/' + closed.value + '/' + closed.done;",
    );
    assert_eq!(
        result.expect("delegate return should run"),
        "1/1/undefined/true"
    );
}

#[test]
fn generator_return_rejects_non_object_delegate_return_result() {
    let result = run_native(
        "var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { \
             next: function() { return { value: 1, done: false }; }, \
             return: function() { return 1; } \
           }; \
         }; \
         function* g() { yield* iterable; } \
         var it = g(); \
         it.next(); \
         var caught = false; \
         try { it.return(42); } catch (e) { caught = true; } \
         caught;",
    );
    assert_eq!(result.expect("bad delegate return should throw"), "true");
}

#[test]
fn generator_return_passes_value_to_yield_star_delegate_return() {
    let result = run_native(
        "var received = 0; \
         var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { \
             next: function() { return { value: 1, done: false }; }, \
             return: function(value) { received = value; return { value: value + 1, done: true }; } \
           }; \
         }; \
         function* g() { yield* iterable; } \
         var it = g(); \
         it.next(); \
         var result = it.return(41); \
         '' + received + '/' + result.value + '/' + result.done;",
    );
    assert_eq!(
        result.expect("delegate return should receive value"),
        "41/42/true"
    );
}

#[test]
fn generator_return_yields_when_delegate_return_is_not_done() {
    let result = run_native(
        "var done = false; \
         var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { \
             next: function() { return { value: 1, done: false }; }, \
             return: function() { return { value: 7, done: done }; } \
           }; \
         }; \
         function* g() { yield* iterable; return 9; } \
         var it = g(); \
         it.next(); \
         var a = it.return(41); \
         done = true; \
         var b = it.return(41); \
         '' + a.value + '/' + a.done + '/' + b.value + '/' + b.done;",
    );
    assert_eq!(
        result.expect("delegate return should be resumable"),
        "7/false/7/true"
    );
}

#[test]
fn generator_throw_forwards_to_yield_star_delegate_throw() {
    let result = run_native(
        "var calls = 0; \
         var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { \
             next: function() { return { value: 1, done: false }; }, \
             throw: function(value) { calls = calls + 1; return { value: value + 1, done: false }; } \
           }; \
         }; \
         function* g() { yield* iterable; } \
         var it = g(); \
         it.next(); \
         var result = it.throw(41); \
         '' + calls + '/' + result.value + '/' + result.done;",
    );
    assert_eq!(result.expect("delegate throw should yield"), "1/42/false");
}

#[test]
fn generator_throw_resumes_after_delegate_done() {
    let result = run_native(
        "var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { \
             next: function() { return { value: 1, done: false }; }, \
             throw: function(value) { return { value: value + 1, done: true }; } \
           }; \
         }; \
         function* g() { var value = yield* iterable; return value + 1; } \
         var it = g(); \
         it.next(); \
         var result = it.throw(40); \
         '' + result.value + '/' + result.done;",
    );
    assert_eq!(
        result.expect("delegate done should resume generator"),
        "42/true"
    );
}

#[test]
fn generator_function_is_not_constructible() {
    let result = run_native(
        "function* g() { yield 1; } \
         var caught = false; \
         try { new g(); } catch (e) { caught = true; } \
         caught;",
    );
    assert_eq!(
        result.expect("constructor call should be catchable"),
        "true"
    );
}

#[test]
fn async_function_call_with_await_returns_a_promise() {
    let result = run_native(
        "var result = 0; \
         async function f() { var value = await Promise.resolve(4); return value + 1; } \
         f().then(function (value) { result = value; }); \
         result;",
    );
    assert_eq!(result.expect("async function should run"), "0");
}

#[test]
fn generator_return_resumes_finally_around_yield_star() {
    let result = run_native(
        "var finalized = false; \
         var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { next: function() { return { done: false }; } }; \
         }; \
         function* g() { \
           try { yield* iterable; } finally { finalized = true; } \
         } \
         var iterator = g(); iterator.next(); \
         var result = iterator.return(7); \
         finalized + '/' + result.value + '/' + result.done;",
    );
    assert_eq!(result.expect("return should resume finally"), "true/7/true");
}

#[test]
fn generator_throw_protocol_error_is_catchable_around_yield_star() {
    let result = run_native(
        "var closed = false; var caught = false; \
         var iterable = {}; \
         iterable[Symbol.iterator] = function() { \
           return { \
             next: function() { return { done: false }; }, \
             return: function() { closed = true; return {}; } \
           }; \
         }; \
         function* g() { \
           try { yield* iterable; } \
           catch (error) { caught = error.name === 'TypeError'; return 9; } \
         } \
         var iterator = g(); iterator.next(); \
         var result = iterator.throw('boom'); \
         closed + '/' + caught + '/' + result.value + '/' + result.done;",
    );
    assert_eq!(
        result.expect("throw protocol error should enter catch"),
        "true/true/9/true"
    );
}

#[test]
fn for_await_emits_async_iterator_opcodes() {
    let chunk = compile("async function f() { for await (var x of values) { x; } }");
    let body = &chunk.functions[0].chunk;
    assert!(
        body.instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::GetAsyncIterator))
    );
    assert!(
        body.instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::AsyncIteratorNext))
    );
    assert_eq!(Instruction::GetAsyncIterator.stack_effect().pops, 1);
    assert_eq!(Instruction::GetAsyncIterator.stack_effect().pushes, 1);
    assert_eq!(Instruction::AsyncIteratorNext.stack_effect().pops, 1);
    assert_eq!(Instruction::AsyncIteratorNext.stack_effect().pushes, 2);
}
