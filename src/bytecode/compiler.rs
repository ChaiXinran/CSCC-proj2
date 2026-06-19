//! AST-to-bytecode compiler.

use std::fmt;

use crate::ast::{
    ArrayElement, BinaryOperator, Expression, FunctionBody, FunctionLiteral, Literal,
    LogicalOperator, ObjectProperty, Program, PropertyName, Statement, UnaryOperator, VariableKind,
};

use super::{Chunk, ChunkError, Constant, EnvironmentCapturePolicy, FunctionTemplate, Instruction};

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
    /// Number of enclosing function bodies; 0 = top-level script.
    function_depth: usize,
}

#[derive(Debug)]
struct LoopContext {
    continue_target: usize,
    break_jumps: Vec<usize>,
}

impl CompileContext {
    fn inside_function(&self) -> bool {
        self.function_depth > 0
    }
}

impl Compiler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a script containing any statement forms.
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
                self.compile_expression(expression, chunk, context)?;
                if !preserve_expression_value {
                    chunk.emit(Instruction::Pop);
                }
                Ok(())
            }
            Statement::Block(statements) => self.compile_statement_list(statements, chunk, context),
            Statement::VariableDeclaration { kind, declarations } => {
                for declarator in declarations {
                    self.compile_variable_declaration(
                        *kind,
                        &declarator.name,
                        declarator.initializer.as_ref(),
                        chunk,
                        context,
                    )?;
                }
                Ok(())
            }
            Statement::If {
                test,
                consequent,
                alternate,
            } => self.compile_if(test, consequent, alternate.as_deref(), chunk, context),
            Statement::While { test, body } => self.compile_while(test, body, chunk, context),
            Statement::Break => self.compile_break(chunk, context),
            Statement::Continue => self.compile_continue(chunk, context),
            Statement::Throw(expression) => {
                self.compile_expression(expression, chunk, context)?;
                chunk.emit(Instruction::Throw);
                Ok(())
            }
            Statement::Return(value) => self.compile_return(value.as_ref(), chunk, context),
            Statement::FunctionDeclaration { name, params, body } => self
                .compile_function_declaration(
                    name,
                    params.iter().map(|p| p.name.as_str()),
                    body,
                    chunk,
                    context,
                ),
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
        self.compile_expression(test, chunk, context)?;
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
        self.compile_expression(test, chunk, context)?;
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

    fn compile_return(
        &mut self,
        value: Option<&Expression>,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if !context.inside_function() {
            return Err(CompileError::unsupported("return outside of a function"));
        }
        if let Some(value) = value {
            self.compile_expression(value, chunk, context)?;
            chunk.emit(Instruction::Return);
        } else {
            chunk.emit(Instruction::ReturnUndefined);
        }
        Ok(())
    }

    /// Compiles a function declaration: emits `DeclareFunction { name, function }`.
    fn compile_function_declaration<'a>(
        &mut self,
        name: &str,
        params: impl Iterator<Item = &'a str>,
        body: &FunctionBody,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let fn_chunk = self.compile_function_body(params, body, context)?;
        let template = FunctionTemplate {
            name: Some(name.to_string()),
            params: fn_chunk.params,
            chunk: fn_chunk.chunk,
            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
        };
        let function_index = chunk
            .add_function(template)
            .map_err(CompileError::from_chunk)?;
        let name_index = chunk
            .add_constant(Constant::String(name.into()))
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::DeclareFunction {
            name: name_index,
            function: function_index,
        });
        Ok(())
    }

    /// Compiles the body of a function literal or declaration into a
    /// `FunctionTemplate` and returns it with parameter names.
    fn compile_function_body<'a>(
        &mut self,
        params: impl Iterator<Item = &'a str>,
        body: &FunctionBody,
        outer_context: &mut CompileContext,
    ) -> Result<CompiledFunction, CompileError> {
        let param_names: Vec<String> = params.map(|s| s.to_string()).collect();
        let mut fn_chunk = Chunk::default();
        let mut fn_context = CompileContext {
            loops: Vec::new(),
            function_depth: outer_context.function_depth + 1,
        };
        // Compile the body statements; no "completion expression" inside function bodies.
        for statement in &body.statements {
            self.compile_statement(statement, &mut fn_chunk, &mut fn_context, false)?;
        }
        // Implicit undefined return at the end of the function
        fn_chunk.emit(Instruction::ReturnUndefined);
        fn_chunk.validate().map_err(CompileError::from_chunk)?;
        Ok(CompiledFunction {
            params: param_names,
            chunk: fn_chunk,
        })
    }

    fn compile_expression(
        &mut self,
        expression: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match expression {
            Expression::Literal(literal) => self.compile_literal(literal, chunk),
            Expression::Unary { operator, argument } => {
                self.compile_unary(*operator, argument, chunk, context)
            }
            Expression::Binary {
                operator,
                left,
                right,
            } => self.compile_binary(*operator, left, right, chunk, context),
            Expression::Logical {
                operator,
                left,
                right,
            } => self.compile_logical(*operator, left, right, chunk, context),
            Expression::Identifier(name) => self.compile_identifier(name, chunk, context),
            Expression::Assignment { target, value } => {
                self.compile_assignment(target, value, chunk, context)
            }
            Expression::Member {
                object,
                property,
                computed,
            } => self.compile_member(object, property, *computed, chunk, context),
            Expression::Call { callee, arguments } => {
                self.compile_call(callee, arguments, chunk, context)
            }
            Expression::Conditional {
                test,
                consequent,
                alternate,
            } => self.compile_conditional(test, consequent, alternate, chunk, context),
            Expression::Construct { callee, arguments } => {
                self.compile_construct(callee, arguments, chunk, context)
            }
            Expression::Array(elements) => self.compile_array(elements, chunk, context),
            Expression::Object(properties) => self.compile_object(properties, chunk, context),
            Expression::Function(literal) => {
                self.compile_function_expression(literal, chunk, context)
            }
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

    fn compile_identifier(
        &mut self,
        name: &str,
        chunk: &mut Chunk,
        context: &CompileContext,
    ) -> Result<(), CompileError> {
        if name == "this" && context.inside_function() {
            chunk.emit(Instruction::LoadThis);
            return Ok(());
        }
        let name_index = self.add_name(name, chunk)?;
        if context.inside_function() {
            chunk.emit(Instruction::LoadName(name_index));
        } else {
            chunk.emit(Instruction::LoadGlobal(name_index));
        }
        Ok(())
    }

    fn compile_unary(
        &mut self,
        operator: UnaryOperator,
        argument: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let instruction = match operator {
            UnaryOperator::Plus => Instruction::UnaryPlus,
            UnaryOperator::Minus => Instruction::Negate,
            UnaryOperator::Not => Instruction::LogicalNot,
            UnaryOperator::Delete => {
                return self.compile_delete(argument, chunk, context);
            }
            UnaryOperator::TypeOf => {
                if let Expression::Identifier(name) = argument {
                    let name_index = self.add_name(name, chunk)?;
                    if context.inside_function() {
                        chunk.emit(Instruction::TypeOfName(name_index));
                    } else {
                        chunk.emit(Instruction::TypeOfGlobal(name_index));
                    }
                    return Ok(());
                }
                Instruction::TypeOf
            }
        };

        self.compile_expression(argument, chunk, context)?;
        chunk.emit(instruction);
        Ok(())
    }

    fn compile_conditional(
        &mut self,
        test: &Expression,
        consequent: &Expression,
        alternate: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        self.compile_expression(test, chunk, context)?;
        let false_jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
        chunk.emit(Instruction::Pop);
        self.compile_expression(consequent, chunk, context)?;
        let end_jump = chunk.emit(Instruction::Jump(usize::MAX));

        let alternate_start = chunk.current_offset();
        chunk
            .patch_jump(false_jump, alternate_start)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop);
        self.compile_expression(alternate, chunk, context)?;

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
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match operator {
            BinaryOperator::LogicalAnd => {
                return self.compile_logical(LogicalOperator::And, left, right, chunk, context);
            }
            BinaryOperator::LogicalOr => {
                return self.compile_logical(LogicalOperator::Or, left, right, chunk, context);
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
            BinaryOperator::In => Instruction::HasProperty,
            BinaryOperator::InstanceOf => Instruction::InstanceOf,
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => unreachable!(),
            BinaryOperator::Equal => {
                return Err(CompileError::unsupported(
                    "binary operator Equal (abstract equality)",
                ));
            }
        };

        self.compile_expression(left, chunk, context)?;
        self.compile_expression(right, chunk, context)?;
        chunk.emit(instruction);
        Ok(())
    }

    fn compile_logical(
        &mut self,
        operator: LogicalOperator,
        left: &Expression,
        right: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        self.compile_expression(left, chunk, context)?;

        let jump = match operator {
            LogicalOperator::And => chunk.emit(Instruction::JumpIfFalse(usize::MAX)),
            LogicalOperator::Or => chunk.emit(Instruction::JumpIfTrue(usize::MAX)),
        };

        chunk.emit(Instruction::Pop);
        self.compile_expression(right, chunk, context)?;
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
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if kind != VariableKind::Var {
            return Err(CompileError::unsupported(format!(
                "variable declaration kind {kind:?}"
            )));
        }

        match initializer {
            Some(initializer) => self.compile_expression(initializer, chunk, context)?,
            None => {
                let undefined = chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::Constant(undefined));
            }
        }

        let name_index = self.add_name(name, chunk)?;
        if context.inside_function() {
            chunk.emit(Instruction::DeclareLocal(name_index));
        } else {
            chunk.emit(Instruction::DeclareGlobal(name_index));
        }
        Ok(())
    }

    fn compile_assignment(
        &mut self,
        target: &Expression,
        value: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match target {
            Expression::Identifier(name) => {
                self.compile_expression(value, chunk, context)?;
                let name_index = self.add_name(name, chunk)?;
                if context.inside_function() {
                    chunk.emit(Instruction::StoreName(name_index));
                } else {
                    chunk.emit(Instruction::StoreGlobal(name_index));
                }
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: false,
            } => {
                // obj.prop = value  →  [object, value] → SetProperty
                let Expression::Identifier(property_name) = property.as_ref() else {
                    return Err(CompileError::unsupported(
                        "non-identifier static member as assignment target",
                    ));
                };
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(value, chunk, context)?;
                let prop_index = self.add_name(property_name, chunk)?;
                chunk.emit(Instruction::SetProperty(prop_index));
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: true,
            } => {
                // obj[key] = value  →  [object, key, value] → SetElement
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(property, chunk, context)?;
                self.compile_expression(value, chunk, context)?;
                chunk.emit(Instruction::SetElement);
                Ok(())
            }
            _ => Err(CompileError::unsupported(format!(
                "assignment target {target:?}"
            ))),
        }
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
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if computed {
            // object[key]  →  push object, push key, GetElement
            self.compile_expression(object, chunk, context)?;
            self.compile_expression(property, chunk, context)?;
            chunk.emit(Instruction::GetElement);
            return Ok(());
        }

        let Expression::Identifier(property_name) = property else {
            return Err(CompileError::unsupported(format!(
                "non-identifier member property {property:?}"
            )));
        };

        self.compile_expression(object, chunk, context)?;
        let property_index = self.add_name(property_name, chunk)?;
        chunk.emit(Instruction::GetProperty(property_index));
        Ok(())
    }

    fn compile_call(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
            message: "call argument count exceeds the u16 bytecode range".into(),
        })?;

        // Static member calls preserve their receiver as `this`. Native
        // functions ignore the receiver, so this also remains compatible with
        // calls such as `assert.sameValue(...)`.
        if let Expression::Member {
            object,
            property,
            computed: false,
        } = callee
        {
            let Expression::Identifier(method_name) = property.as_ref() else {
                return Err(CompileError::unsupported(
                    "computed method call (use obj['method']() separately)",
                ));
            };
            self.compile_expression(object, chunk, context)?;
            let method_index = self.add_name(method_name, chunk)?;
            // Stack after: [method_value, object]
            chunk.emit(Instruction::GetMethod(method_index));
            for argument in arguments {
                self.compile_expression(argument, chunk, context)?;
            }
            chunk.emit(Instruction::CallWithThis(argument_count));
            return Ok(());
        }

        self.compile_expression(callee, chunk, context)?;
        for argument in arguments {
            self.compile_expression(argument, chunk, context)?;
        }
        chunk.emit(Instruction::Call(argument_count));
        Ok(())
    }

    fn compile_construct(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
            message: "construct argument count exceeds the u16 bytecode range".into(),
        })?;

        self.compile_expression(callee, chunk, context)?;
        for argument in arguments {
            self.compile_expression(argument, chunk, context)?;
        }
        chunk.emit(Instruction::Construct(argument_count));
        Ok(())
    }

    fn compile_array(
        &mut self,
        elements: &[ArrayElement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if elements
            .iter()
            .all(|element| matches!(element, ArrayElement::Expression(_)))
        {
            let count = u16::try_from(elements.len()).map_err(|_| CompileError {
                message: "dense array literal element count exceeds the u16 bytecode range".into(),
            })?;
            for element in elements {
                let ArrayElement::Expression(expression) = element else {
                    unreachable!();
                };
                self.compile_expression(expression, chunk, context)?;
            }
            chunk.emit(Instruction::ArrayCreate(count));
            return Ok(());
        }

        let length = u32::try_from(elements.len()).map_err(|_| CompileError {
            message: "sparse array literal length exceeds the u32 bytecode range".into(),
        })?;
        chunk.emit(Instruction::ArrayCreateSparse(length));
        for (index, element) in elements.iter().enumerate() {
            let ArrayElement::Expression(expression) = element else {
                continue;
            };
            self.compile_expression(expression, chunk, context)?;
            let index = u32::try_from(index).map_err(|_| CompileError {
                message: "sparse array element index exceeds the u32 bytecode range".into(),
            })?;
            chunk.emit(Instruction::DefineElement(index));
        }
        Ok(())
    }

    fn compile_object(
        &mut self,
        properties: &[ObjectProperty],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if properties
            .iter()
            .all(|property| matches!(property, ObjectProperty::Data { .. }))
        {
            let count = u16::try_from(properties.len()).map_err(|_| CompileError {
                message: "object literal property count exceeds the u16 bytecode range".into(),
            })?;
            for property in properties {
                let ObjectProperty::Data { key, value } = property else {
                    unreachable!();
                };
                let key_index = chunk
                    .add_constant(Constant::String(property_key(key)))
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::Constant(key_index));
                self.compile_expression(value, chunk, context)?;
            }
            chunk.emit(Instruction::ObjectCreate(count));
            return Ok(());
        }

        chunk.emit(Instruction::ObjectCreateEmpty);
        for property in properties {
            match property {
                ObjectProperty::Data { key, value } => {
                    self.compile_expression(value, chunk, context)?;
                    let key = self.add_name(&property_key(key), chunk)?;
                    chunk.emit(Instruction::DefineDataProperty(key));
                }
                ObjectProperty::Getter { key, body } => {
                    self.compile_accessor_function(std::iter::empty(), body, chunk, context)?;
                    let key = self.add_name(&property_key(key), chunk)?;
                    chunk.emit(Instruction::DefineGetter(key));
                }
                ObjectProperty::Setter {
                    key,
                    parameter,
                    body,
                } => {
                    self.compile_accessor_function(
                        std::iter::once(parameter.name.as_str()),
                        body,
                        chunk,
                        context,
                    )?;
                    let key = self.add_name(&property_key(key), chunk)?;
                    chunk.emit(Instruction::DefineSetter(key));
                }
                ObjectProperty::PrototypeSetter { value } => {
                    self.compile_expression(value, chunk, context)?;
                    chunk.emit(Instruction::SetObjectPrototype);
                }
            }
        }
        Ok(())
    }

    fn compile_delete(
        &mut self,
        argument: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match argument {
            Expression::Member {
                object,
                property,
                computed: false,
            } => {
                let Expression::Identifier(property) = property.as_ref() else {
                    return Err(CompileError::unsupported(
                        "non-identifier static property in delete",
                    ));
                };
                self.compile_expression(object, chunk, context)?;
                let property = self.add_name(property, chunk)?;
                chunk.emit(Instruction::DeleteProperty(property));
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: true,
            } => {
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::DeleteElement);
                Ok(())
            }
            _ => Err(CompileError::unsupported(
                "delete operand other than a member expression",
            )),
        }
    }

    fn compile_accessor_function<'a>(
        &mut self,
        params: impl Iterator<Item = &'a str>,
        body: &FunctionBody,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let compiled = self.compile_function_body(params, body, context)?;
        let template = FunctionTemplate {
            name: None,
            params: compiled.params,
            chunk: compiled.chunk,
            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
        };
        let index = chunk
            .add_function(template)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::CreateFunction(index));
        Ok(())
    }

    fn compile_function_expression(
        &mut self,
        literal: &FunctionLiteral,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let fn_chunk = self.compile_function_body(
            literal.params.iter().map(|p| p.name.as_str()),
            &literal.body,
            context,
        )?;
        let template = FunctionTemplate {
            name: literal.name.clone(),
            params: fn_chunk.params,
            chunk: fn_chunk.chunk,
            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
        };
        let function_index = chunk
            .add_function(template)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::CreateFunction(function_index));
        Ok(())
    }
}

