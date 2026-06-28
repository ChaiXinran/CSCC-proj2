//! B-group contract tests for Native V4 bytecode.
//!
//! Opcode tests use hand-written chunks. Compiler tests use hand-written ASTs;
//! none of these tests depend on the parser, runtime object model, or VM.

use agentjs::{
    ast::{
        ArrayElement, BinaryOperator, Expression, FunctionBody, FunctionParam, Literal,
        ObjectProperty, Program, PropertyName, Statement, UnaryOperator,
    },
    bytecode::{Chunk, ChunkError, Compiler, Constant, Instruction, StackEffect},
};

fn string(chunk: &mut Chunk, value: &str) -> u16 {
    chunk.add_constant(Constant::String(value.into())).unwrap()
}

fn number(chunk: &mut Chunk, value: f64) -> u16 {
    chunk.add_constant(Constant::Number(value)).unwrap()
}

#[test]
fn v4_instructions_publish_their_stack_effects() {
    assert_eq!(
        Instruction::ObjectCreateEmpty.stack_effect(),
        StackEffect::new(0, 1)
    );
    assert_eq!(
        Instruction::ArrayCreateSparse(3).stack_effect(),
        StackEffect::new(0, 1)
    );

    for instruction in [
        Instruction::DefineDataProperty(0),
        Instruction::DefineGetter(0),
        Instruction::DefineSetter(0),
        Instruction::SetObjectPrototype,
        Instruction::DefineElement(2),
    ] {
        assert_eq!(
            instruction.stack_effect(),
            StackEffect::with_required(2, 1, 0)
        );
    }

    assert_eq!(
        Instruction::DeleteProperty(0).stack_effect(),
        StackEffect::new(1, 1)
    );
    for instruction in [
        Instruction::DeleteElement,
        Instruction::HasProperty,
        Instruction::InstanceOf,
    ] {
        assert_eq!(instruction.stack_effect(), StackEffect::new(2, 1));
    }
}

#[test]
fn chunk_validates_v4_object_definition_sequence() {
    let mut chunk = Chunk::default();
    let property = string(&mut chunk, "answer");
    let value = number(&mut chunk, 42.0);

    chunk.emit(Instruction::ObjectCreateEmpty);
    chunk.emit(Instruction::Constant(value));
    chunk.emit(Instruction::DefineDataProperty(property));
    chunk.emit(Instruction::Return);

    assert_eq!(chunk.validate(), Ok(()));
    assert_eq!(chunk.analyze_stack().unwrap().max_depth, 2);
}

#[test]
fn chunk_validates_sparse_array_definition_sequence() {
    let mut chunk = Chunk::default();
    let first = number(&mut chunk, 1.0);
    let third = number(&mut chunk, 3.0);

    chunk.emit(Instruction::ArrayCreateSparse(3));
    chunk.emit(Instruction::Constant(first));
    chunk.emit(Instruction::DefineElement(0));
    chunk.emit(Instruction::Constant(third));
    chunk.emit(Instruction::DefineElement(2));
    chunk.emit(Instruction::Return);

    assert_eq!(chunk.validate(), Ok(()));
    assert_eq!(chunk.analyze_stack().unwrap().max_depth, 2);
}

#[test]
fn chunk_validates_v4_property_queries_and_deletion() {
    let mut named_delete = Chunk::default();
    let property = string(&mut named_delete, "x");
    named_delete.emit(Instruction::ObjectCreateEmpty);
    named_delete.emit(Instruction::DeleteProperty(property));
    named_delete.emit(Instruction::Return);
    assert_eq!(named_delete.validate(), Ok(()));

    for instruction in [
        Instruction::DeleteElement,
        Instruction::HasProperty,
        Instruction::InstanceOf,
    ] {
        let mut chunk = Chunk::default();
        let key = string(&mut chunk, "x");
        chunk.emit(Instruction::Constant(key));
        chunk.emit(Instruction::ObjectCreateEmpty);
        chunk.emit(instruction);
        chunk.emit(Instruction::Return);
        assert_eq!(chunk.validate(), Ok(()), "{instruction:?}");
    }
}

