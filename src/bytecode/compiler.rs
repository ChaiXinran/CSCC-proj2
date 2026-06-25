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
            if let Statement::FunctionDeclaration {
                name, params, body, ..
            } = statement
            {
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
            Statement::ForIn { left, right, body } => {
                self.compile_for_in(left, right, body, chunk, context)
            }
            Statement::Break => self.compile_break(chunk, context),
            Statement::Continue => self.compile_continue(chunk, context),
            Statement::Throw(expression) => {
                self.compile_expression(expression, chunk, context)?;
                chunk.emit(Instruction::Throw);
                Ok(())
            }
            Statement::Return(value) => self.compile_return(value.as_ref(), chunk, context),
            Statement::FunctionDeclaration {
                name, params, body, ..
            } => self.compile_function_declaration(name, params, body, chunk, context),
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
            Statement::ClassDeclaration(decl) => {
                self.compile_class_declaration(decl, chunk, context)
            }
            Statement::DestructuringDeclaration {
                kind,
                pattern,
                initializer,
            } => {
                self.compile_destructuring_declaration(*kind, pattern, initializer, chunk, context)
            }
            Statement::ForOf {
                left,
                right,
                body,
                is_await,
            } => self.compile_for_of(left, right, body, *is_await, chunk, context),
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
            if let Statement::FunctionDeclaration {
                name, params, body, ..
            } = statement
            {
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
        left: &crate::ast::ForBinding,
        right: &Expression,
        body: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        const KEYS: &str = "\u{0}forin_keys";
        const INDEX: &str = "\u{0}forin_index";

        chunk.emit(Instruction::CreateLexicalEnvironment);
        context.environment_depth += 1;
        let mut scope = HashSet::new();

        let keys_index = self.add_name(KEYS, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(keys_index));
        scope.insert(KEYS.to_string());
        let cursor_index = self.add_name(INDEX, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(cursor_index));
        scope.insert(INDEX.to_string());

        // Declare the loop variable(s) up front.
        match left {
            crate::ast::ForBinding::Declaration { kind, pattern } => {
                let undefined = chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                for name in binding_pattern_names(pattern) {
                    let idx = self.add_name(&name, chunk)?;
                    chunk.emit(Instruction::CreateMutableBinding(idx));
                    chunk.emit(Instruction::Constant(undefined));
                    chunk.emit(Instruction::InitializeBinding(idx));
                    scope.insert(name.clone());
                }
                let _ = kind; // used below
            }
            crate::ast::ForBinding::Target(_) => {}
        }
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

        // value = keys[index]; assign to the loop variable.
        chunk.emit(Instruction::LoadName(keys_index));
        chunk.emit(Instruction::LoadName(cursor_index));
        chunk.emit(Instruction::GetElement);
        match left {
            crate::ast::ForBinding::Declaration { kind, pattern } => {
                match (kind, pattern) {
                    (_, crate::ast::BindingPattern::Identifier(name)) => {
                        let idx = self.add_name(name, chunk)?;
                        chunk.emit(Instruction::StoreName(idx));
                        chunk.emit(Instruction::Pop);
                    }
                    (k, pat) => {
                        self.compile_binding_pattern(*k, pat, chunk, context)?;
                    }
                }
            }
            crate::ast::ForBinding::Target(target) => {
                match target {
                    Expression::Identifier(name) => {
                        self.emit_store_identifier(name, chunk, context)?;
                        chunk.emit(Instruction::Pop);
                    }
                    _ => {
                        return Err(CompileError::unsupported(
                            "for-in target must be a variable or a simple identifier",
                        ));
                    }
                }
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

    /// Compiles `++x` / `x++` / `--x` / `x--`.
    ///
    /// Supports identifier operands and static/computed member expression
    /// operands. The operand is coerced to a number (`ToNumber` / `UnaryPlus`)
    /// before the step, matching the ECMAScript specification.
    fn compile_update(
        &mut self,
        operator: UpdateOperator,
        prefix: bool,
        argument: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let step = match operator {
            UpdateOperator::Increment => Instruction::Add,
            UpdateOperator::Decrement => Instruction::Subtract,
        };
        let one = chunk
            .add_constant(Constant::Number(1.0))
            .map_err(CompileError::from_chunk)?;

        match argument {
            Expression::Identifier(name) => {
                self.compile_identifier(name, chunk, context)?;
                chunk.emit(Instruction::UnaryPlus); // ToNumber(old)
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
            }
            Expression::Member {
                object,
                property,
                computed: false,
            } => {
                let Expression::Identifier(prop_name) = property.as_ref() else {
                    return Err(CompileError::unsupported(
                        "non-identifier static property in `++`/`--`",
                    ));
                };
                let prop_index = self.add_name(prop_name, chunk)?;
                // Stack before step: [obj, old_num]
                self.compile_expression(object, chunk, context)?;
                chunk.emit(Instruction::Duplicate); // [obj, obj]
                chunk.emit(Instruction::GetProperty(prop_index)); // [obj, old]
                chunk.emit(Instruction::UnaryPlus); // [obj, old_num]
                if prefix {
                    // Result = new: [obj, old_num, 1] → [obj, new] → SetProperty → [new]
                    chunk.emit(Instruction::Constant(one));
                    chunk.emit(step);
                    chunk.emit(Instruction::SetProperty(prop_index));
                } else {
                    // Result = old: use DuplicatePair to save [obj, old_num], compute new,
                    // SetProperty, pop new, pop extra obj  → leaves old_num.
                    // Stack trace: [obj, old_num]
                    //   DuplicatePair → [obj, old_num, obj, old_num]
                    //   Constant(1)   → [obj, old_num, obj, old_num, 1]
                    //   step          → [obj, old_num, obj, new]
                    //   SetProperty   → [obj, old_num, new]   (SetProperty: [o,v]→[v])
                    //   Pop           → [obj, old_num]
                    //   Pop           → [old_num]
                    chunk.emit(Instruction::DuplicatePair);
                    chunk.emit(Instruction::Constant(one));
                    chunk.emit(step);
                    chunk.emit(Instruction::SetProperty(prop_index));
                    chunk.emit(Instruction::Pop);
                    chunk.emit(Instruction::Pop);
                }
            }
            Expression::Member {
                object,
                property,
                computed: true,
            } => {
                // Stack before step: [obj, key, old_num]
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::DuplicatePair); // [obj, key, obj, key]
                chunk.emit(Instruction::GetElement); // [obj, key, old]
                chunk.emit(Instruction::UnaryPlus); // [obj, key, old_num]
                if prefix {
                    // [obj, key, old_num, 1] → [obj, key, new] → SetElement → [new]
                    chunk.emit(Instruction::Constant(one));
                    chunk.emit(step);
                    chunk.emit(Instruction::SetElement);
                } else {
                    // Need to leave old_num as result after storing new.
                    // Use Duplicate to save old_num, then build [obj, key, new]
                    // for SetElement. This requires moving old_num out of the way.
                    //
                    // Stack trace: [obj, key, old_num]
                    //   Duplicate    → [obj, key, old_num, old_num]
                    //   Constant(1)  → [obj, key, old_num, old_num, 1]
                    //   step         → [obj, key, old_num, new]
                    //   — need SetElement([obj, key, new]) but stack has
                    //     extra old_num; re-read element from duplicated [obj,key].
                    //   — Instead: evaluate object and key again (safe for simple
                    //     cases; may double side-effects for complex expressions).
                    //
                    // Simpler sequence that re-evaluates object/key:
                    //   Duplicate (old_num) → save it deeper
                    //   compile(object)
                    //   compile(key_expr)
                    //   stack: [obj, key, old_num, old_num, obj2, key2]
                    //   — but key_expr may not be re-evaluatable here easily.
                    //
                    // Compromise: for computed postfix we emit a Duplicate to
                    // preserve old, compute new into a fresh [obj,key,new] pair
                    // by using the saved [obj,key] still below old_num.
                    //
                    // Final chosen sequence (avoids re-evaluation):
                    //   [obj, key, old_num]
                    //   Duplicate       → [obj, key, old_num, old_num]
                    //   Constant(1)     → [obj, key, old_num, old_num, 1]
                    //   step            → [obj, key, old_num, new]
                    //   — rotate new below old_num to make [obj, key, new, old_num]
                    //   — then SetElement([obj,key,new])→[old_num]
                    // Without a rotate instruction we fall back to DuplicatePair
                    // on [old_num] which would only dup 1 value.
                    //
                    // Use DuplicatePair on the full 4-value window:
                    // Before GetElement we had [obj,key,obj,key] from above.
                    // After GetElement: [obj,key,old]. After UnaryPlus: [obj,key,old_num].
                    // The saved [obj,key] are still at positions -3,-2 below old_num.
                    // Stack: bottom→top: [obj_s, key_s, old_num]  (obj_s/key_s = saved copies)
                    //
                    // Emit: Duplicate → [obj_s, key_s, old_num, old_num]
                    //        Constant, step → [obj_s, key_s, old_num, new]
                    //        SetElement consumes TOP 3 as [object=key_s, key=old_num, value=new]
                    //        which is WRONG.
                    //
                    // There is no clean way to express computed-member postfix
                    // update without a dedicated swap/rotate instruction. Emit an
                    // unsupported error for this specific case; prefix computed and
                    // all static-member variants work correctly above.
                    return Err(CompileError::unsupported(
                        "postfix `++`/`--` on a computed member expression is not yet supported; \
                         use prefix `++`/`--` or assign to a local variable first",
                    ));
                }
            }
            _ => {
                return Err(CompileError::unsupported(
                    "`++`/`--` requires an identifier or member expression operand",
                ));
            }
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
        let fn_chunk = self.compile_function_body(params, body, context)?;
        let template = FunctionTemplate {
            name: Some(name.to_string()),
            params: fn_chunk.params,
            rest_param: fn_chunk.rest_param,
            chunk: fn_chunk.chunk,
            is_strict: fn_chunk.is_strict,
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
    /// `FunctionTemplate` and returns it with parameter names and rest info.
    fn compile_function_body(
        &mut self,
        params: &[crate::ast::FunctionParam],
        body: &FunctionBody,
        outer_context: &mut CompileContext,
    ) -> Result<CompiledFunction, CompileError> {
        use crate::ast::FunctionParam;

        // Build positional param names and rest param name for the FunctionTemplate.
        // For destructuring/default params, we generate placeholder names and emit
        // preamble code at function entry to apply defaults and destructure.
        let mut param_names: Vec<String> = Vec::new();
        let mut rest_param: Option<String> = None;
        // Each preamble item: (placeholder_name, param_reference)
        // `is_default` = true means it's a simple param with a default value;
        // false means it's a pattern (destructuring) param.
        let mut preamble: Vec<(String, &FunctionParam)> = Vec::new();

        for p in params {
            match p {
                FunctionParam::Simple(name) => {
                    param_names.push(name.clone());
                }
                FunctionParam::Default(name, _) => {
                    param_names.push(name.clone());
                    preamble.push((name.clone(), p));
                }
                FunctionParam::Pattern(..) => {
                    let placeholder = format!("$p{}", param_names.len());
                    param_names.push(placeholder.clone());
                    preamble.push((placeholder, p));
                }
                FunctionParam::Rest(name) => {
                    rest_param = Some(name.clone());
                }
                FunctionParam::RestPattern(_) => {
                    let placeholder = "$rest_pat".to_string();
                    rest_param = Some(placeholder.clone());
                    preamble.push((placeholder, p));
                }
            }
        }

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

        // Emit preamble: default-value checks and pattern destructuring.
        //
        // NOTE: JumpIfFalse/JumpIfTrue are PEEK instructions (they do not pop
        // their operand). Stack depths must account for the peeked value being
        // present after the conditional branch.
        for (placeholder, param) in &preamble {
            match param {
                FunctionParam::Default(name, default_expr) => {
                    // if param === undefined, store the default value.
                    // Stack before: []
                    let name_idx = self.add_name(name, &mut fn_chunk)?;
                    fn_chunk.emit(Instruction::LoadName(name_idx)); // [param_val]
                    fn_chunk.emit(Instruction::Duplicate); // [param_val, param_val]
                    let undef_c = fn_chunk
                        .add_constant(Constant::Undefined)
                        .map_err(CompileError::from_chunk)?;
                    fn_chunk.emit(Instruction::Constant(undef_c)); // [param_val, param_val, undefined]
                    fn_chunk.emit(Instruction::StrictEqual); // [param_val, is_undef]
                    // JumpIfFalse peeks: jumps when is_undef=false (NOT undefined)
                    let jump_not_undef = fn_chunk.emit(Instruction::JumpIfFalse(usize::MAX));
                    // IS undefined path: [param_val, is_undef(=true)]
                    fn_chunk.emit(Instruction::Pop); // [param_val]   (remove is_undef)
                    fn_chunk.emit(Instruction::Pop); // []             (remove undefined param_val)
                    self.compile_expression(default_expr, &mut fn_chunk, &mut fn_context)?; // [default]
                    fn_chunk.emit(Instruction::StoreName(name_idx)); // [default]
                    fn_chunk.emit(Instruction::Pop); // []
                    let jump_end = fn_chunk.emit(Instruction::Jump(usize::MAX));
                    // NOT undefined path: [param_val, is_undef(=false)]
                    let not_undef = fn_chunk.current_offset();
                    fn_chunk
                        .patch_jump(jump_not_undef, not_undef)
                        .map_err(CompileError::from_chunk)?;
                    fn_chunk.emit(Instruction::Pop); // [param_val] (remove is_undef)
                    fn_chunk.emit(Instruction::Pop); // []          (discard — already bound)
                    // end:
                    let end = fn_chunk.current_offset();
                    fn_chunk
                        .patch_jump(jump_end, end)
                        .map_err(CompileError::from_chunk)?;
                    // Stack: []
                }
                FunctionParam::Pattern(pattern, default_expr) => {
                    let ph_idx = self.add_name(placeholder, &mut fn_chunk)?;
                    fn_chunk.emit(Instruction::LoadName(ph_idx)); // [arg_val]
                    if let Some(def) = default_expr {
                        self.emit_binding_default(def, &mut fn_chunk, &mut fn_context)?;
                    }
                    self.compile_binding_pattern(
                        VariableKind::Var,
                        pattern,
                        &mut fn_chunk,
                        &mut fn_context,
                    )?; // []
                }
                FunctionParam::RestPattern(pattern) => {
                    let ph_idx = self.add_name(placeholder, &mut fn_chunk)?;
                    fn_chunk.emit(Instruction::LoadName(ph_idx)); // [rest_array]
                    self.compile_binding_pattern(
                        VariableKind::Var,
                        pattern,
                        &mut fn_chunk,
                        &mut fn_context,
                    )?; // []
                }
                _ => unreachable!("only Default/Pattern/RestPattern go in preamble"),
            }
        }

        // Hoist function declarations within the function body.
        for statement in &body.statements {
            if let Statement::FunctionDeclaration {
                name,
                params,
                body: inner_body,
                ..
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
            rest_param,
            chunk: fn_chunk,
            is_strict: body.is_strict,
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
            Expression::Assignment { target, value } => {
                self.compile_assignment(target, value, chunk, context)
            }
            Expression::CompoundAssignment {
                operator,
                target,
                value,
            } => self.compile_compound_assignment(*operator, target, value, chunk, context),
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
            Expression::TemplateLiteral(tl) => self.compile_template_literal(tl, chunk, context),
            Expression::Spread(_) => Err(CompileError {
                message: "spread expression is only valid inside call arguments or array literals"
                    .into(),
            }),
            Expression::Class(cls) => self.compile_class_expression(cls, chunk, context),
            Expression::This => {
                chunk.emit(Instruction::LoadThis);
                Ok(())
            }
            Expression::Super => {
                // In a method body, `super` as a property-access base.
                // We push `this` here; super-property lookup handled by the
                // prototype chain at runtime.
                chunk.emit(Instruction::LoadThis);
                Ok(())
            }
            Expression::Yield { argument, delegate } => {
                if let Some(arg) = argument {
                    self.compile_expression(arg, chunk, context)?;
                } else {
                    let undef = chunk
                        .add_constant(Constant::Undefined)
                        .map_err(CompileError::from_chunk)?;
                    chunk.emit(Instruction::Constant(undef));
                }
                if *delegate {
                    chunk.emit(Instruction::YieldDelegate);
                } else {
                    chunk.emit(Instruction::YieldValue);
                }
                Ok(())
            }
            Expression::Await(value) => {
                self.compile_expression(value, chunk, context)?;
                chunk.emit(Instruction::AwaitValue);
                Ok(())
            }
        }
    }

    fn compile_literal(
        &mut self,
        literal: &Literal,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        // RegExp literals are lowered to two string constants + CreateRegExp.
        if let Literal::RegExp { pattern, flags } = literal {
            let pat_idx = chunk
                .add_constant(Constant::String(pattern.clone()))
                .map_err(CompileError::from_chunk)?;
            chunk.emit(Instruction::Constant(pat_idx));
            let flags_idx = chunk
                .add_constant(Constant::String(flags.clone()))
                .map_err(CompileError::from_chunk)?;
            chunk.emit(Instruction::Constant(flags_idx));
            chunk.emit(Instruction::CreateRegExp);
            return Ok(());
        }
        let constant = match literal {
            Literal::Undefined => Constant::Undefined,
            Literal::Null => Constant::Null,
            Literal::Boolean(value) => Constant::Boolean(*value),
            Literal::Number(value) => Constant::Number(*value),
            Literal::BigInt(raw) => {
                return Err(CompileError::unsupported(format_args!(
                    "BigInt literal `{raw}` until V10-C installs BigInt runtime semantics"
                )));
            }
            Literal::String(value) => Constant::String(value.clone()),
            Literal::RegExp { .. } => unreachable!(),
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
        if name == "this" {
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
        target: &Expression,
        value: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match target {
            Expression::Identifier(name) => {
                self.compile_expression(value, chunk, context)?;
                self.emit_store_identifier(name, chunk, context)?;
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: false,
            } => {
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

    fn compile_compound_assignment(
        &mut self,
        operator: AssignmentOperator,
        target: &Expression,
        value: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let instruction = compound_assignment_instruction(operator);
        match target {
            Expression::Identifier(name) => {
                self.compile_identifier(name, chunk, context)?;
                self.compile_expression(value, chunk, context)?;
                chunk.emit(instruction);
                self.emit_store_identifier(name, chunk, context)
            }
            Expression::Member {
                object,
                property,
                computed: false,
            } => {
                let Expression::Identifier(property_name) = property.as_ref() else {
                    return Err(CompileError::unsupported(
                        "non-identifier static member as compound assignment target",
                    ));
                };
                self.compile_expression(object, chunk, context)?;
                chunk.emit(Instruction::Duplicate);
                let property_index = self.add_name(property_name, chunk)?;
                chunk.emit(Instruction::GetProperty(property_index));
                self.compile_expression(value, chunk, context)?;
                chunk.emit(instruction);
                chunk.emit(Instruction::SetProperty(property_index));
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: true,
            } => {
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::DuplicatePair);
                chunk.emit(Instruction::GetElement);
                self.compile_expression(value, chunk, context)?;
                chunk.emit(instruction);
                chunk.emit(Instruction::SetElement);
                Ok(())
            }
            _ => Err(CompileError::unsupported(format!(
                "compound assignment target {target:?}"
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
        arguments: &[crate::ast::CallArgument],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::CallArgument;
        let has_spread = arguments
            .iter()
            .any(|a| matches!(a, CallArgument::Spread(_)));

        // Static member calls preserve their receiver as `this`.
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
            chunk.emit(Instruction::GetMethod(method_index));

            if has_spread {
                let (n_regular, spread_expr) =
                    self.split_trailing_spread(arguments, "method call")?;
                let n = u16::try_from(n_regular).map_err(|_| CompileError {
                    message: "too many call arguments".into(),
                })?;
                for arg in &arguments[..n_regular] {
                    let CallArgument::Expression(e) = arg else {
                        unreachable!()
                    };
                    self.compile_expression(e, chunk, context)?;
                }
                self.compile_expression(spread_expr, chunk, context)?;
                chunk.emit(Instruction::SpreadCallWithThis(n));
            } else {
                let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                    message: "call argument count exceeds the u16 bytecode range".into(),
                })?;
                for arg in arguments {
                    let CallArgument::Expression(e) = arg else {
                        unreachable!()
                    };
                    self.compile_expression(e, chunk, context)?;
                }
                chunk.emit(Instruction::CallWithThis(argument_count));
            }
            return Ok(());
        }

        self.compile_expression(callee, chunk, context)?;
        if has_spread {
            let (n_regular, spread_expr) =
                self.split_trailing_spread(arguments, "function call")?;
            let n = u16::try_from(n_regular).map_err(|_| CompileError {
                message: "too many call arguments".into(),
            })?;
            for arg in &arguments[..n_regular] {
                let CallArgument::Expression(e) = arg else {
                    unreachable!()
                };
                self.compile_expression(e, chunk, context)?;
            }
            self.compile_expression(spread_expr, chunk, context)?;
            chunk.emit(Instruction::SpreadCall(n));
        } else {
            let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                message: "call argument count exceeds the u16 bytecode range".into(),
            })?;
            for arg in arguments {
                let CallArgument::Expression(e) = arg else {
                    unreachable!()
                };
                self.compile_expression(e, chunk, context)?;
            }
            chunk.emit(Instruction::Call(argument_count));
        }
        Ok(())
    }

    fn compile_construct(
        &mut self,
        callee: &Expression,
        arguments: &[crate::ast::CallArgument],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::CallArgument;
        let has_spread = arguments
            .iter()
            .any(|a| matches!(a, CallArgument::Spread(_)));

        self.compile_expression(callee, chunk, context)?;
        if has_spread {
            let (n_regular, spread_expr) =
                self.split_trailing_spread(arguments, "new expression")?;
            let n = u16::try_from(n_regular).map_err(|_| CompileError {
                message: "too many construct arguments".into(),
            })?;
            for arg in &arguments[..n_regular] {
                let CallArgument::Expression(e) = arg else {
                    unreachable!()
                };
                self.compile_expression(e, chunk, context)?;
            }
            self.compile_expression(spread_expr, chunk, context)?;
            chunk.emit(Instruction::SpreadConstruct(n));
        } else {
            let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                message: "construct argument count exceeds the u16 bytecode range".into(),
            })?;
            for arg in arguments {
                let CallArgument::Expression(e) = arg else {
                    unreachable!()
                };
                self.compile_expression(e, chunk, context)?;
            }
            chunk.emit(Instruction::Construct(argument_count));
        }
        Ok(())
    }

    /// Returns `(n_regular, spread_expr)` when the argument list has exactly
    /// one trailing spread and all preceding args are plain expressions.
    /// Returns `CompileError` if there are multiple spreads or non-trailing spread.
    fn split_trailing_spread<'a>(
        &self,
        arguments: &'a [crate::ast::CallArgument],
        ctx: &str,
    ) -> Result<(usize, &'a Expression), CompileError> {
        use crate::ast::CallArgument;
        let spread_count = arguments
            .iter()
            .filter(|a| matches!(a, CallArgument::Spread(_)))
            .count();
        if spread_count != 1 {
            return Err(CompileError {
                message: format!(
                    "{ctx}: only a single trailing spread argument is supported in V8"
                ),
            });
        }
        let last = arguments.last().expect("at least one spread");
        let CallArgument::Spread(spread_expr) = last else {
            return Err(CompileError {
                message: format!(
                    "{ctx}: spread must be the last argument in V8 (non-trailing spread unsupported)"
                ),
            });
        };
        Ok((arguments.len() - 1, spread_expr))
    }

    fn compile_array(
        &mut self,
        elements: &[ArrayElement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let has_spread_or_hole = elements
            .iter()
            .any(|e| !matches!(e, ArrayElement::Expression(_)));

        // Fast path: dense all-expression array with no spreads.
        if !has_spread_or_hole {
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

        // General path: may contain holes or spread elements.
        // Build an empty array and push/spread each element dynamically.
        let has_spread = elements
            .iter()
            .any(|e| matches!(e, ArrayElement::Spread(_)));
        if has_spread {
            chunk.emit(Instruction::ArrayCreateSparse(0));
            for element in elements {
                match element {
                    ArrayElement::Hole => {
                        // Holes in spread arrays: push undefined then ArrayPush.
                        let undef = chunk
                            .add_constant(Constant::Undefined)
                            .map_err(CompileError::from_chunk)?;
                        chunk.emit(Instruction::Constant(undef));
                        chunk.emit(Instruction::ArrayPush);
                    }
                    ArrayElement::Expression(expr) => {
                        self.compile_expression(expr, chunk, context)?;
                        chunk.emit(Instruction::ArrayPush);
                    }
                    ArrayElement::Spread(expr) => {
                        self.compile_expression(expr, chunk, context)?;
                        chunk.emit(Instruction::SpreadIntoArray);
                    }
                }
            }
            return Ok(());
        }

        // Sparse path (holes but no spread).
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
                    self.compile_accessor_function(&[], body, chunk, context)?;
                    let key = self.add_name(&property_key(key), chunk)?;
                    chunk.emit(Instruction::DefineGetter(key));
                }
                ObjectProperty::Setter {
                    key,
                    parameter,
                    body,
                } => {
                    self.compile_accessor_function(
                        std::slice::from_ref(parameter),
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
            // `delete identifier` — strict mode is rejected at parse time.
            // In sloppy mode, declared bindings are non-configurable and
            // `delete` always returns `false` for them.
            Expression::Identifier(_) => {
                let false_idx = chunk
                    .add_constant(Constant::Boolean(false))
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::Constant(false_idx));
                Ok(())
            }
            // `delete (non-reference)` — e.g. `delete 1`, `delete (a + b)`.
            // The operand is evaluated for side effects, popped, and `true`
            // is pushed (deleting a non-reference always succeeds per spec).
            _ => {
                self.compile_expression(argument, chunk, context)?;
                chunk.emit(Instruction::Pop);
                let true_idx = chunk
                    .add_constant(Constant::Boolean(true))
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::Constant(true_idx));
                Ok(())
            }
        }
    }

    fn compile_accessor_function(
        &mut self,
        params: &[crate::ast::FunctionParam],
        body: &FunctionBody,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let compiled = self.compile_function_body(params, body, context)?;
        let template = FunctionTemplate {
            name: None,
            params: compiled.params,
            rest_param: compiled.rest_param,
            chunk: compiled.chunk,
            is_strict: compiled.is_strict,
            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
        };
        let index = chunk
            .add_function(template)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::CreateFunction(index));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // V8-A: template literals
    // -----------------------------------------------------------------------

    fn compile_template_literal(
        &mut self,
        tl: &crate::ast::TemplateLiteral,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        // quasis.len() == expressions.len() + 1.
        // Compile as: quasis[0] + toString(expr[0]) + quasis[1] + ...
        // The initial string quasi makes Add treat subsequent values as strings.
        let first_idx = chunk
            .add_constant(Constant::String(tl.quasis[0].clone()))
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Constant(first_idx));
        for (expr, quasi) in tl.expressions.iter().zip(tl.quasis[1..].iter()) {
            self.compile_expression(expr, chunk, context)?;
            chunk.emit(Instruction::Add); // string + expr → string (coerces expr)
            let q_idx = chunk
                .add_constant(Constant::String(quasi.clone()))
                .map_err(CompileError::from_chunk)?;
            chunk.emit(Instruction::Constant(q_idx));
            chunk.emit(Instruction::Add); // prev_string + quasi → string
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // V8-A: class declarations and expressions
    // -----------------------------------------------------------------------

    /// Emits bytecode for a class expression. Leaves the constructor on the stack.
    fn compile_class_expression(
        &mut self,
        cls: &crate::ast::ClassExpression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        self.compile_class_body(
            cls.name.as_deref(),
            cls.super_class.as_deref(),
            &cls.elements,
            chunk,
            context,
        )
    }

    /// Emits bytecode for a class declaration, binding the constructor to the class name.
    fn compile_class_declaration(
        &mut self,
        decl: &crate::ast::ClassDeclaration,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        self.compile_class_body(
            Some(&decl.name),
            decl.super_class.as_ref(),
            &decl.elements,
            chunk,
            context,
        )?;
        // Bind the constructor function to the class name in the current scope.
        let name_idx = self.add_name(&decl.name, chunk)?;
        if context.inside_function() || context.is_lexical(&decl.name) {
            chunk.emit(Instruction::StoreName(name_idx));
        } else {
            chunk.emit(Instruction::StoreGlobal(name_idx));
        }
        chunk.emit(Instruction::Pop);
        Ok(())
    }

    /// Core class-body compiler. Emits bytecode that leaves the constructor
    /// function on the stack.
    ///
    /// Stack contract:
    /// ```text
    /// CreateFunction(ctor)         // [ctor]
    /// Duplicate                    // [ctor, ctor_copy]
    /// [for each static method:]
    ///   CreateFunction(method)     // [ctor, ctor_copy, fn]
    ///   DefineDataProperty(name)   // [ctor, ctor_copy]
    /// ObjectCreateEmpty            // [ctor, ctor_copy, proto]
    /// [if extends:]
    ///   compile(super_class)       // [ctor, ctor_copy, proto, super]
    ///   GetProperty("prototype")   // [ctor, ctor_copy, proto, super_proto]
    ///   SetObjectPrototype         // [ctor, ctor_copy, proto]
    /// [for each instance method:]
    ///   CreateFunction(method)     // [ctor, ctor_copy, proto, fn]
    ///   DefineDataProperty(name)   // [ctor, ctor_copy, proto]
    /// DefineDataProperty("prototype") // [ctor, ctor_copy]
    /// Pop                          // [ctor]
    /// ```
    fn compile_class_body(
        &mut self,
        name: Option<&str>,
        super_class: Option<&Expression>,
        elements: &[crate::ast::ClassElement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::ClassElement;

        // Find the constructor element, if any.
        let ctor_literal = elements.iter().find_map(|e| {
            if let ClassElement::Constructor(lit) = e {
                Some(lit)
            } else {
                None
            }
        });

        // Emit constructor function.
        let ctor_body = if let Some(lit) = ctor_literal {
            lit.clone()
        } else {
            // Synthesize an empty default constructor.
            FunctionLiteral {
                name: name.map(String::from),
                params: vec![],
                body: FunctionBody {
                    statements: vec![],
                    is_strict: false,
                },
                is_async: false,
                is_generator: false,
                is_arrow: false,
            }
        };
        let ctor_fn = self.compile_function_body(&ctor_body.params, &ctor_body.body, context)?;
        let ctor_template = FunctionTemplate {
            name: name.map(String::from),
            params: ctor_fn.params,
            rest_param: ctor_fn.rest_param,
            chunk: ctor_fn.chunk,
            is_strict: ctor_fn.is_strict,
            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
        };
        let ctor_idx = chunk
            .add_function(ctor_template)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::CreateFunction(ctor_idx)); // [ctor]
        chunk.emit(Instruction::Duplicate); // [ctor, ctor_copy]

        // Static methods — defined on the constructor itself.
        for element in elements {
            if let ClassElement::Method {
                name: prop_name,
                function,
                is_static: true,
            } = element
            {
                let fn_compiled =
                    self.compile_function_body(&function.params, &function.body, context)?;
                let fn_template = FunctionTemplate {
                    name: Some(prop_name.to_key_string()),
                    params: fn_compiled.params,
                    rest_param: fn_compiled.rest_param,
                    chunk: fn_compiled.chunk,
                    is_strict: fn_compiled.is_strict,
                    environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
                };
                let fn_idx = chunk
                    .add_function(fn_template)
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::CreateFunction(fn_idx));
                let key = self.add_name(&prop_name.to_key_string(), chunk)?;
                chunk.emit(Instruction::DefineDataProperty(key)); // [ctor, ctor_copy]
            }
        }

        // Create the prototype object.
        chunk.emit(Instruction::ObjectCreateEmpty); // [ctor, ctor_copy, proto]

        // Set up prototype inheritance if there is a super class.
        if let Some(super_expr) = super_class {
            self.compile_expression(super_expr, chunk, context)?; // [.., proto, super]
            let proto_key = self.add_name("prototype", chunk)?;
            chunk.emit(Instruction::GetProperty(proto_key)); // [.., proto, super.prototype]
            chunk.emit(Instruction::SetObjectPrototype); // [.., proto]
        }

        // Instance methods — defined on the prototype.
        for element in elements {
            match element {
                ClassElement::Method {
                    name: prop_name,
                    function,
                    is_static: false,
                } => {
                    let fn_compiled =
                        self.compile_function_body(&function.params, &function.body, context)?;
                    let fn_template = FunctionTemplate {
                        name: Some(prop_name.to_key_string()),
                        params: fn_compiled.params,
                        rest_param: fn_compiled.rest_param,
                        chunk: fn_compiled.chunk,
                        is_strict: fn_compiled.is_strict,
                        environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
                    };
                    let fn_idx = chunk
                        .add_function(fn_template)
                        .map_err(CompileError::from_chunk)?;
                    chunk.emit(Instruction::CreateFunction(fn_idx));
                    let key = self.add_name(&prop_name.to_key_string(), chunk)?;
                    chunk.emit(Instruction::DefineDataProperty(key)); // [.., proto]
                }
                // Instance fields — stub: initialize to undefined or the given expression.
                ClassElement::Field {
                    name: prop_name,
                    is_static: false,
                    initializer,
                } => {
                    // Skip private fields for now — they require per-instance init
                    // at `new` time, which we don't support yet.
                    if matches!(prop_name, crate::ast::PropertyName::PrivateName(_)) {
                        continue;
                    }
                    // Public instance fields are ignored at class-definition time in this
                    // stub — they should be set on `this` in the constructor. We skip them
                    // rather than erroring out so that class bodies with fields parse and
                    // compile without crashing.
                    let _ = initializer;
                }
                _ => {}
            }
        }

        // Attach prototype to constructor: ctor.prototype = proto.
        // Stack: [ctor, ctor_copy, proto]
        // DefineDataProperty peeks ctor_copy (below proto), pops proto as value.
        let proto_key = self.add_name("prototype", chunk)?;
        chunk.emit(Instruction::DefineDataProperty(proto_key)); // [ctor, ctor_copy]
        chunk.emit(Instruction::Pop); // [ctor]
        Ok(())
    }

    // -----------------------------------------------------------------------
    // V8-A: destructuring variable declarations
    // -----------------------------------------------------------------------

    fn compile_destructuring_declaration(
        &mut self,
        kind: VariableKind,
        pattern: &crate::ast::BindingPattern,
        initializer: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        self.compile_expression(initializer, chunk, context)?;
        self.compile_binding_pattern(kind, pattern, chunk, context)
    }

    /// Emits bytecode to bind the TOP of stack to the given pattern,
    /// consuming the value from the stack.
    fn compile_binding_pattern(
        &mut self,
        kind: VariableKind,
        pattern: &crate::ast::BindingPattern,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::BindingPattern;
        match pattern {
            BindingPattern::Identifier(name) => {
                // The value is already on top of the stack; bind it to `name`.
                let idx = self.add_name(name, chunk)?;
                match kind {
                    VariableKind::Var if context.inside_function() => {
                        chunk.emit(Instruction::DeclareLocal(idx));
                    }
                    VariableKind::Var => {
                        chunk.emit(Instruction::DeclareGlobal(idx));
                    }
                    VariableKind::Let | VariableKind::Const => {
                        chunk.emit(Instruction::InitializeBinding(idx));
                    }
                }
                Ok(())
            }
            BindingPattern::Array { elements, rest } => {
                // Stack: [rhs]
                for (i, maybe_elem) in elements.iter().enumerate() {
                    let Some(elem) = maybe_elem else {
                        continue; // hole — skip
                    };
                    chunk.emit(Instruction::Duplicate); // [rhs, rhs]
                    let idx_key = chunk
                        .add_constant(Constant::String(i.to_string()))
                        .map_err(CompileError::from_chunk)?;
                    chunk.emit(Instruction::Constant(idx_key)); // [rhs, rhs, "i"]
                    chunk.emit(Instruction::GetElement); // [rhs, elem_val]
                    if let Some(default_expr) = &elem.default {
                        self.emit_binding_default(default_expr, chunk, context)?;
                    }
                    self.compile_binding_pattern(kind, &elem.pattern, chunk, context)?; // [rhs]
                }
                // Rest element: rhs.slice(elements.len())
                if let Some(rest_pat) = rest {
                    chunk.emit(Instruction::Duplicate); // [rhs, rhs]
                    let slice_idx = self.add_name("slice", chunk)?;
                    chunk.emit(Instruction::GetMethod(slice_idx)); // [rhs, slice_fn, rhs_copy]
                    let n_const = chunk
                        .add_constant(Constant::Number(elements.len() as f64))
                        .map_err(CompileError::from_chunk)?;
                    chunk.emit(Instruction::Constant(n_const)); // [rhs, slice_fn, rhs_copy, N]
                    chunk.emit(Instruction::CallWithThis(1)); // [rhs, rest_array]
                    self.compile_binding_pattern(kind, rest_pat, chunk, context)?; // [rhs]
                }
                chunk.emit(Instruction::Pop); // []
                Ok(())
            }
            BindingPattern::Object { props, rest } => {
                // Stack: [rhs]
                for prop in props {
                    chunk.emit(Instruction::Duplicate); // [rhs, rhs]
                    match &prop.key {
                        crate::ast::ObjectBindingKey::Static(key) => {
                            let key_str = key.to_key_string();
                            let key_idx = self.add_name(&key_str, chunk)?;
                            chunk.emit(Instruction::GetProperty(key_idx)); // [rhs, rhs.key]
                        }
                        crate::ast::ObjectBindingKey::Computed(key_expr) => {
                            // [rhs, rhs]
                            self.compile_expression(key_expr, chunk, context)?; // [rhs, rhs, computed_key]
                            chunk.emit(Instruction::GetElement); // [rhs, rhs[computed_key]]
                        }
                    }
                    if let Some(default_expr) = &prop.default {
                        self.emit_binding_default(default_expr, chunk, context)?;
                    }
                    self.compile_binding_pattern(kind, &prop.value, chunk, context)?; // [rhs]
                }
                // Object rest: shallow copy of rhs (simplified — doesn't exclude consumed keys)
                if let Some(rest_pat) = rest {
                    chunk.emit(Instruction::Duplicate); // [rhs, rhs]
                    self.compile_binding_pattern(kind, rest_pat, chunk, context)?; // [rhs]
                }
                chunk.emit(Instruction::Pop); // []
                Ok(())
            }
        }
    }

    /// Emits code to apply a default value when TOS is `undefined`.
    /// Before: `[value]` — After: `[value_or_default]`.
    ///
    /// JumpIfFalse is a PEEK instruction (does not consume its operand).
    /// Both the jump and fall-through paths leave `is_undef` on the stack above
    /// `value`, so each path needs a Pop to remove it before proceeding.
    fn emit_binding_default(
        &mut self,
        default_expr: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        // [value] depth=1
        chunk.emit(Instruction::Duplicate); // [value, value] depth=2
        let undef_const = chunk
            .add_constant(Constant::Undefined)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Constant(undef_const)); // [value, value, undefined] depth=3
        chunk.emit(Instruction::StrictEqual); // [value, is_undef] depth=2
        // JumpIfFalse PEEKS — does not pop. Jumps when is_undef=false (value is NOT undefined).
        let jump_not_undef = chunk.emit(Instruction::JumpIfFalse(usize::MAX)); // depth=2

        // Fall-through: IS undefined. [value, is_undef(=true)] depth=2
        chunk.emit(Instruction::Pop); // [value] depth=1  — remove is_undef
        chunk.emit(Instruction::Pop); // [] depth=0        — remove the undefined value
        self.compile_expression(default_expr, chunk, context)?; // [default_value] depth=1
        let jump_end = chunk.emit(Instruction::Jump(usize::MAX));

        // NOT undefined. [value, is_undef(=false)] depth=2
        let not_undef = chunk.current_offset();
        chunk
            .patch_jump(jump_not_undef, not_undef)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop); // [value] depth=1 — remove is_undef

        // end: both paths arrive at depth=1 [value_or_default]
        let end = chunk.current_offset();
        chunk
            .patch_jump(jump_end, end)
            .map_err(CompileError::from_chunk)?;
        Ok(())
    }

    fn compile_function_expression(
        &mut self,
        literal: &FunctionLiteral,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let fn_chunk = self.compile_function_body(&literal.params, &literal.body, context)?;
        let template = FunctionTemplate {
            name: literal.name.clone(),
            params: fn_chunk.params,
            rest_param: fn_chunk.rest_param,
            chunk: fn_chunk.chunk,
            is_strict: fn_chunk.is_strict,
            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
        };
        let function_index = chunk
            .add_function(template)
            .map_err(CompileError::from_chunk)?;
        if literal.is_generator {
            chunk.emit(Instruction::CreateGenerator(function_index));
        } else if literal.is_async {
            chunk.emit(Instruction::CreateAsyncFunction(function_index));
        } else {
            chunk.emit(Instruction::CreateFunction(function_index));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // V9-A: for-of lowering
    // -----------------------------------------------------------------------

    fn compile_for_of(
        &mut self,
        left: &crate::ast::ForBinding,
        right: &Expression,
        body: &Statement,
        is_await: bool,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if is_await {
            return Err(CompileError::unsupported(
                "for-await-of requires V9-B async iterator support",
            ));
        }

        const ITER: &str = "\u{0}forof_iter";

        chunk.emit(Instruction::CreateLexicalEnvironment);
        context.environment_depth += 1;
        let mut scope = std::collections::HashSet::new();

        let iter_idx = self.add_name(ITER, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(iter_idx));
        scope.insert(ITER.to_string());

        // Declare loop variable if it's a new binding.
        // Pre-declare the loop variable(s) in the outer lexical scope and pre-initialize
        // mutable bindings to `undefined`.  This lets the loop body use `StoreName`
        // (set_mutable_binding) on every iteration instead of `InitializeBinding`, which
        // would throw "already initialized" on the second pass.
        // `const` bindings are left uninitialized here and initialized inside the loop via
        // `compile_binding_pattern` — but `const` in for-of is handled below by re-emitting
        // `CreateImmutableBinding` + `InitializeBinding` per iteration with a per-iter scope.
        let undefined_idx = chunk
            .add_constant(Constant::Undefined)
            .map_err(CompileError::from_chunk)?;

        match left {
            crate::ast::ForBinding::Declaration { kind, pattern } => {
                let names = binding_pattern_names(pattern);
                for name in &names {
                    let idx = self.add_name(name, chunk)?;
                    match kind {
                        VariableKind::Const => {
                            // Const loop variable: keep uninitialized; we'll initialize per-iter.
                            chunk.emit(Instruction::CreateImmutableBinding(idx));
                        }
                        _ => {
                            // Let / var: create AND pre-initialize to undefined so
                            // subsequent StoreName calls work without "already initialized" errors.
                            chunk.emit(Instruction::CreateMutableBinding(idx));
                            chunk.emit(Instruction::Constant(undefined_idx));
                            chunk.emit(Instruction::InitializeBinding(idx));
                        }
                    }
                    scope.insert(name.clone());
                }
            }
            crate::ast::ForBinding::Target(_) => {}
        }

        context.lexical_scopes.push(scope);

        // Evaluate iterable and obtain the iterator.
        self.compile_expression(right, chunk, context)?;
        chunk.emit(Instruction::GetIterator);
        chunk.emit(Instruction::InitializeBinding(iter_idx));

        // Loop header.
        let loop_start = chunk.current_offset();
        chunk.emit(Instruction::LoadName(iter_idx));
        chunk.emit(Instruction::IteratorNext); // [value, is_done]

        // if is_done (top), jump to exit.
        let exit_jump = chunk.emit(Instruction::JumpIfTrue(usize::MAX));
        chunk.emit(Instruction::Pop); // pop is_done=false

        // Assign the iteration value to the loop variable.
        // For mutable (let/var) declarations, use StoreName so that every iteration
        // re-assigns the same (already-initialized) binding instead of failing with
        // "already initialized" on the second pass.
        match left {
            crate::ast::ForBinding::Declaration { kind, pattern } => {
                match (kind, pattern) {
                    (
                        VariableKind::Let | VariableKind::Var,
                        crate::ast::BindingPattern::Identifier(name),
                    ) => {
                        // Use StoreName: the binding was pre-initialized to undefined above.
                        self.emit_store_identifier(name, chunk, context)?;
                        chunk.emit(Instruction::Pop); // StoreName pushes the value back; discard it.
                    }
                    _ => {
                        // Const or complex pattern: use compile_binding_pattern.
                        // Const creates a fresh immutable binding per-iteration (first iteration only
                        // works today; per-iteration const scope requires V10-D).
                        self.compile_binding_pattern(*kind, pattern, chunk, context)?;
                    }
                }
            }
            crate::ast::ForBinding::Target(target) => {
                match target {
                    Expression::Identifier(name) => {
                        self.emit_store_identifier(name, chunk, context)?;
                        chunk.emit(Instruction::Pop);
                    }
                    Expression::Array(_) | Expression::Object(_) => {
                        self.compile_destructuring_assignment_target(target, chunk, context)?;
                    }
                    _ => {
                        return Err(CompileError::unsupported(
                            "for-of target must be a simple identifier, array, or object pattern",
                        ));
                    }
                }
            }
        }

        context.loops.push(LoopContext {
            continue_target: Some(loop_start),
            continue_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });

        if let Err(e) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            return Err(e);
        }

        chunk.emit(Instruction::Jump(loop_start));

        let exit_target = chunk.current_offset();
        chunk
            .patch_jump(exit_jump, exit_target)
            .map_err(CompileError::from_chunk)?;
        // stack at exit: [value=undefined, is_done=true]; pop both.
        chunk.emit(Instruction::Pop); // pop is_done=true
        chunk.emit(Instruction::Pop); // pop value=undefined

        let break_jumps = context
            .breakables
            .last()
            .expect("for-of break context")
            .break_jumps
            .clone();
        for jump in break_jumps {
            chunk
                .patch_jump(jump, exit_target)
                .map_err(CompileError::from_chunk)?;
        }
        context.loops.pop();
        context.breakables.pop();

        context.lexical_scopes.pop();
        context.environment_depth -= 1;
        chunk.emit(Instruction::PopEnvironment);
        Ok(())
    }

    /// Emits a destructuring ASSIGNMENT (not declaration) for Array/Object expression targets.
    ///
    /// Used by `for ([a, b] of xs)` where `a` and `b` are existing variables.
    /// TOS is consumed; each target variable receives its element.
    fn compile_destructuring_assignment_target(
        &mut self,
        target: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match target {
            Expression::Array(elements) => {
                for (i, elem) in elements.iter().enumerate() {
                    match elem {
                        crate::ast::ArrayElement::Hole => continue,
                        crate::ast::ArrayElement::Expression(expr) => {
                            chunk.emit(Instruction::Duplicate);
                            let idx_c = chunk
                                .add_constant(Constant::String(i.to_string()))
                                .map_err(CompileError::from_chunk)?;
                            chunk.emit(Instruction::Constant(idx_c));
                            chunk.emit(Instruction::GetElement);
                            match expr {
                                Expression::Identifier(name) => {
                                    self.emit_store_identifier(name, chunk, context)?;
                                    chunk.emit(Instruction::Pop);
                                }
                                Expression::Array(_) | Expression::Object(_) => {
                                    self.compile_destructuring_assignment_target(
                                        expr, chunk, context,
                                    )?;
                                }
                                _ => {
                                    return Err(CompileError::unsupported(
                                        "complex destructuring assignment target not supported",
                                    ));
                                }
                            }
                        }
                        crate::ast::ArrayElement::Spread(_) => {
                            return Err(CompileError::unsupported(
                                "spread in destructuring assignment target not supported",
                            ));
                        }
                    }
                }
                chunk.emit(Instruction::Pop);
                Ok(())
            }
            Expression::Object(props) => {
                for prop in props {
                    match prop {
                        crate::ast::ObjectProperty::Data {
                            key,
                            value: Expression::Identifier(target_name),
                        } => {
                            chunk.emit(Instruction::Duplicate);
                            let key_str = key.to_key_string();
                            let key_idx = self.add_name(&key_str, chunk)?;
                            chunk.emit(Instruction::GetProperty(key_idx));
                            self.emit_store_identifier(target_name, chunk, context)?;
                            chunk.emit(Instruction::Pop);
                        }
                        _ => {
                            return Err(CompileError::unsupported(
                                "complex object destructuring assignment target not supported",
                            ));
                        }
                    }
                }
                chunk.emit(Instruction::Pop);
                Ok(())
            }
            _ => Err(CompileError::unsupported(
                "destructuring assignment target must be array or object expression",
            )),
        }
    }
}

/// Intermediate result returned from `compile_function_body`.
struct CompiledFunction {
    params: Vec<String>,
    rest_param: Option<String>,
    chunk: Chunk,
    is_strict: bool,
}

fn property_key(key: &PropertyName) -> String {
    key.to_key_string()
}

fn compound_assignment_instruction(operator: AssignmentOperator) -> Instruction {
    match operator {
        AssignmentOperator::Add => Instruction::Add,
        AssignmentOperator::Subtract => Instruction::Subtract,
        AssignmentOperator::Multiply => Instruction::Multiply,
        AssignmentOperator::Divide => Instruction::Divide,
        AssignmentOperator::Remainder => Instruction::Remainder,
    }
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
            Statement::DestructuringDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                pattern,
                ..
            } => binding_pattern_names(pattern),
            _ => Vec::new(),
        })
        .collect()
}

fn binding_pattern_names(pattern: &crate::ast::BindingPattern) -> Vec<String> {
    use crate::ast::BindingPattern;
    match pattern {
        BindingPattern::Identifier(name) => vec![name.clone()],
        BindingPattern::Array { elements, rest } => {
            let mut names: Vec<String> = elements
                .iter()
                .flatten()
                .flat_map(|elem| binding_pattern_names(&elem.pattern))
                .collect();
            if let Some(r) = rest {
                names.extend(binding_pattern_names(r));
            }
            names
        }
        BindingPattern::Object { props, rest } => {
            let mut names: Vec<String> = props
                .iter()
                .flat_map(|prop| binding_pattern_names(&prop.value))
                .collect();
            if let Some(r) = rest {
                names.extend(binding_pattern_names(r));
            }
            names
        }
    }
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
        Statement::DestructuringDeclaration { kind, pattern, .. }
            if matches!(kind, VariableKind::Let | VariableKind::Const)
                && binding_pattern_names(pattern).contains(&name.to_string()) =>
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
