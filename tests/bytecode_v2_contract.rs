//! Compiler-team contract tests for Native V2 control-flow bytecode.
//!
//! These tests use hand-built AST and bytecode only. They do not depend on the
//! parser, VM, runtime, or Boa.

use agentjs::{
    ast::{Expression, Literal, Program, Statement, UnaryOperator},
    bytecode::{Chunk, ChunkError, Compiler, Constant, Instruction, StackEffect},
};

#[test]
fn v2_instructions_publish_their_stack_and_control_flow_contracts() {
    assert_eq!(Instruction::Jump(4).stack_effect(), StackEffect::new(0, 0));
    assert_eq!(Instruction::TypeOf.stack_effect(), StackEffect::new(1, 1));
    assert_eq!(
        Instruction::TypeOfGlobal(0).stack_effect(),
        StackEffect::new(0, 1)
    );
    assert_eq!(
        Instruction::Construct(2).stack_effect(),
        StackEffect::new(3, 1)
    );
    assert_eq!(Instruction::Throw.stack_effect(), StackEffect::new(1, 0));

    assert_eq!(Instruction::Jump(4).jump_target(), Some(4));
    assert!(!Instruction::Jump(4).has_fallthrough());
    assert!(!Instruction::Throw.has_fallthrough());
    assert!(Instruction::Throw.is_terminator());
}

#[test]
fn chunk_patches_and_validates_unconditional_jumps() {
    let mut chunk = Chunk::default();
    let jump = chunk.emit(Instruction::Jump(usize::MAX));
    chunk.emit(Instruction::ReturnUndefined);
    chunk.patch_jump(jump, 1).unwrap();

    assert_eq!(chunk.instructions[jump], Instruction::Jump(1));
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn chunk_does_not_follow_fallthrough_after_jump_or_throw() {
    let jump_over_underflow = Chunk {
        instructions: vec![
            Instruction::Jump(2),
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ],
        constants: Vec::new(),
        functions: Vec::new(),
    };
    assert_eq!(jump_over_underflow.validate(), Ok(()));

    let throw_ends_flow = Chunk {
        instructions: vec![
            Instruction::Constant(0),
            Instruction::Throw,
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ],
        constants: vec![Constant::String("expected".into())],
        functions: Vec::new(),
    };
    assert_eq!(throw_ends_flow.validate(), Ok(()));
}

#[test]
fn chunk_validates_typeof_global_name_constants() {
    let chunk = Chunk {
        instructions: vec![Instruction::TypeOfGlobal(0), Instruction::Return],
        constants: vec![Constant::Number(1.0)],
        functions: Vec::new(),
    };

    assert_eq!(
        chunk.validate(),
        Err(ChunkError::ExpectedStringConstant {
            offset: 0,
            index: 0,
        })
    );
}

#[test]
fn compiler_emits_typeof_for_values_and_names() {
    let program = Program {
        body: vec![
            Statement::Expression(Expression::Unary {
                operator: UnaryOperator::TypeOf,
                argument: Box::new(Expression::Literal(Literal::Number(1.0))),
            }),
            Statement::Expression(Expression::Unary {
                operator: UnaryOperator::TypeOf,
                argument: Box::new(Expression::Identifier("missing".into())),
            }),
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::TypeOf,
            Instruction::Pop,
            Instruction::TypeOfGlobal(1),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_keeps_block_statements_stack_neutral() {
    let program = Program {
        body: vec![Statement::Block(vec![expression(number(1.0))])],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ]
    );
}

#[test]
fn compiler_emits_if_else_with_condition_cleanup_on_both_paths() {
    let program = Program {
        body: vec![Statement::If {
            test: boolean(true),
            consequent: Box::new(Statement::Empty),
            alternate: Some(Box::new(Statement::Empty)),
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::JumpIfFalse(4),
            Instruction::Pop,
            Instruction::Jump(5),
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ]
    );
}

#[test]
fn compiler_emits_conditional_expression_with_one_value_per_path() {
    let program = Program {
        body: vec![expression(Expression::Conditional {
            test: Box::new(boolean(false)),
            consequent: Box::new(number(1.0)),
            alternate: Box::new(number(2.0)),
        })],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::JumpIfFalse(5),
            Instruction::Pop,
            Instruction::Constant(1),
            Instruction::Jump(7),
            Instruction::Pop,
            Instruction::Constant(2),
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.analyze_stack().unwrap().max_depth, 1);
}

#[test]
fn compiler_emits_while_loop_with_back_edge_and_false_cleanup() {
    let program = Program {
        body: vec![Statement::While {
            test: boolean(false),
            body: Box::new(Statement::Empty),
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::JumpIfFalse(4),
            Instruction::Pop,
            Instruction::Jump(0),
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ]
    );
}

#[test]
fn compiler_patches_break_after_false_condition_cleanup() {
    let program = Program {
        body: vec![Statement::While {
            test: boolean(true),
            body: Box::new(Statement::Break),
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::JumpIfFalse(5),
            Instruction::Pop,
            Instruction::Jump(6),
            Instruction::Jump(0),
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ]
    );
}

#[test]
fn compiler_targets_continue_at_the_current_loop_test() {
    let program = Program {
        body: vec![Statement::While {
            test: boolean(true),
            body: Box::new(Statement::Continue),
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::JumpIfFalse(5),
            Instruction::Pop,
            Instruction::Jump(0),
            Instruction::Jump(0),
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ]
    );
}

#[test]
fn compiler_rejects_loop_control_outside_a_loop() {
    for statement in [Statement::Break, Statement::Continue] {
        let error = Compiler::new()
            .compile_program(&Program {
                body: vec![statement],
            })
            .unwrap_err();
        assert!(error.message.contains("outside of a loop"));
    }
}

#[test]
fn compiler_emits_throw_as_an_abrupt_terminator() {
    let program = Program {
        body: vec![Statement::Throw(Expression::Literal(Literal::String(
            "expected".into(),
        )))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Throw,
            Instruction::ReturnUndefined,
        ]
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiler_emits_construct_callee_then_arguments_left_to_right() {
    let program = Program {
        body: vec![expression(Expression::Construct {
            callee: Box::new(Expression::Identifier("Test262Error".into())),
            arguments: vec![
                Expression::Literal(Literal::String("message".into())),
                number(2.0),
            ],
        })],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Constant(1),
            Instruction::Constant(2),
            Instruction::Construct(2),
            Instruction::Return,
        ]
    );
}

#[test]
fn nested_loop_break_is_patched_to_the_innermost_loop_end() {
    let program = Program {
        body: vec![Statement::While {
            test: boolean(true),
            body: Box::new(Statement::While {
                test: boolean(true),
                body: Box::new(Statement::Break),
            }),
        }],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    let break_target = match chunk.instructions[6] {
        Instruction::Jump(target) => target,
        other => panic!("expected inner break jump, got {other:?}"),
    };
    assert_eq!(break_target, 9);
    assert_eq!(chunk.validate(), Ok(()));
}

fn expression(expression: Expression) -> Statement {
    Statement::Expression(expression)
}

fn number(value: f64) -> Expression {
    Expression::Literal(Literal::Number(value))
}

fn boolean(value: bool) -> Expression {
    Expression::Literal(Literal::Boolean(value))
}
