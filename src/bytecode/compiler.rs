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

#[derive(Debug, Default)]
struct CompileContext {
    loops: Vec<LoopContext>,
}

#[derive(Debug)]
struct LoopContext {
    continue_target: usize,
    break_jumps: Vec<usize>,
}

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
        let mut context = CompileContext::default();
        let completion_expression = completion_expression_index(&program.body);

        for (index, statement) in program.body.iter().enumerate() {
            self.compile_statement(
                statement,
                &mut chunk,
                &mut context,
                Some(index) == completion_expression,
            )?;
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

        chunk.emit(if completion_expression.is_some() {
            Instruction::Return
        } else {
            Instruction::ReturnUndefined
        });
        chunk.validate().map_err(CompileError::from_chunk)?;
        Ok(chunk)
    }

    fn compile_statement(
        &mut self,
        statement: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
        preserve_expression_value: bool,
    ) -> Result<(), CompileError> {
        match statement {
            Statement::Empty => Ok(()),
            Statement::Expression(expression) => {
                self.compile_expression(expression, chunk)?;
                if !preserve_expression_value {
                    chunk.emit(Instruction::Pop);
                }
                Ok(())
            }
            Statement::Block(statements) => self.compile_statement_list(statements, chunk, context),
            Statement::VariableDeclaration {
                kind,
                name,
                initializer,
            } => self.compile_variable_declaration(*kind, name, initializer.as_ref(), chunk),
            Statement::If {
                test,
                consequent,
                alternate,
            } => self.compile_if(test, consequent, alternate.as_deref(), chunk, context),
            Statement::While { test, body } => self.compile_while(test, body, chunk, context),
            Statement::Break => self.compile_break(chunk, context),
            Statement::Continue => self.compile_continue(chunk, context),
            Statement::Throw(expression) => {
                self.compile_expression(expression, chunk)?;
                chunk.emit(Instruction::Throw);
                Ok(())
            }
            Statement::Return(_) => Err(CompileError::unsupported(
                "return statement before the function milestone",
            )),
        }
    }

    fn compile_statement_list(
        &mut self,
        statements: &[Statement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        for statement in statements {
            self.compile_statement(statement, chunk, context, false)?;
        }
        Ok(())
    }

    fn compile_if(
        &mut self,
        test: &Expression,
        consequent: &Statement,
        alternate: Option<&Statement>,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        self.compile_expression(test, chunk)?;
        let false_jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
        chunk.emit(Instruction::Pop);
        self.compile_statement(consequent, chunk, context, false)?;
        let end_jump = chunk.emit(Instruction::Jump(usize::MAX));

        let false_cleanup = chunk.current_offset();
        chunk
            .patch_jump(false_jump, false_cleanup)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop);
        if let Some(alternate) = alternate {
            self.compile_statement(alternate, chunk, context, false)?;
        }

        let end = chunk.current_offset();
        chunk
            .patch_jump(end_jump, end)
            .map_err(CompileError::from_chunk)?;
        Ok(())
    }

    fn compile_while(
        &mut self,
        test: &Expression,
        body: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let loop_start = chunk.current_offset();
        self.compile_expression(test, chunk)?;
        let exit_jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
        chunk.emit(Instruction::Pop);

        context.loops.push(LoopContext {
            continue_target: loop_start,
            break_jumps: Vec::new(),
        });
        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            return Err(error);
        }
        chunk.emit(Instruction::Jump(loop_start));

        let false_cleanup = chunk.current_offset();
        chunk
            .patch_jump(exit_jump, false_cleanup)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop);
        let loop_end = chunk.current_offset();

        let loop_context = context
            .loops
            .pop()
            .expect("the current while loop context must exist");
        for jump in loop_context.break_jumps {
            chunk
                .patch_jump(jump, loop_end)
                .map_err(CompileError::from_chunk)?;
        }
        Ok(())
    }

    fn compile_break(
        &mut self,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if context.loops.is_empty() {
            return Err(CompileError::unsupported(
                "break statement outside of a loop",
            ));
        }
        let jump = chunk.emit(Instruction::Jump(usize::MAX));
        context
            .loops
            .last_mut()
            .expect("checked that a loop context exists")
            .break_jumps
            .push(jump);
        Ok(())
    }

    fn compile_continue(
        &mut self,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let target = context
            .loops
            .last()
            .ok_or_else(|| CompileError::unsupported("continue statement outside of a loop"))?
            .continue_target;
        chunk.emit(Instruction::Jump(target));
        Ok(())
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
            Expression::Conditional {
                test,
                consequent,
                alternate,
            } => self.compile_conditional(test, consequent, alternate, chunk),
            Expression::Construct { callee, arguments } => {
                self.compile_construct(callee, arguments, chunk)
            }
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
                if let Expression::Identifier(name) = argument {
                    let name = self.add_name(name, chunk)?;
                    chunk.emit(Instruction::TypeOfGlobal(name));
                    return Ok(());
                }
                Instruction::TypeOf
            }
        };

        self.compile_expression(argument, chunk)?;
        chunk.emit(instruction);
        Ok(())
    }

    fn compile_conditional(
        &mut self,
        test: &Expression,
        consequent: &Expression,
        alternate: &Expression,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        self.compile_expression(test, chunk)?;
        let false_jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
        chunk.emit(Instruction::Pop);
        self.compile_expression(consequent, chunk)?;
        let end_jump = chunk.emit(Instruction::Jump(usize::MAX));

        let alternate_start = chunk.current_offset();
        chunk
            .patch_jump(false_jump, alternate_start)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop);
        self.compile_expression(alternate, chunk)?;

        let end = chunk.current_offset();
        chunk
            .patch_jump(end_jump, end)
            .map_err(CompileError::from_chunk)?;
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

    fn compile_construct(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
            message: "construct argument count exceeds the u16 bytecode range".into(),
        })?;

        self.compile_expression(callee, chunk)?;
        for argument in arguments {
            self.compile_expression(argument, chunk)?;
        }
        chunk.emit(Instruction::Construct(argument_count));
        Ok(())
    }
}

fn completion_expression_index(statements: &[Statement]) -> Option<usize> {
    let index = statements
        .iter()
        .rposition(|statement| matches!(statement, Statement::Expression(_)))?;
    statements[index + 1..]
        .iter()
        .all(|statement| {
            matches!(
                statement,
                Statement::Empty | Statement::VariableDeclaration { .. }
            )
        })
        .then_some(index)
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
