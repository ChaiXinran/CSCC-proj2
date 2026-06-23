use agentjs::{
    ast::{
        AssignmentOperator, BinaryOperator, Expression, Literal, Program, Statement, UnaryOperator,
        UpdateOperator, VariableDeclarator, VariableKind,
    },
    bytecode::{Chunk, ChunkError, Compiler, Constant, Instruction, StackAnalysis, StackEffect},
    contracts::{NativeError, ProgramCompiler},
};

#[test]
fn compiler_direct_api_accepts_a_hand_built_program() {
    let program = Program::default();
    let chunk = Compiler::new()
        .compile_program(&program)
        .expect("an empty hand-built program should compile");

    assert_eq!(chunk.instructions, [Instruction::ReturnUndefined]);
    assert!(chunk.constants.is_empty());
}

/// Postfix computed-member update (`a[b]++`) is not yet supported because the
/// VM's stack-based design requires a rotate/tuck instruction that does not
/// exist yet. Using this AST node directly (without the parser) verifies that
/// the compiler returns a clean `CompileError` rather than producing malformed
/// bytecode.
#[test]
fn compiler_rejects_unsupported_ast_without_parser_or_vm() {
    let program = Program {
        body: vec![Statement::Expression(Expression::Update {
            operator: UpdateOperator::Increment,
            prefix: false,
            argument: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("a".into())),
                property: Box::new(Expression::Identifier("b".into())),
                computed: true,
            }),
        })],
    };
    let error = Compiler::new()
        .compile_program(&program)
        .expect_err("unsupported AST must return a compile error");

    assert!(
        error.message.contains("not yet supported") || error.message.contains("unsupported"),
        "unexpected error: {}",
        error.message
    );
}

#[test]
fn shared_program_compiler_contract_delegates_to_bytecode_compiler() {
    let mut compiler = Compiler::new();
    let result = ProgramCompiler::compile_program(&mut compiler, &Program::default());

    assert!(result.is_ok());

    let unsupported = Program {
        body: vec![Statement::Expression(Expression::Update {
            operator: UpdateOperator::Increment,
            prefix: false,
            argument: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("a".into())),
                property: Box::new(Expression::Identifier("b".into())),
                computed: true,
            }),
        })],
    };
    assert!(matches!(
        ProgramCompiler::compile_program(&mut compiler, &unsupported),
        Err(NativeError::Compile(_))
    ));
}

