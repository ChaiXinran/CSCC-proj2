//! A/B integration tests for Native V7 bytecode safety metadata.

use agentjs::{
    bytecode::{Chunk, Compiler, Instruction},
    contracts::{NativeFrontend, ProgramCompiler, SourceParser},
    runtime::NativeContext,
    vm::{Vm, VmErrorKind},
};

fn compile(source: &str) -> Chunk {
    let program = NativeFrontend
        .parse_source(source)
        .unwrap_or_else(|error| panic!("front end should accept {source:?}: {error}"));
    let chunk = ProgramCompiler::compile_program(&mut Compiler::new(), &program)
        .unwrap_or_else(|error| panic!("compiler should accept {source:?}: {error}"));
    chunk.validate().unwrap();
    chunk
}

#[test]
fn source_lowering_publishes_cache_safe_metadata() {
    let chunk = compile("function add(a, b) { return a + b; } add(1, 2);");
    let metadata = chunk.cache_metadata().unwrap();

    assert_eq!(metadata.total_functions, 1);
    assert!(metadata.total_instructions >= chunk.instructions.len());
    assert!(metadata.total_constants >= chunk.constants.len());
    assert!(metadata.max_stack_depth >= chunk.analyze_stack().unwrap().max_depth);
}

#[test]
fn vm_rejects_invalid_chunks_before_interpretation() {
    let invalid = Chunk {
        instructions: vec![Instruction::Pop, Instruction::ReturnUndefined],
        constants: Vec::new(),
        functions: Vec::new(),
        handlers: Vec::new(),
    };

    let error = Vm::default()
        .execute_with_context(&invalid, &mut NativeContext::default())
        .unwrap_err();

    assert_eq!(error.kind, VmErrorKind::Runtime);
    assert!(error.message.contains("invalid bytecode chunk"));
    assert!(error.message.contains("requires 1 stack values"));
}
