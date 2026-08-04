//! AST-to-bytecode compiler.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    sync::Arc,
};

use crate::ast::{
    ArrayElement, AssignmentOperator, BinaryOperator, CatchClause, Expression, FunctionBody,
    FunctionLiteral, Literal, LogicalOperator, ModuleDeclaration, ObjectProperty,
    OptionalChainStep, Program, PropertyName, Statement, SwitchCase, UnaryOperator, UpdateOperator,
    VariableKind,
};

use super::{
    Chunk, ChunkError, Constant, DynamicScopePolicy, EnvironmentCapturePolicy, ExceptionHandler,
    FunctionTemplate, HandlerKind, Instruction, LocalBindingLayout, LocalLayout, LocalSlot,
    UpvalueBindingLayout, UpvalueDescriptor, UpvalueLayout, UpvalueSlot,
};

/// Compilation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
    /// If true, this compile-time error corresponds to a spec "Early Error" that
    /// the test harness expects to be classified as a SyntaxError (not Unsupported).
    #[allow(dead_code)]
    pub is_syntax: bool,
}

impl CompileError {
    /// Create an early-error (spec SyntaxError) compile-time failure.
    pub fn syntax(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_syntax: true,
        }
    }
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
    pending_loop_labels: Vec<String>,
    breakables: Vec<BreakContext>,
    lexical_scopes: Vec<HashSet<String>>,
    /// Script/eval chunks keep one completion value at the bottom of the
    /// operand stack. Function bodies leave this false because their result is
    /// controlled by explicit return.
    has_completion_slot: bool,
    /// Whether expression statements currently update the completion slot.
    /// Normal finally clauses temporarily disable updates while retaining the
    /// underlying stack slot.
    tracks_completion: bool,
    environment_depth: u32,
    with_depth: u32,
    /// Number of enclosing function bodies; 0 = top-level script.
    function_depth: usize,
    local_slots: HashMap<String, LocalSlot>,
    available_upvalues: HashMap<String, UpvalueDescriptor>,
    upvalue_slots: RefCell<HashMap<String, UpvalueSlot>>,
    upvalue_layout: RefCell<UpvalueLayout>,
}

#[derive(Debug)]
struct LoopContext {
    labels: Vec<String>,
    /// Hidden iterator binding for a for-of loop. Used to close intervening
    /// iterators when a labelled continue targets an outer loop.
    iterator_binding: Option<u16>,
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
    /// The label on this breakable statement, if any. `None` for unlabeled loops/switches.
    label: Option<String>,
}

#[derive(Debug, Clone)]
enum ObjectRestExcludedKey {
    Static(String),
    Temp(String),
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

    fn is_block_lexical(&self, name: &str) -> bool {
        self.lexical_scopes
            .iter()
            .skip(1)
            .rev()
            .any(|scope| scope.contains(name))
    }

    fn needs_dynamic_name_lookup(&self, name: &str) -> bool {
        self.inside_function() || self.with_depth > 0 || self.is_lexical(name)
    }

    fn local_slot(&self, name: &str) -> Option<LocalSlot> {
        (self.with_depth == 0)
            .then(|| self.local_slots.get(name).copied())
            .flatten()
    }

    fn upvalue_slot(&self, name: &str) -> Option<UpvalueSlot> {
        if self.with_depth != 0 || self.is_lexical(name) {
            return None;
        }
        if let Some(slot) = self.upvalue_slots.borrow().get(name).copied() {
            return Some(slot);
        }
        let descriptor = *self.available_upvalues.get(name)?;
        let mut layout = self.upvalue_layout.borrow_mut();
        let index = u16::try_from(layout.bindings.len()).ok()?;
        let slot = UpvalueSlot(index);
        layout.bindings.push(UpvalueBindingLayout {
            name: name.to_owned(),
            descriptor,
            mutable: true,
        });
        self.upvalue_slots
            .borrow_mut()
            .insert(name.to_owned(), slot);
        Some(slot)
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
    pub fn compile_program(
        &mut self,
        program: &Program,
    ) -> Result<crate::bytecode::SharedChunk, CompileError> {
        let mut chunk = Chunk::default();
        let mut context = CompileContext::default();
        let completion_expression = completion_expression_index(&program.body);
        // Keep the compact legacy lowering when a top-level expression already
        // determines the result. The stack slot is only needed to carry a
        // completion out of nested control flow.
        let tracks_completion = !program.body.is_empty()
            && completion_expression.is_none()
            && statements_need_stack_completion(&program.body)
            && statements_support_stack_completion(&program.body);
        let lexical_scope = self.predeclare_lexical_bindings(&program.body, &mut chunk)?;
        context.lexical_scopes.push(lexical_scope);

        // Hoist all var declarations to the top of the program scope (pre-declare
        // as undefined). This ensures var names from un-executed branches are
        // still accessible in the enclosing scope.
        {
            let mut var_names: Vec<String> = Vec::new();
            collect_var_names(&program.body, &mut var_names);
            if !var_names.is_empty() {
                let undef_const = chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                for name in var_names {
                    let idx = self.add_name(&name, &mut chunk)?;
                    chunk.emit(Instruction::Constant(undef_const));
                    chunk.emit(Instruction::DeclareGlobal(idx));
                }
            }
        }

        // Annex B §B.3.3.2 / §B.3.3.3: pre-hoist block-contained function
        // declaration names (initialised to undefined). The DeclareFunction
        // emitted by compile_block will update each binding to the actual
        // function object when the block is entered.
        {
            let mut annex_b_var_names: Vec<String> = Vec::new();
            let top_lex = collect_top_level_lexical_names(&program.body);
            // Pass empty set — top-level functions are already handled by normal
            // hoisting and are skipped inside collect_annex_b_var_names.
            let empty_hoisted = std::collections::HashSet::new();
            collect_annex_b_var_names(
                &program.body,
                &top_lex,
                &empty_hoisted,
                &mut annex_b_var_names,
            );
            if !annex_b_var_names.is_empty() {
                let undef_const = chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                for name in &annex_b_var_names {
                    let idx = self.add_name(name, &mut chunk)?;
                    chunk.emit(Instruction::Constant(undef_const));
                    chunk.emit(Instruction::DeclareGlobal(idx));
                }
            }
        }

        // Hoist function declarations to the top of the program scope.
        for statement in &program.body {
            if let Statement::FunctionDeclaration {
                name,
                params,
                body,
                is_async,
                is_generator,
            } = statement
            {
                self.compile_function_declaration(
                    name,
                    params,
                    body,
                    (*is_async, *is_generator),
                    &mut chunk,
                    &mut context,
                )?;
            }
        }

        if tracks_completion {
            // Script/eval completion records are represented by one value at
            // the bottom of the operand stack. Every statement preserves it.
            let undefined = chunk
                .add_constant(Constant::Undefined)
                .map_err(CompileError::from_chunk)?;
            chunk.emit(Instruction::Constant(undefined));
            context.has_completion_slot = true;
            context.tracks_completion = true;
        }

        for (index, statement) in program.body.iter().enumerate() {
            if matches!(statement, Statement::FunctionDeclaration { .. }) {
                continue;
            }
            self.compile_statement(
                statement,
                &mut chunk,
                &mut context,
                !tracks_completion && Some(index) == completion_expression,
            )?;
        }
        context.lexical_scopes.pop();
        chunk.emit(if tracks_completion || completion_expression.is_some() {
            Instruction::Return
        } else {
            Instruction::ReturnUndefined
        });
        chunk.validate().map_err(CompileError::from_chunk)?;
        Ok(chunk.into_shared())
    }

    fn compile_statement(
        &mut self,
        statement: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
        preserve_expression_value: bool,
    ) -> Result<(), CompileError> {
        if statement_resets_completion(statement) {
            self.reset_completion_value(chunk, context)?;
        }
        match statement {
            Statement::Empty => Ok(()),
            Statement::Expression(expression) => {
                self.compile_expression(expression, chunk, context)?;
                if context.tracks_completion {
                    chunk.emit(Instruction::Swap);
                    chunk.emit(Instruction::Pop);
                } else if !preserve_expression_value {
                    chunk.emit(Instruction::Pop);
                }
                Ok(())
            }
            Statement::Block(statements) => self.compile_block(statements, chunk, context),
            Statement::VariableDeclaration { kind, declarations } => {
                for declarator in declarations {
                    if let Some(pattern) = &declarator.pattern {
                        let init = declarator
                            .initializer
                            .as_ref()
                            .ok_or_else(|| CompileError {
                                is_syntax: false,
                                message: "destructuring declaration requires an initializer".into(),
                            })?;
                        self.compile_expression(init, chunk, context)?;
                        if *kind == VariableKind::Var {
                            self.compile_binding_pattern_store(pattern, chunk, context)?;
                        } else {
                            self.compile_binding_pattern(*kind, pattern, chunk, context)?;
                        }
                    } else {
                        self.compile_variable_declaration(
                            *kind,
                            &declarator.name,
                            declarator.initializer.as_ref(),
                            chunk,
                            context,
                        )?;
                    }
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
            Statement::Break(label) => self.compile_break(label.as_deref(), chunk, context),
            Statement::Continue(label) => self.compile_continue(label.as_deref(), chunk, context),
            Statement::Throw(expression) => {
                self.compile_expression(expression, chunk, context)?;
                chunk.emit(Instruction::Throw);
                Ok(())
            }
            Statement::Return(value) => self.compile_return(value.as_ref(), chunk, context),
            Statement::FunctionDeclaration {
                name,
                params,
                body,
                is_async,
                is_generator,
            } => self.compile_function_declaration(
                name,
                params,
                body,
                (*is_async, *is_generator),
                chunk,
                context,
            ),
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
            Statement::DoWhile { test, body } => self.compile_do_while(test, body, chunk, context),
            Statement::Labelled { label, body } => {
                self.compile_labelled(label, body, chunk, context)
            }
            Statement::ModuleDeclaration(ModuleDeclaration::Import(_)) => Ok(()),
            Statement::ModuleDeclaration(ModuleDeclaration::Export(decl)) => {
                if let Some(statement) = decl.declaration.as_deref() {
                    self.compile_statement(statement, chunk, context, false)?;
                }
                Ok(())
            }
            Statement::With { object, body } => {
                self.compile_expression(object, chunk, context)?;
                chunk.emit(Instruction::EnterWithEnvironment);
                context.environment_depth += 1;
                context.with_depth += 1;
                let result = self.compile_statement(body, chunk, context, false);
                context.with_depth -= 1;
                chunk.emit(Instruction::PopEnvironment);
                context.environment_depth -= 1;
                result
            }
        }
    }

    fn reset_completion_value(
        &mut self,
        chunk: &mut Chunk,
        context: &CompileContext,
    ) -> Result<(), CompileError> {
        if !context.tracks_completion {
            return Ok(());
        }
        let undefined = chunk
            .add_constant(Constant::Undefined)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Constant(undefined));
        chunk.emit(Instruction::Swap);
        chunk.emit(Instruction::Pop);
        Ok(())
    }

    fn compile_block(
        &mut self,
        statements: &[Statement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        // Annex B B.3.3.1: classify block-level function declarations.
        // "Path B" = Annex B is SKIPPED because the fn name is already visible in an
        // enclosing lexical scope (any let/const/class from outer block OR function body).
        // These must be block-scoped here so they don't leak to the outer var env.
        // "Path A" = Annex B APPLIES; the fn goes to the outer var env (pre-hoisted as undefined
        // at function entry by compile_function_body).
        let mut path_b_fn_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        // Check ALL currently visible lexical scopes. At script level
        // (function_depth == 0), this is the top-level lexical scope; at
        // function level it includes outer function + block scopes.
        let all_visible_lexicals: std::collections::HashSet<&str> = context
            .lexical_scopes
            .iter()
            .flat_map(|scope| scope.iter().map(|s| s.as_str()))
            .collect();
        for stmt in statements {
            if let Statement::FunctionDeclaration { name, .. } = stmt
                && all_visible_lexicals.contains(name.as_str())
            {
                path_b_fn_names.insert(name.clone());
            }
        }

        let names = lexical_names(statements);

        // If there are no let/const/class AND no path-B function names, no block env is needed.
        // Path-A fn decls: compile_statement_list hoists them into the current (outer) env.
        if names.is_empty() && path_b_fn_names.is_empty() {
            return self.compile_statement_list(statements, chunk, context);
        }

        // A block env is needed (for let/const/class and/or path-B fn names).
        // Path-A fn decls must be hoisted to the OUTER env BEFORE the block env is created,
        // so that DeclareFunction binds in the outer scope, not the block scope.
        for stmt in statements {
            if let Statement::FunctionDeclaration {
                name,
                params,
                body,
                is_async,
                is_generator,
            } = stmt
                && !path_b_fn_names.contains(name)
            {
                self.compile_function_declaration(
                    name,
                    params,
                    body,
                    (*is_async, *is_generator),
                    chunk,
                    context,
                )?;
            }
        }

        chunk.emit(Instruction::CreateLexicalEnvironment);
        context.environment_depth += 1;
        let mut scope = self.predeclare_names(&names, statements, chunk)?;
        // Path-B fn names are declared by DeclareFunction below (no pre-declare needed).
        for name in &path_b_fn_names {
            scope.insert(name.clone());
        }
        // Path-A functions need block-scoped bindings that shadow the outer
        // var-scoped bindings. After CreateLexicalEnvironment, load the outer
        // function value and create/initialize a same-named binding here.
        for stmt in statements {
            if let Statement::FunctionDeclaration { name, .. } = stmt
                && !path_b_fn_names.contains(name)
            {
                let name_idx = self.add_name(name, chunk)?;
                scope.insert(name.clone());
                // Load the var-scoped binding from the outer scope.
                chunk.emit(Instruction::LoadName(name_idx));
                // Create a block-scoped mutable binding.
                chunk.emit(Instruction::CreateMutableBinding(name_idx));
                // Initialize it to the loaded function value.
                chunk.emit(Instruction::InitializeBinding(name_idx));
            }
        }
        context.lexical_scopes.push(scope);
        // Hoist path-B fn decls (DeclareFunction into block env), skipping path-A ones.
        for stmt in statements {
            if let Statement::FunctionDeclaration {
                name,
                params,
                body,
                is_async,
                is_generator,
            } = stmt
                && path_b_fn_names.contains(name)
            {
                self.compile_function_declaration(
                    name,
                    params,
                    body,
                    (*is_async, *is_generator),
                    chunk,
                    context,
                )?;
            }
        }
        // Compile non-function statements (all fn decls already handled above).
        for stmt in statements {
            if !matches!(stmt, Statement::FunctionDeclaration { .. }) {
                self.compile_statement(stmt, chunk, context, false)?;
            }
        }

        context.lexical_scopes.pop();
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
                name,
                params,
                body,
                is_async,
                is_generator,
            } = statement
            {
                self.compile_function_declaration(
                    name,
                    params,
                    body,
                    (*is_async, *is_generator),
                    chunk,
                    context,
                )?;
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
            self.reset_completion_value(chunk, context)?;

            let names = lexical_names(&handler.body);
            let mut scope = self.predeclare_names(&names, &handler.body, chunk)?;
            if let Some(crate::ast::CatchParameter::Pattern(parameter)) = &handler.parameter {
                for name in binding_pattern_names(parameter) {
                    let index = self.add_name(&name, chunk)?;
                    chunk.emit(Instruction::CreateMutableBinding(index));
                    scope.insert(name);
                }
                chunk.emit(Instruction::LoadException);
                // Catch parameters use the same binding-pattern machinery as
                // declarations. This preserves iterator closing/defaults and
                // makes `catch ([a, b])` / `catch ({x})` observable correctly.
                self.compile_binding_pattern(VariableKind::Let, parameter, chunk, context)?;
            } else if let Some(crate::ast::CatchParameter::Identifier(parameter)) =
                &handler.parameter
            {
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
                    stack_depth: u32::from(context.has_completion_slot),
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

            let tracks_completion = std::mem::replace(&mut context.tracks_completion, false);
            let finalizer_result = self.compile_block(finalizer, chunk, context);
            context.tracks_completion = tracks_completion;
            finalizer_result?;
            chunk.emit(Instruction::EndFinally);

            if protected_start < finally_target {
                chunk.handlers.push(ExceptionHandler {
                    start: protected_start,
                    end: finally_target,
                    target: finally_target,
                    kind: HandlerKind::Finally,
                    stack_depth: u32::from(context.has_completion_slot),
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
            label: None,
        });
        let mut body_starts = Vec::with_capacity(cases.len());
        for case in cases {
            body_starts.push(chunk.current_offset());
            // Annex B: function declarations in switch case bodies are block-scoped;
            // route them through compile_block so path-B classification works correctly.
            for stmt in &case.consequent {
                self.compile_if_body(stmt, chunk, context)?;
            }
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
        // Annex B B.3.3: a bare FunctionDeclaration in an if-body is block-scoped.
        // Route through compile_block so path-B classification and block-env creation
        // happen correctly (prevents DeclareFunction from overwriting an outer `let f`).
        self.compile_if_body(consequent, chunk, context)?;
        let end_jump = chunk.emit(Instruction::Jump(usize::MAX));

        let false_cleanup = chunk.current_offset();
        chunk
            .patch_jump(false_jump, false_cleanup)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop);
        if let Some(alternate) = alternate {
            self.compile_if_body(alternate, chunk, context)?;
        }

        let end = chunk.current_offset();
        chunk
            .patch_jump(end_jump, end)
            .map_err(CompileError::from_chunk)?;
        Ok(())
    }

    /// Compiles an if-body statement. When the body is a bare `FunctionDeclaration`,
    /// routes through `compile_block` so Annex B path-B classification and the implicit
    /// block scope are handled correctly.
    fn compile_if_body(
        &mut self,
        body: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if matches!(body, Statement::FunctionDeclaration { .. }) {
            self.compile_block(std::slice::from_ref(body), chunk, context)
        } else {
            self.compile_statement(body, chunk, context, false)
        }
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

        let loop_labels = std::mem::take(&mut context.pending_loop_labels);
        context.loops.push(LoopContext {
            labels: loop_labels.clone(),
            iterator_binding: None,
            continue_target: Some(loop_start),
            continue_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
            label: None,
        });
        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            context.pending_loop_labels = loop_labels;
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
        context.pending_loop_labels = loop_labels;
        Ok(())
    }

    fn compile_do_while(
        &mut self,
        test: &Expression,
        body: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let loop_start = chunk.current_offset();

        let loop_labels = std::mem::take(&mut context.pending_loop_labels);
        context.loops.push(LoopContext {
            labels: loop_labels.clone(),
            iterator_binding: None,
            continue_target: None, // patched after body
            continue_jumps: Vec::new(),
            environment_depth: context.environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
            label: None,
        });

        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            context.pending_loop_labels = loop_labels;
            return Err(error);
        }

        // `continue` jumps here (beginning of the test)
        let test_offset = chunk.current_offset();
        let loop_ctx = context.loops.pop().expect("do-while loop context");
        for jump in loop_ctx.continue_jumps {
            chunk
                .patch_jump(jump, test_offset)
                .map_err(CompileError::from_chunk)?;
        }

        // do-while: evaluate test; if false pop and exit; else pop and loop.
        // break jumps past the test entirely (no test value on stack at that point).
        // Structure:
        //   eval test → JumpIfFalse(loop_end_pop) → Pop → Jump(loop_start)
        //   loop_end_pop: Pop   ← falsy condition exit (pops test value)
        //   loop_end:           ← break exit (nothing to pop)
        self.compile_expression(test, chunk, context)?;
        let exit_jump = chunk.emit(Instruction::JumpIfFalse(usize::MAX));
        chunk.emit(Instruction::Pop); // pop truthy test value, then loop
        chunk.emit(Instruction::Jump(loop_start));
        let loop_end_pop = chunk.current_offset();
        chunk
            .patch_jump(exit_jump, loop_end_pop)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Pop); // pop falsy test value
        let loop_end = chunk.current_offset();

        let break_context = context.breakables.pop().expect("do-while break context");
        for jump in break_context.break_jumps {
            chunk
                .patch_jump(jump, loop_end)
                .map_err(CompileError::from_chunk)?;
        }
        context.pending_loop_labels = loop_labels;
        Ok(())
    }

    fn compile_break(
        &mut self,
        label: Option<&str>,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let target_idx = match label {
            None => {
                // Unlabeled break: target the innermost breakable (any label or none).
                // Per spec, unlabeled break only targets loops/switches (label: None),
                // not arbitrary labeled statements.
                context
                    .breakables
                    .iter()
                    .rposition(|b| b.label.is_none())
                    .ok_or_else(|| {
                        CompileError::unsupported("break statement outside of a loop or switch")
                    })?
            }
            Some(name) => {
                // Labeled break: find the breakable context with this label.
                context
                    .breakables
                    .iter()
                    .rposition(|b| b.label.as_deref() == Some(name))
                    .ok_or_else(|| {
                        CompileError::unsupported("break statement outside of a loop or switch")
                    })?
            }
        };
        let target_environment_depth = context.breakables[target_idx].environment_depth;
        for _ in target_environment_depth..context.environment_depth {
            chunk.emit(Instruction::PopEnvironment);
        }
        let jump = chunk.emit(Instruction::Jump(usize::MAX));
        context.breakables[target_idx].break_jumps.push(jump);
        Ok(())
    }

    fn compile_labelled(
        &mut self,
        label: &str,
        body: &Statement,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let labels_iteration = is_iteration_statement(body);
        if labels_iteration {
            context.pending_loop_labels.push(label.to_owned());
        }
        // Push a breakable context for the label so `break <label>` can target it.
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: context.environment_depth,
            label: Some(label.to_owned()),
        });
        let compile_result = self.compile_statement(body, chunk, context, false);
        let labeled_ctx = context.breakables.pop().expect("labeled break context");
        if labels_iteration {
            let pending = context
                .pending_loop_labels
                .pop()
                .expect("labelled iteration must restore its pending label");
            debug_assert_eq!(pending, label);
        }
        compile_result?;
        let end_offset = chunk.current_offset();
        for jump in labeled_ctx.break_jumps {
            chunk
                .patch_jump(jump, end_offset)
                .map_err(CompileError::from_chunk)?;
        }
        Ok(())
    }

    fn compile_continue(
        &mut self,
        label: Option<&str>,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let loop_index =
            match label {
                Some(label) => context
                    .loops
                    .iter()
                    .rposition(|loop_context| {
                        loop_context
                            .labels
                            .iter()
                            .any(|candidate| candidate == label)
                    })
                    .ok_or_else(|| {
                        CompileError::unsupported("continue label does not name an enclosing loop")
                    })?,
                None => context.loops.len().checked_sub(1).ok_or_else(|| {
                    CompileError::unsupported("continue statement outside of a loop")
                })?,
            };
        let target_depth = context.loops[loop_index].environment_depth;
        let continue_target = context.loops[loop_index].continue_target;
        let intervening_iterators: Vec<(u16, u32)> = context.loops[loop_index + 1..]
            .iter()
            .rev()
            .filter_map(|loop_context| {
                loop_context
                    .iterator_binding
                    .map(|binding| (binding, loop_context.environment_depth))
            })
            .collect();
        let mut current_depth = context.environment_depth;
        for (iterator_binding, iterator_environment_depth) in intervening_iterators {
            chunk.emit(Instruction::LoadName(iterator_binding));
            chunk.emit(Instruction::IteratorClose);
            let unwind_to = iterator_environment_depth.saturating_sub(1);
            for _ in unwind_to..current_depth {
                chunk.emit(Instruction::PopEnvironment);
            }
            current_depth = unwind_to;
        }
        for _ in target_depth..current_depth {
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
                    .get_mut(loop_index)
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
                .filter(|_| *kind != VariableKind::Var)
                .flat_map(|declarator| {
                    if let Some(pattern) = &declarator.pattern {
                        binding_pattern_names(pattern)
                            .into_iter()
                            .map(|n| (n, *kind))
                            .collect::<Vec<_>>()
                    } else {
                        vec![(declarator.name.clone(), *kind)]
                    }
                })
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
            Some(Statement::VariableDeclaration { kind, declarations }) => {
                for declarator in declarations {
                    if let Some(pattern) = &declarator.pattern {
                        // Destructuring declarator: compile RHS then bind pattern
                        match &declarator.initializer {
                            Some(expression) => {
                                self.compile_expression(expression, chunk, context)?
                            }
                            None => {
                                let undefined = chunk
                                    .add_constant(Constant::Undefined)
                                    .map_err(CompileError::from_chunk)?;
                                chunk.emit(Instruction::Constant(undefined));
                            }
                        }
                        if *kind == VariableKind::Var {
                            self.compile_binding_pattern_store(pattern, chunk, context)?;
                        } else {
                            self.compile_binding_pattern(*kind, pattern, chunk, context)?;
                        }
                    } else {
                        self.compile_variable_declaration(
                            *kind,
                            &declarator.name,
                            declarator.initializer.as_ref(),
                            chunk,
                            context,
                        )?;
                    }
                }
            }
            Some(other) => {
                // The init expression is evaluated for side effects; it is not
                // the completion value of the surrounding for statement.
                let tracks_completion = std::mem::replace(&mut context.tracks_completion, false);
                let result = self.compile_statement(other, chunk, context, false);
                context.tracks_completion = tracks_completion;
                result?;
            }
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

        // A classic `for` with a `const` declaration has a single immutable
        // binding for the lifetime of the loop.  Unlike `let`, it must not be
        // copied through a per-iteration environment and written back with
        // `StoreName` after the body (that would attempt to update an
        // immutable binding).  `const` heads with an update expression are an
        // early error and are rejected by the parser; for the valid form,
        // there is no per-iteration binding to rotate.
        let needs_iteration_env = !declared.is_empty()
            && declared
                .iter()
                .any(|(_, kind)| *kind != VariableKind::Const);
        if needs_iteration_env {
            for (name, _) in &declared {
                let index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::LoadName(index));
            }
            chunk.emit(Instruction::CreateLexicalEnvironment);
            context.environment_depth += 1;
            let mut iteration_scope = HashSet::new();
            for (name, kind) in &declared {
                let index = self.add_name(name, chunk)?;
                chunk.emit(match kind {
                    VariableKind::Const => Instruction::CreateImmutableBinding(index),
                    _ => Instruction::CreateMutableBinding(index),
                });
                iteration_scope.insert(name.clone());
            }
            for (name, _) in declared.iter().rev() {
                let index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::InitializeBinding(index));
            }
            context.lexical_scopes.push(iteration_scope);
        }

        let continue_environment_depth = context.environment_depth;
        let break_environment_depth = if needs_iteration_env {
            context.environment_depth.saturating_sub(1)
        } else {
            context.environment_depth
        };

        let loop_labels = std::mem::take(&mut context.pending_loop_labels);
        context.loops.push(LoopContext {
            labels: loop_labels.clone(),
            iterator_binding: None,
            continue_target: None,
            continue_jumps: Vec::new(),
            environment_depth: continue_environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: break_environment_depth,
            label: None,
        });

        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            context.pending_loop_labels = loop_labels;
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
        if needs_iteration_env {
            for (name, _) in &declared {
                let index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::LoadName(index));
            }
            context.lexical_scopes.pop();
            chunk.emit(Instruction::PopEnvironment);
            context.environment_depth -= 1;
            for (name, _) in declared.iter().rev() {
                let index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::StoreName(index));
                chunk.emit(Instruction::Pop);
            }
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
        context.pending_loop_labels = loop_labels;
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

