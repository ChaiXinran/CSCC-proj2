//! B-group bytecode contract tests for Native V7.
//!
//! V7 treats `Chunk` validation and stack analysis as cache and VM safety
//! boundaries. These tests intentionally stay below the runtime layer.

use agentjs::{
    ast::{Expression, FunctionBody, FunctionParam, Program, Statement},
    bytecode::{
        Chunk, ChunkCacheMetadata, ChunkError, Compiler, Constant, EnvironmentCapturePolicy,
        ExceptionHandler, FunctionTemplate, HandlerKind, Instruction, StackAnalysis,
    },
};

#[test]
fn high_stack_depth_is_reported_by_chunk_analysis() {
    const DEPTH: usize = 256;

    let mut chunk = Chunk::default();
    chunk.constants.push(Constant::Null);
    for _ in 0..DEPTH {
        chunk.emit(Instruction::Constant(0));
    }
    for _ in 1..DEPTH {
        chunk.emit(Instruction::Pop);
    }
    chunk.emit(Instruction::Return);

    assert_eq!(chunk.validate(), Ok(()));
    assert_eq!(
        chunk.analyze_stack(),
        Ok(StackAnalysis { max_depth: DEPTH })
    );
}

#[test]
fn handler_stack_restore_depth_must_not_conflict_with_normal_flow() {
    let mut chunk = Chunk::default();
    chunk.emit(Instruction::Jump(2));
    chunk.emit(Instruction::ReturnUndefined);
    chunk.emit(Instruction::Pop);
    chunk.emit(Instruction::ReturnUndefined);
    chunk.handlers.push(ExceptionHandler {
        start: 0,
        end: 1,
        target: 2,
        kind: HandlerKind::Catch,
        stack_depth: 1,
        environment_depth: 0,
    });

    assert_eq!(
        chunk.validate(),
        Err(ChunkError::InconsistentStackDepth {
            offset: 2,
            expected: 1,
            actual: 0,
        })
    );
}

#[test]
fn handler_environment_restore_depth_must_not_conflict_with_normal_flow() {
    let mut chunk = Chunk::default();
    chunk.emit(Instruction::Jump(2));
    chunk.emit(Instruction::ReturnUndefined);
    chunk.emit(Instruction::PopEnvironment);
    chunk.emit(Instruction::ReturnUndefined);
    chunk.handlers.push(ExceptionHandler {
        start: 0,
        end: 1,
        target: 2,
        kind: HandlerKind::Finally,
        stack_depth: 0,
        environment_depth: 1,
    });

    assert_eq!(
        chunk.validate(),
        Err(ChunkError::InconsistentEnvironmentDepth {
            offset: 2,
            expected: 1,
            actual: 0,
        })
    );
}

#[test]
fn cache_metadata_recursively_summarizes_compiled_function_chunks() {
    let program = Program {
        body: vec![Statement::FunctionDeclaration {
            name: "add".into(),
            params: vec![
                FunctionParam::Simple("a".into()),
                FunctionParam::Simple("b".into()),
            ],
            body: FunctionBody {
                statements: vec![Statement::Return(Some(Expression::Binary {
                    operator: agentjs::ast::BinaryOperator::Add,
                    left: Box::new(Expression::Identifier("a".into())),
                    right: Box::new(Expression::Identifier("b".into())),
                }))],
                is_strict: false,
            },
            is_async: false,
            is_generator: false,
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    let metadata = chunk.cache_metadata().unwrap();

    assert_eq!(chunk.validate(), Ok(()));
    assert_eq!(metadata.total_functions, 1);
    assert!(metadata.total_instructions > chunk.instructions.len());
    assert!(metadata.total_constants >= chunk.constants.len());
    assert!(metadata.max_stack_depth >= chunk.analyze_stack().unwrap().max_depth);
}

#[test]
fn cache_metadata_rejects_invalid_nested_function_chunks() {
    let invalid_child = Chunk {
        instructions: vec![Instruction::Pop, Instruction::ReturnUndefined],
        constants: Vec::new(),
        functions: Vec::new(),
        handlers: Vec::new(),
        function_body_start: 0,
    };
    let parent = Chunk {
        instructions: vec![Instruction::CreateFunction(0), Instruction::Return],
        constants: Vec::new(),
        functions: vec![FunctionTemplate {
            name: Some("bad".into()),
            params: Vec::new(),
            rest_param: None,
            length_override: None,
            chunk: invalid_child,
            is_strict: false,
            is_async: false,
            is_generator: false,
            environment_policy: EnvironmentCapturePolicy::None,
        }],
        handlers: Vec::new(),
        function_body_start: 0,
    };

    assert_eq!(
        parent.cache_metadata(),
        Err(ChunkError::StackUnderflow {
            offset: 0,
            required: 1,
            available: 0,
        })
    );
}

#[test]
fn cache_metadata_contains_only_static_bytecode_facts() {
    let mut chunk = Chunk::default();
    chunk.constants.push(Constant::String("x".into()));
    chunk.emit(Instruction::LoadGlobal(0));
    chunk.emit(Instruction::Return);

    assert_eq!(
        chunk.cache_metadata(),
        Ok(ChunkCacheMetadata {
            max_stack_depth: 1,
            total_instructions: 2,
            total_constants: 1,
            total_functions: 0,
            total_handlers: 0,
        })
    );
}
