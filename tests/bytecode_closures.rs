//! B-group bytecode contract tests for Native V5.

use agentjs::{
    ast::{
        CatchClause, Expression, Literal, Program, Statement, SwitchCase, VariableDeclarator,
        VariableKind,
    },
    bytecode::{
        Chunk, ChunkError, Compiler, ExceptionHandler, HandlerKind, Instruction, StackEffect,
    },
};

fn number(value: f64) -> Expression {
    Expression::Literal(Literal::Number(value))
}

#[test]
fn v5_instructions_publish_fixed_stack_contracts() {
    assert_eq!(
        Instruction::Duplicate.stack_effect(),
        StackEffect::with_required(1, 0, 1)
    );
    assert_eq!(
        Instruction::LoadException.stack_effect(),
        StackEffect::new(0, 1)
    );
    assert_eq!(
        Instruction::InitializeBinding(0).stack_effect(),
        StackEffect::new(1, 0)
    );
    for instruction in [
        Instruction::CreateLexicalEnvironment,
        Instruction::PopEnvironment,
        Instruction::CreateMutableBinding(0),
        Instruction::CreateImmutableBinding(0),
        Instruction::EndFinally,
    ] {
        assert_eq!(instruction.stack_effect(), StackEffect::new(0, 0));
    }
}

#[test]
fn chunk_validates_handler_ranges_targets_and_stack_depths() {
    let mut valid = Chunk::default();
    valid.emit(Instruction::Constant(0));
    valid.emit(Instruction::Throw);
    valid.emit(Instruction::LoadException);
    valid.emit(Instruction::Return);
    valid
        .constants
        .push(agentjs::bytecode::Constant::Number(1.0));
    valid.handlers.push(ExceptionHandler {
        start: 0,
        end: 2,
        target: 2,
        kind: HandlerKind::Catch,
        stack_depth: 0,
        environment_depth: 0,
    });
    assert_eq!(valid.validate(), Ok(()));

    let mut invalid = valid.clone();
    invalid.handlers[0].end = 0;
    assert!(matches!(
        invalid.validate(),
        Err(ChunkError::InvalidHandlerRange { .. })
    ));

    let mut invalid = valid.clone();
    invalid.handlers[0].target = 99;
    assert!(matches!(
        invalid.validate(),
        Err(ChunkError::InvalidHandlerTarget { .. })
    ));
}

#[test]
fn compiler_lowers_switch_to_generic_comparison_and_jumps() {
    let program = Program {
        body: vec![Statement::Switch {
            discriminant: Expression::Identifier("value".into()),
            cases: vec![
                SwitchCase {
                    test: Some(number(1.0)),
                    consequent: vec![Statement::Break],
                },
                SwitchCase {
                    test: Some(number(2.0)),
                    consequent: vec![Statement::Expression(number(3.0))],
                },
                SwitchCase {
                    test: None,
                    consequent: vec![Statement::Expression(number(4.0))],
                },
            ],
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert!(chunk.instructions.contains(&Instruction::Duplicate));
    assert!(chunk.instructions.contains(&Instruction::StrictEqual));
    assert!(chunk.instructions.iter().all(
        |instruction| !matches!(instruction, Instruction::Jump(target) if *target == usize::MAX)
    ));
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiler_emits_catch_and_finally_handler_metadata() {
    let program = Program {
        body: vec![Statement::Try {
            block: vec![Statement::Throw(number(1.0))],
            handler: Some(CatchClause {
                parameter: Some("error".into()),
                body: vec![Statement::Expression(Expression::Identifier(
                    "error".into(),
                ))],
            }),
            finalizer: Some(vec![Statement::Expression(number(2.0))]),
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(chunk.handlers.len(), 2);
    assert_eq!(chunk.handlers[0].kind, HandlerKind::Catch);
    assert_eq!(chunk.handlers[1].kind, HandlerKind::Finally);
    assert!(chunk.instructions.contains(&Instruction::LoadException));
    assert!(chunk.instructions.contains(&Instruction::EndFinally));
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn lexical_bindings_are_created_before_initialization() {
    let program = Program {
        body: vec![
            Statement::VariableDeclaration {
                kind: VariableKind::Let,
                declarations: vec![VariableDeclarator {
                    name: "mutable".into(),
                    initializer: Some(number(1.0)),
                }],
            },
            Statement::VariableDeclaration {
                kind: VariableKind::Const,
                declarations: vec![VariableDeclarator {
                    name: "fixed".into(),
                    initializer: Some(number(2.0)),
                }],
            },
            Statement::Expression(Expression::Identifier("mutable".into())),
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    let create_mutable = chunk
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, Instruction::CreateMutableBinding(_)))
        .unwrap();
    let create_immutable = chunk
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, Instruction::CreateImmutableBinding(_)))
        .unwrap();
    let first_initialize = chunk
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, Instruction::InitializeBinding(_)))
        .unwrap();
    assert!(create_mutable < first_initialize);
    assert!(create_immutable < first_initialize);
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadName(_)))
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiler_rejects_hand_built_const_without_initializer() {
    let program = Program {
        body: vec![Statement::VariableDeclaration {
            kind: VariableKind::Const,
            declarations: vec![VariableDeclarator {
                name: "missing".into(),
                initializer: None,
            }],
        }],
    };
    assert!(
        Compiler::new()
            .compile_program(&program)
            .unwrap_err()
            .message
            .contains("initializer")
    );
}
