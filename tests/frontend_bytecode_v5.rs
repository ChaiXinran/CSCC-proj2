//! A/B integration tests for Native V5 source lowering.

use agentjs::{
    bytecode::{Compiler, HandlerKind, Instruction},
    contracts::{NativeFrontend, ProgramCompiler, SourceParser},
};

fn compile(source: &str) -> agentjs::bytecode::Chunk {
    let program = NativeFrontend
        .parse_source(source)
        .unwrap_or_else(|error| panic!("front end should accept {source:?}: {error}"));
    let chunk = ProgramCompiler::compile_program(&mut Compiler::new(), &program)
        .unwrap_or_else(|error| panic!("compiler should accept {source:?}: {error}"));
    chunk.validate().unwrap();
    chunk
}

#[test]
fn compiles_try_catch_finally_from_source() {
    let chunk = compile(
        "try { throw 1; } catch (error) { let caught = error; } finally { var done = true; }",
    );
    assert!(
        chunk
            .handlers
            .iter()
            .any(|handler| handler.kind == HandlerKind::Catch)
    );
    assert!(
        chunk
            .handlers
            .iter()
            .any(|handler| handler.kind == HandlerKind::Finally)
    );
    assert!(chunk.instructions.contains(&Instruction::LoadException));
    assert!(chunk.instructions.contains(&Instruction::EndFinally));
}

#[test]
fn compiles_switch_fallthrough_and_break_from_source() {
    let chunk = compile(
        "switch (value) { case 1: result = 1; break; case 2: result = 2; default: result = 3; }",
    );
    assert!(chunk.instructions.contains(&Instruction::Duplicate));
    assert!(chunk.instructions.contains(&Instruction::StrictEqual));
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiles_lexical_declarations_in_two_phases() {
    let chunk = compile("{ let value; const fixed = 1; value = fixed; }");
    assert!(
        chunk
            .instructions
            .contains(&Instruction::CreateLexicalEnvironment)
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::CreateMutableBinding(_)))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::CreateImmutableBinding(_)))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::InitializeBinding(_)))
            .count()
            >= 2
    );
}

#[test]
fn break_and_continue_unwind_nested_lexical_environments() {
    let switch = compile("switch (value) { case 1: { let scoped = 1; break; } default: value; }");
    assert_eq!(switch.validate(), Ok(()));

    let loop_chunk = compile("while (true) { { const scoped = 1; continue; } }");
    assert_eq!(loop_chunk.validate(), Ok(()));
}
