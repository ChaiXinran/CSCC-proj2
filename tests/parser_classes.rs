//! V13-B parser and lowering checks for class and dynamic-import syntax.

use agentjs::{bytecode::Instruction, lexer::Lexer, parser::Parser};

fn compile(source: &str) -> agentjs::bytecode::SharedChunk {
    let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
    let program = Parser::with_source(tokens, source)
        .parse_program()
        .expect("parsing succeeds");
    agentjs::bytecode::Compiler::new()
        .compile_program(&program)
        .expect("compilation succeeds")
}

#[test]
fn dynamic_import_with_options_lowers_to_the_runtime_instruction() {
    let chunk = compile("import('./entry.js', { with: { type: 'json' } })");
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DynamicImport))
    );
}

#[test]
fn async_arrow_concise_body_keeps_await_context_for_dynamic_import() {
    let source = "async () => await import('./fixture.js')";
    let tokens = agentjs::lexer::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    agentjs::parser::Parser::with_source(tokens, source)
        .parse_program()
        .expect("async concise body parses");
}

#[test]
fn module_await_parses_as_a_nested_unary_expression() {
    let source = "void await 1; typeof await 2; let value = 1 + await 3;";
    let tokens = agentjs::lexer::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    agentjs::parser::Parser::with_source(tokens, source)
        .parse_module()
        .expect("nested module await parses");
}

#[test]
fn private_name_is_not_accepted_as_a_standalone_expression() {
    let tokens = Lexer::new("#x").tokenize().expect("lexing succeeds");
    let error = Parser::with_source(tokens, "#x")
        .parse_program()
        .expect_err("private names require a class member access");
    assert!(!error.message.is_empty());
}