#[test]
fn chunk_validates_object_prototype_definition_sequence() {
    let mut chunk = Chunk::default();
    chunk.emit(Instruction::ObjectCreateEmpty);
    chunk.emit(Instruction::ObjectCreateEmpty);
    chunk.emit(Instruction::SetObjectPrototype);
    chunk.emit(Instruction::Return);

    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn v4_name_operands_require_existing_string_constants() {
    for instruction in [
        Instruction::DefineDataProperty(0),
        Instruction::DefineGetter(0),
        Instruction::DefineSetter(0),
        Instruction::DeleteProperty(0),
    ] {
        let chunk = Chunk {
            instructions: vec![instruction, Instruction::ReturnUndefined],
            ..Chunk::default()
        };
        assert_eq!(
            chunk.validate(),
            Err(ChunkError::InvalidConstantIndex {
                offset: 0,
                index: 0,
            })
        );

        let chunk = Chunk {
            instructions: vec![instruction, Instruction::ReturnUndefined],
            constants: vec![Constant::Number(1.0)],
            ..Chunk::default()
        };
        assert_eq!(
            chunk.validate(),
            Err(ChunkError::ExpectedStringConstant {
                offset: 0,
                index: 0,
            })
        );
    }
}

#[test]
fn v4_definition_instructions_detect_stack_underflow() {
    let mut chunk = Chunk::default();
    let property = string(&mut chunk, "x");
    chunk.emit(Instruction::DefineDataProperty(property));
    chunk.emit(Instruction::ReturnUndefined);

    assert_eq!(
        chunk.validate(),
        Err(ChunkError::StackUnderflow {
            offset: 0,
            required: 2,
            available: 0,
        })
    );
}

#[test]
fn construct_evaluates_callee_then_arguments_left_to_right() {
    let call = |name: &str| Expression::Call {
        callee: Box::new(Expression::Identifier(name.into())),
        arguments: Vec::new(),
    };
    let program = Program {
        body: vec![Statement::Expression(Expression::Construct {
            callee: Box::new(call("makeConstructor")),
            arguments: vec![
                agentjs::ast::CallArgument::Expression(call("firstArgument")),
                agentjs::ast::CallArgument::Expression(call("secondArgument")),
            ],
        })],
    };

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Call(0),
            Instruction::LoadGlobal(1),
            Instruction::Call(0),
            Instruction::LoadGlobal(2),
            Instruction::Call(0),
            Instruction::Construct(2),
            Instruction::Return,
        ]
    );
    assert_eq!(
        chunk.constants,
        [
            Constant::String("makeConstructor".into()),
            Constant::String("firstArgument".into()),
            Constant::String("secondArgument".into()),
        ]
    );
}

#[test]
fn construct_rejects_argument_counts_outside_u16() {
    let arguments =
        vec![
            agentjs::ast::CallArgument::Expression(Expression::Literal(Literal::Undefined));
            usize::from(u16::MAX) + 1
        ];
    let program = Program {
        body: vec![Statement::Expression(Expression::Construct {
            callee: Box::new(Expression::Identifier("Constructor".into())),
            arguments,
        })],
    };

    let error = Compiler::new().compile_program(&program).unwrap_err();
    assert!(error.message.contains("construct argument count"));
}

#[test]
fn builtin_call_instructions_publish_generic_stack_contracts() {
    assert_eq!(Instruction::Call(2).stack_effect(), StackEffect::new(3, 1));
    assert_eq!(
        Instruction::CallWithThis(2).stack_effect(),
        StackEffect::with_required(4, 4, 1)
    );
    assert_eq!(
        Instruction::Construct(2).stack_effect(),
        StackEffect::new(3, 1)
    );
    assert_eq!(
        Instruction::GetMethod(0).stack_effect(),
        StackEffect::new(1, 2)
    );
}

#[test]
fn hand_written_builtin_call_shapes_validate_without_special_opcodes() {
    let mut static_method = Chunk::default();
    let object = string(&mut static_method, "Object");
    let create = string(&mut static_method, "create");
    let base = string(&mut static_method, "base");
    static_method.emit(Instruction::LoadGlobal(object));
    static_method.emit(Instruction::GetMethod(create));
    static_method.emit(Instruction::LoadGlobal(base));
    static_method.emit(Instruction::CallWithThis(1));
    static_method.emit(Instruction::Return);
    assert_eq!(static_method.validate(), Ok(()));
    assert_eq!(static_method.analyze_stack().unwrap().max_depth, 3);

    let mut function_call = Chunk::default();
    let function = string(&mut function_call, "Function");
    let prototype = string(&mut function_call, "prototype");
    let call = string(&mut function_call, "call");
    let target = string(&mut function_call, "target");
    let receiver = string(&mut function_call, "receiver");
    function_call.emit(Instruction::LoadGlobal(function));
    function_call.emit(Instruction::GetProperty(prototype));
    function_call.emit(Instruction::GetProperty(call));
    function_call.emit(Instruction::GetMethod(call));
    function_call.emit(Instruction::LoadGlobal(target));
    function_call.emit(Instruction::LoadGlobal(receiver));
    function_call.emit(Instruction::CallWithThis(2));
    function_call.emit(Instruction::Return);
    assert_eq!(function_call.validate(), Ok(()));
    assert_eq!(function_call.analyze_stack().unwrap().max_depth, 4);
}

fn expression(expression: Expression) -> Program {
    Program {
        body: vec![Statement::Expression(expression)],
    }
}

#[test]
fn compiler_uses_generic_calls_for_builtin_identifiers() {
    let program = expression(Expression::Call {
        callee: Box::new(identifier("Array")),
        arguments: vec![
            agentjs::ast::CallArgument::Expression(Expression::Literal(Literal::Number(1.0))),
            agentjs::ast::CallArgument::Expression(Expression::Literal(Literal::Number(2.0))),
        ],
    });

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Constant(1),
            Instruction::Constant(2),
            Instruction::Call(2),
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiler_uses_generic_construct_for_builtin_identifiers() {
    let program = expression(Expression::Construct {
        callee: Box::new(identifier("Array")),
        arguments: vec![agentjs::ast::CallArgument::Expression(Expression::Literal(
            Literal::Number(3.0),
        ))],
    });

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::Constant(1),
            Instruction::Construct(1),
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.validate(), Ok(()));
}