#[test]
fn compiler_maps_all_literal_kinds_into_the_constant_pool() {
    let program = Program {
        body: vec![
            literal_statement(Literal::Undefined),
            literal_statement(Literal::Null),
            literal_statement(Literal::Boolean(true)),
            literal_statement(Literal::Number(262.0)),
            literal_statement(Literal::String("agent".into())),
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.constants,
        [
            Constant::Undefined,
            Constant::Null,
            Constant::Boolean(true),
            Constant::Number(262.0),
            Constant::String("agent".into()),
        ]
    );
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Pop,
            Instruction::Constant(1),
            Instruction::Pop,
            Instruction::Constant(2),
            Instruction::Pop,
            Instruction::Constant(3),
            Instruction::Pop,
            Instruction::Constant(4),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_returns_the_last_expression_value() {
    let program = Program {
        body: vec![
            literal_statement(Literal::Number(1.0)),
            Statement::Empty,
            literal_statement(Literal::Number(2.0)),
            Statement::Empty,
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Pop,
            Instruction::Constant(1),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_returns_undefined_when_program_has_no_expression() {
    let program = Program {
        body: vec![Statement::Empty, Statement::Empty],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert!(chunk.constants.is_empty());
    assert_eq!(chunk.instructions, [Instruction::ReturnUndefined]);
}

#[test]
fn compiler_preserves_completion_value_across_trailing_var_statements() {
    let program = Program {
        body: vec![
            literal_statement(Literal::Number(1.0)),
            variable_declaration(VariableKind::Var, "x", Some(literal(Literal::Number(2.0)))),
            Statement::Empty,
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Constant(1),
            Instruction::DeclareGlobal(2),
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.analyze_stack(), Ok(StackAnalysis { max_depth: 2 }));
}

#[test]
fn compiler_emits_each_v1_unary_instruction_after_its_operand() {
    let program = Program {
        body: vec![
            expression_statement(unary(UnaryOperator::Plus, literal(Literal::Number(1.0)))),
            expression_statement(unary(UnaryOperator::Minus, literal(Literal::Number(2.0)))),
            expression_statement(unary(UnaryOperator::Not, literal(Literal::Boolean(true)))),
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::UnaryPlus,
            Instruction::Pop,
            Instruction::Constant(1),
            Instruction::Negate,
            Instruction::Pop,
            Instruction::Constant(2),
            Instruction::LogicalNot,
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_preserves_nested_unary_evaluation_order() {
    let program = Program {
        body: vec![expression_statement(unary(
            UnaryOperator::Not,
            unary(
                UnaryOperator::Minus,
                unary(UnaryOperator::Plus, literal(Literal::Number(1.0))),
            ),
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::UnaryPlus,
            Instruction::Negate,
            Instruction::LogicalNot,
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_accepts_typeof_after_the_v2_instruction_extension() {
    let program = Program {
        body: vec![expression_statement(unary(
            UnaryOperator::TypeOf,
            literal(Literal::Number(1.0)),
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::TypeOf,
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_maps_all_v1_binary_operators() {
    let cases = [
        (BinaryOperator::Add, Instruction::Add),
        (BinaryOperator::Subtract, Instruction::Subtract),
        (BinaryOperator::Multiply, Instruction::Multiply),
        (BinaryOperator::Divide, Instruction::Divide),
        (BinaryOperator::Remainder, Instruction::Remainder),
        (BinaryOperator::StrictEqual, Instruction::StrictEqual),
        (BinaryOperator::StrictNotEqual, Instruction::StrictNotEqual),
        (BinaryOperator::LessThan, Instruction::LessThan),
        (
            BinaryOperator::LessThanOrEqual,
            Instruction::LessThanOrEqual,
        ),
        (BinaryOperator::GreaterThan, Instruction::GreaterThan),
        (
            BinaryOperator::GreaterThanOrEqual,
            Instruction::GreaterThanOrEqual,
        ),
    ];

    for (operator, expected) in cases {
        let program = Program {
            body: vec![expression_statement(binary(
                operator,
                literal(Literal::Number(1.0)),
                literal(Literal::Number(2.0)),
            ))],
        };
        let chunk = Compiler::new().compile_program(&program).unwrap();

        assert_eq!(
            chunk.instructions,
            [
                Instruction::Constant(0),
                Instruction::Constant(1),
                expected,
                Instruction::Return,
            ],
            "unexpected bytecode for {operator:?}"
        );
    }
}

#[test]
fn compiler_preserves_nested_binary_ast_evaluation_order() {
    let expression = binary(
        BinaryOperator::Add,
        literal(Literal::Number(1.0)),
        binary(
            BinaryOperator::Multiply,
            literal(Literal::Number(2.0)),
            literal(Literal::Number(3.0)),
        ),
    );
    let program = Program {
        body: vec![expression_statement(expression)],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Constant(1),
            Instruction::Constant(2),
            Instruction::Multiply,
            Instruction::Add,
            Instruction::Return,
        ]
    );
    assert_eq!(
        chunk.constants,
        [
            Constant::Number(1.0),
            Constant::Number(2.0),
            Constant::Number(3.0),
        ]
    );
}

#[test]
fn compiler_compiles_left_operand_before_right_operand() {
    let expression = binary(
        BinaryOperator::Subtract,
        unary(UnaryOperator::Minus, literal(Literal::Number(1.0))),
        unary(UnaryOperator::Plus, literal(Literal::Number(2.0))),
    );
    let program = Program {
        body: vec![expression_statement(expression)],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Negate,
            Instruction::Constant(1),
            Instruction::UnaryPlus,
            Instruction::Subtract,
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_emits_abstract_equality() {
    let program = Program {
        body: vec![expression_statement(binary(
            BinaryOperator::Equal,
            literal(Literal::Number(1.0)),
            literal(Literal::String("1".into())),
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Constant(1),
            Instruction::Equal,
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_emits_short_circuit_logical_and() {
    let program = Program {
        body: vec![expression_statement(binary(
            BinaryOperator::LogicalAnd,
            literal(Literal::Boolean(false)),
            identifier("missingName"),
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::JumpIfFalse(4),
            Instruction::Pop,
            Instruction::LoadGlobal(1),
            Instruction::Return,
        ]
    );
    assert_eq!(
        chunk.constants,
        [
            Constant::Boolean(false),
            Constant::String("missingName".into())
        ]
    );
}

#[test]
fn compiler_emits_short_circuit_logical_or() {
    let program = Program {
        body: vec![expression_statement(binary(
            BinaryOperator::LogicalOr,
            literal(Literal::Boolean(true)),
            identifier("missingName"),
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::JumpIfTrue(4),
            Instruction::Pop,
            Instruction::LoadGlobal(1),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_patches_nested_logical_expressions_to_the_next_instruction() {
    let expression = binary(
        BinaryOperator::LogicalOr,
        binary(BinaryOperator::LogicalAnd, identifier("a"), identifier("b")),
        identifier("c"),
    );
    let program = Program {
        body: vec![expression_statement(expression)],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::JumpIfFalse(4),
            Instruction::Pop,
            Instruction::LoadGlobal(1),
            Instruction::JumpIfTrue(7),
            Instruction::Pop,
            Instruction::LoadGlobal(2),
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn logical_short_circuit_keeps_one_value_on_every_path() {
    for operator in [BinaryOperator::LogicalAnd, BinaryOperator::LogicalOr] {
        let program = Program {
            body: vec![expression_statement(binary(
                operator,
                literal(Literal::Boolean(true)),
                literal(Literal::Number(262.0)),
            ))],
        };
        let chunk = Compiler::new().compile_program(&program).unwrap();
        let jump = chunk.instructions[1];

        assert_eq!(jump.jump_target(), Some(4));
        assert_eq!(chunk.instructions[2], Instruction::Pop);
        assert_eq!(chunk.instructions[4], Instruction::Return);
        assert_eq!(chunk.validate(), Ok(()));
    }
}

#[test]
fn compiler_declares_initialized_and_uninitialized_var_bindings() {
    let program = Program {
        body: vec![
            variable_declaration(
                VariableKind::Var,
                "initialized",
                Some(literal(Literal::Number(1.0))),
            ),
            variable_declaration(VariableKind::Var, "empty", None),
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.constants,
        [
            Constant::Number(1.0),
            Constant::String("initialized".into()),
            Constant::Undefined,
            Constant::String("empty".into()),
        ]
    );
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::DeclareGlobal(1),
            Instruction::Constant(2),
            Instruction::DeclareGlobal(3),
            Instruction::ReturnUndefined,
        ]
    );
}

#[test]
fn compiler_loads_identifier_expressions() {
    let program = Program {
        body: vec![
            variable_declaration(
                VariableKind::Var,
                "answer",
                Some(literal(Literal::Number(42.0))),
            ),
            expression_statement(identifier("answer")),
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.constants,
        [
            Constant::Number(42.0),
            Constant::String("answer".into()),
            Constant::String("answer".into()),
        ]
    );
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::DeclareGlobal(1),
            Instruction::LoadGlobal(2),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_stores_identifier_assignment_and_preserves_its_value() {
    let program = Program {
        body: vec![expression_statement(assignment(
            identifier("answer"),
            binary(
                BinaryOperator::Add,
                literal(Literal::Number(40.0)),
                literal(Literal::Number(2.0)),
            ),
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.constants,
        [
            Constant::Number(40.0),
            Constant::Number(2.0),
            Constant::String("answer".into()),
        ]
    );
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Constant(1),
            Instruction::Add,
            Instruction::StoreGlobal(2),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_lowers_compound_assignment_targets() {
    let identifier_program = Program {
        body: vec![expression_statement(Expression::CompoundAssignment {
            operator: AssignmentOperator::Add,
            target: Box::new(identifier("answer")),
            value: Box::new(literal(Literal::Number(2.0))),
        })],
    };
    let chunk = Compiler::new()
        .compile_program(&identifier_program)
        .unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Constant(1),
            Instruction::Add,
            Instruction::StoreGlobal(2),
            Instruction::Return,
        ]
    );

    let static_member_program = Program {
        body: vec![expression_statement(Expression::CompoundAssignment {
            operator: AssignmentOperator::Multiply,
            target: Box::new(member(identifier("object"), identifier("x"), false)),
            value: Box::new(literal(Literal::Number(3.0))),
        })],
    };
    let chunk = Compiler::new()
        .compile_program(&static_member_program)
        .unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Duplicate,
            Instruction::GetProperty(1),
            Instruction::Constant(2),
            Instruction::Multiply,
            Instruction::SetProperty(1),
            Instruction::Return,
        ]
    );

    let computed_member_program = Program {
        body: vec![expression_statement(Expression::CompoundAssignment {
            operator: AssignmentOperator::Subtract,
            target: Box::new(member(
                identifier("object"),
                literal(Literal::String("x".into())),
                true,
            )),
            value: Box::new(literal(Literal::Number(4.0))),
        })],
    };
    let chunk = Compiler::new()
        .compile_program(&computed_member_program)
        .unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Constant(1),
            Instruction::DuplicatePair,
            Instruction::GetElement,
            Instruction::Constant(2),
            Instruction::Subtract,
            Instruction::SetElement,
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiler_creates_lexical_bindings_before_initialization() {
    for (kind, expected_create) in [
        (VariableKind::Let, Instruction::CreateMutableBinding(0)),
        (VariableKind::Const, Instruction::CreateImmutableBinding(0)),
    ] {
        let program = Program {
            body: vec![variable_declaration(
                kind,
                "binding",
                Some(literal(Literal::Number(1.0))),
            )],
        };

        let chunk = Compiler::new().compile_program(&program).unwrap();
        assert_eq!(chunk.instructions[0], expected_create);
        assert!(
            chunk
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::InitializeBinding(_)))
        );
    }
}

#[test]
fn compiler_rejects_non_identifier_assignment_targets() {
    let program = Program {
        body: vec![expression_statement(assignment(
            literal(Literal::Number(1.0)),
            literal(Literal::Number(2.0)),
        ))],
    };

    let error = Compiler::new().compile_program(&program).unwrap_err();

    assert!(error.message.contains("assignment target"));
    assert!(error.message.contains("Literal"));
}

#[test]
fn compiler_emits_non_computed_member_access() {
    let program = Program {
        body: vec![expression_statement(member(
            identifier("assert"),
            identifier("sameValue"),
            false,
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.constants,
        [
            Constant::String("assert".into()),
            Constant::String("sameValue".into()),
        ]
    );
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::GetProperty(1),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_emits_callee_then_arguments_from_left_to_right() {
    let program = Program {
        body: vec![expression_statement(call(
            identifier("check"),
            vec![
                unary(UnaryOperator::Minus, literal(Literal::Number(1.0))),
                binary(
                    BinaryOperator::Add,
                    literal(Literal::Number(2.0)),
                    literal(Literal::Number(3.0)),
                ),
            ],
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Constant(1),
            Instruction::Negate,
            Instruction::Constant(2),
            Instruction::Constant(3),
            Instruction::Add,
            Instruction::Call(2),
            Instruction::Return,
        ]
    );
    assert_eq!(
        chunk.constants,
        [
            Constant::String("check".into()),
            Constant::Number(1.0),
            Constant::Number(2.0),
            Constant::Number(3.0),
        ]
    );
}

#[test]
fn compiler_emits_assert_same_value_call_shape() {
    // V3 preserves the receiver for every static member call. Native Test262
    // helpers ignore `this`, so this shape remains compatible with V1/V2.
    let program = Program {
        body: vec![expression_statement(call(
            member(identifier("assert"), identifier("sameValue"), false),
            vec![
                binary(
                    BinaryOperator::Multiply,
                    literal(Literal::Number(18.0)),
                    literal(Literal::Number(2.0)),
                ),
                literal(Literal::Number(36.0)),
                literal(Literal::String("multiplication".into())),
            ],
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::GetMethod(1),
            Instruction::Constant(2),
            Instruction::Constant(3),
            Instruction::Multiply,
            Instruction::Constant(4),
            Instruction::Constant(5),
            Instruction::CallWithThis(3),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_supports_nested_call_results_as_arguments() {
    let program = Program {
        body: vec![expression_statement(call(
            identifier("outer"),
            vec![call(
                identifier("inner"),
                vec![literal(Literal::Number(1.0))],
            )],
        ))],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::LoadGlobal(1),
            Instruction::Constant(2),
            Instruction::Call(1),
            Instruction::Call(1),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_rejects_non_identifier_static_member_property() {
    // Computed member access `object[expr]` is now supported in V3.
    // Non-identifier static member `object.literal` is still rejected because
    // only `object.identifier` is syntactically valid in non-computed position.
    let invalid_property = Program {
        body: vec![expression_statement(member(
            identifier("object"),
            literal(Literal::String("key".into())),
            false,
        ))],
    };
    let error = Compiler::new()
        .compile_program(&invalid_property)
        .unwrap_err();
    assert!(error.message.contains("non-identifier member property"));
}

#[test]
fn compiler_compiles_computed_member_access_in_v3() {
    // object["key"] is now supported and emits GetElement.
    let computed = Program {
        body: vec![expression_statement(member(
            identifier("object"),
            literal(Literal::String("key".into())),
            true,
        ))],
    };
    let chunk = Compiler::new()
        .compile_program(&computed)
        .expect("computed member access is supported in V3");
    assert!(chunk.instructions.contains(&Instruction::GetElement));
}

#[test]
fn compiler_rejects_call_argument_counts_outside_u16() {
    let arguments = vec![literal(Literal::Null); usize::from(u16::MAX) + 1];
    let program = Program {
        body: vec![expression_statement(call(identifier("many"), arguments))],
    };

    let error = Compiler::new().compile_program(&program).unwrap_err();

    assert!(error.message.contains("argument count"));
    assert!(error.message.contains("u16"));
}

#[test]
fn v1_instructions_publish_their_stack_effects() {
    assert_eq!(
        Instruction::Constant(0).stack_effect(),
        StackEffect::new(0, 1)
    );
    assert_eq!(Instruction::Add.stack_effect(), StackEffect::new(2, 1));
    assert_eq!(
        Instruction::DuplicatePair.stack_effect(),
        StackEffect::with_required(2, 0, 2)
    );
    assert_eq!(
        Instruction::StoreGlobal(0).stack_effect(),
        StackEffect::new(1, 1)
    );
    assert_eq!(
        Instruction::JumpIfFalse(0).stack_effect(),
        StackEffect::with_required(1, 0, 0)
    );
    assert_eq!(Instruction::Call(3).stack_effect(), StackEffect::new(4, 1));
    assert!(Instruction::Return.is_terminator());
    assert!(Instruction::ReturnUndefined.is_terminator());
}

#[test]
fn chunk_reports_constant_pool_overflow_without_truncating() {
    let mut chunk = Chunk {
        instructions: Vec::new(),
        constants: vec![Constant::Null; usize::from(u16::MAX) + 1],
        functions: Vec::new(),
        handlers: Vec::new(),
    };

    assert_eq!(
        chunk.add_constant(Constant::Null),
        Err(ChunkError::ConstantPoolOverflow)
    );
    assert_eq!(chunk.constants.len(), usize::from(u16::MAX) + 1);
}

#[test]
fn constant_pool_can_represent_all_v1_primitive_literals() {
    let mut chunk = Chunk::default();
    let undefined = chunk.add_constant(Constant::Undefined).unwrap();
    let null = chunk.add_constant(Constant::Null).unwrap();
    let boolean = chunk.add_constant(Constant::Boolean(true)).unwrap();
    let number = chunk.add_constant(Constant::Number(262.0)).unwrap();
    let string = chunk
        .add_constant(Constant::String("agent".into()))
        .unwrap();

    assert_eq!([undefined, null, boolean, number, string], [0, 1, 2, 3, 4]);
}

#[test]
fn chunk_emits_offsets_and_patches_conditional_jumps() {
    let mut chunk = Chunk::default();
    let condition = chunk.add_constant(Constant::Boolean(false)).unwrap();
    assert_eq!(chunk.emit(Instruction::Constant(condition)), 0);
    let jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
    assert_eq!(jump, 1);
    assert_eq!(chunk.current_offset(), 2);
    chunk.emit(Instruction::Pop);
    chunk.emit(Instruction::ReturnUndefined);

    chunk.patch_jump(jump, 2).unwrap();

    assert_eq!(chunk.instructions[jump].jump_target(), Some(2));
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn chunk_rejects_invalid_patch_locations_and_malformed_bytecode() {
    let mut chunk = Chunk::default();
    chunk.emit(Instruction::ReturnUndefined);

    assert_eq!(
        chunk.patch_jump(4, 0),
        Err(ChunkError::InvalidInstructionOffset { offset: 4 })
    );
    assert_eq!(
        chunk.patch_jump(0, 0),
        Err(ChunkError::ExpectedJumpInstruction { offset: 0 })
    );

    let invalid_constant = Chunk {
        instructions: vec![Instruction::Constant(0), Instruction::Return],
        constants: Vec::new(),
        functions: Vec::new(),
        handlers: Vec::new(),
    };
    assert_eq!(
        invalid_constant.validate(),
        Err(ChunkError::InvalidConstantIndex {
            offset: 0,
            index: 0
        })
    );

    let invalid_jump = Chunk {
        instructions: vec![Instruction::JumpIfTrue(9), Instruction::ReturnUndefined],
        constants: Vec::new(),
        functions: Vec::new(),
        handlers: Vec::new(),
    };
    assert_eq!(
        invalid_jump.validate(),
        Err(ChunkError::InvalidJumpTarget {
            offset: 0,
            target: 9
        })
    );

    assert_eq!(
        Chunk::default().validate(),
        Err(ChunkError::MissingTerminator)
    );

    let invalid_name = Chunk {
        instructions: vec![Instruction::LoadGlobal(0), Instruction::Return],
        constants: vec![Constant::Number(1.0)],
        functions: Vec::new(),
        handlers: Vec::new(),
    };
    assert_eq!(
        invalid_name.validate(),
        Err(ChunkError::ExpectedStringConstant {
            offset: 0,
            index: 0
        })
    );
}

#[test]
fn chunk_stack_analysis_detects_underflow_and_invalid_return_depths() {
    let underflow = Chunk {
        instructions: vec![Instruction::Pop, Instruction::ReturnUndefined],
        constants: Vec::new(),
        functions: Vec::new(),
        handlers: Vec::new(),
    };
    assert_eq!(
        underflow.validate(),
        Err(ChunkError::StackUnderflow {
            offset: 0,
            required: 1,
            available: 0,
        })
    );

    let empty_condition = Chunk {
        instructions: vec![Instruction::JumpIfFalse(1), Instruction::ReturnUndefined],
        constants: Vec::new(),
        functions: Vec::new(),
        handlers: Vec::new(),
    };
    assert_eq!(
        empty_condition.validate(),
        Err(ChunkError::StackUnderflow {
            offset: 0,
            required: 1,
            available: 0,
        })
    );

    let extra_value = Chunk {
        instructions: vec![
            Instruction::Constant(0),
            Instruction::Constant(0),
            Instruction::Return,
        ],
        constants: vec![Constant::Null],
        functions: Vec::new(),
        handlers: Vec::new(),
    };
    assert_eq!(
        extra_value.validate(),
        Err(ChunkError::InvalidTerminatorStackDepth {
            offset: 2,
            expected: 1,
            actual: 2,
        })
    );
}

#[test]
fn chunk_stack_analysis_rejects_inconsistent_branch_merges() {
    let chunk = Chunk {
        instructions: vec![
            Instruction::Constant(0),
            Instruction::JumpIfFalse(4),
            Instruction::Constant(1),
            Instruction::UnaryPlus,
            Instruction::Pop,
            Instruction::ReturnUndefined,
        ],
        constants: vec![Constant::Boolean(false), Constant::Number(1.0)],
        functions: Vec::new(),
        handlers: Vec::new(),
    };

    assert_eq!(
        chunk.validate(),
        Err(ChunkError::InconsistentStackDepth {
            offset: 4,
            expected: 1,
            actual: 2,
        })
    );
}

#[test]
fn complete_v1_compiler_slice_is_structurally_and_stack_valid() {
    // assert.sameValue is now compiled as GetMethod + CallWithThis, which means
    // the method and its receiver are both on the stack when arguments are pushed.
    // That adds one extra slot: max depth is 5 instead of 4.
    let program = Program {
        body: vec![
            variable_declaration(VariableKind::Var, "x", Some(literal(Literal::Number(18.0)))),
            expression_statement(call(
                member(identifier("assert"), identifier("sameValue"), false),
                vec![
                    binary(
                        BinaryOperator::Divide,
                        binary(
                            BinaryOperator::Divide,
                            identifier("x"),
                            literal(Literal::Number(2.0)),
                        ),
                        literal(Literal::Number(3.0)),
                    ),
                    literal(Literal::Number(3.0)),
                    literal(Literal::String("left-associative division".into())),
                ],
            )),
        ],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();

    assert_eq!(chunk.validate(), Ok(()));
    assert_eq!(chunk.analyze_stack(), Ok(StackAnalysis { max_depth: 5 }));
    assert_eq!(chunk.instructions.last(), Some(&Instruction::Return));
}

fn literal_statement(value: Literal) -> Statement {
    expression_statement(literal(value))
}

fn expression_statement(expression: Expression) -> Statement {
    Statement::Expression(expression)
}

fn literal(value: Literal) -> Expression {
    Expression::Literal(value)
}

fn unary(operator: UnaryOperator, argument: Expression) -> Expression {
    Expression::Unary {
        operator,
        argument: Box::new(argument),
    }
}

fn binary(operator: BinaryOperator, left: Expression, right: Expression) -> Expression {
    Expression::Binary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn identifier(name: &str) -> Expression {
    Expression::Identifier(name.into())
}

fn assignment(target: Expression, value: Expression) -> Expression {
    Expression::Assignment {
        target: Box::new(target),
        value: Box::new(value),
    }
}

fn variable_declaration(
    kind: VariableKind,
    name: &str,
    initializer: Option<Expression>,
) -> Statement {
    Statement::VariableDeclaration {
        kind,
        declarations: vec![VariableDeclarator {
            name: name.into(),
            initializer,
        }],
    }
}

fn member(object: Expression, property: Expression, computed: bool) -> Expression {
    Expression::Member {
        object: Box::new(object),
        property: Box::new(property),
        computed,
    }
}

fn call(callee: Expression, arguments: Vec<Expression>) -> Expression {
    Expression::Call {
        callee: Box::new(callee),
        arguments,
    }
}