/// Intermediate result returned from `compile_function_body`.
struct CompiledFunction {
    params: Vec<String>,
    chunk: Chunk,
}

fn property_key(key: &PropertyName) -> String {
    key.to_key_string()
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

#[cfg(test)]
mod tests {
    use crate::{
        bytecode::{Chunk, Constant, EnvironmentCapturePolicy, Instruction},
        lexer::Lexer,
        parser::Parser,
    };

    use super::Compiler;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn compile(source: &str) -> Chunk {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        let program = Parser::new(tokens)
            .parse_program()
            .expect("parsing succeeds");
        Compiler::new()
            .compile_program(&program)
            .expect("compilation succeeds")
    }

    fn num_const(value: f64) -> Constant {
        Constant::Number(value)
    }

    // -----------------------------------------------------------------------
    // Basic V1/V2 tests
    // -----------------------------------------------------------------------

    #[test]
    fn empty_program_emits_return_undefined() {
        let chunk = compile("");
        assert_eq!(chunk.instructions, [Instruction::ReturnUndefined]);
    }

    #[test]
    fn single_number_is_completion_value() {
        let chunk = compile("42");
        assert_eq!(
            chunk.instructions,
            [Instruction::Constant(0), Instruction::Return]
        );
        assert_eq!(chunk.constants[0], num_const(42.0));
    }

    #[test]
    fn var_declaration_uses_declare_global() {
        let chunk = compile("var x = 1;");
        assert!(chunk.instructions.contains(&Instruction::DeclareGlobal(1)));
    }

    #[test]
    fn method_call_uses_get_method_and_call_with_this_inside_function() {
        // GetMethod + CallWithThis is only emitted inside function bodies.
        let chunk = compile("function f() { return obj.method(1, 2); }");
        let fn_chunk = &chunk.functions[0].chunk;
        assert!(
            fn_chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::GetMethod(_)))
        );
        assert!(
            fn_chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallWithThis(2)))
        );
    }

    #[test]
    fn top_level_method_call_preserves_receiver() {
        let chunk = compile("assert.sameValue(1, 1)");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::GetMethod(_)))
        );
        assert!(chunk.instructions.contains(&Instruction::CallWithThis(2)));
    }

    #[test]
    fn chunk_validates_successfully() {
        let chunk = compile("1 + 2");
        assert!(chunk.validate().is_ok());
    }

    // -----------------------------------------------------------------------
    // V3 compiler tests
    // -----------------------------------------------------------------------

    #[test]
    fn function_declaration_emits_declare_function() {
        let chunk = compile("function add(a, b) { return a + b; }");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::DeclareFunction { .. }))
        );
        assert!(!chunk.functions.is_empty());
    }

    #[test]
    fn function_body_uses_load_name_inside() {
        let chunk = compile("function add(a, b) { return a + b; }");
        let fn_template = &chunk.functions[0];
        assert!(
            fn_template
                .chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadName(_)))
        );
    }

    #[test]
    fn function_body_ends_with_return_undefined() {
        let chunk = compile("function f() { }");
        let fn_template = &chunk.functions[0];
        assert_eq!(
            fn_template.chunk.instructions.last(),
            Some(&Instruction::ReturnUndefined)
        );
    }

    #[test]
    fn return_statement_emits_return() {
        let chunk = compile("function f() { return 1; }");
        let fn_template = &chunk.functions[0];
        assert!(
            fn_template
                .chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::Return))
        );
    }

    #[test]
    fn empty_return_emits_return_undefined() {
        let chunk = compile("function f() { return; }");
        let fn_template = &chunk.functions[0];
        assert!(
            fn_template
                .chunk
                .instructions
                .contains(&Instruction::ReturnUndefined)
        );
    }

    #[test]
    fn function_params_are_recorded() {
        let chunk = compile("function add(a, b) { return a + b; }");
        let fn_template = &chunk.functions[0];
        assert_eq!(fn_template.params, ["a", "b"]);
    }

    #[test]
    fn function_declaration_name_is_recorded() {
        let chunk = compile("function add(a, b) { return a + b; }");
        let fn_template = &chunk.functions[0];
        assert_eq!(fn_template.name, Some("add".into()));
    }

    #[test]
    fn function_capture_policy_is_capture_current() {
        let chunk = compile("function f() { }");
        assert_eq!(
            chunk.functions[0].environment_policy,
            EnvironmentCapturePolicy::CaptureCurrent
        );
    }

    #[test]
    fn array_literal_emits_array_create() {
        let chunk = compile("[1, 2, 3]");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::ArrayCreate(3)))
        );
    }

    #[test]
    fn empty_array_literal() {
        let chunk = compile("[]");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::ArrayCreate(0)))
        );
    }

    #[test]
    fn object_literal_emits_object_create() {
        let chunk = compile("({ a: 1, b: 2 })");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::ObjectCreate(2)))
        );
    }

    #[test]
    fn empty_object_literal() {
        let chunk = compile("({})");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::ObjectCreate(0)))
        );
    }

    #[test]
    fn computed_member_emits_get_element() {
        let chunk = compile("arr[0]");
        assert!(chunk.instructions.contains(&Instruction::GetElement));
    }

    #[test]
    fn computed_member_assignment_emits_set_element() {
        let chunk = compile("arr[0] = 1");
        assert!(chunk.instructions.contains(&Instruction::SetElement));
    }

    #[test]
    fn static_member_assignment_emits_set_property() {
        let chunk = compile("obj.x = 5");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::SetProperty(_)))
        );
    }

    #[test]
    fn function_expression_emits_create_function() {
        let chunk = compile("(function() { })");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::CreateFunction(_)))
        );
    }

    #[test]
    fn nested_function_declarations_compile() {
        let chunk =
            compile("function outer(x) { function inner(y) { return x + y; } return inner(2); }");
        assert!(!chunk.functions.is_empty());
        let outer_fn = &chunk.functions[0];
        // inner function is in outer's function table
        assert!(!outer_fn.chunk.functions.is_empty());
    }

    #[test]
    fn function_var_uses_declare_local() {
        let chunk = compile("function f() { var x = 1; }");
        let fn_chunk = &chunk.functions[0].chunk;
        assert!(
            fn_chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::DeclareLocal(_)))
        );
    }

    #[test]
    fn function_body_chunk_validates() {
        let chunk = compile("function add(a, b) { return a + b; }");
        assert!(chunk.functions[0].chunk.validate().is_ok());
    }
}
