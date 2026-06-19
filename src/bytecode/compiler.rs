//! AST-to-bytecode compiler.

use std::fmt;

use crate::ast::{
    BinaryOperator, Expression, Literal, LogicalOperator, Program, Statement, UnaryOperator,
    VariableKind,
};

use super::{Chunk, ChunkError, Constant, Instruction};

/// Compilation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CompileError {}

/// Compiles an AST into stack-based AgentJS bytecode.
#[derive(Debug, Default)]
pub struct Compiler;

impl Compiler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a script containing empty and literal expression statements.
    ///
    /// This is the compiler team's stable direct API. It reads but never
    /// mutates the AST and returns either a complete [`Chunk`] or an error.
    pub fn compile_program(&mut self, program: &Program) -> Result<Chunk, CompileError> {
        let mut chunk = Chunk::default();
        let last_expression = program
            .body
            .iter()
            .rposition(|statement| matches!(statement, Statement::Expression(_)));

        for (index, statement) in program.body.iter().enumerate() {
            match statement {
                Statement::Empty => {}
                Statement::Expression(expression) => {
                    self.compile_expression(expression, &mut chunk)?;
                    if Some(index) != last_expression {
                        chunk.emit(Instruction::Pop);
                    }
                }
                Statement::VariableDeclaration { kind, declarations } => {
                    for declarator in declarations {
                        self.compile_variable_declaration(
                            *kind,
                            &declarator.name,
                            declarator.initializer.as_ref(),
                            &mut chunk,
                        )?;
                    }
                }
                unsupported => {
                    return Err(CompileError::unsupported(format!(
                        "statement {unsupported:?}"
                    )));
                }
            }
        }

        chunk.emit(if last_expression.is_some() {
            Instruction::Return
        } else {
            Instruction::ReturnUndefined
        });
        chunk.validate().map_err(CompileError::from_chunk)?;
        Ok(chunk)
    }

    fn compile_expression(
        &mut self,
        expression: &Expression,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        match expression {
            Expression::Literal(literal) => self.compile_literal(literal, chunk),
            Expression::Unary { operator, argument } => {
                self.compile_unary(*operator, argument, chunk)
            }
            Expression::Binary {
                operator,
                left,
                right,
            } => self.compile_binary(*operator, left, right, chunk),
            Expression::Logical {
                operator,
                left,
                right,
            } => self.compile_logical(*operator, left, right, chunk),
            Expression::Identifier(name) => {
                let name = self.add_name(name, chunk)?;
                chunk.emit(Instruction::LoadGlobal(name));
                Ok(())
            }
            Expression::Assignment { target, value } => {
                self.compile_assignment(target, value, chunk)
            }
            Expression::Member {
                object,
                property,
                computed,
            } => self.compile_member(object, property, *computed, chunk),
            Expression::Call { callee, arguments } => self.compile_call(callee, arguments, chunk),
            unsupported => Err(CompileError::unsupported(format!(
                "expression {unsupported:?}"
            ))),
        }
    }

    fn compile_literal(
        &mut self,
        literal: &Literal,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        let constant = match literal {
            Literal::Undefined => Constant::Undefined,
            Literal::Null => Constant::Null,
            Literal::Boolean(value) => Constant::Boolean(*value),
            Literal::Number(value) => Constant::Number(*value),
            Literal::String(value) => Constant::String(value.clone()),
        };
        let index = chunk
            .add_constant(constant)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Constant(index));
        Ok(())
    }

    fn compile_unary(
        &mut self,
        operator: UnaryOperator,
        argument: &Expression,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        let instruction = match operator {
            UnaryOperator::Plus => Instruction::UnaryPlus,
            UnaryOperator::Minus => Instruction::Negate,
            UnaryOperator::Not => Instruction::LogicalNot,
            UnaryOperator::TypeOf => {
                return Err(CompileError::unsupported("unary operator TypeOf"));
            }
        };

        self.compile_expression(argument, chunk)?;
        chunk.emit(instruction);
        Ok(())
    }

    fn compile_binary(
        &mut self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        match operator {
            BinaryOperator::LogicalAnd => {
                return self.compile_logical(LogicalOperator::And, left, right, chunk);
            }
            BinaryOperator::LogicalOr => {
                return self.compile_logical(LogicalOperator::Or, left, right, chunk);
            }
            _ => {}
        }

        let instruction = match operator {
            BinaryOperator::Add => Instruction::Add,
            BinaryOperator::Subtract => Instruction::Subtract,
            BinaryOperator::Multiply => Instruction::Multiply,
            BinaryOperator::Divide => Instruction::Divide,
            BinaryOperator::Remainder => Instruction::Remainder,
            BinaryOperator::StrictEqual => Instruction::StrictEqual,
            BinaryOperator::StrictNotEqual => Instruction::StrictNotEqual,
            BinaryOperator::LessThan => Instruction::LessThan,
            BinaryOperator::LessThanOrEqual => Instruction::LessThanOrEqual,
            BinaryOperator::GreaterThan => Instruction::GreaterThan,
            BinaryOperator::GreaterThanOrEqual => Instruction::GreaterThanOrEqual,
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => unreachable!(),
            BinaryOperator::Equal => {
                return Err(CompileError::unsupported(
                    "binary operator Equal (abstract equality)",
                ));
            }
        };

        self.compile_expression(left, chunk)?;
        self.compile_expression(right, chunk)?;
        chunk.emit(instruction);
        Ok(())
    }

    fn compile_logical(
        &mut self,
        operator: LogicalOperator,
        left: &Expression,
        right: &Expression,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        self.compile_expression(left, chunk)?;

        let jump = match operator {
            LogicalOperator::And => chunk.emit(Instruction::JumpIfFalse(usize::MAX)),
            LogicalOperator::Or => chunk.emit(Instruction::JumpIfTrue(usize::MAX)),
        };

        chunk.emit(Instruction::Pop);
        self.compile_expression(right, chunk)?;
        chunk
            .patch_jump(jump, chunk.current_offset())
            .map_err(CompileError::from_chunk)?;
        Ok(())
    }

    fn compile_variable_declaration(
        &mut self,
        kind: VariableKind,
        name: &str,
        initializer: Option<&Expression>,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        if kind != VariableKind::Var {
            return Err(CompileError::unsupported(format!(
                "variable declaration kind {kind:?}"
            )));
        }

        match initializer {
            Some(initializer) => self.compile_expression(initializer, chunk)?,
            None => {
                let undefined = chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::Constant(undefined));
            }
        }

        let name = self.add_name(name, chunk)?;
        chunk.emit(Instruction::DeclareGlobal(name));
        Ok(())
    }

    fn compile_assignment(
        &mut self,
        target: &Expression,
        value: &Expression,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        let Expression::Identifier(name) = target else {
            return Err(CompileError::unsupported(format!(
                "assignment target {target:?}"
            )));
        };

        self.compile_expression(value, chunk)?;
        let name = self.add_name(name, chunk)?;
        chunk.emit(Instruction::StoreGlobal(name));
        Ok(())
    }

    fn add_name(&mut self, name: &str, chunk: &mut Chunk) -> Result<u16, CompileError> {
        chunk
            .add_constant(Constant::String(name.into()))
            .map_err(CompileError::from_chunk)
    }

    fn compile_member(
        &mut self,
        object: &Expression,
        property: &Expression,
        computed: bool,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        if computed {
            return Err(CompileError::unsupported(
                "computed member access object[property]",
            ));
        }
        let Expression::Identifier(property) = property else {
            return Err(CompileError::unsupported(format!(
                "non-identifier member property {property:?}"
            )));
        };

        self.compile_expression(object, chunk)?;
        let property = self.add_name(property, chunk)?;
        chunk.emit(Instruction::GetProperty(property));
        Ok(())
    }

    fn compile_call(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
            message: "call argument count exceeds the u16 bytecode range".into(),
        })?;

        self.compile_expression(callee, chunk)?;
        for argument in arguments {
            self.compile_expression(argument, chunk)?;
        }
        chunk.emit(Instruction::Call(argument_count));
        Ok(())
    }
}

impl CompileError {
    fn unsupported(node: impl fmt::Display) -> Self {
        Self {
            message: format!("bytecode compiler does not support {node} yet"),
        }
    }

    fn from_chunk(error: ChunkError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}