        // Lexical bindings remain uninitialized during RHS evaluation (TDZ);
        // each iteration receives a fresh binding below. `var` is hoisted by
        // declaration instantiation and is not part of this hidden scope.
        match left {
            crate::ast::ForBinding::Declaration { kind, pattern } => {
                if matches!(kind, VariableKind::Let | VariableKind::Const) {
                    for name in binding_pattern_names(pattern) {
                        let idx = self.add_name(&name, chunk)?;
                        chunk.emit(if *kind == VariableKind::Const {
                            Instruction::CreateImmutableBinding(idx)
                        } else {
                            Instruction::CreateMutableBinding(idx)
                        });
                        scope.insert(name);
                    }
                }
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
        let has_iteration_environment = matches!(
            left,
            crate::ast::ForBinding::Declaration {
                kind: VariableKind::Let | VariableKind::Const,
                ..
            }
        );
        if let crate::ast::ForBinding::Declaration { kind, pattern } = left
            && has_iteration_environment
        {
            chunk.emit(Instruction::CreateLexicalEnvironment);
            context.environment_depth += 1;
            let mut iteration_scope = HashSet::new();
            for name in binding_pattern_names(pattern) {
                let idx = self.add_name(&name, chunk)?;
                chunk.emit(if *kind == VariableKind::Const {
                    Instruction::CreateImmutableBinding(idx)
                } else {
                    Instruction::CreateMutableBinding(idx)
                });
                iteration_scope.insert(name);
            }
            context.lexical_scopes.push(iteration_scope);
        }
        match left {
            crate::ast::ForBinding::Declaration { kind, pattern } => {
                if has_iteration_environment {
                    self.compile_binding_pattern(*kind, pattern, chunk, context)?;
                } else {
                    match (kind, pattern) {
                        (_, crate::ast::BindingPattern::Identifier(name)) => {
                            let idx = self.add_name(name, chunk)?;
                            chunk.emit(Instruction::StoreName(idx));
                            chunk.emit(Instruction::Pop);
                        }
                        (_, pat) => {
                            // Use store (not initialize) — bindings were already initialized before the loop.
                            self.compile_binding_pattern_store(pat, chunk, context)?;
                        }
                    }
                }
            }
            crate::ast::ForBinding::Target(target) => match target {
                Expression::Identifier(name) => {
                    self.emit_store_identifier(name, chunk, context)?;
                    chunk.emit(Instruction::Pop);
                }
                Expression::Member { .. } => {
                    self.assign_dstr_element_to_target(target, chunk, context)?;
                }
                _ => return Err(CompileError::unsupported("for-in assignment target")),
            },
        }

        let outer_environment_depth = if has_iteration_environment {
            context.environment_depth - 1
        } else {
            context.environment_depth
        };
        let loop_labels = std::mem::take(&mut context.pending_loop_labels);
        context.loops.push(LoopContext {
            labels: loop_labels.clone(),
            iterator_binding: None,
            continue_target: None,
            continue_jumps: Vec::new(),
            environment_depth: outer_environment_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: outer_environment_depth,
            label: None,
        });

        if let Err(error) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            context.pending_loop_labels = loop_labels;
            return Err(error);
        }

        if has_iteration_environment {
            context.lexical_scopes.pop();
            chunk.emit(Instruction::PopEnvironment);
            context.environment_depth -= 1;
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
        context.pending_loop_labels = loop_labels;
        Ok(())
    }

    /// Compiles `++x` / `x++` / `--x` / `x--`.
    ///
    /// Supports identifier operands and static/computed member expression
    /// operands. Numeric coercion is handled by VM `Increment` / `Decrement`,
    /// so BigInt operands keep BigInt arithmetic while numbers keep Number
    /// arithmetic.
    fn compile_update(
        &mut self,
        operator: UpdateOperator,
        prefix: bool,
        argument: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let step = match operator {
            UpdateOperator::Increment => Instruction::Increment,
            UpdateOperator::Decrement => Instruction::Decrement,
        };

        match argument {
            Expression::Identifier(name) => {
                self.compile_identifier(name, chunk, context)?;
                if prefix {
                    chunk.emit(step);
                    self.emit_store_identifier(name, chunk, context)?;
                } else {
                    // Postfix: ToNumeric coerces the old value so we return oldNum,
                    // not the original uncoerced type (e.g. false → 0, "1" → 1).
                    chunk.emit(Instruction::ToNumeric);
                    chunk.emit(Instruction::Duplicate);
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
                let prop_name = match property.as_ref() {
                    Expression::Identifier(name) => name.clone(),
                    Expression::PrivateName(name) => format!("\x00#{name}"),
                    _ => {
                        return Err(CompileError::unsupported(
                            "non-identifier static property in `++`/`--`",
                        ));
                    }
                };
                let prop_index = self.add_name(&prop_name, chunk)?;
                // Stack before step: [obj, old]
                self.compile_expression(object, chunk, context)?;
                chunk.emit(Instruction::Duplicate); // [obj, obj]
                chunk.emit(Instruction::GetProperty(prop_index)); // [obj, old]
                if prefix {
                    // Result = new: [obj, old] -> [obj, new] -> SetProperty -> [new]
                    chunk.emit(step);
                    chunk.emit(Instruction::SetProperty(prop_index));
                } else {
                    // Postfix result is ToNumeric(old).
                    // Stack trace: [obj, old]
                    //   ToNumeric     -> [obj, old_num]
                    //   DuplicatePair -> [obj, old_num, obj, old_num]
                    //   step          -> [obj, old_num, obj, new_num]
                    //   SetProperty   -> [obj, old_num, new_num]  (pops obj+new_num, pushes new_num)
                    //   Pop           -> [obj, old_num]           (remove new_num)
                    //   Swap          -> [old_num, obj]
                    //   Pop           -> [old_num]                ✓
                    chunk.emit(Instruction::ToNumeric);
                    chunk.emit(Instruction::DuplicatePair);
                    chunk.emit(step);
                    chunk.emit(Instruction::SetProperty(prop_index));
                    chunk.emit(Instruction::Pop);
                    chunk.emit(Instruction::Swap);
                    chunk.emit(Instruction::Pop);
                }
            }
            Expression::Member {
                object,
                property,
                computed: true,
            } => {
                // Stack before step: [obj, key, old]
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::DuplicatePair); // [obj, key, obj, key]
                chunk.emit(Instruction::GetElement); // [obj, key, old]
                if prefix {
                    // [obj, key, old] -> [obj, key, new] -> SetElement -> [new]
                    chunk.emit(step);
                    chunk.emit(Instruction::SetElement);
                } else {
                    // Preserve the evaluated reference and return the old
                    // ToNumeric value after writing the incremented one.
                    chunk.emit(Instruction::ToNumeric);
                    chunk.emit(Instruction::Duplicate);
                    chunk.emit(step);
                    chunk.emit(Instruction::SetElementKeepOld);
                    return Ok(());
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
        if let Some(slot) = context.local_slot(name) {
            chunk.emit(Instruction::StoreLocal(slot));
        } else if let Some(slot) = context.upvalue_slot(name) {
            chunk.emit(Instruction::StoreUpvalue(slot));
        } else if context.needs_dynamic_name_lookup(name) {
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
        function_kind: (bool, bool),
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let (is_async, is_generator) = function_kind;
        let fn_chunk = self.compile_function_body(params, body, context)?;
        let template = FunctionTemplate {
            name: Some(name.to_string()),
            params: fn_chunk.params,
            rest_param: fn_chunk.rest_param,
            length_override: Some(fn_chunk.length),
            chunk: fn_chunk.chunk.into_shared(),
            is_strict: fn_chunk.is_strict,
            is_async,
            is_generator,
            is_arrow: false,
            binds_name_in_activation: false,
            is_derived_constructor: false,
            is_constructable: !is_async && !is_generator,
            has_own_prototype_property: !is_async || is_generator,
            prototype_writable: true,
            uses_arguments: fn_chunk.uses_arguments,
            local_layout: fn_chunk.local_layout,
            upvalue_layout: fn_chunk.upvalue_layout,
            dynamic_scope: fn_chunk.dynamic_scope,
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

        // Function.length = number of params before the first default/rest/destructured param.
        let mut length: u32 = 0;
        let mut seen_non_simple = false;
        for p in params {
            match p {
                FunctionParam::Simple(name) => {
                    param_names.push(name.clone());
                    if !seen_non_simple {
                        length += 1;
                    }
                }
                FunctionParam::Default(name, _) => {
                    param_names.push(name.clone());
                    preamble.push((name.clone(), p));
                    seen_non_simple = true;
                }
                FunctionParam::Pattern(_, None) => {
                    // Destructuring without default — counts toward length.
                    let placeholder = format!("$p{}", param_names.len());
                    param_names.push(placeholder.clone());
                    preamble.push((placeholder, p));
                    if !seen_non_simple {
                        length += 1;
                    }
                }
                FunctionParam::Pattern(..) => {
                    // Destructuring with a default — stops counting.
                    let placeholder = format!("$p{}", param_names.len());
                    param_names.push(placeholder.clone());
                    preamble.push((placeholder, p));
                    seen_non_simple = true;
                }
                FunctionParam::Rest(name) => {
                    rest_param = Some(name.clone());
                    seen_non_simple = true;
                }
                FunctionParam::RestPattern(_) => {
                    let placeholder = "$rest_pat".to_string();
                    rest_param = Some(placeholder.clone());
                    preamble.push((placeholder, p));
                    seen_non_simple = true;
                }
            }
        }

        let mut local_layout = LocalLayout::default();
        let mut local_slots = HashMap::new();
        let mut add_local = |name: String, lexical: bool| -> Result<(), CompileError> {
            if local_slots.contains_key(&name) {
                return Ok(());
            }
            let index = u16::try_from(local_layout.bindings.len())
                .map_err(|_| CompileError::unsupported("too many function-local bindings"))?;
            local_slots.insert(name.clone(), LocalSlot(index));
            local_layout.bindings.push(LocalBindingLayout {
                name,
                mutable: true,
                initialized_at_entry: false,
                lexical,
            });
            Ok(())
        };
        for name in &param_names {
            add_local(name.clone(), true)?;
        }
        if let Some(name) = &rest_param {
            add_local(name.clone(), true)?;
        }
        let mut function_var_names = Vec::new();
        collect_var_names(&body.statements, &mut function_var_names);
        let function_var_set: HashSet<String> = function_var_names.iter().cloned().collect();
        for name in &function_var_names {
            add_local(name.clone(), false)?;
        }
        // FunctionDeclarationInstantiation creates direct function-body
        // declarations in the same variable environment as `var`. Do not
        // include nested block functions here: their lexical/Annex B rules are
        // handled by the existing declaration lowering.
        for statement in &body.statements {
            if let Statement::FunctionDeclaration { name, .. } = statement {
                // Non-simple parameter lists have a distinct parameter
                // environment. Its `arguments` object must remain visible to
                // default expressions before a body declaration named
                // `arguments` replaces the body binding.
                if name == "arguments" && !preamble.is_empty() {
                    continue;
                }
                add_local(name.clone(), false)?;
            }
        }

        let mut available_upvalues = HashMap::new();
        if outer_context.with_depth == 0 {
            let local_hops = u16::try_from(outer_context.environment_depth)
                .map_err(|_| CompileError::unsupported("upvalue environment chain too deep"))?;
            for (name, slot) in &outer_context.local_slots {
                if outer_context.is_block_lexical(name) {
                    continue;
                }
                available_upvalues.insert(
                    name.clone(),
                    UpvalueDescriptor {
                        environment_hops: local_hops,
                        local_slot: *slot,
                    },
                );
            }
            let inherited_hops = local_hops
                .checked_add(1)
                .ok_or_else(|| CompileError::unsupported("upvalue environment chain too deep"))?;
            for (name, descriptor) in &outer_context.available_upvalues {
                if outer_context.is_lexical(name) {
                    continue;
                }
                available_upvalues
                    .entry(name.clone())
                    .or_insert_with(|| UpvalueDescriptor {
                        environment_hops: descriptor
                            .environment_hops
                            .saturating_add(inherited_hops),
                        local_slot: descriptor.local_slot,
                    });
            }
        }

        let mut fn_chunk = Chunk::default();
        let mut fn_context = CompileContext {
            loops: Vec::new(),
            pending_loop_labels: Vec::new(),
            breakables: Vec::new(),
            lexical_scopes: Vec::new(),
            has_completion_slot: false,
            tracks_completion: false,
            environment_depth: 0,
            with_depth: 0,
            function_depth: outer_context.function_depth + 1,
            local_slots,
            available_upvalues,
            upvalue_slots: RefCell::new(HashMap::new()),
            upvalue_layout: RefCell::new(UpvalueLayout::default()),
        };
        let mut lexical_scope =
            self.predeclare_lexical_bindings(&body.statements, &mut fn_chunk)?;
        // Include formal parameter names and `arguments` in the function body's lexical scope
        // so that compile_block can classify Annex B fn decls whose name matches a param as
        // path-B (block-scoped) instead of path-A (hoisting into the outer var env).
        for name in &param_names {
            if !name.starts_with('$') {
                lexical_scope.insert(name.clone());
            }
        }
        if let Some(rest) = &rest_param {
            lexical_scope.insert(rest.clone());
        }
        lexical_scope.insert("arguments".into());
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
                    if let Some(slot) = fn_context.local_slot(name) {
                        fn_chunk.emit(Instruction::LoadLocal(slot)); // [param_val]
                    } else {
                        fn_chunk.emit(Instruction::LoadName(name_idx)); // [param_val]
                    }
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
                    // Spec: infer function name when default is an anonymous function.
                    if is_anonymous_function_definition(default_expr) {
                        fn_chunk.emit(Instruction::SetFunctionName(name_idx));
                    }
                    if let Some(slot) = fn_context.local_slot(name) {
                        fn_chunk.emit(Instruction::StoreLocal(slot)); // [default]
                    } else {
                        fn_chunk.emit(Instruction::StoreName(name_idx)); // [default]
                    }
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
                    if let Some(slot) = fn_context.local_slot(placeholder) {
                        fn_chunk.emit(Instruction::LoadLocal(slot)); // [arg_val]
                    } else {
                        fn_chunk.emit(Instruction::LoadName(ph_idx)); // [arg_val]
                    }
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
                    if let Some(slot) = fn_context.local_slot(placeholder) {
                        fn_chunk.emit(Instruction::LoadLocal(slot)); // [rest_array]
                    } else {
                        fn_chunk.emit(Instruction::LoadName(ph_idx)); // [rest_array]
                    }
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

        // Hoist all var declarations to the top of the function scope (pre-declare as undefined).
        {
            // Exclude names that are already bound as parameters (those are DeclareLocal'd by the runtime).
            let param_set: std::collections::HashSet<&str> =
                param_names.iter().map(|s| s.as_str()).collect();
            let var_names: Vec<String> = function_var_names
                .into_iter()
                .filter(|name| !param_set.contains(name.as_str()))
                .collect();
            if !var_names.is_empty() {
                let undef_const = fn_chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                for name in var_names {
                    fn_chunk.emit(Instruction::Constant(undef_const));
                    let slot = fn_context
                        .local_slot(&name)
                        .ok_or_else(|| CompileError::unsupported("missing local slot"))?;
                    fn_chunk.emit(Instruction::InitializeLocal(slot));
                }
            }
        }

        // Annex B B.3.3.1: in sloppy mode, pre-declare block-level function declaration names
        // as var=undefined so they are accessible (as undefined) before the containing block
        // executes (e.g. `init = f` before `{ function f() {} }`).
        if !body.is_strict {
            let annex_b_candidates = collect_annex_b_fn_candidates(&body.statements);
            if !annex_b_candidates.is_empty() {
                let param_set: std::collections::HashSet<&str> =
                    param_names.iter().map(|s| s.as_str()).collect();
                let fn_body_lexical_set: std::collections::HashSet<String> =
                    lexical_names(&body.statements).into_iter().collect();
                let undef_const = fn_chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                let mut already_hoisted: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for name in annex_b_candidates {
                    if already_hoisted.contains(&name) {
                        continue;
                    }
                    // An explicit `var name` was already initialized through
                    // its activation slot above. Annex B reuses that binding;
                    // initializing the same slot twice is a TypeError.
                    if function_var_set.contains(&name) {
                        already_hoisted.insert(name);
                        continue;
                    }
                    // Skip: already bound as a formal param, 'arguments' (always in paramNames),
                    // or would conflict with a function-scope lexical declaration.
                    if param_set.contains(name.as_str())
                        || name == "arguments"
                        || fn_body_lexical_set.contains(&name)
                    {
                        continue;
                    }
                    let idx = self.add_name(&name, &mut fn_chunk)?;
                    fn_chunk.emit(Instruction::Constant(undef_const));
                    if let Some(slot) = fn_context.local_slot(&name) {
                        fn_chunk.emit(Instruction::InitializeLocal(slot));
                    } else {
                        fn_chunk.emit(Instruction::DeclareLocal(idx));
                    }
                    already_hoisted.insert(name);
                }
            }
        }

        // Hoist function declarations within the function body.
        for statement in &body.statements {
            if let Statement::FunctionDeclaration {
                name,
                params,
                body: inner_body,
                is_async,
                is_generator,
            } = statement
            {
                self.compile_function_declaration(
                    name,
                    params,
                    inner_body,
                    (*is_async, *is_generator),
                    &mut fn_chunk,
                    &mut fn_context,
                )?;
            }
        }
        fn_chunk.function_body_start = fn_chunk.current_offset();
        // Compile the body statements; no "completion expression" inside function bodies.
        for statement in &body.statements {
            if !matches!(statement, Statement::FunctionDeclaration { .. }) {
                self.compile_statement(statement, &mut fn_chunk, &mut fn_context, false)?;
            }
        }
        fn_context.lexical_scopes.pop();
        // Implicit undefined return at the end of the function
        fn_chunk.emit(Instruction::ReturnUndefined);
        let has_direct_eval = fn_chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DirectEval(_)));
        let has_with = fn_chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::EnterWithEnvironment));
        let dynamic_scope = match (has_direct_eval, has_with) {
            (false, false) => DynamicScopePolicy::Static,
            (true, false) => DynamicScopePolicy::DirectEval,
            (false, true) => DynamicScopePolicy::With,
            (true, true) => DynamicScopePolicy::DirectEvalAndWith,
        };
        if dynamic_scope != DynamicScopePolicy::Static {
            for offset in 0..fn_chunk.instructions.len() {
                let replacement = match fn_chunk.instructions[offset] {
                    Instruction::LoadLocal(slot) => Some((slot, 0_u8)),
                    Instruction::StoreLocal(slot) => Some((slot, 1)),
                    Instruction::InitializeLocal(slot) => Some((slot, 2)),
                    _ => None,
                };
                let Some((slot, kind)) = replacement else {
                    continue;
                };
                let binding = local_layout
                    .bindings
                    .get(usize::from(slot.0))
                    .ok_or_else(|| CompileError::unsupported("invalid local slot"))?;
                let name = binding.name.clone();
                let name_index = self.add_name(&name, &mut fn_chunk)?;
                fn_chunk.instructions[offset] = match kind {
                    0 => Instruction::LoadName(name_index),
                    1 => Instruction::StoreName(name_index),
                    _ => Instruction::DeclareLocal(name_index),
                };
            }
            local_layout.bindings.clear();
            for offset in 0..fn_chunk.instructions.len() {
                let (slot, store) = match fn_chunk.instructions[offset] {
                    Instruction::LoadUpvalue(slot) => (slot, false),
                    Instruction::StoreUpvalue(slot) => (slot, true),
                    _ => continue,
                };
                let name = fn_context
                    .upvalue_layout
                    .borrow()
                    .bindings
                    .get(usize::from(slot.0))
                    .ok_or_else(|| CompileError::unsupported("invalid upvalue slot"))?
                    .name
                    .clone();
                let name_index = self.add_name(&name, &mut fn_chunk)?;
                fn_chunk.instructions[offset] = if store {
                    Instruction::StoreName(name_index)
                } else {
                    Instruction::LoadName(name_index)
                };
            }
            fn_context.upvalue_layout.borrow_mut().bindings.clear();
            for template in &mut fn_chunk.functions {
                deoptimize_template_upvalues(template)?;
            }
        }
        fn_chunk.validate().map_err(CompileError::from_chunk)?;
        let uses_arguments = function_needs_arguments_object(&fn_chunk);
        let upvalue_layout = fn_context.upvalue_layout.into_inner();
        Ok(CompiledFunction {
            length,
            params: param_names,
            rest_param,
            chunk: fn_chunk,
            is_strict: body.is_strict,
            uses_arguments,
            local_layout: Arc::new(local_layout),
            upvalue_layout: Arc::new(upvalue_layout),
            dynamic_scope,
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
            Expression::TaggedTemplate { tag, template } => {
                self.compile_tagged_template(tag, template, chunk, context)
            }
            Expression::Parenthesized(inner) => self.compile_expression(inner, chunk, context),
            Expression::Spread(_) => Err(CompileError {
                is_syntax: false,
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
            Expression::NewTarget => {
                // Emit the `new.target` meta-property. In non-constructor calls this
                // is `undefined`; in constructor calls it holds the constructor function.
                // We emit a dedicated instruction so the VM can inspect the call frame.
                chunk.emit(Instruction::LoadNewTarget);
                Ok(())
            }
            Expression::ImportMeta => {
                // `import.meta` — module meta-object. Runtime support lives in B/C;
                // for now emit an empty object so the parse shape is accepted.
                chunk.emit(Instruction::ObjectCreateEmpty);
                Ok(())
            }
            Expression::Await(value) => {
                self.compile_expression(value, chunk, context)?;
                chunk.emit(Instruction::AwaitValue);
                Ok(())
            }
            Expression::DynamicImport { specifier, options } => {
                self.compile_expression(specifier, chunk, context)?;
                if let Some(options) = options {
                    self.compile_expression(options, chunk, context)?;
                } else {
                    let undefined = chunk
                        .add_constant(Constant::Undefined)
                        .map_err(CompileError::from_chunk)?;
                    chunk.emit(Instruction::Constant(undefined));
                }
                chunk.emit(Instruction::DynamicImport);
                Ok(())
            }
            Expression::Sequence(exprs) => {
                for (i, expr) in exprs.iter().enumerate() {
                    self.compile_expression(expr, chunk, context)?;
                    if i + 1 < exprs.len() {
                        chunk.emit(Instruction::Pop);
                    }
                }
                Ok(())
            }
            Expression::OptionalChain { base, steps } => {
                self.compile_optional_chain(base, steps, false, chunk, context)
            }
            Expression::PrivateName(_) => Err(CompileError::unsupported(
                "standalone private name expression",
            )),
        }
    }

    fn compile_optional_chain(
        &mut self,
        base: &Expression,
        steps: &[OptionalChainStep],
        preserve_final_receiver: bool,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::CallArgument;

        let first_step_is_call = matches!(steps.first(), Some(OptionalChainStep::Call { .. }));
        let mut call_has_receiver = false;
        let mut callable_base = base;
        while let Expression::Parenthesized(inner) = callable_base {
            callable_base = inner;
        }
        if first_step_is_call {
            if let Expression::Member {
                object,
                property,
                computed,
            } = callable_base
            {
                if matches!(object.as_ref(), Expression::Super) {
                    if *computed {
                        self.compile_expression(property, chunk, context)?;
                        chunk.emit(Instruction::GetSuperElementMethod);
                    } else {
                        let name = match property.as_ref() {
                            Expression::Identifier(name) => name.clone(),
                            Expression::PrivateName(name) => format!("\x00#{name}"),
                            other => {
                                return Err(CompileError::unsupported(format!(
                                    "non-identifier optional super method property {other:?}"
                                )));
                            }
                        };
                        let index = self.add_name(&name, chunk)?;
                        chunk.emit(Instruction::GetSuperMethod(index));
                    }
                } else {
                    self.compile_expression(object, chunk, context)?;
                    if *computed {
                        self.compile_expression(property, chunk, context)?;
                        chunk.emit(Instruction::GetElementMethod);
                    } else {
                        let name = match property.as_ref() {
                            Expression::Identifier(name) => name.clone(),
                            Expression::PrivateName(name) => format!("\x00#{name}"),
                            other => {
                                return Err(CompileError::unsupported(format!(
                                    "non-identifier optional method property {other:?}"
                                )));
                            }
                        };
                        let index = self.add_name(&name, chunk)?;
                        chunk.emit(Instruction::GetMethod(index));
                    }
                }
                call_has_receiver = true;
            } else if let Expression::OptionalChain {
                base: nested_base,
                steps: nested_steps,
            } = callable_base
                && matches!(nested_steps.last(), Some(OptionalChainStep::Member { .. }))
            {
                self.compile_optional_chain(nested_base, nested_steps, true, chunk, context)?;
                call_has_receiver = true;
            } else {
                self.compile_expression(base, chunk, context)?;
            }
        } else {
            self.compile_expression(base, chunk, context)?;
        }

        // Collect offsets of `Jump(placeholder)` instructions in the null paths so we
        // can back-patch them all to the same `all_done` target once we know it.
        let mut null_path_jumps: Vec<usize> = Vec::new();

        for (step_index, step) in steps.iter().enumerate() {
            let is_optional = match step {
                OptionalChainStep::Member { optional, .. }
                | OptionalChainStep::Call { optional, .. } => *optional,
            };

            if is_optional {
                let optional_receiver_call =
                    matches!(step, OptionalChainStep::Call { .. }) && call_has_receiver;
                if optional_receiver_call {
                    // GetMethod/GetElementMethod leave [callee, receiver]. Put the
                    // callee on top for the nullish test, then restore the call
                    // layout on the non-nullish path.
                    chunk.emit(Instruction::Swap);
                }
                // Peek the top value: if not null/undefined skip the null path.
                let skip_null = chunk.emit(Instruction::JumpIfNotNullish(usize::MAX));
                // Null path: discard the nullish value, push undefined, jump to all_done.
                chunk.emit(Instruction::Pop);
                if optional_receiver_call {
                    chunk.emit(Instruction::Pop);
                }
                let undef_idx = chunk
                    .add_constant(Constant::Undefined)
                    .map_err(CompileError::from_chunk)?;
                chunk.emit(Instruction::Constant(undef_idx));
                if preserve_final_receiver {
                    chunk.emit(Instruction::Constant(undef_idx));
                }
                let jump_to_done = chunk.emit(Instruction::Jump(usize::MAX));
                null_path_jumps.push(jump_to_done);
                // Patch the skip-null jump to the instruction that follows.
                chunk
                    .patch_jump(skip_null, chunk.current_offset())
                    .map_err(CompileError::from_chunk)?;
                if optional_receiver_call {
                    chunk.emit(Instruction::Swap);
                }
            }

            match step {
                OptionalChainStep::Member {
                    property, computed, ..
                } => {
                    let followed_by_call = matches!(
                        steps.get(step_index + 1),
                        Some(OptionalChainStep::Call { .. })
                    );
                    let needs_receiver = followed_by_call
                        || (preserve_final_receiver && step_index + 1 == steps.len());
                    if *computed {
                        self.compile_expression(property, chunk, context)?;
                        chunk.emit(if needs_receiver {
                            Instruction::GetElementMethod
                        } else {
                            Instruction::GetElement
                        });
                    } else {
                        let name = match property.as_ref() {
                            Expression::Identifier(n) => n.clone(),
                            Expression::PrivateName(n) => format!("\x00#{n}"),
                            other => {
                                return Err(CompileError::unsupported(format!(
                                    "non-identifier optional member property {other:?}"
                                )));
                            }
                        };
                        let idx = self.add_name(&name, chunk)?;
                        chunk.emit(if needs_receiver {
                            Instruction::GetMethod(idx)
                        } else {
                            Instruction::GetProperty(idx)
                        });
                    }
                    call_has_receiver = needs_receiver;
                }
                OptionalChainStep::Call { arguments, .. } => {
                    let has_spread = arguments
                        .iter()
                        .any(|a| matches!(a, CallArgument::Spread(_)));
                    if has_spread {
                        return Err(CompileError::unsupported(
                            "spread argument in optional call is not yet supported",
                        ));
                    }
                    let n = u16::try_from(arguments.len()).map_err(|_| CompileError {
                        is_syntax: false,
                        message: "too many optional call arguments".into(),
                    })?;
                    for arg in arguments {
                        let CallArgument::Expression(e) = arg else {
                            unreachable!()
                        };
                        self.compile_expression(e, chunk, context)?;
                    }
                    chunk.emit(if call_has_receiver {
                        Instruction::CallWithThis(n)
                    } else {
                        Instruction::Call(n)
                    });
                    call_has_receiver = false;
                }
            }
        }

        // Back-patch all null-path jumps to point here.
        let all_done = chunk.current_offset();
        for jump_offset in null_path_jumps {
            chunk
                .patch_jump(jump_offset, all_done)
                .map_err(CompileError::from_chunk)?;
        }

        Ok(())
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
            Literal::BigInt(raw) => Constant::BigInt(parse_bigint_literal(raw)?),
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
        if let Some(slot) = context.local_slot(name) {
            chunk.emit(Instruction::LoadLocal(slot));
            return Ok(());
        }
        if let Some(slot) = context.upvalue_slot(name) {
            chunk.emit(Instruction::LoadUpvalue(slot));
            return Ok(());
        }
        let name_index = self.add_name(name, chunk)?;
        if context.needs_dynamic_name_lookup(name) {
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
            UnaryOperator::BitwiseNot => Instruction::BitwiseNot,
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
                    if let Some(slot) = context.local_slot(name) {
                        chunk.emit(Instruction::LoadLocal(slot));
                        chunk.emit(Instruction::TypeOf);
                        return Ok(());
                    }
                    if let Some(slot) = context.upvalue_slot(name) {
                        chunk.emit(Instruction::LoadUpvalue(slot));
                        chunk.emit(Instruction::TypeOf);
                        return Ok(());
                    }
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
            BinaryOperator::NullishCoalescing => {
                return self.compile_logical(LogicalOperator::Nullish, left, right, chunk, context);
            }
            _ => {}
        }

        let instruction = match operator {
            BinaryOperator::Add => Instruction::Add,
            BinaryOperator::Subtract => Instruction::Subtract,
            BinaryOperator::Multiply => Instruction::Multiply,
            BinaryOperator::Divide => Instruction::Divide,
            BinaryOperator::Remainder => Instruction::Remainder,
            BinaryOperator::Exponentiation => Instruction::Exponentiation,
            BinaryOperator::BitwiseAnd => Instruction::BitwiseAnd,
            BinaryOperator::BitwiseOr => Instruction::BitwiseOr,
            BinaryOperator::BitwiseXor => Instruction::BitwiseXor,
            BinaryOperator::LeftShift => Instruction::LeftShift,
            BinaryOperator::RightShift => Instruction::RightShift,
            BinaryOperator::UnsignedRightShift => Instruction::UnsignedRightShift,
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
            BinaryOperator::LogicalAnd
            | BinaryOperator::LogicalOr
            | BinaryOperator::NullishCoalescing => unreachable!(),
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
            LogicalOperator::Nullish => chunk.emit(Instruction::JumpIfNotNullish(usize::MAX)),
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
        let inferred_name = if name == "\0module_default" {
            "default"
        } else {
            name
        };
        if kind == VariableKind::Var {
            if let Some(initializer) = initializer {
                let name_index = self.add_name(name, chunk)?;
                self.compile_expression(initializer, chunk, context)?;
                if is_anonymous_function_definition(initializer) {
                    let inferred_name_index = self.add_name(inferred_name, chunk)?;
                    chunk.emit(Instruction::SetFunctionName(inferred_name_index));
                }
                if context.inside_function() {
                    if let Some(slot) = context.local_slot(name) {
                        chunk.emit(Instruction::StoreLocal(slot));
                    } else {
                        chunk.emit(Instruction::StoreName(name_index));
                    }
                } else {
                    chunk.emit(Instruction::StoreGlobal(name_index));
                }
                chunk.emit(Instruction::Pop);
            }
            return Ok(());
        }

        if kind == VariableKind::Const && initializer.is_none() {
            return Err(CompileError::unsupported(
                "const declaration without an initializer",
            ));
        }

        match initializer {
            Some(initializer) => {
                self.compile_expression(initializer, chunk, context)?;
                // Spec: anonymous function definition in a variable declarator → infer name.
                if is_anonymous_function_definition(initializer) {
                    let name_idx = self.add_name(inferred_name, chunk)?;
                    chunk.emit(Instruction::SetFunctionName(name_idx));
                }
            }
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
                // Spec: anonymous function definition assigned to identifier → infer name.
                if is_anonymous_function_definition(value) {
                    let name_idx = self.add_name(name, chunk)?;
                    chunk.emit(Instruction::SetFunctionName(name_idx));
                }
                self.emit_store_identifier(name, chunk, context)?;
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: false,
            } => {
                let property_name = match property.as_ref() {
                    Expression::Identifier(name) => name.clone(),
                    Expression::PrivateName(name) => format!("\x00#{name}"),
                    _ => {
                        return Err(CompileError::unsupported(
                            "non-identifier static member as assignment target",
                        ));
                    }
                };
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(value, chunk, context)?;
                let prop_index = self.add_name(&property_name, chunk)?;
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
            Expression::Array(elements) => {
                self.compile_expression(value, chunk, context)?;
                self.compile_array_destructuring_assignment(elements, chunk, context)?;
                Ok(())
            }
            Expression::Object(props) => {
                self.compile_expression(value, chunk, context)?;
                self.compile_object_destructuring_assignment(props, chunk, context)?;
                Ok(())
            }
            _ => Err(CompileError::unsupported(format!(
                "assignment target {target:?}"
            ))),
        }
    }

    /// Compiles array destructuring assignment: `[a, , b, ...rest] = rhs`.
    /// The RHS value is already on the stack. Leaves the RHS on the stack (assignment result).
    fn compile_array_destructuring_assignment(
        &mut self,
        elements: &[ArrayElement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::ArrayElement;

        const RHS_SLOT: &str = "\u{0}dstr_rhs";
        const ITER_SLOT: &str = "\u{0}dstr_iter";

        chunk.emit(Instruction::CreateLexicalEnvironment);
        context.environment_depth += 1;
        let mut scope = std::collections::HashSet::new();

        let rhs_idx = self.add_name(RHS_SLOT, chunk)?;
        let iter_idx = self.add_name(ITER_SLOT, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(rhs_idx));
        scope.insert(RHS_SLOT.to_string());
        chunk.emit(Instruction::CreateMutableBinding(iter_idx));
        scope.insert(ITER_SLOT.to_string());

        // rhs is on stack. Store it, then GetIterator and store the iterator.
        chunk.emit(Instruction::InitializeBinding(rhs_idx)); // pops rhs, stores
        chunk.emit(Instruction::LoadName(rhs_idx)); // push rhs back
        chunk.emit(Instruction::GetIterator); // rhs → iter
        chunk.emit(Instruction::InitializeBinding(iter_idx)); // store iter (pops)

        let undef_idx = chunk
            .add_constant(Constant::Undefined)
            .map_err(CompileError::from_chunk)?;
        context.lexical_scopes.push(scope);

        let protected_start = chunk.current_offset();
        let handler_environment_depth = context.environment_depth;
        for elem in elements {
            match elem {
                ArrayElement::Hole => {
                    // Consume one iterator value and discard.
                    chunk.emit(Instruction::LoadName(iter_idx));
                    chunk.emit(Instruction::IteratorNext); // → [value, done]
                    chunk.emit(Instruction::Pop); // pop done
                    chunk.emit(Instruction::Pop); // pop value
                }
                ArrayElement::Expression(target_expr) => {
                    let assignment_target =
                        if let Expression::Assignment { target, .. } = target_expr {
                            target.as_ref()
                        } else {
                            target_expr
                        };
                    let precomputed_member =
                        self.precompute_computed_member_target(assignment_target, chunk, context)?;
                    chunk.emit(Instruction::LoadName(iter_idx));
                    chunk.emit(Instruction::IteratorNext); // → [value, done]

                    let done_jump = chunk.emit(Instruction::JumpIfTrue(usize::MAX)); // if done, jump
                    chunk.emit(Instruction::Pop); // pop done=false → [value]

                    // value is on stack: handle optional default.
                    if let Expression::Assignment {
                        target,
                        value: default_expr,
                    } = target_expr
                    {
                        // [value]; if value is undefined, use default instead (null does NOT trigger).
                        let skip_default = chunk.emit(Instruction::JumpIfNotUndefined(usize::MAX));
                        chunk.emit(Instruction::Pop); // pop undefined
                        self.compile_expression(default_expr, chunk, context)?;
                        // Spec: SetFunctionName when anon fn default assigned to identifier ref.
                        if is_anonymous_function_definition(default_expr)
                            && let Expression::Identifier(binding_name) = target.as_ref()
                        {
                            let nm = self.add_name(binding_name, chunk)?;
                            chunk.emit(Instruction::SetFunctionName(nm));
                        }
                        let after_default = chunk.current_offset();
                        chunk
                            .patch_jump(skip_default, after_default)
                            .map_err(CompileError::from_chunk)?;
                        if let Some((object_idx, key_idx)) = precomputed_member {
                            self.assign_to_precomputed_member(object_idx, key_idx, chunk)?;
                        } else {
                            self.assign_dstr_element_to_target(target, chunk, context)?;
                        }
                    } else if let Some((object_idx, key_idx)) = precomputed_member {
                        self.assign_to_precomputed_member(object_idx, key_idx, chunk)?;
                    } else {
                        self.assign_dstr_element_to_target(target_expr, chunk, context)?;
                    }

                    let after_jump = chunk.emit(Instruction::Jump(usize::MAX));

                    // done=true path: value is undefined (or use default).
                    let done_target = chunk.current_offset();
                    chunk
                        .patch_jump(done_jump, done_target)
                        .map_err(CompileError::from_chunk)?;
                    chunk.emit(Instruction::Pop); // pop done=true
                    chunk.emit(Instruction::Pop); // pop iterator value placeholder
                    if let Expression::Assignment {
                        target,
                        value: default_expr,
                    } = target_expr
                    {
                        self.compile_expression(default_expr, chunk, context)?;
                        if is_anonymous_function_definition(default_expr)
                            && let Expression::Identifier(binding_name) = target.as_ref()
                        {
                            let nm = self.add_name(binding_name, chunk)?;
                            chunk.emit(Instruction::SetFunctionName(nm));
                        }
                        if let Some((object_idx, key_idx)) = precomputed_member {
                            self.assign_to_precomputed_member(object_idx, key_idx, chunk)?;
                        } else {
                            self.assign_dstr_element_to_target(target, chunk, context)?;
                        }
                    } else {
                        chunk.emit(Instruction::Constant(undef_idx));
                        if let Some((object_idx, key_idx)) = precomputed_member {
                            self.assign_to_precomputed_member(object_idx, key_idx, chunk)?;
                        } else {
                            self.assign_dstr_element_to_target(target_expr, chunk, context)?;
                        }
                    }

                    let after_target = chunk.current_offset();
                    chunk
                        .patch_jump(after_jump, after_target)
                        .map_err(CompileError::from_chunk)?;
                }
                ArrayElement::Spread(rest_target) => {
                    let precomputed_member =
                        self.precompute_computed_member_target(rest_target, chunk, context)?;
                    // Collect remaining iterator values into an array.
                    chunk.emit(Instruction::ArrayCreate(0)); // [] on stack
                    let loop_start = chunk.current_offset();
                    chunk.emit(Instruction::LoadName(iter_idx));
                    chunk.emit(Instruction::IteratorNext); // → [array, value, done]
                    let exit_jump = chunk.emit(Instruction::JumpIfTrue(usize::MAX));
                    chunk.emit(Instruction::Pop); // pop done=false → [array, value]
                    chunk.emit(Instruction::ArrayPush); // [array, value] → [array]
                    chunk.emit(Instruction::Jump(loop_start));
                    let exit_target = chunk.current_offset();
                    chunk
                        .patch_jump(exit_jump, exit_target)
                        .map_err(CompileError::from_chunk)?;
                    chunk.emit(Instruction::Pop); // pop done=true → [array]
                    chunk.emit(Instruction::Pop); // pop iterator value placeholder
                    if let Some((object_idx, key_idx)) = precomputed_member {
                        self.assign_to_precomputed_member(object_idx, key_idx, chunk)?;
                    } else {
                        self.assign_dstr_element_to_target(rest_target, chunk, context)?;
                    }
                }
            }
        }
        let protected_end = chunk.current_offset();
        let normal_exit = chunk.emit(Instruction::Jump(usize::MAX));

        let finally_target = chunk.current_offset();
        chunk.emit(Instruction::LoadName(iter_idx));
        chunk.emit(Instruction::IteratorClose);
        chunk.emit(Instruction::EndFinally);

        let after_finally = chunk.current_offset();
        chunk
            .patch_jump(normal_exit, after_finally)
            .map_err(CompileError::from_chunk)?;
        if protected_start < protected_end {
            chunk.handlers.push(ExceptionHandler {
                start: protected_start,
                end: protected_end,
                target: finally_target,
                kind: HandlerKind::Finally,
                stack_depth: u32::from(context.has_completion_slot),
                environment_depth: handler_environment_depth,
            });
        }

        chunk.emit(Instruction::LoadName(iter_idx));
        chunk.emit(Instruction::IteratorClose);
        chunk.emit(Instruction::LoadName(rhs_idx));
        context.lexical_scopes.pop();
        chunk.emit(Instruction::PopEnvironment);
        context.environment_depth -= 1;
        Ok(())
    }

    fn precompute_computed_member_target(
        &mut self,
        target: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<Option<(u16, u16)>, CompileError> {
        let Expression::Member {
            object,
            property,
            computed: true,
        } = target
        else {
            return Ok(None);
        };

        let suffix = chunk.current_offset();
        let object_name = format!("\u{0}dstr_ref_obj{suffix}");
        let key_name = format!("\u{0}dstr_ref_key{suffix}");
        let object_idx = self.add_name(&object_name, chunk)?;
        let key_idx = self.add_name(&key_name, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(object_idx));
        chunk.emit(Instruction::CreateMutableBinding(key_idx));
        if let Some(scope) = context.lexical_scopes.last_mut() {
            scope.insert(object_name);
            scope.insert(key_name);
        }
        self.compile_expression(object, chunk, context)?;
        chunk.emit(Instruction::InitializeBinding(object_idx));
        self.compile_expression(property, chunk, context)?;
        chunk.emit(Instruction::InitializeBinding(key_idx));
        Ok(Some((object_idx, key_idx)))
    }

    fn assign_to_precomputed_member(
        &mut self,
        object_idx: u16,
        key_idx: u16,
        chunk: &mut Chunk,
    ) -> Result<(), CompileError> {
        let value_name = format!("\u{0}dstr_ref_val{}", chunk.current_offset());
        let value_idx = self.add_name(&value_name, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(value_idx));
        chunk.emit(Instruction::InitializeBinding(value_idx));
        chunk.emit(Instruction::LoadName(object_idx));
        chunk.emit(Instruction::LoadName(key_idx));
        chunk.emit(Instruction::LoadName(value_idx));
        chunk.emit(Instruction::SetElement);
        chunk.emit(Instruction::Pop);
        Ok(())
    }

    /// Assigns the top-of-stack value to a single destructuring assignment target.
    /// Pops the value (does NOT leave it on the stack).
    fn assign_dstr_element_to_target(
        &mut self,
        target: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match target {
            Expression::Identifier(name) => {
                self.emit_store_identifier(name, chunk, context)?;
                chunk.emit(Instruction::Pop); // StoreName leaves value; discard it
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: false,
            } => {
                let prop_name = match property.as_ref() {
                    Expression::Identifier(n) => n.clone(),
                    Expression::PrivateName(n) => format!("#{n}"),
                    _ => {
                        return Err(CompileError::unsupported(
                            "non-identifier member in destructuring assignment",
                        ));
                    }
                };
                // Stack: [value]. Need [object, value] for SetProperty.
                // Use Swap after pushing object: push obj first, then Swap.
                // [value] → [value, object] → [object, value] via Swap.
                // Actually simpler: compile a no-op store via the Assignment compile path.
                // Or: duplicate stack trick: SetProperty pops [obj, val] → [val], then Pop.
                // Let's do: (value is on TOS)
                //   compile object  → [value, object]
                //   Swap            → [object, value]
                //   SetProperty     → [value] (leaves val on stack)
                //   Pop             → []
                self.compile_expression(object, chunk, context)?; // [value, object]
                chunk.emit(Instruction::Swap); // [object, value]
                let prop_idx = self.add_name(&prop_name, chunk)?;
                chunk.emit(Instruction::SetProperty(prop_idx)); // [object, value] → [value]
                chunk.emit(Instruction::Pop);
                Ok(())
            }
            Expression::Member {
                object,
                property,
                computed: true,
            } => {
                // Stack: [value]. Need [object, key, value] for SetElement.
                // ponytail: use a one-binding lexical scope as the missing rotate/tuck;
                // replace with a dedicated stack instruction if this path gets hot.
                const VAL_SLOT: &str = "\u{0}dstr_mv";
                chunk.emit(Instruction::CreateLexicalEnvironment);
                context.environment_depth += 1;
                let val_idx = self.add_name(VAL_SLOT, chunk)?;
                chunk.emit(Instruction::CreateMutableBinding(val_idx));
                chunk.emit(Instruction::InitializeBinding(val_idx)); // store value
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::LoadName(val_idx));
                chunk.emit(Instruction::SetElement); // → [value]
                chunk.emit(Instruction::Pop);
                chunk.emit(Instruction::PopEnvironment);
                context.environment_depth -= 1;
                Ok(())
            }
            Expression::Array(elements) => {
                // Nested array destructuring: value is an iterable.
                self.compile_array_destructuring_assignment(elements, chunk, context)?;
                chunk.emit(Instruction::Pop); // discard nested rhs
                Ok(())
            }
            Expression::Object(props) => {
                self.compile_object_destructuring_assignment(props, chunk, context)?;
                chunk.emit(Instruction::Pop);
                Ok(())
            }
            _ => Err(CompileError::unsupported(format!(
                "complex destructuring assignment target: {target:?}"
            ))),
        }
    }

    /// Compiles object destructuring assignment: `{a, b: c, ...rest} = rhs`.
    /// The RHS value is already on the stack. Leaves the RHS on the stack.
    fn compile_object_destructuring_assignment(
        &mut self,
        props: &[ObjectProperty],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::{ObjectProperty, PropertyName};

        const RHS_SLOT: &str = "\u{0}dstr_obj_rhs";

        chunk.emit(Instruction::RequireObjectCoercible);
        chunk.emit(Instruction::CreateLexicalEnvironment);
        context.environment_depth += 1;
        let mut scope = std::collections::HashSet::new();

        let rhs_idx = self.add_name(RHS_SLOT, chunk)?;
        chunk.emit(Instruction::CreateMutableBinding(rhs_idx));
        scope.insert(RHS_SLOT.to_string());
        chunk.emit(Instruction::InitializeBinding(rhs_idx)); // store rhs (pops)

        context.lexical_scopes.push(scope);
        let mut rest_excluded = Vec::new();

        for prop in props {
            match prop {
                ObjectProperty::Data {
                    key,
                    value: val_target,
                } => {
                    let key_name = match key {
                        PropertyName::Computed(_) => {
                            return Err(CompileError::unsupported(
                                "computed key in object destructuring assignment",
                            ));
                        }
                        _ => key.to_key_string(),
                    };
                    rest_excluded.push(ObjectRestExcludedKey::Static(key_name.clone()));
                    chunk.emit(Instruction::LoadName(rhs_idx));
                    let key_idx = self.add_name(&key_name, chunk)?;
                    chunk.emit(Instruction::GetProperty(key_idx)); // → [prop_value]

                    if let Expression::Assignment {
                        target,
                        value: default_expr,
                    } = val_target
                    {
                        // Defaults only apply when value is undefined, not null.
                        let skip_default = chunk.emit(Instruction::JumpIfNotUndefined(usize::MAX));
                        chunk.emit(Instruction::Pop);
                        self.compile_expression(default_expr, chunk, context)?;
                        // Spec: SetFunctionName when anon fn default assigned to identifier ref.
                        if is_anonymous_function_definition(default_expr)
                            && let Expression::Identifier(binding_name) = target.as_ref()
                        {
                            let nm = self.add_name(binding_name, chunk)?;
                            chunk.emit(Instruction::SetFunctionName(nm));
                        }
                        let after_default = chunk.current_offset();
                        chunk
                            .patch_jump(skip_default, after_default)
                            .map_err(CompileError::from_chunk)?;
                        self.assign_dstr_element_to_target(target, chunk, context)?;
                    } else {
                        self.assign_dstr_element_to_target(val_target, chunk, context)?;
                    }
                }
                ObjectProperty::ComputedData {
                    key,
                    value: val_target,
                } => {
                    chunk.emit(Instruction::LoadName(rhs_idx));
                    self.compile_expression(key, chunk, context)?;
                    let temp_name = format!("\u{0}dstr_obj_key{}", rest_excluded.len());
                    let temp_idx = self.add_name(&temp_name, chunk)?;
                    chunk.emit(Instruction::CreateMutableBinding(temp_idx));
                    if let Some(scope) = context.lexical_scopes.last_mut() {
                        scope.insert(temp_name.clone());
                    }
                    chunk.emit(Instruction::Duplicate);
                    chunk.emit(Instruction::InitializeBinding(temp_idx));
                    rest_excluded.push(ObjectRestExcludedKey::Temp(temp_name));
                    chunk.emit(Instruction::GetElement); // → [prop_value]
                    self.assign_dstr_element_to_target(val_target, chunk, context)?;
                }
                ObjectProperty::Spread(rest_target) => {
                    chunk.emit(Instruction::LoadName(rhs_idx));
                    if let Some(excluded_count) =
                        Some(self.emit_object_rest_excluded_keys(&rest_excluded, chunk)?)
                    {
                        chunk.emit(Instruction::CopyDataPropertiesExcluded(excluded_count));
                        self.assign_dstr_element_to_target(rest_target, chunk, context)?;
                        continue;
                    }
                    // Create a shallow copy of rhs into a new object (simplified: copies all props).
                    chunk.emit(Instruction::ObjectCreateEmpty); // → [{}]
                    chunk.emit(Instruction::LoadName(rhs_idx)); // → [{}, rhs]
                    chunk.emit(Instruction::SpreadObject); // [{}, rhs] → [{}+rhs_props]
                    self.assign_dstr_element_to_target(rest_target, chunk, context)?;
                }
                _ => {
                    return Err(CompileError::unsupported(
                        "unsupported property in object destructuring assignment",
                    ));
                }
            }
        }

        // Leave the original rhs on the stack as the assignment expression result.
        chunk.emit(Instruction::LoadName(rhs_idx));
        context.lexical_scopes.pop();
        chunk.emit(Instruction::PopEnvironment);
        context.environment_depth -= 1;
        Ok(())
    }

    fn compile_compound_assignment(
        &mut self,
        operator: AssignmentOperator,
        target: &Expression,
        value: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if let Some(instruction) = compound_assignment_instruction(operator) {
            return self.compile_compound_assignment_arithmetic(
                instruction,
                target,
                value,
                chunk,
                context,
            );
        }
        // Logical compound assignments: &&=, ||=, ??=
        // For identifier targets: load; jump-if-short-circuit (keeps value); pop; rhs; store.
        // For member targets: we need the object available for the store, so we use a Swap+Pop
        // on the short-circuit path to remove the extra object copy.
        let jump_variant = match operator {
            AssignmentOperator::LogicalAnd => 0u8,
            AssignmentOperator::LogicalOr => 1,
            AssignmentOperator::NullishCoalescing => 2,
            _ => unreachable!(),
        };

        match target {
            Expression::Member {
                object,
                property,
                computed,
            } => {
                if *computed {
                    // Computed logical assign: `a[b] ??= rhs`
                    // Stack protocol:
                    //   eval obj → [obj]
                    //   eval key → [obj, key]
                    //   DuplicatePair → [obj, key, obj, key]
                    //   GetElement → [obj, key, old_val]
                    //   JumpIfXxx(sc) — observes old_val
                    //   Pop → [obj, key]              (drop old_val on non-SC path)
                    //   eval rhs → [obj, key, rhs]
                    //   SetElement → [rhs]
                    //   Jump(end)
                    //  sc: Swap;Pop;Swap;Pop → [old_val]  (drop obj,key)
                    //  end:
                    self.compile_expression(object, chunk, context)?;
                    self.compile_expression(property, chunk, context)?;
                    chunk.emit(Instruction::DuplicatePair);
                    chunk.emit(Instruction::GetElement);
                    let jump_instr = match jump_variant {
                        0 => chunk.emit(Instruction::JumpIfFalse(usize::MAX)),
                        1 => chunk.emit(Instruction::JumpIfTrue(usize::MAX)),
                        _ => chunk.emit(Instruction::JumpIfNotNullish(usize::MAX)),
                    };
                    chunk.emit(Instruction::Pop); // drop old_val
                    self.compile_expression(value, chunk, context)?;
                    chunk.emit(Instruction::SetElement);
                    let jump_end = chunk.emit(Instruction::Jump(usize::MAX));
                    let sc_target = chunk.current_offset();
                    // [obj, key, old_val] → keep only old_val
                    chunk.emit(Instruction::Swap); // [obj, old_val, key]
                    chunk.emit(Instruction::Pop); // [obj, old_val]
                    chunk.emit(Instruction::Swap); // [old_val, obj]
                    chunk.emit(Instruction::Pop); // [old_val]
                    let end_target = chunk.current_offset();
                    chunk
                        .patch_jump(jump_instr, sc_target)
                        .map_err(CompileError::from_chunk)?;
                    chunk
                        .patch_jump(jump_end, end_target)
                        .map_err(CompileError::from_chunk)?;
                    return Ok(());
                }
                let prop_name = match property.as_ref() {
                    Expression::Identifier(n) => n.clone(),
                    Expression::PrivateName(n) => format!("#{n}"),
                    _ => {
                        return Err(CompileError::unsupported(
                            "non-identifier static member as logical assignment target",
                        ));
                    }
                };
                // emit: compile(obj), Duplicate, GetProperty → [obj, old_val]
                self.compile_expression(object, chunk, context)?;
                chunk.emit(Instruction::Duplicate);
                {
                    let prop_index = self.add_name(&prop_name, chunk)?;
                    chunk.emit(Instruction::GetProperty(prop_index));
                    // Stack: [obj, old_val]
                    let jump_instr = match jump_variant {
                        0 => chunk.emit(Instruction::JumpIfFalse(usize::MAX)),
                        1 => chunk.emit(Instruction::JumpIfTrue(usize::MAX)),
                        _ => chunk.emit(Instruction::JumpIfNotNullish(usize::MAX)),
                    };
                    // Non-short-circuit path: pop old_val, eval rhs, SetProperty
                    chunk.emit(Instruction::Pop); // [obj]
                    self.compile_expression(value, chunk, context)?; // [obj, new_val]
                    chunk.emit(Instruction::SetProperty(prop_index)); // [new_val]
                    let jump_end = chunk.emit(Instruction::Jump(usize::MAX));
                    // Short-circuit path: [obj, old_val] → Swap → [old_val, obj] → Pop → [old_val]
                    let sc_target = chunk.current_offset();
                    chunk.emit(Instruction::Swap);
                    chunk.emit(Instruction::Pop);
                    let end_target = chunk.current_offset();
                    chunk
                        .patch_jump(jump_instr, sc_target)
                        .map_err(CompileError::from_chunk)?;
                    chunk
                        .patch_jump(jump_end, end_target)
                        .map_err(CompileError::from_chunk)?;
                }
            }
            _ => {
                self.compile_load_target(target, chunk, context)?;
                let jump_instr = match jump_variant {
                    0 => chunk.emit(Instruction::JumpIfFalse(usize::MAX)),
                    1 => chunk.emit(Instruction::JumpIfTrue(usize::MAX)),
                    _ => chunk.emit(Instruction::JumpIfNotNullish(usize::MAX)),
                };
                chunk.emit(Instruction::Pop);
                self.compile_expression(value, chunk, context)?;
                if is_anonymous_function_definition(value)
                    && let Expression::Identifier(name) = target
                {
                    let name_index = self.add_name(name, chunk)?;
                    chunk.emit(Instruction::SetFunctionName(name_index));
                }
                self.emit_store_target(target, chunk, context)?;
                chunk
                    .patch_jump(jump_instr, chunk.current_offset())
                    .map_err(CompileError::from_chunk)?;
            }
        }
        Ok(())
    }

    fn compile_compound_assignment_arithmetic(
        &mut self,
        instruction: Instruction,
        target: &Expression,
        value: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
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
                let property_name = match property.as_ref() {
                    Expression::Identifier(name) => name.clone(),
                    Expression::PrivateName(name) => format!("\x00#{name}"),
                    _ => {
                        return Err(CompileError::unsupported(
                            "non-identifier static member as compound assignment target",
                        ));
                    }
                };
                self.compile_expression(object, chunk, context)?;
                chunk.emit(Instruction::Duplicate);
                let property_index = self.add_name(&property_name, chunk)?;
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

    fn compile_load_target(
        &mut self,
        target: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match target {
            Expression::Identifier(name) => self.compile_identifier(name, chunk, context),
            _ => Err(CompileError::unsupported(
                "non-identifier target for logical compound assignment",
            )),
        }
    }

    fn emit_store_target(
        &mut self,
        target: &Expression,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        match target {
            Expression::Identifier(name) => self.emit_store_identifier(name, chunk, context),
            _ => Err(CompileError::unsupported(
                "non-identifier target for logical compound assignment",
            )),
        }
    }

    fn add_name(&mut self, name: &str, chunk: &mut Chunk) -> Result<u16, CompileError> {
        chunk
            .add_constant(Constant::String(name.into()))
            .map_err(CompileError::from_chunk)
    }

    fn emit_object_rest_excluded_keys(
        &mut self,
        excluded: &[ObjectRestExcludedKey],
        chunk: &mut Chunk,
    ) -> Result<u16, CompileError> {
        let count = u16::try_from(excluded.len()).map_err(|_| {
            CompileError::unsupported("too many excluded keys in object rest pattern")
        })?;
        for key in excluded {
            match key {
                ObjectRestExcludedKey::Static(name) => {
                    let key_idx = self.add_name(name, chunk)?;
                    chunk.emit(Instruction::Constant(key_idx));
                }
                ObjectRestExcludedKey::Temp(name) => {
                    let key_idx = self.add_name(name, chunk)?;
                    chunk.emit(Instruction::LoadName(key_idx));
                }
            }
        }
        Ok(count)
    }

    fn compile_member(
        &mut self,
        object: &Expression,
        property: &Expression,
        computed: bool,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        if matches!(object, Expression::Super) {
            if computed {
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::GetSuperElementMethod);
            } else {
                let property_name = match property {
                    Expression::Identifier(name) => name.clone(),
                    Expression::PrivateName(name) => format!("\x00#{name}"),
                    _ => {
                        return Err(CompileError::unsupported(format!(
                            "non-identifier super property {property:?}"
                        )));
                    }
                };
                let property_index = self.add_name(&property_name, chunk)?;
                chunk.emit(Instruction::GetSuperMethod(property_index));
            }
            // Super lookup opcodes also preserve the receiver for a following
            // call. A plain property read only keeps the resolved value.
            chunk.emit(Instruction::Pop);
            return Ok(());
        }

        if computed {
            // object[key]  →  push object, push key, GetElement
            self.compile_expression(object, chunk, context)?;
            self.compile_expression(property, chunk, context)?;
            chunk.emit(Instruction::GetElement);
            return Ok(());
        }

        let property_name = match property {
            Expression::Identifier(name) => name.clone(),
            Expression::PrivateName(name) => format!("\x00#{name}"),
            _ => {
                return Err(CompileError::unsupported(format!(
                    "non-identifier member property {property:?}"
                )));
            }
        };

        self.compile_expression(object, chunk, context)?;
        let property_index = self.add_name(&property_name, chunk)?;
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
        let mut callee = callee;
        while let Expression::Parenthesized(inner) = callee {
            callee = inner;
        }
        let has_spread = arguments
            .iter()
            .any(|a| matches!(a, CallArgument::Spread(_)));

        if matches!(callee, Expression::Super) {
            let super_name = self.add_name("\u{0}class_super", chunk)?;
            chunk.emit(Instruction::LoadName(super_name));
            if has_spread {
                if matches!(
                    arguments,
                    [CallArgument::Spread(Expression::Identifier(name))]
                        if name == "\0default_ctor_args"
                ) {
                    self.compile_expression(
                        match &arguments[0] {
                            CallArgument::Spread(expression) => expression,
                            CallArgument::Expression(_) => unreachable!(),
                        },
                        chunk,
                        context,
                    )?;
                    chunk.emit(Instruction::SuperForwardCall);
                } else {
                    self.compile_argument_list_array(arguments, chunk, context)?;
                    chunk.emit(Instruction::SuperSpreadCall(0));
                }
            } else {
                for argument in arguments {
                    let CallArgument::Expression(expression) = argument else {
                        unreachable!()
                    };
                    self.compile_expression(expression, chunk, context)?;
                }
                let count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                    is_syntax: false,
                    message: "super() argument count exceeds the u16 bytecode range".into(),
                })?;
                chunk.emit(Instruction::SuperCall(count));
            }
            return Ok(());
        }

        if let Expression::Member {
            object,
            property,
            computed: false,
        } = callee
            && matches!(object.as_ref(), Expression::Super)
        {
            let method_name = match property.as_ref() {
                Expression::Identifier(name) => name.clone(),
                Expression::PrivateName(name) => format!("\x00#{name}"),
                _ => {
                    return Err(CompileError::unsupported(format!(
                        "non-identifier super method property {property:?}"
                    )));
                }
            };
            let method_index = self.add_name(&method_name, chunk)?;
            chunk.emit(Instruction::GetSuperMethod(method_index));

            if has_spread {
                self.compile_argument_list_array(arguments, chunk, context)?;
                chunk.emit(Instruction::SpreadCallWithThis(0));
            } else {
                let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                    is_syntax: false,
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

        if let Expression::Member {
            object,
            property,
            computed: true,
        } = callee
            && matches!(object.as_ref(), Expression::Super)
        {
            self.compile_expression(property, chunk, context)?;
            chunk.emit(Instruction::GetSuperElementMethod);

            if has_spread {
                self.compile_argument_list_array(arguments, chunk, context)?;
                chunk.emit(Instruction::SpreadCallWithThis(0));
            } else {
                let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                    is_syntax: false,
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

        // Parentheses do not erase the Reference produced by an optional
        // member chain: `(object?.method)()` must still call with `object` as
        // `this`. Compile the final member as a method reference and keep the
        // same explicit-receiver call layout used by ordinary member calls.
        if let Expression::OptionalChain { base, steps } = callee
            && matches!(steps.last(), Some(OptionalChainStep::Member { .. }))
        {
            self.compile_optional_chain(base, steps, true, chunk, context)?;
            if has_spread {
                self.compile_argument_list_array(arguments, chunk, context)?;
                chunk.emit(Instruction::SpreadCallWithThis(0));
            } else {
                let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                    is_syntax: false,
                    message: "call argument count exceeds the u16 bytecode range".into(),
                })?;
                for arg in arguments {
                    let CallArgument::Expression(expression) = arg else {
                        unreachable!()
                    };
                    self.compile_expression(expression, chunk, context)?;
                }
                chunk.emit(Instruction::CallWithThis(argument_count));
            }
            return Ok(());
        }

        // Static member calls preserve their receiver as `this`.
        if let Expression::Member {
            object,
            property,
            computed,
        } = callee
        {
            self.compile_expression(object, chunk, context)?;
            if *computed {
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::GetElementMethod);
            } else {
                let method_name = match property.as_ref() {
                    Expression::Identifier(name) => name.clone(),
                    Expression::PrivateName(name) => format!("\x00#{name}"),
                    _ => {
                        return Err(CompileError::unsupported(format!(
                            "non-identifier method property {property:?}"
                        )));
                    }
                };
                let method_index = self.add_name(&method_name, chunk)?;
                chunk.emit(Instruction::GetMethod(method_index));
            }

            if has_spread {
                self.compile_argument_list_array(arguments, chunk, context)?;
                chunk.emit(Instruction::SpreadCallWithThis(0));
            } else {
                let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                    is_syntax: false,
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

        // Computed member calls also preserve their receiver as `this`.
        if let Expression::Member {
            object,
            property,
            computed: true,
        } = callee
        {
            self.compile_expression(object, chunk, context)?;
            self.compile_expression(property, chunk, context)?;
            chunk.emit(Instruction::GetElementMethod);

            if has_spread {
                self.compile_argument_list_array(arguments, chunk, context)?;
                chunk.emit(Instruction::SpreadCallWithThis(0));
            } else {
                let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                    is_syntax: false,
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

        let is_direct_eval = matches!(callee, Expression::Identifier(name) if name == "eval");
        self.compile_expression(callee, chunk, context)?;
        if has_spread {
            self.compile_argument_list_array(arguments, chunk, context)?;
            chunk.emit(Instruction::SpreadCall(0));
        } else {
            let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                is_syntax: false,
                message: "call argument count exceeds the u16 bytecode range".into(),
            })?;
            for arg in arguments {
                let CallArgument::Expression(e) = arg else {
                    unreachable!()
                };
                self.compile_expression(e, chunk, context)?;
            }
            chunk.emit(if is_direct_eval {
                Instruction::DirectEval(argument_count)
            } else {
                Instruction::Call(argument_count)
            });
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
            self.compile_argument_list_array(arguments, chunk, context)?;
            chunk.emit(Instruction::SpreadConstruct(0));
        } else {
            let argument_count = u16::try_from(arguments.len()).map_err(|_| CompileError {
                is_syntax: false,
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

    /// Builds a single iterable argument list while preserving left-to-right
    /// evaluation for ordinary and spread arguments.
    fn compile_argument_list_array(
        &mut self,
        arguments: &[crate::ast::CallArgument],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::CallArgument;
        chunk.emit(Instruction::ArrayCreateSparse(0));
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) => {
                    self.compile_expression(expression, chunk, context)?;
                    chunk.emit(Instruction::ArrayPush);
                }
                CallArgument::Spread(expression) => {
                    self.compile_expression(expression, chunk, context)?;
                    chunk.emit(Instruction::SpreadIntoArray);
                }
            }
        }
        Ok(())
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
                is_syntax: false,
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
            is_syntax: false,
            message: "sparse array literal length exceeds the u32 bytecode range".into(),
        })?;
        chunk.emit(Instruction::ArrayCreateSparse(length));
        for (index, element) in elements.iter().enumerate() {
            let ArrayElement::Expression(expression) = element else {
                continue;
            };
            self.compile_expression(expression, chunk, context)?;
            let index = u32::try_from(index).map_err(|_| CompileError {
                is_syntax: false,
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
        // CoverInitializedName (`{a = expr}`) is only valid as a destructuring target.
        // If we reach the compiler with one, it's always a value context → SyntaxError.
        for prop in properties {
            if let ObjectProperty::Data {
                key: PropertyName::Identifier(key_name),
                value: Expression::Assignment { target, .. },
            } = prop
                && matches!(target.as_ref(), Expression::Identifier(t) if t == key_name)
            {
                return Err(CompileError::syntax(format!(
                    "invalid use of `{{{key_name} = value}}` in object literal: only valid in destructuring assignment"
                )));
            }
        }
        if properties
            .iter()
            .all(|property| matches!(property, ObjectProperty::Data { value, .. } if !matches!(value, Expression::Function(_))))
        {
            let count = u16::try_from(properties.len()).map_err(|_| CompileError {
                is_syntax: false,
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
                    let key_name = property_key(key);
                    let key = self.add_name(&key_name, chunk)?;
                    if is_anonymous_function_definition(value) {
                        chunk.emit(Instruction::SetFunctionName(key));
                    }
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
                    if let PropertyName::Computed(expr) = key {
                        chunk.emit(Instruction::Duplicate);
                        self.compile_expression(expr, chunk, context)?;
                        self.compile_accessor_function(None, &[], body, chunk, context)?;
                        chunk.emit(Instruction::DefineComputedGetter);
                    } else {
                        let accessor_name = accessor_function_name("get", &property_key(key));
                        self.compile_accessor_function(
                            Some(accessor_name),
                            &[],
                            body,
                            chunk,
                            context,
                        )?;
                        let key = self.add_name(&property_key(key), chunk)?;
                        chunk.emit(Instruction::DefineGetter(key));
                    }
                }
                ObjectProperty::Setter {
                    key,
                    parameter,
                    body,
                } => {
                    if let PropertyName::Computed(expr) = key {
                        chunk.emit(Instruction::Duplicate);
                        self.compile_expression(expr, chunk, context)?;
                        self.compile_accessor_function(
                            None,
                            std::slice::from_ref(parameter),
                            body,
                            chunk,
                            context,
                        )?;
                        chunk.emit(Instruction::DefineComputedSetter);
                    } else {
                        let accessor_name = accessor_function_name("set", &property_key(key));
                        self.compile_accessor_function(
                            Some(accessor_name),
                            std::slice::from_ref(parameter),
                            body,
                            chunk,
                            context,
                        )?;
                        let key = self.add_name(&property_key(key), chunk)?;
                        chunk.emit(Instruction::DefineSetter(key));
                    }
                }
                ObjectProperty::PrototypeSetter { value } => {
                    self.compile_expression(value, chunk, context)?;
                    chunk.emit(Instruction::SetObjectPrototype);
                }
                ObjectProperty::Spread(expr) => {
                    self.compile_expression(expr, chunk, context)?;
                    chunk.emit(Instruction::SpreadObject);
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
                if matches!(object.as_ref(), Expression::Super) {
                    chunk.emit(Instruction::ThrowReferenceError);
                    return Ok(());
                }
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
                if matches!(object.as_ref(), Expression::Super) {
                    chunk.emit(Instruction::ThrowReferenceError);
                    return Ok(());
                }
                self.compile_expression(object, chunk, context)?;
                self.compile_expression(property, chunk, context)?;
                chunk.emit(Instruction::DeleteElement);
                Ok(())
            }
            // `delete identifier` — strict mode is rejected at parse time.
            // In sloppy mode, unresolvable references return `true`, declared
            // bindings return `false`, and configurable global properties can
            // be deleted.
            Expression::Identifier(name) => {
                let name_index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::DeleteName(name_index));
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
        name: Option<String>,
        params: &[crate::ast::FunctionParam],
        body: &FunctionBody,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let compiled = self.compile_function_body(params, body, context)?;
        let template = FunctionTemplate {
            name,
            params: compiled.params,
            rest_param: compiled.rest_param,
            length_override: Some(compiled.length),
            chunk: compiled.chunk.into_shared(),
            is_strict: compiled.is_strict,
            is_async: false,
            is_generator: false,
            is_arrow: false,
            binds_name_in_activation: false,
            is_derived_constructor: false,
            is_constructable: false,
            has_own_prototype_property: false,
            prototype_writable: false,
            uses_arguments: compiled.uses_arguments,
            local_layout: compiled.local_layout,
            upvalue_layout: compiled.upvalue_layout,
            dynamic_scope: compiled.dynamic_scope,
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
        // Each substitution uses ToString directly. Lowering to string `+` would
        // use ToPrimitive(default), which is observably different for objects
        // such as Temporal values whose valueOf method intentionally throws.
        let first_idx = chunk
            .add_constant(Constant::String(tl.quasis[0].clone()))
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::Constant(first_idx));
        for (expr, quasi) in tl.expressions.iter().zip(tl.quasis[1..].iter()) {
            self.compile_expression(expr, chunk, context)?;
            chunk.emit(Instruction::ToString);
            chunk.emit(Instruction::Add); // string + expr → string (coerces expr)
            let q_idx = chunk
                .add_constant(Constant::String(quasi.clone()))
                .map_err(CompileError::from_chunk)?;
            chunk.emit(Instruction::Constant(q_idx));
            chunk.emit(Instruction::Add); // prev_string + quasi → string
        }
        Ok(())
    }

    fn compile_tagged_template(
        &mut self,
        tag: &Expression,
        template: &crate::ast::TemplateLiteral,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        let raw_array = Expression::Array(
            template
                .raw_quasis
                .iter()
                .cloned()
                .map(|text| {
                    crate::ast::ArrayElement::Expression(Expression::Literal(Literal::String(text)))
                })
                .collect(),
        );
        // A template object is array-like: its cooked strings are indexed
        // properties and `raw` contains the corresponding source strings.
        // The object model currently cannot freeze the pair here, but emitting
        // the indexed cooked values avoids silently presenting every template
        // character as `undefined` to the tag.
        let mut template_properties = template
            .quasis
            .iter()
            .enumerate()
            .map(|(index, text)| ObjectProperty::Data {
                key: PropertyName::String(index.to_string()),
                value: Expression::Literal(Literal::String(text.clone())),
            })
            .collect::<Vec<_>>();
        template_properties.push(ObjectProperty::Data {
            key: PropertyName::Identifier("raw".into()),
            value: raw_array,
        });
        let template_object = Expression::Object(template_properties);
        let mut arguments = Vec::with_capacity(template.expressions.len() + 1);
        arguments.push(crate::ast::CallArgument::Expression(template_object));
        arguments.extend(
            template
                .expressions
                .iter()
                .cloned()
                .map(crate::ast::CallArgument::Expression),
        );
        self.compile_call(tag, &arguments, chunk, context)
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
        // Class declarations are lexical (like `let`), so use InitializeBinding
        // which pops the value from the stack without pushing it back.
        // StoreGlobal peeks-and-pops, so still needs a Pop after.
        let name_idx = self.add_name(&decl.name, chunk)?;
        if context.inside_function() || context.is_lexical(&decl.name) {
            // InitializeBinding: pop value, store — no push back. No Pop needed.
            chunk.emit(Instruction::InitializeBinding(name_idx));
        } else {
            // StoreGlobal: pop, store, push back. Needs Pop.
            chunk.emit(Instruction::StoreGlobal(name_idx));
            chunk.emit(Instruction::Pop);
        }
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
    /// Returns the property key used to store a class member named by `prop_name`.
    /// Private names get a NUL-prefix so they are hidden from `hasOwnProperty("#name")`
    /// checks while remaining accessible through private-access expressions that use the
    /// same prefix.
    fn class_member_storage_key(prop_name: &crate::ast::PropertyName) -> String {
        match prop_name {
            crate::ast::PropertyName::PrivateName(n) => format!("\x00#{n}"),
            other => other.to_key_string(),
        }
    }

    fn insert_instance_field_initializers_after_super(
        statements: Vec<Statement>,
        initializers: Vec<Statement>,
    ) -> Vec<Statement> {
        let mut output = Vec::with_capacity(statements.len() + initializers.len());
        let mut pending_initializers = Some(initializers);
        for statement in statements {
            let is_super_statement = matches!(
                &statement,
                Statement::Expression(Expression::Call { callee, .. })
                    if matches!(callee.as_ref(), Expression::Super)
            );
            output.push(statement);
            if is_super_statement && let Some(mut initializers) = pending_initializers.take() {
                output.append(&mut initializers);
            }
        }
        if let Some(mut initializers) = pending_initializers {
            output.append(&mut initializers);
        }
        output
    }

    fn compile_class_body(
        &mut self,
        name: Option<&str>,
        super_class: Option<&Expression>,
        elements: &[crate::ast::ClassElement],
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::ClassElement;

        // A private name belongs to a particular evaluation of a class, not
        // to its spelling. Keep a class lexical environment alive while all
        // constructor/method closures are created and allocate fresh brands
        // every time evaluation reaches this class expression.
        let mut private_names = Vec::new();
        for element in elements {
            let name = match element {
                ClassElement::Field {
                    name: crate::ast::PropertyName::PrivateName(name),
                    ..
                } => Some(name),
                ClassElement::Method {
                    name: crate::ast::PropertyName::PrivateName(name),
                    is_getter: false,
                    is_setter: false,
                    ..
                } => Some(name),
                _ => None,
            };
            if let Some(name) = name
                && !private_names.contains(name)
            {
                private_names.push(name.clone());
            }
        }
        let private_brand_env = !private_names.is_empty();
        if private_brand_env {
            chunk.emit(Instruction::CreateLexicalEnvironment);
            context.environment_depth += 1;
            for name in &private_names {
                let index = self.add_name(name, chunk)?;
                chunk.emit(Instruction::CreatePrivateBrand(index));
            }
        }

        // Find the constructor element, if any.
        let ctor_literal = elements.iter().find_map(|e| {
            if let ClassElement::Constructor(lit) = e {
                Some(lit)
            } else {
                None
            }
        });

        // Collect public instance fields for field initializer injection.
        // Computed fields use synthetic bindings evaluated at class-definition time.
        struct InstanceField {
            /// The property name to define on `this` (None = computed via binding).
            static_name: Option<String>,
            /// Name of the synthetic binding holding the computed key (for computed fields).
            computed_binding: Option<String>,
            initializer: Option<Expression>,
        }

        let mut computed_field_env = false;
        let mut computed_field_bindings = vec![None; elements.len()];
        let mut instance_field_specs: Vec<InstanceField> = Vec::new();

        // Evaluate every computed field key exactly once in source order,
        // regardless of whether the field is static or instance-owned.
        for (field_idx, element) in elements.iter().enumerate() {
            let ClassElement::Field {
                name: crate::ast::PropertyName::Computed(key_expr),
                ..
            } = element
            else {
                continue;
            };
            if !computed_field_env {
                chunk.emit(Instruction::CreateLexicalEnvironment);
                computed_field_env = true;
            }
            let binding_name = format!("__cfield_key_{field_idx}__");
            let binding_idx = self.add_name(&binding_name, chunk)?;
            chunk.emit(Instruction::CreateMutableBinding(binding_idx));
            self.compile_expression(key_expr, chunk, context)?;
            chunk.emit(Instruction::ToPropertyKey);
            chunk.emit(Instruction::InitializeBinding(binding_idx));
            computed_field_bindings[field_idx] = Some(binding_name);
        }

        // Collect instance initialization records after key evaluation.
        for (field_idx, element) in elements.iter().enumerate() {
            if let ClassElement::Method {
                name: crate::ast::PropertyName::PrivateName(name),
                is_static: false,
                is_getter: false,
                is_setter: false,
                ..
            } = element
            {
                instance_field_specs.push(InstanceField {
                    static_name: Some(format!("\0#init#{name}")),
                    computed_binding: None,
                    initializer: Some(Expression::Member {
                        object: Box::new(Expression::This),
                        property: Box::new(Expression::Identifier(format!(
                            "\0methodsource#{name}"
                        ))),
                        computed: false,
                    }),
                });
            } else if let ClassElement::Field {
                name: prop_name,
                is_static: false,
                initializer,
            } = element
            {
                if matches!(prop_name, crate::ast::PropertyName::Computed(_)) {
                    // Open a synthetic lexical scope once (so the constructor can capture it).
                    // Convert once at class evaluation time, before any instance
                    // initializer runs, as required by ClassFieldDefinitionEvaluation.
                    instance_field_specs.push(InstanceField {
                        static_name: None,
                        computed_binding: computed_field_bindings[field_idx].clone(),
                        initializer: initializer.as_ref().map(|b| *b.clone()),
                    });
                } else {
                    // Use the NUL-prefixed storage key for private fields so that
                    // `hasOwnProperty("#name")` returns false while the field is still
                    // accessible via `this.#name` (which also uses the NUL prefix).
                    let key = match prop_name {
                        crate::ast::PropertyName::PrivateName(name) => {
                            format!("\0#init#{name}")
                        }
                        _ => format!("\0#fieldinit#{}", Self::class_member_storage_key(prop_name)),
                    };
                    instance_field_specs.push(InstanceField {
                        static_name: Some(key),
                        computed_binding: None,
                        initializer: initializer.as_ref().map(|b| *b.clone()),
                    });
                }
            }
        }

        // Synthesize field-init statements prepended to constructor body.
        let field_init_stmts: Vec<Statement> = instance_field_specs
            .into_iter()
            .map(|spec| {
                let value = spec
                    .initializer
                    .unwrap_or(Expression::Literal(Literal::Undefined));
                if let Some(static_name) = spec.static_name {
                    // Non-computed: `this.fieldName = value`
                    Statement::Expression(Expression::Assignment {
                        target: Box::new(Expression::Member {
                            object: Box::new(Expression::This),
                            property: Box::new(Expression::Identifier(static_name)),
                            computed: false,
                        }),
                        value: Box::new(value),
                    })
                } else {
                    // Computed: `this[__cfield_key_N__] = value`
                    let binding = spec.computed_binding.unwrap();
                    Statement::Expression(Expression::Assignment {
                        target: Box::new(Expression::Member {
                            object: Box::new(Expression::This),
                            property: Box::new(Expression::Identifier(binding)),
                            computed: true,
                        }),
                        value: Box::new(value),
                    })
                }
            })
            .collect();

        // Evaluate and capture the superclass before creating the constructor so
        // the constructor closure can resolve `super()` through this lexical
        // binding. The value is reused below for both constructor and prototype
        // inheritance setup.
        let super_binding = if let Some(super_expr) = super_class {
            chunk.emit(Instruction::CreateLexicalEnvironment);
            context.environment_depth += 1;
            let super_name = "\u{0}class_super";
            let super_idx = self.add_name(super_name, chunk)?;
            chunk.emit(Instruction::CreateMutableBinding(super_idx));
            self.compile_expression(super_expr, chunk, context)?;
            chunk.emit(Instruction::InitializeBinding(super_idx));
            Some(super_idx)
        } else {
            None
        };

        // Emit constructor function.
        let ctor_body = if let Some(lit) = ctor_literal {
            if field_init_stmts.is_empty() {
                lit.clone()
            } else if super_class.is_some() {
                let mut body = lit.clone();
                body.body.statements = Self::insert_instance_field_initializers_after_super(
                    body.body.statements,
                    field_init_stmts,
                );
                body
            } else {
                let mut body = lit.clone();
                let mut new_stmts = field_init_stmts;
                new_stmts.append(&mut body.body.statements);
                body.body.statements = new_stmts;
                body
            }
        } else {
            // Synthesize a default constructor with field initializations.
            // Class bodies are always strict per spec.
            let (params, statements) = if super_class.is_some() {
                let mut statements = vec![Statement::Expression(Expression::Call {
                    callee: Box::new(Expression::Super),
                    arguments: vec![crate::ast::CallArgument::Spread(Expression::Identifier(
                        "\0default_ctor_args".into(),
                    ))],
                })];
                statements.extend(field_init_stmts);
                (
                    vec![crate::ast::FunctionParam::Rest(
                        "\0default_ctor_args".into(),
                    )],
                    statements,
                )
            } else {
                (vec![], field_init_stmts)
            };
            FunctionLiteral {
                name: name.map(String::from),
                params,
                body: FunctionBody {
                    statements,
                    is_strict: true,
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
            length_override: Some(ctor_fn.length),
            chunk: ctor_fn.chunk.into_shared(),
            is_strict: true, // class bodies are always strict
            is_async: false,
            is_generator: false,
            is_arrow: false,
            binds_name_in_activation: false,
            is_derived_constructor: super_class.is_some(),
            is_constructable: true,
            has_own_prototype_property: true,
            prototype_writable: false,
            uses_arguments: ctor_fn.uses_arguments,
            local_layout: ctor_fn.local_layout,
            upvalue_layout: ctor_fn.upvalue_layout,
            dynamic_scope: ctor_fn.dynamic_scope,
            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
        };
        let ctor_idx = chunk
            .add_function(ctor_template)
            .map_err(CompileError::from_chunk)?;
        chunk.emit(Instruction::CreateFunction(ctor_idx)); // [ctor]
        chunk.emit(Instruction::Duplicate); // [ctor, ctor_copy]

        if let Some(super_idx) = super_binding {
            chunk.emit(Instruction::LoadName(super_idx)); // [ctor, ctor_copy, super]
            let set_constructor_parent = chunk.emit(Instruction::JumpIfNotNullish(usize::MAX));
            chunk.emit(Instruction::Pop); // null heritage keeps %Function.prototype%
            let constructor_parent_done = chunk.emit(Instruction::Jump(usize::MAX));
            let set_constructor_parent_target = chunk.current_offset();
            chunk.emit(Instruction::SetObjectPrototype); // [ctor, ctor_copy]
            let constructor_parent_done_target = chunk.current_offset();
            chunk
                .patch_jump(set_constructor_parent, set_constructor_parent_target)
                .map_err(CompileError::from_chunk)?;
            chunk
                .patch_jump(constructor_parent_done, constructor_parent_done_target)
                .map_err(CompileError::from_chunk)?;
        }

        // Static methods — defined on the constructor itself.
        for element in elements {
            if let ClassElement::Method {
                name: prop_name,
                function,
                is_static: true,
                is_getter,
                is_setter,
            } = element
            {
                let base_name = prop_name.to_key_string();
                let fn_name = if *is_getter {
                    accessor_function_name("get", &base_name)
                } else if *is_setter {
                    accessor_function_name("set", &base_name)
                } else {
                    base_name
                };
                let storage_key = match prop_name {
                    crate::ast::PropertyName::PrivateName(name) if !*is_getter && !*is_setter => {
                        format!("\0methodsource#{name}")
                    }
                    _ => Self::class_member_storage_key(prop_name),
                };
                let fn_compiled =
                    self.compile_function_body(&function.params, &function.body, context)?;
                let fn_template = FunctionTemplate {
                    name: if matches!(prop_name, crate::ast::PropertyName::Computed(_)) {
                        None
                    } else {
                        Some(fn_name.clone())
                    },
                    params: fn_compiled.params,
                    rest_param: fn_compiled.rest_param,
                    length_override: Some(fn_compiled.length),
                    chunk: fn_compiled.chunk.into_shared(),
                    is_strict: true, // class methods are always strict
                    is_async: function.is_async,
                    is_generator: function.is_generator,
                    is_arrow: false,
                    binds_name_in_activation: false,
                    is_derived_constructor: false,
                    is_constructable: false,
                    has_own_prototype_property: false,
                    prototype_writable: false,
                    uses_arguments: fn_compiled.uses_arguments,
                    local_layout: fn_compiled.local_layout,
                    upvalue_layout: fn_compiled.upvalue_layout,
                    dynamic_scope: fn_compiled.dynamic_scope,
                    environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
                };
                let fn_idx = chunk
                    .add_function(fn_template)
                    .map_err(CompileError::from_chunk)?;
                if let crate::ast::PropertyName::Computed(key_expr) = prop_name {
                    // Computed key: evaluate expression, then create fn, then define.
                    // Stack before: [ctor, ctor_copy]
                    self.compile_expression(key_expr, chunk, context)?; // [ctor, ctor_copy, key]
                    chunk.emit(Instruction::CreateFunction(fn_idx)); // [ctor, ctor_copy, key, fn]
                    if *is_getter {
                        chunk.emit(Instruction::DefineClassGetterComputed);
                    } else if *is_setter {
                        chunk.emit(Instruction::DefineClassSetterComputed);
                    } else {
                        chunk.emit(Instruction::DefineClassMethodComputed);
                    }
                } else {
                    chunk.emit(Instruction::CreateFunction(fn_idx));
                    let key = self.add_name(&storage_key, chunk)?;
                    if *is_getter {
                        chunk.emit(Instruction::DefineClassGetter(key));
                    } else if *is_setter {
                        chunk.emit(Instruction::DefineClassSetter(key));
                    } else {
                        chunk.emit(Instruction::DefineClassMethod(key));
                    }
                    if let crate::ast::PropertyName::PrivateName(name) = prop_name
                        && !*is_getter
                        && !*is_setter
                    {
                        let source = self.add_name(&storage_key, chunk)?;
                        let init = self.add_name(&format!("\0#init#{name}"), chunk)?;
                        chunk.emit(Instruction::Duplicate);
                        chunk.emit(Instruction::Duplicate);
                        chunk.emit(Instruction::GetProperty(source));
                        chunk.emit(Instruction::SetProperty(init));
                        chunk.emit(Instruction::Pop);
                    }
                }
                // [ctor, ctor_copy]
            }
        }

        // Duplicate ctor_copy for use as the `prototype.constructor` value.
        chunk.emit(Instruction::Duplicate); // [ctor, ctor_copy, ctor_for_constructor]

        // Create the prototype object.
        chunk.emit(Instruction::ObjectCreateEmpty); // [ctor, ctor_copy, ctor_for_constructor, proto]

        // Set up prototype inheritance if there is a super class.
        if let Some(super_idx) = super_binding {
            chunk.emit(Instruction::LoadName(super_idx)); // [.., proto, super]
            chunk.emit(Instruction::GetClassHeritagePrototype); // [.., proto, super.prototype]
            chunk.emit(Instruction::SetObjectPrototype); // [.., proto]
        }

        // Instance methods — defined on the prototype.
        for element in elements {
            if let ClassElement::Method {
                name: prop_name,
                function,
                is_static: false,
                is_getter,
                is_setter,
            } = element
            {
                let base_name = prop_name.to_key_string();
                let fn_name = if *is_getter {
                    accessor_function_name("get", &base_name)
                } else if *is_setter {
                    accessor_function_name("set", &base_name)
                } else {
                    base_name
                };
                let storage_key = match prop_name {
                    crate::ast::PropertyName::PrivateName(name) if !*is_getter && !*is_setter => {
                        format!("\0methodsource#{name}")
                    }
                    _ => Self::class_member_storage_key(prop_name),
                };
                let fn_compiled =
                    self.compile_function_body(&function.params, &function.body, context)?;
                let fn_template = FunctionTemplate {
                    name: if matches!(prop_name, crate::ast::PropertyName::Computed(_)) {
                        None
                    } else {
                        Some(fn_name.clone())
                    },
                    params: fn_compiled.params,
                    rest_param: fn_compiled.rest_param,
                    length_override: Some(fn_compiled.length),
                    chunk: fn_compiled.chunk.into_shared(),
                    is_strict: true, // class methods are always strict
                    is_async: function.is_async,
                    is_generator: function.is_generator,
                    is_arrow: false,
                    binds_name_in_activation: false,
                    is_derived_constructor: false,
                    is_constructable: false,
                    has_own_prototype_property: false,
                    prototype_writable: false,
                    uses_arguments: fn_compiled.uses_arguments,
                    local_layout: fn_compiled.local_layout,
                    upvalue_layout: fn_compiled.upvalue_layout,
                    dynamic_scope: fn_compiled.dynamic_scope,
                    environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
                };
                let fn_idx = chunk
                    .add_function(fn_template)
                    .map_err(CompileError::from_chunk)?;
                if let crate::ast::PropertyName::Computed(key_expr) = prop_name {
                    // Computed key: evaluate before creating function.
                    self.compile_expression(key_expr, chunk, context)?; // [.., proto, key]
                    chunk.emit(Instruction::CreateFunction(fn_idx)); // [.., proto, key, fn]
                    if *is_getter {
                        chunk.emit(Instruction::DefineClassGetterComputed);
                    } else if *is_setter {
                        chunk.emit(Instruction::DefineClassSetterComputed);
                    } else {
                        chunk.emit(Instruction::DefineClassMethodComputed);
                    }
                } else {
                    chunk.emit(Instruction::CreateFunction(fn_idx));
                    let key = self.add_name(&storage_key, chunk)?;
                    if *is_getter {
                        chunk.emit(Instruction::DefineClassGetter(key));
                    } else if *is_setter {
                        chunk.emit(Instruction::DefineClassSetter(key));
                    } else {
                        chunk.emit(Instruction::DefineClassMethod(key));
                    }
                }
                // [.., proto]
            }
        }

        // Set proto.constructor = ctor (non-enumerable, per spec).
        // Stack: [ctor, ctor_copy, ctor_for_constructor, proto]
        // Swap so ctor_for_constructor is on top: [ctor, ctor_copy, proto, ctor_for_constructor]
        chunk.emit(Instruction::Swap);
        let ctor_key = self.add_name("constructor", chunk)?;
        chunk.emit(Instruction::DefineClassMethod(ctor_key)); // [ctor, ctor_copy, proto]

        // Attach prototype to constructor: ctor.prototype = proto (non-enumerable, non-configurable).
        // Stack: [ctor, ctor_copy, proto]
        // DefineClassPrototype peeks ctor_copy (below proto), pops proto as value.
        chunk.emit(Instruction::DefineClassPrototype); // [ctor, ctor_copy]
        chunk.emit(Instruction::Pop); // [ctor]

        if super_binding.is_some() {
            chunk.emit(Instruction::PopEnvironment);
            context.environment_depth -= 1;
        }

        // Static fields — initialized on the constructor after the class is created.
        // Stack: [ctor]
        for (element_idx, element) in elements.iter().enumerate() {
            match element {
                ClassElement::Field {
                    name: prop_name,
                    is_static: true,
                    initializer,
                } => {
                    let init_val = initializer
                        .as_ref()
                        .map_or(Expression::Literal(Literal::Undefined), |b| *b.clone());
                    // Stack: [ctor]
                    if matches!(prop_name, crate::ast::PropertyName::Computed(_)) {
                        chunk.emit(Instruction::Duplicate); // [ctor, ctor]
                        let binding_name = computed_field_bindings[element_idx]
                            .as_ref()
                            .expect("computed field key binding");
                        let binding_idx = self.add_name(binding_name, chunk)?;
                        chunk.emit(Instruction::LoadName(binding_idx)); // [ctor, ctor, key]
                        self.compile_expression(&init_val, chunk, context)?; // [ctor, ctor, key, val]
                        chunk.emit(Instruction::DefineDataPropertyComputed); // [ctor, ctor]
                        chunk.emit(Instruction::Pop); // [ctor]
                    } else {
                        let init_body = FunctionBody {
                            statements: vec![Statement::Return(Some(init_val))],
                            is_strict: true,
                        };
                        let init_fn = self.compile_function_body(&[], &init_body, context)?;
                        let init_template = FunctionTemplate {
                            name: None,
                            params: init_fn.params,
                            rest_param: init_fn.rest_param,
                            length_override: Some(0),
                            chunk: init_fn.chunk.into_shared(),
                            is_strict: true,
                            is_async: false,
                            is_generator: false,
                            is_arrow: false,
                            binds_name_in_activation: false,
                            is_derived_constructor: false,
                            is_constructable: false,
                            has_own_prototype_property: false,
                            prototype_writable: false,
                            uses_arguments: init_fn.uses_arguments,
                            local_layout: init_fn.local_layout,
                            upvalue_layout: init_fn.upvalue_layout,
                            dynamic_scope: init_fn.dynamic_scope,
                            environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
                        };
                        let init_idx = chunk
                            .add_function(init_template)
                            .map_err(CompileError::from_chunk)?;
                        chunk.emit(Instruction::Duplicate); // [ctor, ctor]
                        chunk.emit(Instruction::Duplicate); // [ctor, ctor, ctor]
                        chunk.emit(Instruction::CreateFunction(init_idx)); // [ctor, ctor, ctor, fn]
                        chunk.emit(Instruction::SetFunctionHomeObject);
                        chunk.emit(Instruction::Swap); // [ctor, fn, ctor]
                        chunk.emit(Instruction::CallWithThis(0)); // [ctor, value]
                        if initializer
                            .as_deref()
                            .is_some_and(is_anonymous_function_definition)
                        {
                            let inferred_name = match prop_name {
                                crate::ast::PropertyName::PrivateName(name) => format!("#{name}"),
                                _ => prop_name.to_key_string(),
                            };
                            let name_idx = self.add_name(&inferred_name, chunk)?;
                            chunk.emit(Instruction::SetFunctionName(name_idx));
                        }
                        let (key, private) = match prop_name {
                            crate::ast::PropertyName::PrivateName(name) => {
                                (format!("\0#init#{name}"), true)
                            }
                            _ => (Self::class_member_storage_key(prop_name), false),
                        };
                        let key_idx = self.add_name(&key, chunk)?;
                        chunk.emit(if private {
                            Instruction::SetProperty(key_idx)
                        } else {
                            Instruction::DefineDataProperty(key_idx)
                        }); // [ctor, ctor]
                        chunk.emit(Instruction::Pop); // [ctor]
                    }
                }
                ClassElement::StaticBlock(statements) => {
                    let block_body = FunctionBody {
                        statements: statements.clone(),
                        is_strict: true,
                    };
                    let block_fn = self.compile_function_body(&[], &block_body, context)?;
                    let block_template = FunctionTemplate {
                        name: None,
                        params: block_fn.params,
                        rest_param: block_fn.rest_param,
                        length_override: Some(block_fn.length),
                        chunk: block_fn.chunk.into_shared(),
                        is_strict: true,
                        is_generator: false,
                        is_async: false,
                        is_arrow: false,
                        binds_name_in_activation: false,
                        is_derived_constructor: false,
                        is_constructable: false,
                        has_own_prototype_property: false,
                        prototype_writable: true,
                        uses_arguments: block_fn.uses_arguments,
                        local_layout: block_fn.local_layout,
                        upvalue_layout: block_fn.upvalue_layout,
                        dynamic_scope: block_fn.dynamic_scope,
                        environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
                    };
                    let block_idx = chunk
                        .add_function(block_template)
                        .map_err(CompileError::from_chunk)?;
                    // ponytail: use a zero-arg function call so existing call-frame
                    // machinery supplies the class constructor as `this`.
                    chunk.emit(Instruction::Duplicate); // [ctor, ctor]
                    chunk.emit(Instruction::CreateFunction(block_idx)); // [ctor, ctor, fn]
                    chunk.emit(Instruction::SetFunctionHomeObject);
                    chunk.emit(Instruction::Swap); // [ctor, fn, ctor]
                    chunk.emit(Instruction::CallWithThis(0)); // [ctor, result]
                    chunk.emit(Instruction::Pop); // [ctor]
                }
                _ => {}
            }
        }

        if computed_field_env {
            chunk.emit(Instruction::PopEnvironment);
        }

        if private_brand_env {
            chunk.emit(Instruction::PopEnvironment);
            context.environment_depth -= 1;
        }

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
        if kind == VariableKind::Var {
            self.compile_binding_pattern_store(pattern, chunk, context)
        } else {
            self.compile_binding_pattern(kind, pattern, chunk, context)
        }
    }

    /// Like `compile_binding_pattern` but uses `StoreName` (write to existing binding)
    /// instead of `InitializeBinding`. Used for for-in/for-of loop variables where
    /// the bindings were already initialized before the loop started.
    fn compile_binding_pattern_store(
        &mut self,
        pattern: &crate::ast::BindingPattern,
        chunk: &mut Chunk,
        context: &mut CompileContext,
    ) -> Result<(), CompileError> {
        use crate::ast::BindingPattern;
        match pattern {
            BindingPattern::Identifier(name) => {
                let idx = self.add_name(name, chunk)?;
                chunk.emit(Instruction::StoreName(idx));
                chunk.emit(Instruction::Pop);
                Ok(())
            }
            BindingPattern::Array { elements, rest } => {
                // Stack: [rhs] — convert to iterator so any iterable works.
                chunk.emit(Instruction::GetIterator); // [iterator]
                for maybe_elem in elements.iter() {
                    chunk.emit(Instruction::Duplicate); // [iterator, iterator]
                    chunk.emit(Instruction::IteratorNext); // [iterator, value, done]
                    chunk.emit(Instruction::Pop); // [iterator, value] — discard done flag
                    if let Some(elem) = maybe_elem {
                        if let Some(default_expr) = &elem.default {
                            let infer = if let BindingPattern::Identifier(n) = &elem.pattern {
                                Some(n.as_str())
                            } else {
                                None
                            };
                            self.emit_binding_default_with_name(
                                default_expr,
                                infer,
                                chunk,
                                context,
                            )?;
                        }
                        self.compile_binding_pattern_store(&elem.pattern, chunk, context)?;
                    } else {
                        chunk.emit(Instruction::Pop); // elided slot — advance iterator, discard value
                    }
                    // Stack: [iterator]
                }
                if let Some(rest_pat) = rest {
                    // Drain remaining elements into an array, consuming the iterator.
                    chunk.emit(Instruction::IterableToArray); // [rest_array]
                    self.compile_binding_pattern_store(rest_pat, chunk, context)?;
                    // Stack: []
                } else {
                    // Close the iterator (calls return() if not exhausted).
                    chunk.emit(Instruction::IteratorClose); // []
                }
                Ok(())
            }
            BindingPattern::Object { props, rest } => {
                let has_rest = rest.is_some();
                let needs_rest_env = has_rest
                    && props
                        .iter()
                        .any(|prop| matches!(prop.key, crate::ast::ObjectBindingKey::Computed(_)));
                if needs_rest_env {
                    chunk.emit(Instruction::CreateLexicalEnvironment);
                    context.environment_depth += 1;
                    context
                        .lexical_scopes
                        .push(std::collections::HashSet::new());
                }
                chunk.emit(Instruction::RequireObjectCoercible);
                let mut rest_excluded = Vec::new();
                for prop in props {
                    chunk.emit(Instruction::Duplicate);
                    match &prop.key {
                        crate::ast::ObjectBindingKey::Static(key) => {
                            let key_str = key.to_key_string();
                            rest_excluded.push(ObjectRestExcludedKey::Static(key_str.clone()));
                            let key_idx = self.add_name(&key_str, chunk)?;
                            chunk.emit(Instruction::GetProperty(key_idx));
                        }
                        crate::ast::ObjectBindingKey::Computed(key_expr) => {
                            self.compile_expression(key_expr, chunk, context)?;
                            if needs_rest_env {
                                let temp_name = format!("\u{0}dstr_obj_key{}", rest_excluded.len());
                                let temp_idx = self.add_name(&temp_name, chunk)?;
                                chunk.emit(Instruction::CreateMutableBinding(temp_idx));
                                if let Some(scope) = context.lexical_scopes.last_mut() {
                                    scope.insert(temp_name.clone());
                                }
                                chunk.emit(Instruction::Duplicate);
                                chunk.emit(Instruction::InitializeBinding(temp_idx));
                                rest_excluded.push(ObjectRestExcludedKey::Temp(temp_name));
                            }
                            chunk.emit(Instruction::GetElement);
                        }
                    }
                    if let Some(default_expr) = &prop.default {
                        let infer = if let BindingPattern::Identifier(n) = &prop.value {
                            Some(n.as_str())
                        } else {
                            None
                        };
                        self.emit_binding_default_with_name(default_expr, infer, chunk, context)?;
                    }
                    self.compile_binding_pattern_store(&prop.value, chunk, context)?;
                }
                if let Some(rest_pat) = rest {
                    chunk.emit(Instruction::Duplicate);
                    let excluded_count =
                        self.emit_object_rest_excluded_keys(&rest_excluded, chunk)?;
                    chunk.emit(Instruction::CopyDataPropertiesExcluded(excluded_count));
                    self.compile_binding_pattern_store(rest_pat, chunk, context)?;
                }
                chunk.emit(Instruction::Pop);
                if needs_rest_env {
                    context.lexical_scopes.pop();
                    chunk.emit(Instruction::PopEnvironment);
                    context.environment_depth -= 1;
                }
                Ok(())
            }
        }
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
                // Stack: [rhs] — convert to iterator so any iterable works.
                chunk.emit(Instruction::GetIterator); // [iterator]
                for maybe_elem in elements.iter() {
                    chunk.emit(Instruction::Duplicate); // [iterator, iterator]
                    chunk.emit(Instruction::IteratorNext); // [iterator, value, done]
                    chunk.emit(Instruction::Pop); // [iterator, value] — discard done flag
                    if let Some(elem) = maybe_elem {
                        if let Some(default_expr) = &elem.default {
                            let infer =
                                if let crate::ast::BindingPattern::Identifier(n) = &elem.pattern {
                                    Some(n.as_str())
                                } else {
                                    None
                                };
                            self.emit_binding_default_with_name(
                                default_expr,
                                infer,
                                chunk,
                                context,
                            )?;
                        }
                        self.compile_binding_pattern(kind, &elem.pattern, chunk, context)?;
                    } else {
                        chunk.emit(Instruction::Pop); // elided slot — advance iterator, discard value
                    }
                    // Stack: [iterator]
                }
                if let Some(rest_pat) = rest {
                    // Drain remaining elements into an array, consuming the iterator.
                    chunk.emit(Instruction::IterableToArray); // [rest_array]
                    self.compile_binding_pattern(kind, rest_pat, chunk, context)?;
                    // Stack: []
                } else {
                    // Close the iterator (calls return() if not exhausted).
                    chunk.emit(Instruction::IteratorClose); // []
                }
                Ok(())
            }
            BindingPattern::Object { props, rest } => {
                // Stack: [rhs]
                let has_rest = rest.is_some();
                let needs_rest_env = has_rest
                    && props
                        .iter()
                        .any(|prop| matches!(prop.key, crate::ast::ObjectBindingKey::Computed(_)));
                if needs_rest_env {
                    chunk.emit(Instruction::CreateLexicalEnvironment);
                    context.environment_depth += 1;
                    context
                        .lexical_scopes
                        .push(std::collections::HashSet::new());
                }
                chunk.emit(Instruction::RequireObjectCoercible);
                let mut rest_excluded = Vec::new();
                for prop in props {
                    chunk.emit(Instruction::Duplicate); // [rhs, rhs]
                    match &prop.key {
                        crate::ast::ObjectBindingKey::Static(key) => {
                            let key_str = key.to_key_string();
                            rest_excluded.push(ObjectRestExcludedKey::Static(key_str.clone()));
                            let key_idx = self.add_name(&key_str, chunk)?;
                            chunk.emit(Instruction::GetProperty(key_idx)); // [rhs, rhs.key]
                        }
                        crate::ast::ObjectBindingKey::Computed(key_expr) => {
                            // [rhs, rhs]
                            self.compile_expression(key_expr, chunk, context)?; // [rhs, rhs, computed_key]
                            if needs_rest_env {
                                let temp_name = format!("\u{0}dstr_obj_key{}", rest_excluded.len());
                                let temp_idx = self.add_name(&temp_name, chunk)?;
                                chunk.emit(Instruction::CreateMutableBinding(temp_idx));
                                if let Some(scope) = context.lexical_scopes.last_mut() {
                                    scope.insert(temp_name.clone());
                                }
                                chunk.emit(Instruction::Duplicate);
                                chunk.emit(Instruction::InitializeBinding(temp_idx));
                                rest_excluded.push(ObjectRestExcludedKey::Temp(temp_name));
                            }
                            chunk.emit(Instruction::GetElement); // [rhs, rhs[computed_key]]
                        }
                    }
                    if let Some(default_expr) = &prop.default {
                        // Pass identifier name for function name inference in default values.
                        let infer = if let crate::ast::BindingPattern::Identifier(n) = &prop.value {
                            Some(n.as_str())
                        } else {
                            None
                        };
                        self.emit_binding_default_with_name(default_expr, infer, chunk, context)?;
                    }
                    self.compile_binding_pattern(kind, &prop.value, chunk, context)?;
                    // [rhs]
                }
                // Object rest: shallow copy of rhs (simplified — doesn't exclude consumed keys)
                if let Some(rest_pat) = rest {
                    chunk.emit(Instruction::Duplicate); // [rhs, rhs]
                    let excluded_count =
                        self.emit_object_rest_excluded_keys(&rest_excluded, chunk)?;
                    chunk.emit(Instruction::CopyDataPropertiesExcluded(excluded_count));
                    self.compile_binding_pattern(kind, rest_pat, chunk, context)?;
                    // [rhs]
                }
                chunk.emit(Instruction::Pop); // []
                if needs_rest_env {
                    context.lexical_scopes.pop();
                    chunk.emit(Instruction::PopEnvironment);
                    context.environment_depth -= 1;
                }
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
        self.emit_binding_default_with_name(default_expr, None, chunk, context)
    }

    fn emit_binding_default_with_name(
        &mut self,
        default_expr: &Expression,
        infer_name: Option<&str>,
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
        // Spec: infer function name when default is an anonymous function definition.
        if let Some(name) = infer_name
            && is_anonymous_function_definition(default_expr)
        {
            let name_idx = self.add_name(name, chunk)?;
            chunk.emit(Instruction::SetFunctionName(name_idx));
        }
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
            length_override: Some(fn_chunk.length),
            chunk: fn_chunk.chunk.into_shared(),
            is_strict: fn_chunk.is_strict,
            is_async: literal.is_async,
            is_generator: literal.is_generator,
            is_arrow: literal.is_arrow,
            binds_name_in_activation: literal.name.is_some(),
            is_derived_constructor: false,
            is_constructable: !literal.is_arrow && !literal.is_async && !literal.is_generator,
            has_own_prototype_property: !literal.is_arrow
                && (!literal.is_async || literal.is_generator),
            prototype_writable: true,
            uses_arguments: fn_chunk.uses_arguments,
            local_layout: fn_chunk.local_layout,
            upvalue_layout: fn_chunk.upvalue_layout,
            dynamic_scope: fn_chunk.dynamic_scope,
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
        use crate::ast::VariableKind;

        const ITER: &str = "\u{0}forof_iter";

        let is_const = matches!(
            left,
            crate::ast::ForBinding::Declaration {
                kind: VariableKind::Const,
                ..
            }
        );

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
        // `const` bindings are NOT created here — each iteration gets its own fresh
        // lexical scope with new immutable bindings (ECMAScript §13.7.5.13.b).
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
                            // Const bindings are created per-iteration; nothing to do here.
                        }
                        VariableKind::Let => {
                            // Let bindings live in the loop's lexical environment and
                            // are reassigned on each iteration.
                            chunk.emit(Instruction::CreateMutableBinding(idx));
                            chunk.emit(Instruction::Constant(undefined_idx));
                            chunk.emit(Instruction::InitializeBinding(idx));
                        }
                        VariableKind::Var => {
                            // Var bindings were hoisted by declaration instantiation.
                            // Creating another binding here breaks duplicate var
                            // patterns and incorrectly hides the outer var binding.
                        }
                    }
                    if *kind != VariableKind::Var {
                        scope.insert(name.clone());
                    }
                }
            }
            crate::ast::ForBinding::Target(_) => {}
        }

        context.lexical_scopes.push(scope);

        // Evaluate iterable and obtain the iterator.
        self.compile_expression(right, chunk, context)?;
        chunk.emit(if is_await {
            Instruction::GetAsyncIterator
        } else {
            Instruction::GetIterator
        });
        chunk.emit(Instruction::InitializeBinding(iter_idx));

        // Loop header.
        let loop_start = chunk.current_offset();
        chunk.emit(Instruction::LoadName(iter_idx));
        chunk.emit(if is_await {
            Instruction::AsyncIteratorNext
        } else {
            Instruction::IteratorNext
        }); // [value, is_done]

        // if is_done (top), jump to exit.
        let exit_jump = chunk.emit(Instruction::JumpIfTrue(usize::MAX));
        chunk.emit(Instruction::Pop); // pop is_done=false

        // Capture the outer scope depth before any per-iteration scope.  Loop/break
        // contexts are anchored here so that `continue` and `break` unwind the
        // per-iteration scope (if any) before jumping to their targets.
        let outer_env_depth = context.environment_depth;
        let iteration_protected_start = chunk.current_offset();

        // Assign the iteration value to the loop variable.
        // For let/var, the binding was pre-initialized before the loop; use StoreName
        // (write to existing binding) so every iteration re-assigns without "already
        // initialized" errors.  For const, push a fresh per-iteration lexical scope,
        // create immutable bindings, and initialize them from the iterator value.
        match left {
            crate::ast::ForBinding::Declaration { kind, pattern } => {
                match kind {
                    VariableKind::Let | VariableKind::Var => {
                        // Pre-initialized above; just store.
                        self.compile_binding_pattern_store(pattern, chunk, context)?;
                    }
                    VariableKind::Const => {
                        // ECMAScript §13.7.5.13.b: each iteration creates a new
                        // lexical environment with fresh immutable bindings.
                        chunk.emit(Instruction::CreateLexicalEnvironment);
                        context.environment_depth += 1;
                        let names = binding_pattern_names(pattern);
                        for name in &names {
                            let idx = self.add_name(name, chunk)?;
                            chunk.emit(Instruction::CreateImmutableBinding(idx));
                        }
                        // Initialize from the value on the stack.
                        self.compile_binding_pattern(*kind, pattern, chunk, context)?;
                    }
                }
            }
            crate::ast::ForBinding::Target(target) => {
                self.assign_dstr_element_to_target(target, chunk, context)?;
            }
        }

        let loop_labels = std::mem::take(&mut context.pending_loop_labels);
        context.loops.push(LoopContext {
            labels: loop_labels.clone(),
            iterator_binding: Some(iter_idx),
            continue_target: Some(loop_start),
            continue_jumps: Vec::new(),
            environment_depth: outer_env_depth,
        });
        context.breakables.push(BreakContext {
            break_jumps: Vec::new(),
            environment_depth: outer_env_depth,
            label: None,
        });

        if let Err(e) = self.compile_statement(body, chunk, context, false) {
            context.loops.pop();
            context.breakables.pop();
            context.pending_loop_labels = loop_labels;
            return Err(e);
        }

        // Abrupt completion while assigning the iteration value or executing
        // the body must close the iterator. The VM's finally handlers preserve
        // throw/return completions, while direct break/continue jumps retain
        // their existing dedicated control-flow paths.
        if !is_const {
            let iteration_protected_end = chunk.current_offset();
            let normal_iteration_exit = chunk.emit(Instruction::Jump(usize::MAX));
            let iterator_close_target = chunk.current_offset();
            chunk.emit(Instruction::LoadName(iter_idx));
            chunk.emit(Instruction::IteratorClose);
            chunk.emit(Instruction::EndFinally);
            let after_iterator_close = chunk.current_offset();
            chunk
                .patch_jump(normal_iteration_exit, after_iterator_close)
                .map_err(CompileError::from_chunk)?;
            if iteration_protected_start < iteration_protected_end {
                chunk.handlers.push(ExceptionHandler {
                    start: iteration_protected_start,
                    end: iteration_protected_end,
                    target: iterator_close_target,
                    kind: HandlerKind::Finally,
                    stack_depth: u32::from(context.has_completion_slot),
                    environment_depth: outer_env_depth,
                });
            }
        }

        // For const, pop the per-iteration scope before jumping back to the loop
        // header.  (break/continue already unwind this scope via their depth diff.)
        if is_const {
            context.environment_depth -= 1;
            chunk.emit(Instruction::PopEnvironment);
        }
        chunk.emit(Instruction::Jump(loop_start));

        let exit_target = chunk.current_offset();
        chunk
            .patch_jump(exit_jump, exit_target)
            .map_err(CompileError::from_chunk)?;
        // stack at exit: [value=undefined, is_done=true]; pop both.
        chunk.emit(Instruction::Pop); // pop is_done=true
        chunk.emit(Instruction::Pop); // pop value=undefined
        let natural_exit_jump = chunk.emit(Instruction::Jump(usize::MAX));

        // A `break` arrives with an empty operand stack, so it must not share
        // the natural-exhaustion cleanup above. Close the still-live iterator
        // before both paths rejoin at the lexical-scope cleanup.
        let break_target = chunk.current_offset();
        chunk.emit(Instruction::LoadName(iter_idx));
        chunk.emit(Instruction::IteratorClose);

        let loop_end = chunk.current_offset();
        chunk
            .patch_jump(natural_exit_jump, loop_end)
            .map_err(CompileError::from_chunk)?;

        let break_jumps = context
            .breakables
            .last()
            .expect("for-of break context")
            .break_jumps
            .clone();
        for jump in break_jumps {
            chunk
                .patch_jump(jump, break_target)
                .map_err(CompileError::from_chunk)?;
        }
        context.loops.pop();
        context.breakables.pop();

        context.lexical_scopes.pop();
        context.environment_depth -= 1;
        chunk.emit(Instruction::PopEnvironment);
        context.pending_loop_labels = loop_labels;
        Ok(())
    }
}

fn parse_bigint_literal(raw: &str) -> Result<crate::runtime::BigIntValue, CompileError> {
    crate::runtime::bigint::parse_bigint_literal(raw)
        .map_err(|_| CompileError::unsupported(format_args!("invalid BigInt literal `{raw}`")))
}

fn is_iteration_statement(statement: &Statement) -> bool {
    match statement {
        Statement::While { .. }
        | Statement::DoWhile { .. }
        | Statement::For { .. }
        | Statement::ForIn { .. }
        | Statement::ForOf { .. } => true,
        Statement::Labelled { body, .. } => is_iteration_statement(body),
        _ => false,
    }
}

/// Intermediate result returned from `compile_function_body`.
struct CompiledFunction {
    /// Function.length — params before the first default/rest/pattern-with-default.
    length: u32,
    params: Vec<String>,
    rest_param: Option<String>,
    chunk: Chunk,
    is_strict: bool,
    uses_arguments: bool,
    local_layout: Arc<LocalLayout>,
    upvalue_layout: Arc<UpvalueLayout>,
    dynamic_scope: DynamicScopePolicy,
}

fn deoptimize_template_upvalues(template: &mut FunctionTemplate) -> Result<(), CompileError> {
    let layout = template.upvalue_layout.clone();
    let chunk = Arc::make_mut(&mut template.chunk);
    for offset in 0..chunk.instructions.len() {
        let (slot, store) = match chunk.instructions[offset] {
            Instruction::LoadUpvalue(slot) => (slot, false),
            Instruction::StoreUpvalue(slot) => (slot, true),
            _ => continue,
        };
        let name = layout
            .bindings
            .get(usize::from(slot.0))
            .ok_or_else(|| CompileError::unsupported("invalid nested upvalue slot"))?
            .name
            .clone();
        let name_index = chunk
            .add_constant(Constant::String(name))
            .map_err(CompileError::from_chunk)?;
        chunk.instructions[offset] = if store {
            Instruction::StoreName(name_index)
        } else {
            Instruction::LoadName(name_index)
        };
    }
    for child in &mut chunk.functions {
        deoptimize_template_upvalues(child)?;
    }
    template.upvalue_layout = Arc::new(UpvalueLayout::default());
    Ok(())
}

fn function_needs_arguments_object(chunk: &Chunk) -> bool {
    fn referenced_name(chunk: &Chunk, index: u16) -> Option<&str> {
        match chunk.constants.get(index as usize) {
            Some(Constant::String(name)) => Some(name.as_str()),
            _ => None,
        }
    }

    let direct_reference = chunk.instructions.iter().any(|instruction| {
        let name = match instruction {
            Instruction::LoadName(index)
            | Instruction::StoreName(index)
            | Instruction::TypeOfName(index)
            | Instruction::DeclareLocal(index)
            | Instruction::CreateMutableBinding(index)
            | Instruction::CreateImmutableBinding(index)
            | Instruction::InitializeBinding(index) => referenced_name(chunk, *index),
            _ => None,
        };
        matches!(name, Some("arguments" | "eval"))
    });

    direct_reference
        || chunk
            .functions
            .iter()
            .any(|template| template.is_arrow && template.uses_arguments)
}

fn property_key(key: &PropertyName) -> String {
    key.to_key_string()
}

fn accessor_function_name(prefix: &str, key: &str) -> String {
    format!("{prefix} {key}")
}

fn compound_assignment_instruction(operator: AssignmentOperator) -> Option<Instruction> {
    match operator {
        AssignmentOperator::Add => Some(Instruction::Add),
        AssignmentOperator::Subtract => Some(Instruction::Subtract),
        AssignmentOperator::Multiply => Some(Instruction::Multiply),
        AssignmentOperator::Divide => Some(Instruction::Divide),
        AssignmentOperator::Remainder => Some(Instruction::Remainder),
        AssignmentOperator::Exponentiation => Some(Instruction::Exponentiation),
        AssignmentOperator::BitwiseAnd => Some(Instruction::BitwiseAnd),
        AssignmentOperator::BitwiseOr => Some(Instruction::BitwiseOr),
        AssignmentOperator::BitwiseXor => Some(Instruction::BitwiseXor),
        AssignmentOperator::LeftShift => Some(Instruction::LeftShift),
        AssignmentOperator::RightShift => Some(Instruction::RightShift),
        AssignmentOperator::UnsignedRightShift => Some(Instruction::UnsignedRightShift),
        // Logical compound assignments have short-circuit semantics; handled separately.
        AssignmentOperator::LogicalAnd
        | AssignmentOperator::LogicalOr
        | AssignmentOperator::NullishCoalescing => None,
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
                .flat_map(|declaration| {
                    if let Some(pattern) = &declaration.pattern {
                        binding_pattern_names(pattern)
                    } else {
                        vec![declaration.name.clone()]
                    }
                })
                .collect(),
            Statement::DestructuringDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                pattern,
                ..
            } => binding_pattern_names(pattern),
            Statement::ClassDeclaration(decl) => vec![decl.name.clone()],
            Statement::ModuleDeclaration(ModuleDeclaration::Import(decl)) => decl
                .entries
                .iter()
                .map(|entry| entry.local_name.clone())
                .collect(),
            Statement::ModuleDeclaration(ModuleDeclaration::Export(decl)) => decl
                .declaration
                .as_deref()
                .map(|statement| lexical_names(std::slice::from_ref(statement)))
                .unwrap_or_default(),
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
                && declarations.iter().any(|declaration| {
                    if let Some(pattern) = &declaration.pattern {
                        binding_pattern_names(pattern).contains(&name.to_string())
                    } else {
                        declaration.name == name
                    }
                }) =>
        {
            Some(*kind)
        }
        Statement::DestructuringDeclaration { kind, pattern, .. }
            if matches!(kind, VariableKind::Let | VariableKind::Const)
                && binding_pattern_names(pattern).contains(&name.to_string()) =>
        {
            Some(*kind)
        }
        Statement::ClassDeclaration(decl) if decl.name == name => Some(VariableKind::Let),
        Statement::ModuleDeclaration(ModuleDeclaration::Import(decl))
            if decl.entries.iter().any(|entry| entry.local_name == name) =>
        {
            Some(VariableKind::Const)
        }
        Statement::ModuleDeclaration(ModuleDeclaration::Export(decl)) => decl
            .declaration
            .as_deref()
            .and_then(|statement| lexical_kind(std::slice::from_ref(statement), name)),
        _ => None,
    })
}

/// Returns `true` if `expr` is an "anonymous function definition" per ECMAScript:
/// a function/arrow/generator/async expression or class expression WITHOUT an explicit name.
/// Used for name-inference: `let f = expr` → `f.name = "f"`.
fn is_anonymous_function_definition(expr: &Expression) -> bool {
    match expr {
        Expression::Function(lit) => lit.name.is_none(),
        Expression::Class(cls) => cls.name.is_none(),
        // Parentheses are transparent for IsAnonymousFunctionDefinition. Keep
        // other cover expressions (notably comma expressions) opaque so that
        // `(0, function () {})` does not receive an inferred name.
        Expression::Parenthesized(inner) => is_anonymous_function_definition(inner),
        _ => false,
    }
}

/// Collect names of function declarations that are Annex B B.3.3.1 candidates:
/// function declarations appearing directly inside block statements, IfStatement
/// bodies (bare fn decl, no braces), or SwitchStatement case bodies.
/// Recurses into control-flow constructs but NOT into nested function bodies.
fn collect_annex_b_fn_candidates(statements: &[Statement]) -> Vec<String> {
    let mut names = Vec::new();
    // Initialize with the function body's own top-level lexical names (let/const/class).
    // A block-level fn decl whose name conflicts with one of these is skipped (var F would
    // produce an early error against the outer let/const F).
    let enclosing: std::collections::HashSet<String> =
        lexical_names(statements).into_iter().collect();
    for stmt in statements {
        annex_b_collect_stmt(stmt, &enclosing, &mut names);
    }
    names
}

/// `enclosing_block_lexicals` = union of let/const/class names from all enclosing blocks
/// that are INSIDE the current function body (not the function body itself).
/// A block-level fn decl is a valid Annex B candidate only if its name is NOT already in
/// this set (otherwise `var F` would conflict with an enclosing lexical binding → early error →
/// Annex B skipped).
fn annex_b_collect_stmt(
    stmt: &Statement,
    enclosing_block_lexicals: &std::collections::HashSet<String>,
    names: &mut Vec<String>,
) {
    match stmt {
        Statement::Block(stmts) => {
            // Lexical names introduced by THIS block.
            let block_own: std::collections::HashSet<String> =
                lexical_names(stmts).into_iter().collect();
            // Combined = parent enclosing + this block's own lexicals.
            let mut combined = enclosing_block_lexicals.clone();
            combined.extend(block_own);

            // Function declarations DIRECTLY in this block: candidate only if name is
            // NOT in the ENCLOSING blocks' lexicals (this block's own names don't matter
            // for the early-error check on var F in an outer scope).
            for inner in stmts {
                if let Statement::FunctionDeclaration { name, .. } = inner
                    && !enclosing_block_lexicals.contains(name)
                    && !names.contains(name)
                {
                    names.push(name.clone());
                }
            }
            // Recurse with the combined set (now this block's names are "enclosing" for nested).
            for inner in stmts {
                if !matches!(inner, Statement::FunctionDeclaration { .. }) {
                    annex_b_collect_stmt(inner, &combined, names);
                }
            }
        }
        Statement::If {
            consequent,
            alternate,
            ..
        } => {
            // Bare fn decl as if/else body (no `{ }`) = Annex B candidate if not conflicting.
            if let Statement::FunctionDeclaration { name, .. } = consequent.as_ref() {
                if !enclosing_block_lexicals.contains(name) && !names.contains(name) {
                    names.push(name.clone());
                }
            } else {
                annex_b_collect_stmt(consequent, enclosing_block_lexicals, names);
            }
            if let Some(alt) = alternate {
                if let Statement::FunctionDeclaration { name, .. } = alt.as_ref() {
                    if !enclosing_block_lexicals.contains(name) && !names.contains(name) {
                        names.push(name.clone());
                    }
                } else {
                    annex_b_collect_stmt(alt, enclosing_block_lexicals, names);
                }
            }
        }
        Statement::While { body, .. }
        | Statement::DoWhile { body, .. }
        | Statement::Labelled { body, .. }
        | Statement::With { body, .. } => {
            annex_b_collect_stmt(body, enclosing_block_lexicals, names);
        }
        // `for (let x; ...) body` — the loop's let/const init creates a lexical scope
        // that is enclosing for the body. Must include those names in the conflict check.
        Statement::For { init, body, .. } => {
            let mut inner_enclosing = enclosing_block_lexicals.clone();
            if let Some(init_stmt) = init
                && let Statement::VariableDeclaration {
                    kind: VariableKind::Let | VariableKind::Const,
                    declarations,
                } = init_stmt.as_ref()
            {
                for decl in declarations {
                    if let Some(pat) = &decl.pattern {
                        for n in binding_pattern_names(pat) {
                            inner_enclosing.insert(n);
                        }
                    } else {
                        inner_enclosing.insert(decl.name.clone());
                    }
                }
            }
            annex_b_collect_stmt(body, &inner_enclosing, names);
        }
        Statement::ForIn { left, body, .. } | Statement::ForOf { left, body, .. } => {
            let mut inner_enclosing = enclosing_block_lexicals.clone();
            if let crate::ast::ForBinding::Declaration {
                kind: VariableKind::Let | VariableKind::Const,
                pattern,
            } = left
            {
                for n in binding_pattern_names(pattern) {
                    inner_enclosing.insert(n);
                }
            }
            annex_b_collect_stmt(body, &inner_enclosing, names);
        }
        Statement::Try {
            block,
            handler,
            finalizer,
        } => {
            for s in block {
                annex_b_collect_stmt(s, enclosing_block_lexicals, names);
            }
            if let Some(catch_clause) = handler {
                for s in &catch_clause.body {
                    annex_b_collect_stmt(s, enclosing_block_lexicals, names);
                }
            }
            if let Some(fin) = finalizer {
                for s in fin {
                    annex_b_collect_stmt(s, enclosing_block_lexicals, names);
                }
            }
        }
        Statement::Switch { cases, .. } => {
            // Switch cases share a single lexical scope. Collect all let/const names from
            // ALL case bodies — these act as enclosing lexicals for nested blocks inside.
            let mut switch_body_lexicals = enclosing_block_lexicals.clone();
            for case in cases {
                for s in &case.consequent {
                    if let Statement::VariableDeclaration {
                        kind: VariableKind::Let | VariableKind::Const,
                        declarations,
                    } = s
                    {
                        for decl in declarations {
                            if let Some(pat) = &decl.pattern {
                                for n in binding_pattern_names(pat) {
                                    switch_body_lexicals.insert(n);
                                }
                            } else {
                                switch_body_lexicals.insert(decl.name.clone());
                            }
                        }
                    }
                }
            }
            for case in cases {
                // Function declarations DIRECTLY in switch case bodies = Annex B candidates
                // (if not conflicting with enclosing scope lexicals, NOT switch-body lexicals).
                for s in &case.consequent {
                    if let Statement::FunctionDeclaration { name, .. } = s
                        && !enclosing_block_lexicals.contains(name)
                        && !names.contains(name)
                    {
                        names.push(name.clone());
                    }
                }
                // Recurse with the combined switch-body lexicals as the new enclosing.
                for s in &case.consequent {
                    if !matches!(s, Statement::FunctionDeclaration { .. }) {
                        annex_b_collect_stmt(s, &switch_body_lexicals, names);
                    }
                }
            }
        }
        // FunctionDeclaration at top level of function body: hoisted normally, not Annex B.
        // All other statements: no Annex B candidates inside.
        _ => {}
    }
}

// ── Shared statement-sub-statement walker ────────────────────────────────────
//
// Both `collect_var_names` and `collect_annex_b_fn_candidates` need to recurse
// into nested statements (but not into nested function bodies). This helper
// centralizes the "which statements have sub-statements" knowledge so the two
// traversals only contain their collection logic.

/// Calls `f` for each direct sub-statement of `stmt`, without descending into
/// nested function bodies. Returns `false` when the stmt itself is a
/// function-like boundary (declaration / expression).
fn for_each_sub_statement(stmt: &Statement, f: &mut impl FnMut(&Statement)) -> bool {
    // Don't descend into function bodies — they have their own var scope.
    match stmt {
        Statement::FunctionDeclaration { .. }
        | Statement::Expression(Expression::Function(_))
        | Statement::Expression(Expression::Class(_))
        | Statement::ClassDeclaration { .. }
        | Statement::ModuleDeclaration(_) => return false,
        _ => {}
    }
    match stmt {
        Statement::Block(stmts) => {
            for s in stmts {
                f(s);
            }
        }
        Statement::If {
            consequent,
            alternate,
            ..
        } => {
            f(consequent);
            if let Some(alt) = alternate {
                f(alt);
            }
        }
        Statement::While { body, .. }
        | Statement::DoWhile { body, .. }
        | Statement::Labelled { body, .. }
        | Statement::With { body, .. } => f(body),
        Statement::For { init, body, .. } => {
            if let Some(s) = init {
                f(s);
            }
            f(body);
        }
        Statement::ForIn { body, .. } | Statement::ForOf { body, .. } => f(body),
        Statement::Try {
            block,
            handler,
            finalizer,
        } => {
            for s in block {
                f(s);
            }
            if let Some(h) = handler {
                for s in &h.body {
                    f(s);
                }
            }
            if let Some(fin) = finalizer {
                for s in fin {
                    f(s);
                }
            }
        }
        Statement::Switch { cases, .. } => {
            for case in cases {
                for s in &case.consequent {
                    f(s);
                }
            }
        }
        _ => {}
    }
    true
}

/// Collects let/const/class names declared at the top level of `statements`.
pub(crate) fn collect_top_level_lexical_names(
    statements: &[Statement],
) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for stmt in statements {
        match stmt {
            Statement::VariableDeclaration { kind, declarations } => {
                if matches!(
                    kind,
                    crate::ast::VariableKind::Let | crate::ast::VariableKind::Const
                ) {
                    for d in declarations {
                        if d.pattern.is_none() {
                            names.insert(d.name.clone());
                        }
                    }
                }
            }
            Statement::ClassDeclaration(cls) => {
                names.insert(cls.name.clone());
            }
            _ => {}
        }
    }
    names
}

/// Collects block-contained function declaration names eligible for Annex B
/// §B.3.3.3 pre-hoisting. A name is eligible when replacing the function
/// declaration with a `var` declaration would NOT produce an Early Error —
/// i.e. the name is not already bound as a lexical name (let/const/class) in
/// any enclosing block within the eval body.
pub(crate) fn collect_annex_b_var_names(
    statements: &[Statement],
    enclosing_lex_names: &std::collections::HashSet<String>,
    already_hoisted: &std::collections::HashSet<String>,
    result: &mut Vec<String>,
) {
    // Collect lexical names declared at this level (in blocks, not top-level
    // which is already in enclosing_lex_names).
    let mut block_level_lex_names = enclosing_lex_names.clone();

    for stmt in statements {
        match stmt {
            Statement::Block(body) => {
                // Collect lexical names in this block.
                let mut inner_lex_names = block_level_lex_names.clone();
                for s in body {
                    match s {
                        Statement::VariableDeclaration { kind, declarations } => {
                            if matches!(
                                kind,
                                crate::ast::VariableKind::Let | crate::ast::VariableKind::Const
                            ) {
                                for d in declarations {
                                    if d.pattern.is_none() {
                                        inner_lex_names.insert(d.name.clone());
                                    }
                                }
                            }
                        }
                        Statement::ClassDeclaration(cls) => {
                            inner_lex_names.insert(cls.name.clone());
                        }
                        _ => {}
                    }
                }
                // Recurse: function declarations inside this block are checked
                // against block_level_lex_names (the names visible OUTSIDE this
                // block). Nested blocks see inner_lex_names.
                for s in body {
                    match s {
                        Statement::FunctionDeclaration { name, .. } => {
                            // Annex B §B.3.3.3 step ii: would `var <name>` be an
                            // early error? If name is already lexically declared
                            // in an enclosing scope → yes → skip.
                            if !block_level_lex_names.contains(name) && !result.contains(name) {
                                result.push(name.clone());
                            }
                        }
                        Statement::Block(_) => {
                            collect_annex_b_var_names(
                                std::slice::from_ref(s),
                                &inner_lex_names,
                                already_hoisted,
                                result,
                            );
                        }
                        _ => {
                            // Check sub-statements for nested blocks with fn decls.
                            collect_annex_b_var_names_in_sub(
                                s,
                                &inner_lex_names,
                                already_hoisted,
                                result,
                            );
                        }
                    }
                }
                // Update the block-level lex names for subsequent statements.
                block_level_lex_names = inner_lex_names;
            }
            Statement::FunctionDeclaration { .. } => {
                // Top-level fn decl is handled by compile_program's normal
                // hoisting; skip here.
            }
            Statement::VariableDeclaration { kind, declarations } => {
                if matches!(
                    kind,
                    crate::ast::VariableKind::Let | crate::ast::VariableKind::Const
                ) {
                    for d in declarations {
                        if d.pattern.is_none() {
                            block_level_lex_names.insert(d.name.clone());
                        }
                    }
                }
            }
            Statement::ClassDeclaration(cls) => {
                block_level_lex_names.insert(cls.name.clone());
            }
            _ => {
                collect_annex_b_var_names_in_sub(
                    stmt,
                    &block_level_lex_names,
                    already_hoisted,
                    result,
                );
            }
        }
    }
}

/// Recurses into sub-statements (if/while/for/try/switch bodies) to find
/// nested blocks with Annex B function declarations.
fn collect_annex_b_var_names_in_sub(
    stmt: &Statement,
    enclosing_lex_names: &std::collections::HashSet<String>,
    already_hoisted: &std::collections::HashSet<String>,
    result: &mut Vec<String>,
) {
    // Collect bare FunctionDeclaration names from nested scopes
    // (switch-case, if-else), then don't descend further.
    if let Statement::FunctionDeclaration { name, .. } = stmt {
        if !enclosing_lex_names.contains(name)
            && !already_hoisted.contains(name)
            && !result.contains(name)
        {
            result.push(name.clone());
        }
        return;
    }
    // Don't descend into function/class expressions or class declarations.
    if matches!(
        stmt,
        Statement::Expression(Expression::Function(_))
            | Statement::Expression(Expression::Class(_))
            | Statement::ClassDeclaration { .. }
            | Statement::ModuleDeclaration(_)
    ) {
        return;
    }
    match stmt {
        Statement::If {
            consequent,
            alternate,
            ..
        } => {
            collect_annex_b_var_names_in_if_body(
                consequent,
                enclosing_lex_names,
                already_hoisted,
                result,
            );
            if let Some(alt) = alternate {
                collect_annex_b_var_names_in_if_body(
                    alt,
                    enclosing_lex_names,
                    already_hoisted,
                    result,
                );
            }
        }
        Statement::While { body, .. }
        | Statement::DoWhile { body, .. }
        | Statement::With { body, .. } => {
            collect_annex_b_var_names_in_if_body(
                body,
                enclosing_lex_names,
                already_hoisted,
                result,
            );
        }
        Statement::For { body, init, .. } => {
            let mut body_lex = enclosing_lex_names.clone();
            if let Some(init_stmt) = init {
                add_lexical_names_from_var_decl(init_stmt, &mut body_lex);
            }
            collect_annex_b_var_names_in_if_body_with_lex(body, &body_lex, already_hoisted, result);
        }
        Statement::ForIn { left, body, .. } | Statement::ForOf { left, body, .. } => {
            let mut body_lex = enclosing_lex_names.clone();
            add_lexical_names_from_for_binding(left, &mut body_lex);
            collect_annex_b_var_names_in_if_body_with_lex(body, &body_lex, already_hoisted, result);
        }
        Statement::Try {
            block,
            handler,
            finalizer,
            ..
        } => {
            collect_annex_b_var_names(block, enclosing_lex_names, already_hoisted, result);
            if let Some(h) = handler {
                let mut handler_lex = enclosing_lex_names.clone();
                if let Some(crate::ast::CatchParameter::Identifier(name)) = &h.parameter {
                    handler_lex.insert(name.clone());
                }
                collect_annex_b_var_names(&h.body, &handler_lex, already_hoisted, result);
            }
            if let Some(f) = finalizer {
                collect_annex_b_var_names(f, enclosing_lex_names, already_hoisted, result);
            }
        }
        Statement::Switch { cases, .. } => {
            for case in cases {
                collect_annex_b_var_names(
                    &case.consequent,
                    enclosing_lex_names,
                    already_hoisted,
                    result,
                );
            }
        }
        Statement::Block(body) => {
            collect_annex_b_var_names(body, enclosing_lex_names, already_hoisted, result);
        }
        Statement::Labelled { body, .. } => {
            collect_annex_b_var_names_in_if_body(
                body,
                enclosing_lex_names,
                already_hoisted,
                result,
            );
        }
        _ => {}
    }
}

fn collect_annex_b_var_names_in_if_body(
    body: &Statement,
    enclosing_lex_names: &std::collections::HashSet<String>,
    already_hoisted: &std::collections::HashSet<String>,
    result: &mut Vec<String>,
) {
    match body {
        Statement::FunctionDeclaration { name, .. } => {
            if !enclosing_lex_names.contains(name)
                && !already_hoisted.contains(name)
                && !result.contains(name)
            {
                result.push(name.clone());
            }
        }
        Statement::Block(block_body) => {
            collect_annex_b_var_names(block_body, enclosing_lex_names, already_hoisted, result);
        }
        _ => {
            collect_annex_b_var_names_in_sub(body, enclosing_lex_names, already_hoisted, result);
        }
    }
}

/// Like `collect_annex_b_var_names_in_if_body` but uses a pre-computed
/// lexical name set that already includes loop-header declarations.
fn collect_annex_b_var_names_in_if_body_with_lex(
    body: &Statement,
    enclosing_lex_names: &std::collections::HashSet<String>,
    already_hoisted: &std::collections::HashSet<String>,
    result: &mut Vec<String>,
) {
    match body {
        Statement::FunctionDeclaration { name, .. } => {
            if !enclosing_lex_names.contains(name)
                && !already_hoisted.contains(name)
                && !result.contains(name)
            {
                result.push(name.clone());
            }
        }
        Statement::Block(block_body) => {
            collect_annex_b_var_names(block_body, enclosing_lex_names, already_hoisted, result);
        }
        _ => {
            collect_annex_b_var_names_in_sub(body, enclosing_lex_names, already_hoisted, result);
        }
    }
}

/// Extracts let/const names from a ForBinding (for-in / for-of left side).
fn add_lexical_names_from_for_binding(
    left: &crate::ast::ForBinding,
    names: &mut std::collections::HashSet<String>,
) {
    if let crate::ast::ForBinding::Declaration { kind, pattern } = left
        && matches!(
            kind,
            crate::ast::VariableKind::Let | crate::ast::VariableKind::Const
        )
        && let crate::ast::BindingPattern::Identifier(name) = pattern
    {
        names.insert(name.clone());
    }
}

/// Extracts let/const names from a VariableDeclaration statement used as a
/// for-loop init clause.
fn add_lexical_names_from_var_decl(
    stmt: &Statement,
    names: &mut std::collections::HashSet<String>,
) {
    if let Statement::VariableDeclaration { kind, declarations } = stmt
        && matches!(
            kind,
            crate::ast::VariableKind::Let | crate::ast::VariableKind::Const
        )
    {
        for d in declarations {
            if d.pattern.is_none() {
                names.insert(d.name.clone());
            }
        }
    }
}

/// Collect all `var`-declared names from a statement list, recursing into
/// nested blocks/if/loops/try, but NOT into nested function bodies.
/// Used to hoist `var` declarations before executing a program or function.
fn collect_var_names(statements: &[Statement], names: &mut Vec<String>) {
    for stmt in statements {
        collect_var_names_in(stmt, names);
    }
}

fn collect_var_names_in(stmt: &Statement, names: &mut Vec<String>) {
    // Collect var names from this statement itself.
    match stmt {
        Statement::VariableDeclaration {
            kind: crate::ast::VariableKind::Var,
            declarations,
        } => {
            for decl in declarations {
                if let Some(pat) = &decl.pattern {
                    for n in binding_pattern_names(pat) {
                        if !names.contains(&n) {
                            names.push(n);
                        }
                    }
                } else if !names.contains(&decl.name) {
                    names.push(decl.name.clone());
                }
            }
        }
        Statement::DestructuringDeclaration {
            kind: crate::ast::VariableKind::Var,
            pattern,
            ..
        } => {
            for n in binding_pattern_names(pattern) {
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        Statement::ForIn { left, .. } | Statement::ForOf { left, .. } => {
            if let crate::ast::ForBinding::Declaration {
                kind: crate::ast::VariableKind::Var,
                pattern,
            } = left
            {
                for n in binding_pattern_names(pattern) {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
        }
        Statement::ModuleDeclaration(ModuleDeclaration::Export(declaration)) => {
            if let Some(statement) = declaration.declaration.as_deref() {
                collect_var_names_in(statement, names);
            }
        }
        _ => {}
    }
    // Recurse into sub-statements via the shared walker.
    for_each_sub_statement(stmt, &mut |sub| collect_var_names_in(sub, names));
}

/// The operand-stack completion slot cannot cross an explicit `throw` or a
/// user-authored try handler because terminator validation and handler entry
/// use the chunk's zero-based stack contract. Those programs retain the older
/// top-level completion path until completion records become a shared VM type.
fn statements_support_stack_completion(statements: &[Statement]) -> bool {
    statements.iter().all(statement_supports_stack_completion)
}

fn statements_need_stack_completion(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::Block(_)
                | Statement::If { .. }
                | Statement::While { .. }
                | Statement::DoWhile { .. }
                | Statement::For { .. }
                | Statement::ForIn { .. }
                | Statement::ForOf { .. }
                | Statement::Labelled { .. }
                | Statement::Switch { .. }
                | Statement::With { .. }
        )
    })
}

fn statement_resets_completion(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::If { .. }
            | Statement::While { .. }
            | Statement::DoWhile { .. }
            | Statement::For { .. }
            | Statement::ForIn { .. }
            | Statement::ForOf { .. }
            | Statement::Labelled { .. }
            | Statement::Try { .. }
            | Statement::Switch { .. }
            | Statement::With { .. }
    )
}

fn statement_supports_stack_completion(statement: &Statement) -> bool {
    match statement {
        Statement::Throw(_) | Statement::Try { .. } => false,
        Statement::FunctionDeclaration { .. } | Statement::ClassDeclaration(_) => false,
        Statement::Block(statements) => statements_support_stack_completion(statements),
        Statement::If {
            consequent,
            alternate,
            ..
        } => {
            statement_supports_stack_completion(consequent)
                && alternate
                    .as_deref()
                    .is_none_or(statement_supports_stack_completion)
        }
        Statement::While { body, .. }
        | Statement::DoWhile { body, .. }
        | Statement::For { body, .. }
        | Statement::ForIn { body, .. }
        | Statement::ForOf { body, .. }
        | Statement::Labelled { body, .. }
        | Statement::With { body, .. } => statement_supports_stack_completion(body),
        Statement::Switch { cases, .. } => cases
            .iter()
            .all(|case| statements_support_stack_completion(&case.consequent)),
        Statement::ModuleDeclaration(ModuleDeclaration::Export(declaration)) => declaration
            .declaration
            .as_deref()
            .is_none_or(statement_supports_stack_completion),
        // Function/class bodies execute in their own chunks and therefore do
        // not constrain the surrounding script's completion representation.
        _ => true,
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
            is_syntax: false,
            message: format!("bytecode compiler does not support {node} yet"),
        }
    }

    fn from_chunk(error: ChunkError) -> Self {
        Self {
            is_syntax: false,
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bytecode::{Constant, EnvironmentCapturePolicy, Instruction},
        lexer::Lexer,
        parser::Parser,
    };

    use super::Compiler;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn compile(source: &str) -> crate::bytecode::SharedChunk {
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
    fn template_substitution_uses_string_hint_conversion() {
        let chunk = compile("`${value}`");
        assert!(chunk.instructions.contains(&Instruction::ToString));
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
    fn parenthesized_optional_member_call_preserves_receiver() {
        let chunk = compile("(object?.method)()");
        assert!(
            chunk
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::GetMethod(_))),
            "{:?}",
            chunk.instructions
        );
        assert!(
            chunk.instructions.contains(&Instruction::CallWithThis(0)),
            "{:?}",
            chunk.instructions
        );
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
    fn function_body_uses_local_slots_for_parameters() {
        let chunk = compile("function add(a, b) { return a + b; }");
        let fn_template = &chunk.functions[0];
        assert!(
            fn_template
                .chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadLocal(_)))
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
    fn computed_postfix_update_preserves_the_evaluated_reference() {
        let chunk = compile("obj[key()]++");
        assert!(chunk.instructions.contains(&Instruction::SetElementKeepOld));
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
    fn function_var_uses_local_slot_initialization() {
        let chunk = compile("function f() { var x = 1; }");
        let fn_chunk = &chunk.functions[0].chunk;
        assert!(
            fn_chunk
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::InitializeLocal(_)))
        );
    }

    #[test]
    fn function_body_chunk_validates() {
        let chunk = compile("function add(a, b) { return a + b; }");
        assert!(chunk.functions[0].chunk.validate().is_ok());
    }
}
