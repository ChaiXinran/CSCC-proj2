//! AST-to-bytecode compiler.

use std::{collections::HashSet, fmt};

use crate::ast::{
    ArrayElement, AssignmentOperator, BinaryOperator, CatchClause, Expression, FunctionBody,
    FunctionLiteral, Literal, LogicalOperator, ObjectProperty, Program, PropertyName, Statement,
    SwitchCase, UnaryOperator, UpdateOperator, VariableKind,
};

use super::{
    Chunk, ChunkError, Constant, EnvironmentCapturePolicy, ExceptionHandler, FunctionTemplate,
    HandlerKind, Instruction,
};

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
    breakables: Vec<BreakContext>,
    lexical_scopes: Vec<HashSet<String>>,
    environment_depth: u32,
    /// Number of enclosing function bodies; 0 = top-level script.
    function_depth: usize,
}

#[derive(Debug)]
struct LoopContext {
    /// `Some` for loops whose continue target is already emitted (e.g. `while`,
    /// which continues to the test). `None` for `for` loops, where `continue`
    /// must reach the not-yet-emitted update clause via `continue_jumps`.
    continue_target: Option<usize>,
    continue_jumps: Vec<usize>,
    environment_depth: u32,
}

#[derive(Debug)]
struct BreakContext {
    break_jumps: Vec<usize>,
    environment_depth: u32,
}

impl CompileContext {
    fn inside_function(&self) -> bool {
        self.function_depth > 0
    }

