//! A/B integration tests: source text through the Native V2 front end and
//! bytecode compiler. VM behavior is intentionally outside this test boundary.

use agentjs::{
    bytecode::{Compiler, Instruction},
    contracts::{NativeFrontend, ProgramCompiler, SourceParser},
};

fn compile(source: &str) -> agentjs::bytecode::Chunk {
    let program = NativeFrontend
        .parse_source(source)
        .unwrap_or_else(|error| panic!("front end should accept {source:?}: {error}"));
    ProgramCompiler::compile_program(&mut Compiler::new(), &program)
        .unwrap_or_else(|error| panic!("compiler should accept {source:?}: {error}"))
}

#[test]
fn compiles_if_else_source_into_valid_control_flow() {
    let chunk = compile("var x = 0; if (true) { x = 1; } else { x = 2; }");

    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::JumpIfFalse(_)))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Jump(_)))
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiles_multiple_var_declarators_from_frontend_contract() {
    let chunk = compile("var a, b = 1; b;");

    assert_eq!(
        chunk
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::DeclareGlobal(_)))
            .count(),
        2
    );
    assert_eq!(chunk.instructions.last(), Some(&Instruction::Return));
}

#[test]
fn compiles_while_break_and_continue_from_source() {
    let chunk = compile(
        "var i = 0; while (i < 4) { i = i + 1; if (i === 2) continue; if (i === 3) break; }",
    );

    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Jump(0..)))
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiles_conditional_typeof_throw_and_construct_source() {
    let chunk = compile(
        "var x = true ? typeof missing : \"unused\"; if (x !== \"undefined\") throw new Test262Error(\"bad\");",
    );

    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::TypeOfGlobal(_)))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Construct(1)))
    );
    assert!(chunk.instructions.contains(&Instruction::Throw));
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiles_the_checked_in_v2_example() {
    let chunk = compile(include_str!("../examples/v2.js"));

    assert_eq!(chunk.instructions.last(), Some(&Instruction::Return));
    assert_eq!(chunk.validate(), Ok(()));
}
