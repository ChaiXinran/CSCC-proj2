//! A/B integration tests: V3 source text through the Native front end and
//! bytecode compiler. VM execution is covered separately.

use agentjs::{
    bytecode::{Compiler, Instruction},
    contracts::{NativeFrontend, ProgramCompiler, SourceParser},
};

fn compile(source: &str) -> agentjs::bytecode::Chunk {
    let program = NativeFrontend
        .parse_source(source)
        .unwrap_or_else(|error| panic!("front end should accept the source: {error}"));
    let chunk = ProgramCompiler::compile_program(&mut Compiler::new(), &program)
        .unwrap_or_else(|error| panic!("compiler should accept the source: {error}"));
    chunk
        .validate()
        .unwrap_or_else(|error| panic!("compiled chunk should validate: {error}"));
    chunk
}

#[test]
fn compiles_function_declarations_calls_and_returns() {
    let chunk = compile("function add(a, b) { return a + b; } add(1, 2);");

    assert!(matches!(
        chunk.instructions.first(),
        Some(Instruction::DeclareFunction { .. })
    ));
    assert_eq!(chunk.functions.len(), 1);
    assert_eq!(chunk.functions[0].params, ["a", "b"]);
    assert!(
        chunk.functions[0]
            .chunk
            .instructions
            .contains(&Instruction::Return)
    );
}

#[test]
fn compiles_closures_objects_arrays_and_member_operations() {
    for source in [
        "function outer(x) { function inner(y) { return x + y; } return inner(2); } outer(1);",
        "var obj = { a: 1, b: 2 }; obj.a + obj['b'];",
        "var arr = [1, 2, 3]; arr[0] + arr.length;",
        "var obj = { x: 1 }; obj.x = 5; obj['x'];",
        "var obj = { value: 7, get: function () { return this.value; } }; obj.get();",
    ] {
        compile(source);
    }
}

#[test]
fn compiles_the_available_v3_test262_candidates() {
    for source in [
        include_str!("../test262/test/language/statements/function/S13_A1.js"),
        include_str!("../test262/test/language/statements/function/S13_A4_T1.js"),
        include_str!("../test262/test/language/statements/return/S12.9_A3.js"),
    ] {
        compile(source);
    }
}