    fn is_lexical(&self, name: &str) -> bool {
        self.lexical_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
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
        let lexical_scope = self.predeclare_lexical_bindings(&program.body, &mut chunk)?;
        context.lexical_scopes.push(lexical_scope);

        // Hoist function declarations to the top of the program scope.
        for statement in &program.body {
            if let Statement::FunctionDeclaration { name, params, body } = statement {
                self.compile_function_declaration(name, params, body, &mut chunk, &mut context)?;
            }
        }

        for (index, statement) in program.body.iter().enumerate() {
            if matches!(statement, Statement::FunctionDeclaration { .. }) {
                continue;
            }
            self.compile_statement(
                statement,
                &mut chunk,
                &mut context,
                Some(index) == completion_expression,
            )?;
        }
        context.lexical_scopes.pop();

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
            Statement::Block(statements) => self.compile_block(statements, chunk, context),
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
            Statement::For {
                init,
                test,
                update,
                body,
            } => self.compile_for(
                init.as_deref(),
                test.as_ref(),
                update.as_ref(),
                body,
                chunk,
                context,
            ),
            Statement::ForIn {
                declaration,
                target,
                right,
                body,
            } => self.compile_for_in(*declaration, target, right, body, chunk, context),
            Statement::Break => self.compile_break(chunk, context),
            Statement::Continue => self.compile_continue(chunk, context),
            Statement::Throw(expression) => {
                self.compile_expression(expression, chunk, context)?;
                chunk.emit(Instruction::Throw);
                Ok(())
            }
            Statement::Return(value) => self.compile_return(value.as_ref(), chunk, context),
            Statement::FunctionDeclaration { name, params, body } => {
                self.compile_function_declaration(name, params, body, chunk, context)
            }
            Statement::Try {
                block,
                handler,
                finalizer,
            } => self.compile_try(
                block,
                handler.as_ref(),
                finalizer.as_deref(),
                chunk,
                context,
            ),
            Statement::Switch {
                discriminant,
                cases,
            } => self.compile_switch(discriminant, cases, chunk, context),
        }
    }

    fn compile_block(
        &mut self,
        statements: &[Statement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let names = lexical_names(statements);
        if names.is_empty() {
            return self.compile_statement_list(statements, chunk, context);
        }

        chunk.emit(Instruction::CreateLexicalEnvironment);
        context.environment_depth += 1;
        let scope = self.predeclare_names(&names, statements, chunk)?;
        context.lexical_scopes.push(scope);
        let result = self.compile_statement_list(statements, chunk, context);
        context.lexical_scopes.pop();
        result?;
        chunk.emit(Instruction::PopEnvironment);
        context.environment_depth -= 1;
        Ok(())
    }

    fn compile_statement_list(
        &mut self,
        statements: &[Statement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        // Hoist function declarations to the top of the current scope.
        for statement in statements {
            if let Statement::FunctionDeclaration { name, params, body } = statement {
                self.compile_function_declaration(name, params, body, chunk, context)?;
            }
        }
        for statement in statements {
            if !matches!(statement, Statement::FunctionDeclaration { .. }) {
                self.compile_statement(statement, chunk, context, false)?;
            }
        }
        Ok(())
    }

    fn predeclare_lexical_bindings(
        &mut self,
        statements: &[Statement],
        chunk: &mut Chunk,
    ) -> Result<HashSet<String>, CompileError> {
        let names = lexical_names(statements);
        self.predeclare_names(&names, statements, chunk)
    }

    fn predeclare_names(
        &mut self,
        names: &[String],
        statements: &[Statement],
        chunk: &mut Chunk,
    ) -> Result<HashSet<String>, CompileError> {
        let mut scope = HashSet::new();
        for name in names {
            let kind = lexical_kind(statements, name)
                .expect("lexical name must originate from a declaration");
            let index = self.add_name(name, chunk)?;
            chunk.emit(match kind {
                VariableKind::Let => Instruction::CreateMutableBinding(index),
                VariableKind::Const => Instruction::CreateImmutableBinding(index),
                VariableKind::Var => unreachable!(),
            });
            scope.insert(name.clone());
        }
        Ok(scope)
    }

    fn compile_try(
        &mut self,
        block: &[Statement],
        handler: Option<&CatchClause>,
        finalizer: Option<&[Statement]>,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let protected_start = chunk.current_offset();
        let handler_environment_depth = context.environment_depth;
        self.compile_block(block, chunk, context)?;
        let protected_end = chunk.current_offset();
        let normal_exit = chunk.emit(Instruction::Jump(usize::MAX));

        let mut catch_exit = None;
        if let Some(handler) = handler {
            let catch_target = chunk.current_offset();
            chunk.emit(Instruction::CreateLexicalEnvironment);
            context.environment_depth += 1;

            let names = lexical_names(&handler.body);
            let mut scope = self.predeclare_names(&names, &handler.body, chunk)?;
            if let Some(parameter) = &handler.parameter {
                let parameter_index = self.add_name(parameter, chunk)?;
                chunk.emit(Instruction::CreateMutableBinding(parameter_index));
                scope.insert(parameter.clone());
                chunk.emit(Instruction::LoadException);
                chunk.emit(Instruction::InitializeBinding(parameter_index));
            } else {
                chunk.emit(Instruction::LoadException);
                chunk.emit(Instruction::Pop);
            }

            context.lexical_scopes.push(scope);
            let result = self.compile_statement_list(&handler.body, chunk, context);
            context.lexical_scopes.pop();
            result?;
            chunk.emit(Instruction::PopEnvironment);
            context.environment_depth -= 1;
            catch_exit = Some(chunk.emit(Instruction::Jump(usize::MAX)));

            if protected_start < protected_end {
                chunk.handlers.push(ExceptionHandler {
                    start: protected_start,
                    end: protected_end,
                    target: catch_target,
                    kind: HandlerKind::Catch,
                    stack_depth: 0,
                    environment_depth: handler_environment_depth,
                });
            }
        }

        if let Some(finalizer) = finalizer {
            let finally_target = chunk.current_offset();
            chunk
                .patch_jump(normal_exit, finally_target)
                .map_err(CompileError::from_chunk)?;
            if let Some(catch_exit) = catch_exit {
                chunk
                    .patch_jump(catch_exit, finally_target)
                    .map_err(CompileError::from_chunk)?;
            }

            self.compile_block(finalizer, chunk, context)?;
            chunk.emit(Instruction::EndFinally);

            if protected_start < finally_target {
                chunk.handlers.push(ExceptionHandler {
                    start: protected_start,
                    end: finally_target,
                    target: finally_target,
                    kind: HandlerKind::Finally,
                    stack_depth: 0,
                    environment_depth: handler_environment_depth,
                });
            }
        } else {
            let end = chunk.current_offset();
            chunk
                .patch_jump(normal_exit, end)
                .map_err(CompileError::from_chunk)?;
            if let Some(catch_exit) = catch_exit {
                chunk
                    .patch_jump(catch_exit, end)
                    .map_err(CompileError::from_chunk)?;
            }
        }

        Ok(())
    }

    fn compile_switch(
        &mut self,
        discriminant: &Expression,
        cases: &[SwitchCase],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        self.compile_expression(discriminant, chunk, context)?;

        let lexical_statements: Vec<Statement> = cases
            .iter()
            .flat_map(|case| case.consequent.iter().cloned())
            .collect();
        let lexical_names = lexical_names(&lexical_statements);
        let has_lexical_scope = !lexical_names.is_empty();
        if has_lexical_scope {
            chunk.emit(Instruction::CreateLexicalEnvironment);
            context.environment_depth += 1;
        }
        let scope = self.predeclare_names(&lexical_names, &lexical_statements, chunk)?;
        context.lexical_scopes.push(scope);

        let mut match_jumps = Vec::new();
        for (index, case) in cases.iter().enumerate() {
            let Some(test) = &case.test else {
                continue;
            };
            chunk.emit(Instruction::Duplicate);
            self.compile_expression(test, chunk, context)?;
            chunk.emit(Instruction::StrictEqual);
            let jump = chunk.emit(Instruction::JumpIfTrue(usize::MAX));
            chunk.emit(Instruction::Pop);
            match_jumps.push((index, jump));
        }

        let default_index = cases.iter().position(|case| case.test.is_none());
        chunk.emit(Instruction::Pop);
        let default_dispatch = chunk.emit(Instruction::Jump(usize::MAX));

        let mut case_stubs = Vec::new();
        for (case_index, match_jump) in match_jumps {
            let stub = chunk.current_offset();
            chunk
                .patch_jump(match_jump, stub)
                .map_err(CompileError::from_chunk)?;
            chunk.emit(Instruction::Pop);
            chunk.emit(Instruction::Pop);
            let body_jump = chunk.emit(Instruction::Jump(usize::MAX));
            case_stubs.push((case_index, body_jump));
        }

        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        let mut body_starts = Vec::with_capacity(cases.len());
        for case in cases {
            body_starts.push(chunk.current_offset());
            self.compile_statement_list(&case.consequent, chunk, context)?;
        }

        let cleanup = chunk.current_offset();
        if has_lexical_scope {
            chunk.emit(Instruction::PopEnvironment);
            context.environment_depth -= 1;
        }
        let end = chunk.current_offset();

        for (case_index, body_jump) in case_stubs {
            chunk
                .patch_jump(body_jump, body_starts[case_index])
                .map_err(CompileError::from_chunk)?;
        }
        let default_target = default_index.map_or(cleanup, |index| body_starts[index]);
        chunk
            .patch_jump(default_dispatch, default_target)
            .map_err(CompileError::from_chunk)?;

        let break_context = context
            .breakables
            .pop()
            .expect("switch break context must exist");
        for jump in break_context.break_jumps {
            chunk
                .patch_jump(jump, cleanup)
                .map_err(CompileError::from_chunk)?;
        }
        context.lexical_scopes.pop();

        debug_assert!(cleanup <= end);
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
            continue_target: Some(loop_start),
            continue_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            return Err(error);
        }
        chunk.emit(Instruction::Jump(loop_start));

        let false_cleanup = chunk.current_offset();
        chunk
            .patch_jump(exit_jump, false_cleanup)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop);
        let loop_end = chunk.current_offset();

        context
            .loops
            .pop()
            .expect("the current while loop context must exist");
        let break_context = context
            .breakables
            .pop()
            .expect("the current while break context must exist");
        for jump in break_context.break_jumps {
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
        if context.breakables.is_empty() {
            return Err(CompileError::unsupported(
                "break statement outside of a loop or switch",
            ));
        }
        let target_environment_depth = context
            .breakables
            .last()
            .expect("checked that a breakable context exists")
            .environment_depth;
        for _ in target_environment_depth..context.environment_depth {
            chunk.emit(Instruction::PopEnvironment);
        }
        let jump = chunk.emit(Instruction::Jump(usize::MAX));
        context
            .breakables
            .last_mut()
            .expect("checked that a breakable context exists")
            .break_jumps
            .push(jump);
        Ok(())
    }

    fn compile_continue(
        &mut self,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let loop_context = context
            .loops
            .last()
            .ok_or_else(|| CompileError::unsupported("continue statement outside of a loop"))?;
        let target_depth = loop_context.environment_depth;
        let continue_target = loop_context.continue_target;
        for _ in target_depth..context.environment_depth {
            chunk.emit(Instruction::PopEnvironment);
        }
        match continue_target {
            Some(target) => {
                chunk.emit(Instruction::Jump(target));
            }
            None => {
                let jump = chunk.emit(Instruction::Jump(usize::MAX));
                context
                    .loops
                    .last_mut()
                    .expect("loop context exists")
                    .continue_jumps
                    .push(jump);
            }
        }
        Ok(())
    }

    /// Compiles a C-style `for (init; test; update) body`. `continue` targets
    /// the update clause; bindings declared in `init` live in a per-loop lexical
    /// environment (simplified single-binding scope, sufficient for the common
    /// `var`/`let` counter pattern).
    fn compile_for(
        &mut self,
        init: Option<&Statement>,
        test: Option<&Expression>,
        update: Option<&Expression>,
        body: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let declared: Vec<(String, VariableKind)> = match init {
            Some(Statement::VariableDeclaration { kind, declarations }) => declarations
                .iter()
                .map(|declarator| (declarator.name.clone(), *kind))
                .collect(),
            _ => Vec::new(),
        };
        let needs_env = !declared.is_empty();
        if needs_env {
            chunk.emit(Instruction::CreateLexicalEnvironment);
            context.environment_depth += 1;
            let mut scope = HashSet::new();
            for (name, kind) in &declared {
                let index = self.add_name(name, chunk)?;
                chunk.emit(match kind {
                    VariableKind::Const => Instruction::CreateImmutableBinding(index),
                    _ => Instruction::CreateMutableBinding(index),
                });
                scope.insert(name.clone());
            }
            context.lexical_scopes.push(scope);
        }

        // init
        match init {
            Some(Statement::VariableDeclaration { declarations, .. }) => {
                for declarator in declarations {
                    match &declarator.initializer {
                        Some(expression) => self.compile_expression(expression, chunk, context)?,
                        None => {
                            let undefined = chunk
                                .add_constant(Constant::Undefined)
                                .map_err(CompileError::from_chunk)?;
                            chunk.emit(Instruction::Constant(undefined));
                        }
                    }
                    let index = self.add_name(&declarator.name, chunk)?;
                    chunk.emit(Instruction::InitializeBinding(index));
                }
            }
            Some(other) => self.compile_statement(other, chunk, context, false)?,
            None => {}
        }

        let loop_start = chunk.current_offset();
        let exit_jump = match test {
            Some(test_expression) => {
                self.compile_expression(test_expression, chunk, context)?;
                let jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
                chunk.emit(Instruction::Pop);
                Some(jump)
            }
            None => None,
        };

        context.loops.push(LoopContext {
            continue_target: None,
            continue_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });

        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            return Err(error);
        }

        // continue lands on the update clause.
        let update_target = chunk.current_offset();
        let continue_jumps = context
            .loops
            .last()
            .expect("for loop context exists")
            .continue_jumps
            .clone();
        for jump in continue_jumps {
            chunk
                .patch_jump(jump, update_target)
                .map_err(CompileError::from_chunk)?;
        }
        if let Some(update_expression) = update {
            self.compile_expression(update_expression, chunk, context)?;
            chunk.emit(Instruction::Pop);
        }
        chunk.emit(Instruction::Jump(loop_start));

        let exit = chunk.current_offset();
        if let Some(jump) = exit_jump {
            chunk
                .patch_jump(jump, exit)
                .map_err(CompileError::from_chunk)?;
            chunk.emit(Instruction::Pop);
        }
        let loop_end = chunk.current_offset();

        context.loops.pop().expect("for loop context exists");
        let break_context = context.breakables.pop().expect("for break context exists");
        for jump in break_context.break_jumps {
            chunk
                .patch_jump(jump, loop_end)
                .map_err(CompileError::from_chunk)?;
        }

        if needs_env {
            context.lexical_scopes.pop();
            chunk.emit(Instruction::PopEnvironment);
            context.environment_depth -= 1;
        }
        Ok(())
    }

    /// Compiles `for (left in right) body` by materializing the enumeration
    /// keys into an array (via `ForInKeys`) and walking it with a hidden index,
    /// both held in a per-loop lexical environment.
    fn compile_for_in(
        &mut self,
        declaration: Option<VariableKind>,
        target: &Expression,
        right: &Expression,
        body: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        // Hidden binding names contain a NUL so they cannot collide with any
        // user identifier.
        const KEYS: &str = "\u{0}forin_keys";
        const INDEX: &str = "\u{0}forin_index";

        // Member targets without a declaration are not yet supported; reject
        // before emitting anything so no partial state leaks.
        if declaration.is_none() && !matches!(target, Expression::Identifier(_)) {
            return Err(CompileError::unsupported(
                "for-in target must be a variable or a simple identifier",
            ));
        }

        chunk.emit(Instruction::CreateLexicalEnvironment);
        context.environment_depth += 1;
        let mut scope = HashSet::new();

        let keys_index = self.add_name(KEYS, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(keys_index));
        scope.insert(KEYS.to_string());
        let cursor_index = self.add_name(INDEX, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(cursor_index));
        scope.insert(INDEX.to_string());

        let loop_var = match (declaration, target) {
            (Some(_), Expression::Identifier(name)) => {
                let var_index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::CreateMutableBinding(var_index));
                let undefined = chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::Constant(undefined));
                chunk.emit(Instruction::InitializeBinding(var_index));
                scope.insert(name.clone());
                Some(name.clone())
            }
            _ => None,
        };
        context.lexical_scopes.push(scope);

        // keys = ForInKeys(ToObject(right)); index = 0
        self.compile_expression(right, chunk, context)?;
        chunk.emit(Instruction::ForInKeys);
        chunk.emit(Instruction::InitializeBinding(keys_index));
        let zero = chunk
            .add_constant(Constant::Number(0.0))
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Constant(zero));
        chunk.emit(Instruction::InitializeBinding(cursor_index));

        let loop_start = chunk.current_offset();
        chunk.emit(Instruction::LoadName(cursor_index));
        chunk.emit(Instruction::LoadName(keys_index));
        let length_index = self.add_name("length", chunk)?;
        chunk.emit(Instruction::GetProperty(length_index));
        chunk.emit(Instruction::LessThan);
        let exit_jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
        chunk.emit(Instruction::Pop);

        // value = keys[index]; assign to the loop target.
        chunk.emit(Instruction::LoadName(keys_index));
        chunk.emit(Instruction::LoadName(cursor_index));
        chunk.emit(Instruction::GetElement);
        match &loop_var {
            Some(name) => {
                let var_index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::StoreName(var_index));
                chunk.emit(Instruction::Pop);
            }
            None => {
                let Expression::Identifier(name) = target else {
                    unreachable!("member targets rejected above");
                };
                self.emit_store_identifier(name, chunk, context)?;
                chunk.emit(Instruction::Pop);
            }
        }

        context.loops.push(LoopContext {
            continue_target: None,
            continue_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });

        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            return Err(error);
        }

        let update_target = chunk.current_offset();
        let continue_jumps = context
            .loops
            .last()
            .expect("for-in loop context exists")
            .continue_jumps
            .clone();
        for jump in continue_jumps {
            chunk
                .patch_jump(jump, update_target)
                .map_err(CompileError::from_chunk)?;
        }
        chunk.emit(Instruction::LoadName(cursor_index));
        let one = chunk
            .add_constant(Constant::Number(1.0))
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Constant(one));
        chunk.emit(Instruction::Add);
        chunk.emit(Instruction::StoreName(cursor_index));
        chunk.emit(Instruction::Pop);
        chunk.emit(Instruction::Jump(loop_start));

        let exit = chunk.current_offset();
        chunk
            .patch_jump(exit_jump, exit)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop);
        let loop_end = chunk.current_offset();

        context.loops.pop().expect("for-in loop context exists");
        let break_context = context
            .breakables
            .pop()
            .expect("for-in break context exists");
        for jump in break_context.break_jumps {
            chunk
                .patch_jump(jump, loop_end)
                .map_err(CompileError::from_chunk)?;
        }

        context.lexical_scopes.pop();
        chunk.emit(Instruction::PopEnvironment);
        context.environment_depth -= 1;
        Ok(())
    }

    /// Compiles `++x` / `x++` / `--x` / `x--` on an identifier operand. The
    /// operand is read with `ToNumber` semantics so `"5"++` yields `6`.
    fn compile_update(
        &mut self,
        operator: UpdateOperator,
        prefix: bool,
        argument: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let Expression::Identifier(name) = argument else {
            return Err(CompileError::unsupported(
                "`++`/`--` is only supported on identifier operands",
            ));
        };
        let step = match operator {
            UpdateOperator::Increment => Instruction::Add,
            UpdateOperator::Decrement => Instruction::Subtract,
        };
        self.compile_identifier(name, chunk, context)?;
        chunk.emit(Instruction::UnaryPlus); // ToNumber(old)
        let one = chunk
            .add_constant(Constant::Number(1.0))
            .map_err(CompileError::from_chunk)?;
        if prefix {
            chunk.emit(Instruction::Constant(one));
            chunk.emit(step);
            self.emit_store_identifier(name, chunk, context)?;
        } else {
            chunk.emit(Instruction::Duplicate);
            chunk.emit(Instruction::Constant(one));
            chunk.emit(step);
            self.emit_store_identifier(name, chunk, context)?;
            chunk.emit(Instruction::Pop);
        }
        Ok(())
    }

    /// Emits the store instruction for an identifier assignment target.
    fn emit_store_identifier(
        &mut self,
        name: &str,
        chunk: &mut Chunk,
        context: &CompileContext,
    ) -> Result<(), CompileError> {
        let index = self.add_name(name, chunk)?;
        if context.inside_function() || context.is_lexical(name) {
            chunk.emit(Instruction::StoreName(index));
        } else {
            chunk.emit(Instruction::StoreGlobal(index));
        }
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
    fn compile_function_declaration(
        &mut self,
        name: &str,
        params: &[crate::ast::FunctionParam],
        body: &FunctionBody,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let fn_chunk =
            self.compile_function_body(params.iter().map(|p| p.name.as_str()), body, context)?;
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
            breakables: Vec::new(),
            lexical_scopes: Vec::new(),
            environment_depth: 0,
            function_depth: outer_context.function_depth + 1,
        };
        let lexical_scope = self.predeclare_lexical_bindings(&body.statements, &mut fn_chunk)?;
        fn_context.lexical_scopes.push(lexical_scope);
        // Hoist function declarations within the function body.
        for statement in &body.statements {
            if let Statement::FunctionDeclaration {
                name,
                params,
                body: inner_body,
            } = statement
            {
                self.compile_function_declaration(
                    name,
                    params,
                    inner_body,
                    &mut fn_chunk,
                    &mut fn_context,
                )?;
            }
        }
        // Compile the body statements; no "completion expression" inside function bodies.
        for statement in &body.statements {
            if !matches!(statement, Statement::FunctionDeclaration { .. }) {
                self.compile_statement(statement, &mut fn_chunk, &mut fn_context, false)?;
            }
        }
        fn_context.lexical_scopes.pop();
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
            Expression::Update {
                operator,
                prefix,
                argument,
            } => self.compile_update(*operator, *prefix, argument, chunk, context),
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
            Expression::Assignment {
                operator,
                target,
                value,
            } => self.compile_assignment(operator, target, value, chunk, context),
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
        if context.inside_function() || context.is_lexical(name) {
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
            UnaryOperator::Void => {
                self.compile_expression(argument, chunk, context)?;
                chunk.emit(Instruction::Pop);
                return self.compile_literal(&Literal::Undefined, chunk);
            }
            UnaryOperator::Delete => {
                return self.compile_delete(argument, chunk, context);
            }
            UnaryOperator::TypeOf => {
                if let Expression::Identifier(name) = argument {
                    let name_index = self.add_name(name, chunk)?;
                    if context.inside_function() || context.is_lexical(name) {
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
            BinaryOperator::Equal => Instruction::Equal,
            BinaryOperator::NotEqual => Instruction::NotEqual,
            BinaryOperator::StrictEqual => Instruction::StrictEqual,
            BinaryOperator::StrictNotEqual => Instruction::StrictNotEqual,
            BinaryOperator::LessThan => Instruction::LessThan,
            BinaryOperator::LessThanOrEqual => Instruction::LessThanOrEqual,
            BinaryOperator::GreaterThan => Instruction::GreaterThan,
            BinaryOperator::GreaterThanOrEqual => Instruction::GreaterThanOrEqual,
            BinaryOperator::In => Instruction::HasProperty,
            BinaryOperator::InstanceOf => Instruction::InstanceOf,
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => unreachable!(),
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
        if kind == VariableKind::Const && initializer.is_none() {
            return Err(CompileError::unsupported(
                "const declaration without an initializer",
            ));
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
        match kind {
            VariableKind::Var if context.inside_function() => {
                chunk.emit(Instruction::DeclareLocal(name_index));
            }
            VariableKind::Var => {
                chunk.emit(Instruction::DeclareGlobal(name_index));
            }
            VariableKind::Let | VariableKind::Const => {
                chunk.emit(Instruction::InitializeBinding(name_index));
            }
        }
        Ok(())
    }

    fn compile_assignment(
        &mut self,
        operator: &AssignmentOperator,
        target: &Expression,
        value: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match (operator, target) {
            // ── Simple `=` on identifier ───────────────────────────────────────
            (AssignmentOperator::Assign, Expression::Identifier(name)) => {
                self.compile_expression(value, chunk, context)?;
                self.emit_store_identifier(name, chunk, context)?;
                Ok(())
            }
            // ── Compound `op=` on identifier ──────────────────────────────────
            (op, Expression::Identifier(name)) => {
                // load current value, compile rhs, apply op, store back
                self.compile_identifier(name, chunk, context)?;
                self.compile_expression(value, chunk, context)?;
                let instr = compound_op_instruction(op);
                chunk.emit(instr);
                self.emit_store_identifier(name, chunk, context)?;
                Ok(())
            }
            // ── Simple `=` on static member ───────────────────────────────────
            (
                AssignmentOperator::Assign,
                Expression::Member {
                    object,
                    property,
                    computed: false,
                },
            ) => {
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
            // ── Simple `=` on computed member ─────────────────────────────────
            (
                AssignmentOperator::Assign,
                Expression::Member {
                    object,
                    property,
                    computed: true,
                },
            ) => {
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
                ObjectProperty::ComputedData { key, value } => {
                    chunk.emit(Instruction::Duplicate);
                    self.compile_expression(key, chunk, context)?;
                    self.compile_expression(value, chunk, context)?;
                    chunk.emit(Instruction::SetElement);
                    chunk.emit(Instruction::Pop);
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

fn compound_op_instruction(op: &AssignmentOperator) -> Instruction {
    match op {
        AssignmentOperator::PlusAssign => Instruction::Add,
        AssignmentOperator::MinusAssign => Instruction::Subtract,
        AssignmentOperator::MulAssign => Instruction::Multiply,
        AssignmentOperator::DivAssign => Instruction::Divide,
        AssignmentOperator::ModAssign => Instruction::Remainder,
        AssignmentOperator::Assign => unreachable!("Assign handled before compound branch"),
    }
}

fn property_key(key: &PropertyName) -> String {
    key.to_key_string()
}

fn lexical_names(statements: &[Statement]) -> Vec<String> {
    statements
        .iter()
        .flat_map(|statement| match statement {
            Statement::VariableDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                declarations,
            } => declarations
                .iter()
                .map(|declaration| declaration.name.clone())
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn lexical_kind(statements: &[Statement], name: &str) -> Option<VariableKind> {
    statements.iter().find_map(|statement| match statement {
        Statement::VariableDeclaration { kind, declarations }
            if matches!(kind, VariableKind::Let | VariableKind::Const)
                && declarations
                    .iter()
                    .any(|declaration| declaration.name == name) =>
        {
            Some(*kind)
        }
        _ => None,
    })
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