#[test]
fn compiler_preserves_receiver_for_builtin_style_method_calls() {
    let program = expression(Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(identifier("Object")),
            property: Box::new(identifier("create")),
            computed: false,
        }),
        arguments: vec![agentjs::ast::CallArgument::Expression(identifier("base"))],
    });

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::GetMethod(1),
            Instruction::LoadGlobal(2),
            Instruction::CallWithThis(1),
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.validate(), Ok(()));
}

fn identifier(name: &str) -> Expression {
    Expression::Identifier(name.into())
}

fn member(object: &str, property: Expression, computed: bool) -> Expression {
    Expression::Member {
        object: Box::new(identifier(object)),
        property: Box::new(property),
        computed,
    }
}

#[test]
fn compiler_lowers_v4_property_operators() {
    let delete_named = expression(Expression::Unary {
        operator: UnaryOperator::Delete,
        argument: Box::new(member("object", identifier("x"), false)),
    });
    let chunk = Compiler::new().compile_program(&delete_named).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::LoadGlobal(0),
            Instruction::DeleteProperty(1),
            Instruction::Return,
        ]
    );

    let delete_computed = expression(Expression::Unary {
        operator: UnaryOperator::Delete,
        argument: Box::new(member(
            "object",
            Expression::Literal(Literal::String("x".into())),
            true,
        )),
    });
    let chunk = Compiler::new().compile_program(&delete_computed).unwrap();
    assert!(chunk.instructions.contains(&Instruction::DeleteElement));

    for (operator, expected) in [
        (BinaryOperator::In, Instruction::HasProperty),
        (BinaryOperator::InstanceOf, Instruction::InstanceOf),
    ] {
        let program = expression(Expression::Binary {
            operator,
            left: Box::new(identifier("left")),
            right: Box::new(identifier("right")),
        });
        let chunk = Compiler::new().compile_program(&program).unwrap();
        assert_eq!(chunk.instructions[2], expected);
    }
}

#[test]
fn compiler_lowers_v4_object_property_kinds_in_source_order() {
    let program = expression(Expression::Object(vec![
        ObjectProperty::Data {
            key: PropertyName::Identifier("value".into()),
            value: Expression::Literal(Literal::Number(1.0)),
        },
        ObjectProperty::Getter {
            key: PropertyName::Identifier("x".into()),
            body: FunctionBody {
                statements: vec![Statement::Return(Some(Expression::Literal(
                    Literal::Number(2.0),
                )))],
                is_strict: false,
            },
        },
        ObjectProperty::Setter {
            key: PropertyName::Identifier("x".into()),
            parameter: FunctionParam::Simple("v".into()),
            body: FunctionBody {
                statements: Vec::new(),
                is_strict: false,
            },
        },
        ObjectProperty::PrototypeSetter {
            value: identifier("base"),
        },
    ]));

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::ObjectCreateEmpty,
            Instruction::Constant(0),
            Instruction::DefineDataProperty(1),
            Instruction::CreateFunction(0),
            Instruction::DefineGetter(2),
            Instruction::CreateFunction(1),
            Instruction::DefineSetter(2),
            Instruction::LoadGlobal(3),
            Instruction::SetObjectPrototype,
            Instruction::Return,
        ]
    );
    assert_eq!(chunk.functions.len(), 2);
    assert!(chunk.functions[0].params.is_empty());
    assert_eq!(chunk.functions[1].params, ["v"]);
}

#[test]
fn compiler_lowers_sparse_arrays_without_materializing_holes() {
    let program = expression(Expression::Array(vec![
        ArrayElement::Expression(Expression::Literal(Literal::Number(1.0))),
        ArrayElement::Hole,
        ArrayElement::Expression(Expression::Literal(Literal::Number(3.0))),
    ]));

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::ArrayCreateSparse(3),
            Instruction::Constant(0),
            Instruction::DefineElement(0),
            Instruction::Constant(1),
            Instruction::DefineElement(2),
            Instruction::Return,
        ]
    );
}

#[test]
fn compiler_keeps_dense_arrays_on_the_v3_compatible_path() {
    let program = expression(Expression::Array(vec![
        ArrayElement::Expression(Expression::Literal(Literal::Number(1.0))),
        ArrayElement::Expression(Expression::Literal(Literal::Number(2.0))),
    ]));

    let chunk = Compiler::new().compile_program(&program).unwrap();
    assert_eq!(
        chunk.instructions,
        [
            Instruction::Constant(0),
            Instruction::Constant(1),
            Instruction::ArrayCreate(2),
            Instruction::Return,
        ]
    );
}
