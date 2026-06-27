//! Statement parsing helpers.

use std::collections::HashSet;

use crate::{
    ast::{
        ArrayBindingElement, BindingPattern, CatchClause, Expression, ForBinding, FunctionBody,
        FunctionParam, ObjectBindingKey, ObjectBindingProp, ObjectProperty, PropertyName,
        Statement, SwitchCase, VariableDeclarator, VariableKind,
    },
    lexer::{Keyword, TokenKind},
    parser::{
        ParseError, Parser, describe, is_reserved_identifier_name, is_strict_future_reserved,
        is_strict_future_reserved_keyword,
    },
};

impl Parser {
    pub(super) fn parse_module_item(&mut self) -> Result<Statement, ParseError> {
        match self.peek().kind {
            TokenKind::Keyword(Keyword::Import) => self.parse_import_declaration(),
            TokenKind::Keyword(Keyword::Export) => self.parse_export_declaration(),
            _ => self.parse_statement(),
        }
    }

    /// Parses a single statement.
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match &self.peek().kind {
            TokenKind::Punctuator(';') => {
                self.advance();
                Ok(Statement::Empty)
            }
            TokenKind::Punctuator('{') => self.parse_block(),
            // `let` in sloppy mode may be used as an identifier when followed
            // by a line terminator (ASI terminates it as an expression statement).
            // `let [` always starts a destructuring declaration regardless.
            TokenKind::Keyword(Keyword::Let)
                if !self.is_strict
                    && self
                        .tokens
                        .get(self.cursor + 1)
                        .is_some_and(|t| t.line_terminator_before)
                    // If the next token is `await` in an async context or `yield`
                    // in a generator/strict context, ASI does not apply — the
                    // parser must attempt a let-binding (which will fail), producing
                    // a SyntaxError per ECMAScript.
                    && !(matches!(
                        self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                        Some(TokenKind::Keyword(Keyword::Await))
                    ) && self.is_async_context)
                    && !(matches!(
                        self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                        Some(TokenKind::Keyword(Keyword::Yield))
                    ) && (self.is_strict || self.is_generator_context))
                    && !matches!(
                        self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                        Some(TokenKind::Punctuator('['))
                    ) =>
            {
                self.parse_expression_statement()
            }
            TokenKind::Keyword(Keyword::Var | Keyword::Let | Keyword::Const) => {
                self.parse_variable_declaration()
            }
            TokenKind::Keyword(Keyword::Function) => self.parse_function_declaration(),
            // V9-A: `async function` declaration (contextual: `async` is an Identifier)
            // Per spec, `async` with a Unicode escape (e.g. `async`) cannot serve
            // as the contextual keyword, so we check `has_identifier_escape` here.
            TokenKind::Identifier(name)
                if name == "async"
                    && !self.peek().has_identifier_escape
                    && matches!(
                        self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                        Some(TokenKind::Keyword(Keyword::Function))
                    )
                    && !self
                        .tokens
                        .get(self.cursor + 1)
                        .is_some_and(|t| t.line_terminator_before) =>
            {
                self.parse_async_function_declaration()
            }
            TokenKind::Keyword(Keyword::Return) => self.parse_return(),
            TokenKind::Keyword(Keyword::If) => self.parse_if(),
            TokenKind::Keyword(Keyword::While) => self.parse_while(),
            TokenKind::Keyword(Keyword::Do) => self.parse_do_while(),
            TokenKind::Keyword(Keyword::For) => self.parse_for(),
            TokenKind::Keyword(Keyword::Break) => self.parse_break(),
            TokenKind::Keyword(Keyword::Continue) => self.parse_continue(),
            TokenKind::Keyword(Keyword::Throw) => self.parse_throw(),
            TokenKind::Keyword(Keyword::Try) => self.parse_try(),
            TokenKind::Keyword(Keyword::Switch) => self.parse_switch(),
            TokenKind::Keyword(Keyword::Class) => self.parse_class_declaration(),
            // Labelled statement: identifier followed by `:`.
            // `await` and `yield` are valid labels in appropriate contexts,
            // but not when they are reserved in the current context (e.g. async/generator).
            TokenKind::Identifier(_)
                if matches!(
                    self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                    Some(TokenKind::Punctuator(':'))
                ) && self.label_identifier_is_valid() =>
            {
                self.parse_labelled_statement()
            }
            // `await` is a valid label in non-module (script) mode.
            TokenKind::Keyword(Keyword::Await)
                if !self.is_async_context
                    && matches!(
                        self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                        Some(TokenKind::Punctuator(':'))
                    ) =>
            {
                self.parse_labelled_statement()
            }
            // `yield` is a valid label in non-strict, non-generator mode.
            TokenKind::Keyword(Keyword::Yield)
                if !self.is_strict
                    && !self.is_generator_context
                    && matches!(
                        self.tokens.get(self.cursor + 1).map(|t| &t.kind),
                        Some(TokenKind::Punctuator(':'))
                    ) =>
            {
                self.parse_labelled_statement()
            }
            _ => self.parse_expression_statement(),
        }
    }

    /// Parses `{ statement* }`.
    pub(super) fn parse_block(&mut self) -> Result<Statement, ParseError> {
        Ok(Statement::Block(self.parse_block_statements()?))
    }

    /// Parses a braced statement list and returns its contents.
    fn parse_block_statements(&mut self) -> Result<Vec<Statement>, ParseError> {
        self.expect_punctuator('{')?;
        self.enter_depth()?;
        let mut body = Vec::new();
        while !self.check_punctuator('}') && !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        self.leave_depth();
        self.expect_punctuator('}')?;
        self.validate_lexical_declarations(&body)?;
        Ok(body)
    }

    /// Parses `[async] function [*] name(params) { body }`.
    ///
    /// V9-A: handles `async function` and `function*` declarations.
    /// Function declarations are not allowed at statement level inside other
    /// functions in strict mode, but V3 permits them anywhere a statement is
    /// allowed.
    fn parse_function_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `function`
        let is_generator = self.eat_operator("*");
        // Parse the name in the OUTER (non-generator) context so that `yield` is
        // a valid binding identifier for `function* yield()` in non-strict mode.
        let name = self.expect_identifier()?;
        let outer_generator = self.is_generator_context;
        self.is_generator_context = is_generator;
        let params = self.parse_param_list()?;
        let is_nspl = Self::params_are_non_simple(&params);
        let body_strict = self.peek_body_has_use_strict();
        // NSPL + "use strict" in body is always a SyntaxError.
        if is_nspl && body_strict {
            self.is_generator_context = outer_generator;
            return Err(self.error(
                "\"use strict\" directive is not allowed in function with non-simple parameters"
                    .into(),
            ));
        }
        // Duplicate params forbidden in: generators, strict mode, NSPL, or when body has "use strict".
        if is_generator || self.is_strict || body_strict || is_nspl {
            self.check_duplicate_params(&params)?;
        }
        // Strict param names forbidden in strict mode or when body is strict.
        if (self.is_strict || body_strict) && !is_generator {
            self.check_strict_params(&params)?;
            if matches!(name.as_str(), "eval" | "arguments")
                || crate::parser::is_strict_future_reserved(&name)
            {
                return Err(self.error(format!(
                    "function name `{name}` is not allowed in strict mode"
                )));
            }
        }
        let body = self.parse_function_body()?;
        self.is_generator_context = outer_generator;
        // Check for param/lexical conflicts (BoundNames vs LexicallyDeclaredNames).
        self.validate_params_vs_lexical(&params, &body.statements)?;
        Ok(Statement::FunctionDeclaration {
            name,
            params,
            body,
            is_async: false,
            is_generator,
        })
    }

    /// Parses `async function [*] name(params) { body }` at statement level.
    fn parse_async_function_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `async` (Identifier)
        self.advance(); // `function`
        let is_generator = self.eat_operator("*");
        let outer_async = self.is_async_context;
        let outer_generator = self.is_generator_context;
        self.is_async_context = true;
        self.is_generator_context = is_generator;
        let name = self.expect_identifier()?;
        let params = self.parse_param_list()?;
        // Async (and async-generator) functions always require unique parameter names.
        self.check_duplicate_params(&params)?;
        let is_nspl = Self::params_are_non_simple(&params);
        if is_nspl && self.peek_body_has_use_strict() {
            self.is_async_context = outer_async;
            self.is_generator_context = outer_generator;
            return Err(self.error(
                "\"use strict\" directive is not allowed in function with non-simple parameters"
                    .into(),
            ));
        }
        let body = self.parse_function_body()?;
        self.is_async_context = outer_async;
        self.is_generator_context = outer_generator;
        // Check for param/lexical conflicts.
        self.validate_params_vs_lexical(&params, &body.statements)?;
        Ok(Statement::FunctionDeclaration {
            name,
            params,
            body,
            is_async: true,
            is_generator,
        })
    }

    /// Parses a parameter list `(name, name, ..., ...rest)`.
    /// Supports destructuring patterns (`{a,b}`, `[a,b]`), default values (`x=1`),
    /// and rest destructuring (`...[a,b]`).
    pub(super) fn parse_param_list(&mut self) -> Result<Vec<FunctionParam>, ParseError> {
        self.expect_punctuator('(')?;
        let mut params = Vec::new();
        if !self.check_punctuator(')') {
            loop {
                if self.check_spread() {
                    // rest parameter: `...name` or `...[pat]` or `...{pat}`
                    self.advance(); // consume `...`
                    if self.check_punctuator('[') || self.check_punctuator('{') {
                        let pat = self.parse_binding_pattern()?;
                        params.push(FunctionParam::RestPattern(pat));
                    } else {
                        let rest_name = self.expect_identifier()?;
                        params.push(FunctionParam::Rest(rest_name));
                    }
                    // rest parameter must be last; trailing comma is a SyntaxError
                    if self.check_punctuator(',') {
                        return Err(
                            self.error("rest parameter must be last formal parameter".into())
                        );
                    }
                    break;
                }
                // destructuring parameter: `{a,b}` or `[a,b]`
                if self.check_punctuator('[') || self.check_punctuator('{') {
                    let pat = self.parse_binding_pattern()?;
                    let default_value = if self.eat_operator("=") {
                        Some(Box::new(self.parse_assignment()?))
                    } else {
                        None
                    };
                    params.push(FunctionParam::Pattern(pat, default_value));
                } else {
                    let name = self.expect_identifier()?;
                    if self.eat_operator("=") {
                        let default_val = self.parse_assignment()?;
                        params.push(FunctionParam::Default(name, Box::new(default_val)));
                    } else {
                        params.push(FunctionParam::Simple(name));
                    }
                }
                if !self.eat_punctuator(',') {
                    break;
                }
                // trailing comma before `)` is valid
                if self.check_punctuator(')') {
                    break;
                }
            }
        }
        self.expect_punctuator(')')?;
        Ok(params)
    }

    /// Parses `{ statement* }` as a function body, tracking function_depth so
    /// that `return` inside is accepted.
    pub(super) fn parse_function_body(&mut self) -> Result<FunctionBody, ParseError> {
        self.expect_punctuator('{')?;
        let outer_loop_depth = self.loop_depth;
        let outer_switch_depth = self.switch_depth;
        let outer_strict = self.is_strict;
        let outer_labels = std::mem::take(&mut self.label_stack);
        self.loop_depth = 0;
        self.switch_depth = 0;
        self.function_depth += 1;
        let mut statements = Vec::new();
        let mut function_is_strict = outer_strict;
        let result = (|| {
            // Scan for a "use strict" directive prologue before parsing statements.
            self.consume_directive_prologue()?;
            function_is_strict = self.is_strict;
            while !self.check_punctuator('}') && !self.at_eof() {
                statements.push(self.parse_statement()?);
            }
            self.expect_punctuator('}')
        })();
        self.function_depth -= 1;
        self.loop_depth = outer_loop_depth;
        self.switch_depth = outer_switch_depth;
        self.is_strict = outer_strict;
        self.label_stack = outer_labels;
        result?;
        self.validate_lexical_declarations(&statements)?;
        Ok(FunctionBody {
            statements,
            is_strict: function_is_strict,
        })
    }

    /// Looks ahead to check if the next function body (starting with `{`) has
    /// an explicit `"use strict"` directive. Does not consume tokens.
    /// Needed for the NSPL + "use strict" early error check.
    pub(super) fn peek_body_has_use_strict(&self) -> bool {
        // Cursor is at `{`. Directive string is at cursor+1.
        let Some(tok) = self.tokens.get(self.cursor + 1) else {
            return false;
        };
        let TokenKind::String(s) = &tok.kind else {
            return false;
        };
        if s != "use strict" {
            return false;
        }
        let Some(after) = self.tokens.get(self.cursor + 2) else {
            return true;
        };
        matches!(
            after.kind,
            TokenKind::Punctuator(';') | TokenKind::Punctuator('}') | TokenKind::Eof
        ) || after.line_terminator_before
    }

    /// Returns `true` if any parameter in the list makes it non-simple:
    /// destructuring patterns, rest parameters, or parameters with defaults.
    pub(super) fn params_are_non_simple(params: &[FunctionParam]) -> bool {
        params.iter().any(|p| {
            matches!(
                p,
                FunctionParam::Pattern(..)
                    | FunctionParam::RestPattern(_)
                    | FunctionParam::Rest(_)
                    | FunctionParam::Default(..)
            )
        })
    }

    /// Checks for duplicate bound names across all parameters, including those
    /// inside destructuring patterns. Arrow functions always require this check
    /// (UniqueFormalParameters); strict-mode / generator functions do too.
    pub(super) fn check_duplicate_params(
        &self,
        params: &[FunctionParam],
    ) -> Result<(), ParseError> {
        let mut seen: HashSet<String> = HashSet::new();
        for p in params {
            let mut names: Vec<String> = Vec::new();
            collect_param_bound_names(p, &mut names);
            for name in names {
                if !seen.insert(name.clone()) {
                    return Err(
                        self.error(format!("duplicate parameter name `{name}` is not allowed"))
                    );
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    /// Old flat-only version (kept to satisfy the borrow checker).
    fn _check_duplicate_params_flat_only(
        &self,
        params: &[FunctionParam],
    ) -> Result<(), ParseError> {
        let mut seen: HashSet<&str> = HashSet::new();
        for p in params {
            let name = p.name();
            if name.is_empty() {
                continue;
            }
            if !seen.insert(name) {
                return Err(self.error(format!(
                    "duplicate parameter name `{name}` is not allowed in strict mode"
                )));
            }
        }
        Ok(())
    }

    /// Validates that no parameter name is `eval`, `arguments`, or a strict-mode
    /// future reserved word. Called retroactively when a function body turns out
    /// to be strict (inner `"use strict"` directive).
    pub(super) fn check_strict_params(&self, params: &[FunctionParam]) -> Result<(), ParseError> {
        for p in params {
            let name = p.name();
            if matches!(name, "eval" | "arguments")
                || crate::parser::is_strict_future_reserved(name)
            {
                return Err(self.error(format!(
                    "`{name}` cannot be used as a parameter name in strict mode"
                )));
            }
        }
        Ok(())
    }

    /// Scans for ECMAScript directive prologues (`"use strict";`) at the start
    /// of a script or function body. Sets `self.is_strict` if found.
    /// The cursor must be positioned just after the opening `{` (or at the
    /// start of a script). Only `ExpressionStatement(StringLiteral)` nodes
    /// optionally followed by a semicolon are directive candidates.
    pub(super) fn consume_directive_prologue(&mut self) -> Result<(), ParseError> {
        // Track legacy-escape tokens seen before "use strict" is encountered.
        let mut legacy_escape_spans: Vec<crate::lexer::Span> = Vec::new();
        loop {
            // A directive is a String literal token followed by an optional `;`.
            let tok = self.peek().clone();
            let TokenKind::String(ref value) = tok.kind else {
                break;
            };
            // Peek ahead: the token AFTER the string must be a `;`, `}`, a
            // line terminator boundary, or EOF -- otherwise this isn't a directive.
            let next_is_directive_end = {
                let after = self.tokens.get(self.cursor + 1);
                match after {
                    None => true,
                    Some(t) => {
                        matches!(
                            t.kind,
                            TokenKind::Punctuator(';')
                                | TokenKind::Punctuator('}')
                                | TokenKind::Eof
                        ) || t.line_terminator_before
                    }
                }
            };
            if !next_is_directive_end {
                break;
            }
            let is_use_strict = value == "use strict";
            // Strict-mode early error: once strict mode is active, any string
            // literal -- even one still inside the directive prologue -- must not
            // contain a legacy escape sequence.
            if self.is_strict && tok.has_legacy_escape {
                return Err(ParseError {
                    span: tok.span,
                    message:
                        "octal escape sequences are not allowed in strict mode string literals"
                            .into(),
                });
            }
            // If not yet strict, track legacy-escape spans so we can retroactively
            // reject them if "use strict" appears later in the prologue.
            if !self.is_strict && tok.has_legacy_escape {
                legacy_escape_spans.push(tok.span);
            }
            self.advance(); // consume string token
            self.eat_punctuator(';');
            if is_use_strict {
                // "use strict" found: any earlier legacy-escape string is now illegal.
                if let Some(&span) = legacy_escape_spans.first() {
                    return Err(ParseError {
                        span,
                        message:
                            "octal escape sequences are not allowed in strict mode string literals"
                                .into(),
                    });
                }
                self.is_strict = true;
            }
        }
        Ok(())
    }
    /// Parses `return;` or `return expression;`.
    ///
    /// ECMAScript treats a line terminator between `return` and its expression
    /// as an implicit semicolon (restricted production). If the next token is on
    /// a new line, `return;` is produced without consuming the expression.
    fn parse_return(&mut self) -> Result<Statement, ParseError> {
        if self.function_depth == 0 {
            return Err(self.error("illegal `return` statement outside of a function".into()));
        }
        self.advance(); // `return`

        // Restricted production: a line terminator after `return` = implicit `;`
        if self.peek().line_terminator_before
            || matches!(
                self.peek().kind,
                TokenKind::Punctuator(';') | TokenKind::Eof
            )
        {
            self.eat_punctuator(';');
            return Ok(Statement::Return(None));
        }

        let value = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(Statement::Return(Some(value)))
    }

    fn parse_import_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `import`
        let mut entries = Vec::new();

        if let TokenKind::String(source) = self.peek().kind.clone() {
            self.advance();
            self.expect_semicolon()?;
            return Ok(Statement::ModuleDeclaration(
                crate::ast::ModuleDeclaration::Import(crate::ast::ImportDeclaration {
                    source,
                    entries,
                }),
            ));
        }

        if self.eat_operator("*") {
            self.expect_identifier_name_exact("as")?;
            let local_name = self.expect_identifier()?;
            entries.push(crate::ast::ImportEntry {
                imported_name: "*".into(),
                local_name,
            });
        } else {
            if let TokenKind::Identifier(local_name) = self.peek().kind.clone() {
                self.advance();
                entries.push(crate::ast::ImportEntry {
                    imported_name: "default".into(),
                    local_name,
                });
                if self.eat_punctuator(',') {
                    if self.eat_operator("*") {
                        self.expect_identifier_name_exact("as")?;
                        let local_name = self.expect_identifier()?;
                        entries.push(crate::ast::ImportEntry {
                            imported_name: "*".into(),
                            local_name,
                        });
                    } else {
                        self.parse_named_import_entries(&mut entries)?;
                    }
                }
            } else {
                self.parse_named_import_entries(&mut entries)?;
            }
        }

        self.expect_identifier_name_exact("from")?;
        let source = self.expect_string_literal("module specifier")?;
        self.expect_semicolon()?;
        Ok(Statement::ModuleDeclaration(
            crate::ast::ModuleDeclaration::Import(crate::ast::ImportDeclaration {
                source,
                entries,
            }),
        ))
    }

    fn parse_named_import_entries(
        &mut self,
        entries: &mut Vec<crate::ast::ImportEntry>,
    ) -> Result<(), ParseError> {
        self.expect_punctuator('{')?;
        while !self.check_punctuator('}') {
            let imported_name = self.expect_module_export_name()?;
            let local_name = if self.eat_identifier_name("as") {
                self.expect_module_binding_name()?
            } else {
                self.validate_module_binding_identifier(&imported_name)?;
                imported_name.clone()
            };
            entries.push(crate::ast::ImportEntry {
                imported_name,
                local_name,
            });
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_punctuator('}')
    }

    fn parse_export_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `export`
        let mut entries = Vec::new();
        let mut source = None;
        let mut declaration = None;

        if self.eat_operator("*") {
            if self.eat_identifier_name("as") {
                let export_name = self.expect_module_export_name()?;
                entries.push(crate::ast::ExportEntry {
                    export_name,
                    local_name: None,
                });
            } else {
                entries.push(crate::ast::ExportEntry {
                    export_name: "*".into(),
                    local_name: None,
                });
            }
            self.expect_identifier_name_exact("from")?;
            source = Some(self.expect_string_literal("module specifier")?);
            self.expect_semicolon()?;
        } else if self.check_punctuator('{') {
            self.parse_export_entries(&mut entries)?;
            if self.eat_identifier_name("from") {
                source = Some(self.expect_string_literal("module specifier")?);
            }
            self.expect_semicolon()?;
        } else if matches!(
            self.peek().kind,
            TokenKind::Keyword(Keyword::Var | Keyword::Let | Keyword::Const)
        ) {
            let stmt = self.parse_variable_declaration()?;
            add_export_decl_names(&stmt, &mut entries);
            declaration = Some(Box::new(stmt));
        } else if self.check_keyword(Keyword::Function) {
            let stmt = self.parse_function_declaration()?;
            add_export_decl_names(&stmt, &mut entries);
            declaration = Some(Box::new(stmt));
        } else if self.check_keyword(Keyword::Class) {
            let stmt = self.parse_class_declaration()?;
            add_export_decl_names(&stmt, &mut entries);
            declaration = Some(Box::new(stmt));
        } else if self.eat_keyword(Keyword::Default) {
            let local_name = if self.check_keyword(Keyword::Function) {
                let stmt = self.parse_export_default_function_declaration()?;
                let name = export_default_decl_name(&stmt);
                declaration = Some(Box::new(stmt));
                name
            } else if self.check_keyword(Keyword::Class) {
                let stmt = self.parse_export_default_class_declaration()?;
                let name = export_default_decl_name(&stmt);
                declaration = Some(Box::new(stmt));
                name
            } else {
                let expr = self.parse_assignment()?;
                self.expect_semicolon()?;
                declaration = Some(Box::new(Statement::Expression(expr)));
                None
            };
            entries.push(crate::ast::ExportEntry {
                export_name: "default".into(),
                local_name,
            });
        } else {
            return Err(self.error("unsupported export declaration".into()));
        }

        Ok(Statement::ModuleDeclaration(
            crate::ast::ModuleDeclaration::Export(crate::ast::ExportDeclaration {
                entries,
                source,
                declaration,
            }),
        ))
    }

    fn parse_export_entries(
        &mut self,
        entries: &mut Vec<crate::ast::ExportEntry>,
    ) -> Result<(), ParseError> {
        self.expect_punctuator('{')?;
        while !self.check_punctuator('}') {
            let local_name = self.expect_module_export_name()?;
            let export_name = if self.eat_identifier_name("as") {
                self.expect_module_export_name()?
            } else {
                local_name.clone()
            };
            entries.push(crate::ast::ExportEntry {
                export_name,
                local_name: Some(local_name),
            });
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_punctuator('}')
    }

    fn parse_export_default_function_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `function`
        let is_generator = self.eat_operator("*");
        let name = if self.check_punctuator('(') {
            "*default*".into()
        } else {
            self.expect_identifier()?
        };
        let outer_generator = self.is_generator_context;
        self.is_generator_context = is_generator;
        let params = self.parse_param_list()?;
        let is_nspl = Self::params_are_non_simple(&params);
        let body_strict = self.peek_body_has_use_strict();
        if is_nspl && body_strict {
            self.is_generator_context = outer_generator;
            return Err(self.error(
                "\"use strict\" directive is not allowed in function with non-simple parameters"
                    .into(),
            ));
        }
        if is_generator || self.is_strict || body_strict || is_nspl {
            self.check_duplicate_params(&params)?;
        }
        if self.is_strict || body_strict {
            self.check_strict_params(&params)?;
            if name != "*default*"
                && (matches!(name.as_str(), "eval" | "arguments")
                    || crate::parser::is_strict_future_reserved(&name))
            {
                self.is_generator_context = outer_generator;
                return Err(self.error(format!(
                    "function name `{name}` is not allowed in strict mode"
                )));
            }
        }
        let body = self.parse_function_body()?;
        self.is_generator_context = outer_generator;
        self.validate_params_vs_lexical(&params, &body.statements)?;
        Ok(Statement::FunctionDeclaration {
            name,
            params,
            body,
            is_async: false,
            is_generator,
        })
    }

    fn parse_export_default_class_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `class`
        let outer_strict = self.is_strict;
        self.is_strict = true;
        let name = if self.check_punctuator('{') || self.check_keyword(Keyword::Extends) {
            "*default*".into()
        } else {
            self.expect_class_name()?
        };
        self.is_strict = outer_strict;
        let super_class = if self.eat_keyword(Keyword::Extends) {
            Some(self.parse_assignment()?)
        } else {
            None
        };
        let elements = self.parse_class_body()?;
        Ok(Statement::ClassDeclaration(crate::ast::ClassDeclaration {
            name,
            super_class,
            elements,
        }))
    }

    fn eat_identifier_name(&mut self, name: &str) -> bool {
        match self.peek().kind.clone() {
            TokenKind::Identifier(value) if value == name => {
                self.advance();
                true
            }
            TokenKind::Keyword(keyword) if keyword.as_str() == name => {
                self.advance();
                true
            }
            _ => false,
        }
    }

    fn expect_identifier_name_exact(&mut self, name: &str) -> Result<(), ParseError> {
        if self.eat_identifier_name(name) {
            Ok(())
        } else {
            Err(self.error(format!(
                "expected `{name}` but found {}",
                describe(&self.peek().kind)
            )))
        }
    }

    fn expect_string_literal(&mut self, label: &str) -> Result<String, ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::String(value) => {
                if self.is_strict && token.has_legacy_escape {
                    return Err(self.error(format!(
                        "{label} cannot contain a legacy escape in strict mode"
                    )));
                }
                self.advance();
                Ok(value)
            }
            _ => Err(self.error(format!(
                "expected {label} string literal but found {}",
                describe(&self.peek().kind)
            ))),
        }
    }

    fn expect_module_binding_name(&mut self) -> Result<String, ParseError> {
        let name = self.expect_identifier()?;
        self.validate_module_binding_identifier(&name)?;
        Ok(name)
    }

    fn expect_module_export_name(&mut self) -> Result<String, ParseError> {
        match self.peek().kind.clone() {
            TokenKind::String(value) => {
                // ponytail: the lexer currently maps lone surrogate escapes to U+FFFD.
                // Upgrade path: carry an explicit ill-formed-string flag on Token.
                if value.contains(char::REPLACEMENT_CHARACTER) {
                    return Err(self.error(
                        "module export names must be well-formed Unicode strings".into(),
                    ));
                }
                self.advance();
                Ok(value)
            }
            _ => self.expect_identifier_name(),
        }
    }

    fn validate_module_binding_identifier(&self, name: &str) -> Result<(), ParseError> {
        if matches!(name, "eval" | "arguments" | "await" | "yield")
            || is_reserved_identifier_name(name)
            || is_strict_future_reserved(name)
            || is_strict_future_reserved_keyword(name)
            || !is_identifier_like(name)
        {
            Err(self.error(format!(
                "`{name}` cannot be used as an imported binding name in module code"
            )))
        } else {
            Ok(())
        }
    }

    /// Parses `var`/`let`/`const` declarations.
    fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        let kind = match self.advance().kind {
            TokenKind::Keyword(Keyword::Var) => VariableKind::Var,
            TokenKind::Keyword(Keyword::Let) => VariableKind::Let,
            TokenKind::Keyword(Keyword::Const) => VariableKind::Const,
            _ => unreachable!("variable declaration starts with a declaration keyword"),
        };

        // Destructuring: `let [a, b] = ...` or `const { x } = ...`
        if self.check_punctuator('[') || self.check_punctuator('{') {
            let pattern = self.parse_binding_pattern()?;
            if !self.eat_operator("=") {
                return Err(self.error("destructuring declaration requires an initializer".into()));
            }
            let initializer = self.parse_assignment()?;
            self.expect_semicolon()?;
            return Ok(Statement::DestructuringDeclaration {
                kind,
                pattern,
                initializer,
            });
        }

        let mut declarations = Vec::new();
        loop {
            let name = self.expect_identifier()?;
            let initializer = if self.eat_operator("=") {
                Some(self.parse_assignment()?)
            } else {
                None
            };
            if kind == VariableKind::Const && initializer.is_none() {
                return Err(self.error("`const` declarations require an initializer".into()));
            }
            declarations.push(VariableDeclarator {
                name,
                pattern: None,
                initializer,
            });
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_semicolon()?;
        Ok(Statement::VariableDeclaration { kind, declarations })
    }

    /// Parses a binding pattern: `name`, `[a, b, c]`, or `{ x, y: z }`.
    pub(super) fn parse_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        if self.check_punctuator('[') {
            return self.parse_array_binding_pattern();
        }
        if self.check_punctuator('{') {
            return self.parse_object_binding_pattern();
        }
        let name = self.expect_identifier()?;
        Ok(BindingPattern::Identifier(name))
    }

    fn parse_array_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        self.expect_punctuator('[')?;
        let mut elements: Vec<Option<ArrayBindingElement>> = Vec::new();
        let mut rest = None;
        while !self.check_punctuator(']') && !self.at_eof() {
            // hole
            if self.eat_punctuator(',') {
                elements.push(None);
                continue;
            }
            // rest element: `...name` or `...[pat]` or `...{pat}`
            if self.check_spread() {
                self.advance(); // consume `...`
                let rest_pat = self.parse_binding_pattern()?;
                rest = Some(Box::new(rest_pat));
                self.eat_punctuator(','); // optional trailing comma
                break;
            }
            let pat = self.parse_binding_pattern()?;
            let default = if self.eat_operator("=") {
                Some(Box::new(self.parse_assignment()?))
            } else {
                None
            };
            elements.push(Some(ArrayBindingElement {
                pattern: pat,
                default,
            }));
            if !self.eat_punctuator(',') {
                break;
            }
        }
        self.expect_punctuator(']')?;
        Ok(BindingPattern::Array { elements, rest })
    }

    fn parse_object_binding_pattern(&mut self) -> Result<BindingPattern, ParseError> {
        self.expect_punctuator('{')?;
        let mut props: Vec<ObjectBindingProp> = Vec::new();
        let mut rest = None;
        while !self.check_punctuator('}') && !self.at_eof() {
            // rest property: `...name`
            if self.check_spread() {
                self.advance(); // consume `...`
                let name = self.expect_identifier()?;
                rest = Some(Box::new(BindingPattern::Identifier(name)));
                self.eat_punctuator(',');
                break;
            }
            // computed key: `[expr]: pattern`
            if self.eat_punctuator('[') {
                let key_expr = self.parse_assignment()?;
                self.expect_punctuator(']')?;
                self.expect_punctuator(':')?;
                let value = self.parse_binding_pattern()?;
                let default = if self.eat_operator("=") {
                    Some(Box::new(self.parse_assignment()?))
                } else {
                    None
                };
                props.push(ObjectBindingProp {
                    key: ObjectBindingKey::Computed(Box::new(key_expr)),
                    value,
                    default,
                });
            } else {
                // static key: identifier (including keywords), string, or number
                let (key_prop_name, shorthand_name, key_had_escape) =
                    self.parse_object_binding_key()?;
                let (value, default) = if self.eat_punctuator(':') {
                    // `{ key: pattern }` or `{ key: pattern = default }`
                    let pat = self.parse_binding_pattern()?;
                    let def = if self.eat_operator("=") {
                        Some(Box::new(self.parse_assignment()?))
                    } else {
                        None
                    };
                    (pat, def)
                } else {
                    // `{ name }` shorthand, possibly `{ name = default }`
                    let shorthand = shorthand_name.ok_or_else(|| {
                        self.error(
                            "expected `:` after non-identifier property key in binding pattern"
                                .into(),
                        )
                    })?;
                    // The shorthand name is a binding identifier �?reserved words are forbidden.
                    if is_reserved_identifier_name(&shorthand) {
                        return Err(self.error(format!(
                            "reserved word `{shorthand}` cannot be used as a binding identifier"
                        )));
                    }
                    if self.is_strict && matches!(shorthand.as_str(), "arguments" | "eval") {
                        return Err(self.error(format!(
                            "`{shorthand}` cannot be used as a binding identifier in strict mode"
                        )));
                    }
                    // In strict mode, future-reserved words and strict-future keywords
                    // cannot be shorthand binding identifiers.
                    if self.is_strict
                        && (crate::parser::is_strict_future_reserved(&shorthand)
                            || crate::parser::is_strict_future_reserved_keyword(&shorthand))
                    {
                        return Err(self.error(format!(
                            "`{shorthand}` cannot be used as a binding identifier in strict mode"
                        )));
                    }
                    // An identifier escape that resolves to a strict-future-reserved word
                    // is also forbidden in strict mode.
                    if key_had_escape
                        && self.is_strict
                        && crate::parser::is_strict_future_reserved_keyword(&shorthand)
                    {
                        return Err(self.error(format!(
                            "identifier escape sequence resolves to reserved word `{shorthand}`"
                        )));
                    }
                    let def = if self.eat_operator("=") {
                        Some(Box::new(self.parse_assignment()?))
                    } else {
                        None
                    };
                    (BindingPattern::Identifier(shorthand), def)
                };
                props.push(ObjectBindingProp {
                    key: ObjectBindingKey::Static(key_prop_name),
                    value,
                    default,
                });
            }
            if !self.eat_punctuator(',') {
                break;
            }
            // trailing comma before `}` is valid
            if self.check_punctuator('}') {
                break;
            }
        }
        self.expect_punctuator('}')?;
        Ok(BindingPattern::Object { props, rest })
    }

    /// Parses the key portion of an object binding property.
    /// Returns `(PropertyName, Option<shorthand_binding_name>)`.
    /// `shorthand_name` is `Some` only for plain-identifier keys that can serve
    /// as both key and shorthand binding target: `{ foo }` �?key `"foo"`, binding `"foo"`.
    /// Returns `(property_name, shorthand_binding_name, had_escape_sequence)`.
    fn parse_object_binding_key(
        &mut self,
    ) -> Result<(PropertyName, Option<String>, bool), ParseError> {
        let tok = self.peek().clone();
        let had_escape = tok.has_identifier_escape;
        match tok.kind {
            TokenKind::String(s) => {
                self.advance();
                Ok((PropertyName::String(s), None, false))
            }
            TokenKind::Number(n) => {
                self.advance();
                Ok((PropertyName::Number(n), None, false))
            }
            TokenKind::Keyword(kw) => {
                self.advance();
                Ok((
                    PropertyName::Identifier(kw.as_str().into()),
                    None,
                    had_escape,
                ))
            }
            TokenKind::Identifier(name) => {
                self.advance();
                Ok((
                    PropertyName::Identifier(name.clone()),
                    Some(name),
                    had_escape,
                ))
            }
            _ => Err(self.error(format!(
                "expected property name, got {}",
                describe(&tok.kind)
            ))),
        }
    }

    /// Parses a class declaration: `class Name { ... }`.
    fn parse_class_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `class`
        // Class names are always in strict mode; reject strict-future reserved words
        // and escaped-identifier forms thereof.
        let outer_strict = self.is_strict;
        self.is_strict = true;
        let name_result = self.expect_class_name();
        self.is_strict = outer_strict;
        let name = name_result?;
        let super_class = if self.eat_keyword(Keyword::Extends) {
            Some(self.parse_assignment()?)
        } else {
            None
        };
        let elements = self.parse_class_body()?;
        Ok(Statement::ClassDeclaration(crate::ast::ClassDeclaration {
            name,
            super_class,
            elements,
        }))
    }

    /// Parses `if (test) consequent` with an optional `else`.
    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `if`
        self.expect_punctuator('(')?;
        let test = self.parse_expression()?;
        self.expect_punctuator(')')?;
        let consequent = Box::new(self.parse_substatement("if")?);
        let alternate = if self.eat_keyword(Keyword::Else) {
            Some(Box::new(self.parse_substatement("else")?))
        } else {
            None
        };
        Ok(Statement::If {
            test,
            consequent,
            alternate,
        })
    }

    /// Parses `while (test) body`, tracking loop depth so the body may contain
    /// `break`/`continue`.
    fn parse_while(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `while`
        self.expect_punctuator('(')?;
        let test = self.parse_expression()?;
        self.expect_punctuator(')')?;

        Ok(Statement::While {
            test,
            body: self.parse_loop_body()?,
        })
    }

    /// Parses `do body while (test);`.
    fn parse_do_while(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `do`
        let body = self.parse_loop_body()?;
        if !self.check_keyword(Keyword::While) {
            return Err(self.error("expected `while` after `do` body".into()));
        }
        self.advance(); // `while`
        self.expect_punctuator('(')?;
        let test = self.parse_expression()?;
        self.expect_punctuator(')')?;
        // Optional semicolon after `do-while`
        self.eat_punctuator(';');
        Ok(Statement::DoWhile { test, body })
    }

    /// Returns true if the tokens starting at `cursor` (after any label chains)
    /// begin an IterationStatement keyword.
    fn peek_iteration_after_labels(&self) -> bool {
        let mut pos = self.cursor;
        loop {
            match &self.tokens.get(pos).map(|t| &t.kind) {
                Some(TokenKind::Keyword(Keyword::While | Keyword::For | Keyword::Do)) => {
                    return true;
                }
                // Another label chain: identifier followed by ':'
                Some(TokenKind::Identifier(_)) => {
                    if let Some(next) = self.tokens.get(pos + 1)
                        && matches!(next.kind, TokenKind::Punctuator(':'))
                    {
                        pos += 2;
                        continue;
                    }
                    return false;
                }
                _ => return false,
            }
        }
    }

    /// Parses a class name identifier, enforcing strict-mode rules plus
    /// rejecting `let`, `static`, and `yield` (which are reserved in class contexts)
    /// and their escaped forms.
    fn expect_class_name(&mut self) -> Result<String, ParseError> {
        use crate::parser::{is_strict_future_reserved, is_strict_future_reserved_keyword};
        let tok = self.peek().clone();
        // `yield` is a keyword that must be rejected as a class name.
        if matches!(tok.kind, TokenKind::Keyword(Keyword::Yield)) {
            return Err(self.error("`yield` cannot be used as a class name".into()));
        }
        // `static` and `let` are Identifiers/Keywords that resolve to restricted names.
        if let TokenKind::Identifier(ref name) = tok.kind {
            // Check escaped forms resolving to strict-future keywords (let, static, yield).
            if tok.has_identifier_escape && is_strict_future_reserved_keyword(name) {
                return Err(self.error(format!(
                    "identifier escape sequence resolves to reserved word `{name}`"
                )));
            }
            // `let` and `static` are also not allowed as class names.
            if is_strict_future_reserved(name) || is_strict_future_reserved_keyword(name) {
                return Err(
                    self.error(format!("`{name}` is not a valid class name in strict mode"))
                );
            }
        }
        // Delegates the rest (reserved words, `await` in module context, etc.) to `expect_identifier`.
        self.expect_identifier()
    }

    /// Returns true if the current Identifier token is a valid LabelIdentifier
    /// in the current context (`await` is reserved in async; `yield` in strict/generator).
    fn label_identifier_is_valid(&self) -> bool {
        if let TokenKind::Identifier(name) = &self.peek().kind {
            if name == "await" && self.is_async_context {
                return false;
            }
            if name == "yield" && (self.is_strict || self.is_generator_context) {
                return false;
            }
        }
        true
    }

    /// Parses `label: statement`.
    fn parse_labelled_statement(&mut self) -> Result<Statement, ParseError> {
        let label = match self.peek().kind.clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                name
            }
            // `await` and `yield` may be used as labels in appropriate contexts
            // (checked in parse_statement before calling this method).
            TokenKind::Keyword(Keyword::Await) => {
                self.advance();
                "await".into()
            }
            TokenKind::Keyword(Keyword::Yield) => {
                self.advance();
                "yield".into()
            }
            _ => unreachable!("checked before calling"),
        };
        self.expect_punctuator(':')?;
        // Determine if this label wraps an IterationStatement (allows `continue label`)
        let is_iteration = self.peek_iteration_after_labels();
        if self.label_stack.iter().any(|(existing, _)| existing == &label) {
            return Err(self.error(format!("duplicate label `{label}`")));
        }
        self.label_stack.push((label.clone(), is_iteration));
        let body = self.parse_statement()?;
        self.label_stack.pop();
        // Spec: IsLabelledFunction must be false for all labelled statements; in strict mode
        // even a bare function declaration wrapped by a label is a SyntaxError.
        // In sloppy mode, only transitively-labelled function declarations are rejected.
        if self.is_strict && matches!(body, Statement::FunctionDeclaration { .. }) {
            return Err(self.error(
                "function declarations are not allowed as labelled statement bodies in strict mode"
                    .into(),
            ));
        }
        if is_labelled_function(&body) {
            return Err(self.error(
                "labelled function declarations are not allowed in labelled statement bodies"
                    .into(),
            ));
        }
        // Lexical declarations (const/let) and certain declarations are not statements
        // and cannot appear in single-statement positions including label bodies.
        let decl_kind_err = match &body {
            Statement::VariableDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                ..
            }
            | Statement::DestructuringDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                ..
            } => Some("lexical declarations"),
            Statement::ClassDeclaration(_) => Some("class declarations"),
            Statement::FunctionDeclaration {
                is_generator: true, ..
            } => Some("generator function declarations"),
            Statement::FunctionDeclaration { is_async: true, .. } => {
                Some("async function declarations")
            }
            _ => None,
        };
        if let Some(kind) = decl_kind_err {
            return Err(self.error(format!(
                "{kind} are not allowed in labelled statement bodies"
            )));
        }
        Ok(Statement::Labelled {
            label,
            body: Box::new(body),
        })
    }

    /// Returns `true` if the current token is the contextual keyword `of`.
    fn check_contextual_of(&self) -> bool {
        matches!(&self.peek().kind, TokenKind::Identifier(s) if s == "of")
    }

    /// Parses both `for (init; test; update) body`, `for (left in right) body`,
    /// and V9-A `for [await] (left of right) body`.
    fn parse_for(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `for`

        // V9-A: `for await (left of right)` �?the `await` must immediately follow `for`
        // without a line terminator. We accept it syntactically but it only makes
        // sense inside an async function; runtime will surface the error otherwise.
        let is_await = matches!(&self.peek().kind, TokenKind::Keyword(Keyword::Await));
        if is_await {
            self.advance(); // `await`
        }

        self.expect_punctuator('(')?;

        // Empty init: `for (; test; update)`.
        if self.eat_punctuator(';') {
            return self.parse_for_classic_rest(None);
        }

        // Declaration head: `var`/`let`/`const`.
        if let TokenKind::Keyword(keyword @ (Keyword::Var | Keyword::Let | Keyword::Const)) =
            self.peek().kind
        {
            let kind = match keyword {
                Keyword::Var => VariableKind::Var,
                Keyword::Let => VariableKind::Let,
                Keyword::Const => VariableKind::Const,
                _ => unreachable!(),
            };
            self.advance(); // declaration keyword

            // `for (let/const/var [a,b] of/in right)` �?destructuring for-of/for-in
            if self.check_punctuator('[') || self.check_punctuator('{') {
                let pattern = self.parse_binding_pattern()?;
                if self.check_contextual_of() {
                    self.advance(); // `of`
                    let right = self.allowing_in(|p| p.parse_assignment())?;
                    self.expect_punctuator(')')?;
                    let body = self.parse_loop_body()?;
                    return Ok(Statement::ForOf {
                        left: ForBinding::Declaration { kind, pattern },
                        right,
                        body,
                        is_await,
                    });
                }
                if self.check_keyword(Keyword::In) {
                    self.advance(); // `in`
                    let right = self.parse_expression()?;
                    self.expect_punctuator(')')?;
                    let body = self.parse_loop_body()?;
                    return Ok(Statement::ForIn {
                        left: ForBinding::Declaration { kind, pattern },
                        right,
                        body,
                    });
                }
                // Classic for: `for (const [x, y] = init; test; update)`
                let initializer = if self.eat_operator("=") {
                    let expr = self.parse_assignment()?;
                    Some(expr)
                } else {
                    None
                };
                if kind == VariableKind::Const && initializer.is_none() {
                    return Err(self.error("`const` declarations require an initializer".into()));
                }
                let decl = Statement::VariableDeclaration {
                    kind,
                    declarations: vec![VariableDeclarator {
                        name: String::new(),
                        pattern: Some(pattern),
                        initializer,
                    }],
                };
                self.expect_semicolon()?; // consume `;` between init and test
                return self.parse_for_classic_rest(Some(Box::new(decl)));
            }

            let name = self.expect_identifier()?;

            if self.check_keyword(Keyword::In) {
                self.advance(); // `in`
                let right = self.parse_expression()?;
                self.expect_punctuator(')')?;
                let body = self.parse_loop_body()?;
                return Ok(Statement::ForIn {
                    left: ForBinding::Declaration {
                        kind,
                        pattern: crate::ast::BindingPattern::Identifier(name),
                    },
                    right,
                    body,
                });
            }

            // V9-A: `for (let x of right)`
            if self.check_contextual_of() {
                self.advance(); // `of`
                let right = self.allowing_in(|p| p.parse_assignment())?;
                self.expect_punctuator(')')?;
                let body = self.parse_loop_body()?;
                return Ok(Statement::ForOf {
                    left: ForBinding::Declaration {
                        kind,
                        pattern: crate::ast::BindingPattern::Identifier(name),
                    },
                    right,
                    body,
                    is_await,
                });
            }

            let init = self.parse_for_declaration_tail(kind, name)?;
            return self.parse_for_classic_rest(Some(Box::new(init)));
        }

        // Expression head: either a for-in/for-of target or a C-style init expression.
        // `in` is suppressed at the top level so `x in obj` stops at `in`.
        self.no_in = true;
        let expression = self.parse_expression();
        self.no_in = false;
        let expression = expression?;

        if self.check_keyword(Keyword::In) {
            if !matches!(
                expression,
                Expression::Identifier(_) | Expression::Member { .. }
            ) {
                return Err(self.error("invalid left-hand side in for-in loop".into()));
            }
            self.advance(); // `in`
            let right = self.parse_expression()?;
            self.expect_punctuator(')')?;
            let body = self.parse_loop_body()?;
            return Ok(Statement::ForIn {
                left: ForBinding::Target(expression),
                right,
                body,
            });
        }

        // V9-A: `for (expr of right)` �?also accepts Array/Object destructuring targets.
        if self.check_contextual_of() {
            if !matches!(
                expression,
                Expression::Identifier(_)
                    | Expression::Member { .. }
                    | Expression::Array(_)
                    | Expression::Object(_)
            ) {
                return Err(self.error("invalid left-hand side in for-of loop".into()));
            }
            self.advance(); // `of`
            let right = self.allowing_in(|p| p.parse_assignment())?;
            self.expect_punctuator(')')?;
            let body = self.parse_loop_body()?;
            return Ok(Statement::ForOf {
                left: ForBinding::Target(expression),
                right,
                body,
                is_await,
            });
        }

        self.expect_punctuator(';')?;
        self.parse_for_classic_rest(Some(Box::new(Statement::Expression(expression))))
    }

    /// Parses the remaining declarators of a `for` C-style declaration init,
    /// consuming the trailing `;`. `kind`/`first_name` are the already-consumed
    /// declaration keyword and first binding name.
    fn parse_for_declaration_tail(
        &mut self,
        kind: VariableKind,
        first_name: String,
    ) -> Result<Statement, ParseError> {
        let mut declarations = Vec::new();
        let mut name = first_name;
        loop {
            let initializer = if self.eat_operator("=") {
                Some(self.parse_assignment()?)
            } else {
                None
            };
            if kind == VariableKind::Const && initializer.is_none() {
                return Err(self.error("`const` declarations require an initializer".into()));
            }
            declarations.push(VariableDeclarator {
                name,
                pattern: None,
                initializer,
            });
            if !self.eat_punctuator(',') {
                break;
            }
            name = self.expect_identifier()?;
        }
        self.expect_semicolon()?;
        Ok(Statement::VariableDeclaration { kind, declarations })
    }

    /// Parses `test; update) body` after a C-style `for` header's init clause
    /// and its terminating `;` have been consumed.
    fn parse_for_classic_rest(
        &mut self,
        init: Option<Box<Statement>>,
    ) -> Result<Statement, ParseError> {
        let test = if self.check_punctuator(';') {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_punctuator(';')?;

        let update = if self.check_punctuator(')') {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_punctuator(')')?;

        let body = self.parse_loop_body()?;
        Ok(Statement::For {
            init,
            test,
            update,
            body,
        })
    }

    /// Parses a loop body, tracking loop depth so `break`/`continue` are valid.
    fn parse_loop_body(&mut self) -> Result<Box<Statement>, ParseError> {
        self.loop_depth += 1;
        let body = self.parse_substatement("loop");
        self.loop_depth -= 1;
        Ok(Box::new(body?))
    }

    fn parse_substatement(&mut self, context: &str) -> Result<Statement, ParseError> {
        if self.is_strict && self.check_keyword(Keyword::Function) {
            return Err(self.error(format!(
                "function declarations are not allowed as {context} single-statement bodies in strict mode"
            )));
        }
        let stmt = self.parse_statement()?;
        // Spec: IsLabelledFunction must be false for all single-statement bodies.
        // Annex B.3.4 exempts plain function declarations in `if` bodies in sloppy mode,
        // but loop bodies have no such exemption — always reject bare function declarations.
        let is_loop = context == "loop";
        let kind_err = match &stmt {
            Statement::VariableDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                ..
            }
            | Statement::DestructuringDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                ..
            } => Some("lexical declarations"),
            Statement::FunctionDeclaration {
                is_generator: true, ..
            } => Some("generator function declarations"),
            Statement::FunctionDeclaration { is_async: true, .. } => {
                Some("async function declarations")
            }
            Statement::FunctionDeclaration { .. } if is_loop => {
                // Loop bodies: function declarations are always forbidden (no Annex B exception)
                Some("function declarations")
            }
            Statement::ClassDeclaration(_) => Some("class declarations"),
            _ => None,
        };
        if let Some(kind) = kind_err {
            return Err(self.error(format!(
                "{kind} are not allowed as {context} single-statement bodies"
            )));
        }
        // Labelled function declarations are forbidden in all single-statement positions.
        if is_labelled_function(&stmt) {
            return Err(self.error(format!(
                "labelled function declarations are not allowed as {context} single-statement bodies"
            )));
        }
        Ok(stmt)
    }

    /// Parses `break [label];`, rejecting it outside any loop or switch.
    fn parse_break(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `break`
        // Optional label: only if no line terminator before the identifier
        let label = if !self.peek().line_terminator_before {
            if let TokenKind::Identifier(name) = self.peek().kind.clone() {
                self.advance();
                Some(name)
            } else {
                None
            }
        } else {
            None
        };
        match &label {
            None => {
                if self.loop_depth == 0 && self.switch_depth == 0 {
                    return Err(
                        self.error("illegal `break` statement outside of a loop or switch".into())
                    );
                }
            }
            Some(name) => {
                if !self.label_stack.iter().any(|(l, _)| l == name) {
                    return Err(
                        self.error(format!("undefined label `{name}` in `break` statement"))
                    );
                }
            }
        }
        self.expect_semicolon()?;
        Ok(Statement::Break(label))
    }

    /// Parses `continue [label];`, rejecting it outside any loop.
    fn parse_continue(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `continue`
        // Optional label: only if no line terminator before the identifier
        let label = if !self.peek().line_terminator_before {
            if let TokenKind::Identifier(name) = self.peek().kind.clone() {
                self.advance();
                Some(name)
            } else {
                None
            }
        } else {
            None
        };
        match &label {
            None => {
                if self.loop_depth == 0 {
                    return Err(self.error("illegal `continue` statement outside of a loop".into()));
                }
            }
            Some(name) => {
                if !self
                    .label_stack
                    .iter()
                    .any(|(l, is_iter)| l == name && *is_iter)
                {
                    return Err(self.error(format!(
                        "undefined or non-iteration label `{name}` in `continue` statement"
                    )));
                }
            }
        }
        self.expect_semicolon()?;
        Ok(Statement::Continue(label))
    }

    /// Parses `throw expression;`.
    fn parse_throw(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `throw`
        if self.peek().line_terminator_before {
            return Err(self.error("illegal newline after `throw`".into()));
        }
        if matches!(
            self.peek().kind,
            TokenKind::Punctuator(';') | TokenKind::Eof
        ) {
            return Err(self.error("`throw` must be followed by an expression".into()));
        }
        let argument = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(Statement::Throw(argument))
    }

    /// Parses `try` with a catch clause, a finally clause, or both.
    fn parse_try(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `try`
        let block = self.parse_block_statements()?;

        let handler = if self.eat_keyword(Keyword::Catch) {
            let parameter = if self.eat_punctuator('(') {
                let parameter = self.expect_identifier()?;
                self.expect_punctuator(')')?;
                Some(parameter)
            } else {
                None
            };
            let body = self.parse_block_statements()?;
            if let Some(parameter) = &parameter
                && direct_lexical_names(&body)
                    .into_iter()
                    .any(|name| name == parameter)
            {
                return Err(self.error(format!(
                    "catch parameter `{parameter}` conflicts with a lexical declaration"
                )));
            }
            Some(CatchClause { parameter, body })
        } else {
            None
        };

        let finalizer = if self.eat_keyword(Keyword::Finally) {
            Some(self.parse_block_statements()?)
        } else {
            None
        };

        if handler.is_none() && finalizer.is_none() {
            return Err(self.error("`try` requires a `catch` or `finally` clause".into()));
        }

        Ok(Statement::Try {
            block,
            handler,
            finalizer,
        })
    }

    /// Parses a switch statement, preserving case order and fall-through.
    fn parse_switch(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // `switch`
        self.expect_punctuator('(')?;
        let discriminant = self.parse_expression()?;
        self.expect_punctuator(')')?;
        self.expect_punctuator('{')?;

        self.switch_depth += 1;
        let cases = self.parse_switch_cases();
        self.switch_depth -= 1;
        let cases = cases?;
        self.expect_punctuator('}')?;

        Ok(Statement::Switch {
            discriminant,
            cases,
        })
    }

    fn parse_switch_cases(&mut self) -> Result<Vec<SwitchCase>, ParseError> {
        let mut cases = Vec::new();
        let mut saw_default = false;

        while !self.check_punctuator('}') && !self.at_eof() {
            let test = if self.eat_keyword(Keyword::Case) {
                let test = self.parse_expression()?;
                self.expect_punctuator(':')?;
                Some(test)
            } else if self.eat_keyword(Keyword::Default) {
                if saw_default {
                    return Err(self.error("a switch may contain only one `default` clause".into()));
                }
                saw_default = true;
                self.expect_punctuator(':')?;
                None
            } else {
                return Err(self.error("expected `case`, `default`, or `}` in switch".into()));
            };

            let mut consequent = Vec::new();
            while !self.check_punctuator('}')
                && !self.check_keyword(Keyword::Case)
                && !self.check_keyword(Keyword::Default)
                && !self.at_eof()
            {
                consequent.push(self.parse_statement()?);
            }
            cases.push(SwitchCase { test, consequent });
        }

        let mut lexical_names = HashSet::new();
        for name in cases
            .iter()
            .flat_map(|case| direct_lexical_names(&case.consequent))
        {
            if !lexical_names.insert(name) {
                return Err(self.error(format!("duplicate lexical declaration `{name}` in switch")));
            }
        }
        // Spec: SwitchStatement early error — var-declared names must not overlap lexical names.
        for name in cases
            .iter()
            .flat_map(|case| var_declared_names(&case.consequent))
        {
            if lexical_names.contains(name) {
                return Err(self.error(format!(
                    "var `{name}` conflicts with a lexical declaration in switch"
                )));
            }
        }

        Ok(cases)
    }

    pub(super) fn validate_lexical_declarations(
        &self,
        statements: &[Statement],
    ) -> Result<(), ParseError> {
        let mut lexical_names = HashSet::new();
        for name in direct_lexical_names(statements) {
            if !lexical_names.insert(name) {
                return Err(self.error(format!("duplicate lexical declaration `{name}`")));
            }
        }
        for name in var_declared_names(statements) {
            if lexical_names.contains(name) {
                return Err(self.error(format!(
                    "var declaration `{name}` conflicts with a lexical declaration"
                )));
            }
        }
        Ok(())
    }

    pub(super) fn validate_module_declarations(
        &self,
        statements: &[Statement],
    ) -> Result<(), ParseError> {
        let mut local_names: HashSet<String> = direct_lexical_names(statements)
            .into_iter()
            .map(str::to_owned)
            .collect();
        local_names.extend(var_declared_names(statements).into_iter().map(str::to_owned));

        let mut exported_names = HashSet::new();
        for statement in statements {
            if module_item_contains_forbidden_meta(statement) {
                return Err(self.error("`super` and `new.target` are not allowed in module code".into()));
            }
            let Statement::ModuleDeclaration(crate::ast::ModuleDeclaration::Export(decl)) =
                statement
            else {
                continue;
            };

            for entry in &decl.entries {
                if entry.export_name != "*" && !exported_names.insert(entry.export_name.as_str()) {
                    return Err(self.error(format!(
                        "duplicate export name `{}`",
                        entry.export_name
                    )));
                }
                if decl.source.is_none()
                    && let Some(local_name) = &entry.local_name
                    && !local_names.contains(local_name)
                {
                    return Err(self.error(format!(
                        "exported binding `{local_name}` is not declared in this module"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validates that no formal parameter name is also declared with `let`/`const`
    /// in the function body (the BoundNames/LexicallyDeclaredNames early error).
    pub(super) fn validate_params_vs_lexical(
        &self,
        params: &[FunctionParam],
        body_statements: &[Statement],
    ) -> Result<(), ParseError> {
        let lexical: HashSet<&str> = direct_lexical_names(body_statements).into_iter().collect();
        for p in params {
            let name = p.name();
            if !name.is_empty() && lexical.contains(name) {
                return Err(self.error(format!(
                    "parameter `{name}` conflicts with a lexical declaration in the function body"
                )));
            }
        }
        Ok(())
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        if self.at_eof() {
            return Err(self.error(format!(
                "expected a statement but found {}",
                describe(&self.peek().kind)
            )));
        }
        let expression = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(Statement::Expression(expression))
    }
}

/// Returns true if `stmt` is a LabelledStatement that (transitively) wraps a
/// FunctionDeclaration. Annex B allows bare function declarations in sloppy mode
/// single-statement bodies; however the LabelledFunction early error (IsLabelledFunction
/// via a label) applies regardless of mode.
fn is_labelled_function(stmt: &Statement) -> bool {
    match stmt {
        Statement::Labelled { body, .. } => {
            matches!(body.as_ref(), Statement::FunctionDeclaration { .. })
                || is_labelled_function(body)
        }
        _ => false,
    }
}

fn module_item_contains_forbidden_meta(statement: &Statement) -> bool {
    match statement {
        Statement::Expression(expr) | Statement::Throw(expr) => expr_contains_forbidden_meta(expr),
        Statement::Block(statements) => statements.iter().any(module_item_contains_forbidden_meta),
        Statement::If {
            test,
            consequent,
            alternate,
        } => {
            expr_contains_forbidden_meta(test)
                || module_item_contains_forbidden_meta(consequent)
                || alternate
                    .as_deref()
                    .is_some_and(module_item_contains_forbidden_meta)
        }
        Statement::While { test, body } | Statement::DoWhile { test, body } => {
            expr_contains_forbidden_meta(test) || module_item_contains_forbidden_meta(body)
        }
        Statement::For {
            init,
            test,
            update,
            body,
        } => {
            init.as_deref()
                .is_some_and(module_item_contains_forbidden_meta)
                || test.as_ref().is_some_and(expr_contains_forbidden_meta)
                || update.as_ref().is_some_and(expr_contains_forbidden_meta)
                || module_item_contains_forbidden_meta(body)
        }
        Statement::ForIn { right, body, .. } | Statement::ForOf { right, body, .. } => {
            expr_contains_forbidden_meta(right) || module_item_contains_forbidden_meta(body)
        }
        Statement::Labelled { body, .. } => module_item_contains_forbidden_meta(body),
        Statement::Try {
            block,
            handler,
            finalizer,
        } => {
            block.iter().any(module_item_contains_forbidden_meta)
                || handler
                    .as_ref()
                    .is_some_and(|handler| handler.body.iter().any(module_item_contains_forbidden_meta))
                || finalizer
                    .as_ref()
                    .is_some_and(|body| body.iter().any(module_item_contains_forbidden_meta))
        }
        Statement::Switch {
            discriminant,
            cases,
        } => {
            expr_contains_forbidden_meta(discriminant)
                || cases
                    .iter()
                    .flat_map(|case| &case.consequent)
                    .any(module_item_contains_forbidden_meta)
        }
        Statement::VariableDeclaration { declarations, .. } => declarations
            .iter()
            .filter_map(|declaration| declaration.initializer.as_ref())
            .any(expr_contains_forbidden_meta),
        Statement::DestructuringDeclaration { initializer, .. } => {
            expr_contains_forbidden_meta(initializer)
        }
        Statement::ModuleDeclaration(crate::ast::ModuleDeclaration::Export(decl)) => decl
            .declaration
            .as_deref()
            .is_some_and(module_item_contains_forbidden_meta),
        Statement::Empty
        | Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Return(_)
        | Statement::FunctionDeclaration { .. }
        | Statement::ClassDeclaration(_)
        | Statement::ModuleDeclaration(crate::ast::ModuleDeclaration::Import(_)) => false,
    }
}

fn expr_contains_forbidden_meta(expr: &Expression) -> bool {
    match expr {
        Expression::Super | Expression::NewTarget => true,
        Expression::Unary { argument, .. } | Expression::Update { argument, .. } => {
            expr_contains_forbidden_meta(argument)
        }
        Expression::Binary { left, right, .. }
        | Expression::Logical { left, right, .. }
        | Expression::Assignment {
            target: left,
            value: right,
        }
        | Expression::CompoundAssignment {
            target: left,
            value: right,
            ..
        }
        | Expression::Member {
            object: left,
            property: right,
            ..
        } => expr_contains_forbidden_meta(left) || expr_contains_forbidden_meta(right),
        Expression::Call { callee, arguments } | Expression::Construct { callee, arguments } => {
            expr_contains_forbidden_meta(callee)
                || arguments.iter().any(call_arg_contains_forbidden_meta)
        }
        Expression::Conditional {
            test,
            consequent,
            alternate,
        } => {
            expr_contains_forbidden_meta(test)
                || expr_contains_forbidden_meta(consequent)
                || expr_contains_forbidden_meta(alternate)
        }
        Expression::Array(elements) => elements.iter().any(|element| match element {
            crate::ast::ArrayElement::Expression(expr) | crate::ast::ArrayElement::Spread(expr) => {
                expr_contains_forbidden_meta(expr)
            }
            crate::ast::ArrayElement::Hole => false,
        }),
        Expression::Object(properties) => properties.iter().any(|property| match property {
            ObjectProperty::Data { value, .. } | ObjectProperty::PrototypeSetter { value } => {
                expr_contains_forbidden_meta(value)
            }
            ObjectProperty::ComputedData { key, value } => {
                expr_contains_forbidden_meta(key) || expr_contains_forbidden_meta(value)
            }
            ObjectProperty::Spread(expr) => expr_contains_forbidden_meta(expr),
            ObjectProperty::Getter { .. } | ObjectProperty::Setter { .. } => false,
        }),
        Expression::TemplateLiteral(template) => {
            template.expressions.iter().any(expr_contains_forbidden_meta)
        }
        Expression::Spread(expr) | Expression::Await(expr) => expr_contains_forbidden_meta(expr),
        Expression::Yield { argument, .. } => argument
            .as_deref()
            .is_some_and(expr_contains_forbidden_meta),
        Expression::Sequence(expressions) => expressions.iter().any(expr_contains_forbidden_meta),
        Expression::Literal(_)
        | Expression::Identifier(_)
        | Expression::Function(_)
        | Expression::Class(_)
        | Expression::This
        | Expression::PrivateName(_) => false,
    }
}

fn call_arg_contains_forbidden_meta(arg: &crate::ast::CallArgument) -> bool {
    match arg {
        crate::ast::CallArgument::Expression(expr) | crate::ast::CallArgument::Spread(expr) => {
            expr_contains_forbidden_meta(expr)
        }
    }
}

fn is_identifier_like(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_alphanumeric())
}

fn direct_lexical_names(statements: &[Statement]) -> Vec<&str> {
    statements
        .iter()
        .flat_map(|statement| match statement {
            Statement::VariableDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                declarations,
            } => declarations
                .iter()
                .map(|declaration| declaration.name.as_str())
                .collect::<Vec<_>>(),
            Statement::DestructuringDeclaration {
                kind: VariableKind::Let | VariableKind::Const,
                pattern,
                ..
            } => binding_pattern_name_strs(pattern),
            Statement::FunctionDeclaration { name, .. } => vec![name.as_str()],
            Statement::ClassDeclaration(cls) => vec![cls.name.as_str()],
            Statement::ModuleDeclaration(crate::ast::ModuleDeclaration::Import(decl)) => decl
                .entries
                .iter()
                .map(|entry| entry.local_name.as_str())
                .collect(),
            Statement::ModuleDeclaration(crate::ast::ModuleDeclaration::Export(decl)) => decl
                .declaration
                .as_deref()
                .map(|statement| direct_lexical_names(std::slice::from_ref(statement)))
                .unwrap_or_default(),
            _ => Vec::new(),
        })
        .collect()
}

fn var_declared_names(statements: &[Statement]) -> Vec<&str> {
    let mut names = Vec::new();
    for statement in statements {
        collect_var_declared_names(statement, &mut names);
    }
    names
}

fn collect_var_declared_names<'a>(statement: &'a Statement, names: &mut Vec<&'a str>) {
    match statement {
        Statement::VariableDeclaration {
            kind: VariableKind::Var,
            declarations,
        } => names.extend(
            declarations
                .iter()
                .map(|declaration| declaration.name.as_str()),
        ),
        Statement::Block(statements) => {
            names.extend(var_declared_names(statements));
        }
        Statement::If {
            consequent,
            alternate,
            ..
        } => {
            collect_var_declared_names(consequent, names);
            if let Some(alternate) = alternate {
                collect_var_declared_names(alternate, names);
            }
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_var_declared_names(body, names)
        }
        Statement::Labelled { body, .. } => collect_var_declared_names(body, names),
        Statement::Try {
            block,
            handler,
            finalizer,
        } => {
            names.extend(var_declared_names(block));
            if let Some(handler) = handler {
                names.extend(var_declared_names(&handler.body));
            }
            if let Some(finalizer) = finalizer {
                names.extend(var_declared_names(finalizer));
            }
        }
        Statement::Switch { cases, .. } => {
            for case in cases {
                names.extend(var_declared_names(&case.consequent));
            }
        }
        Statement::For { init, body, .. } => {
            if let Some(init) = init {
                collect_var_declared_names(init, names);
            }
            collect_var_declared_names(body, names);
        }
        Statement::ForIn {
            left:
                ForBinding::Declaration {
                    kind: VariableKind::Var,
                    pattern: crate::ast::BindingPattern::Identifier(name),
                },
            body,
            ..
        } => {
            names.push(name.as_str());
            collect_var_declared_names(body, names);
        }
        Statement::ForIn { body, .. } => collect_var_declared_names(body, names),
        // V9-A: for-of with a `var` binding declares the name.
        Statement::ForOf {
            left:
                ForBinding::Declaration {
                    kind: VariableKind::Var,
                    pattern: crate::ast::BindingPattern::Identifier(name),
                },
            body,
            ..
        } => {
            names.push(name.as_str());
            collect_var_declared_names(body, names);
        }
        Statement::ForOf { body, .. } => collect_var_declared_names(body, names),
        Statement::ModuleDeclaration(crate::ast::ModuleDeclaration::Export(decl)) => {
            if let Some(statement) = decl.declaration.as_deref() {
                collect_var_declared_names(statement, names);
            }
        }
        Statement::ModuleDeclaration(crate::ast::ModuleDeclaration::Import(_)) => {}
        Statement::FunctionDeclaration { .. }
        | Statement::Empty
        | Statement::Expression(_)
        | Statement::Return(_)
        | Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Throw(_)
        | Statement::VariableDeclaration { .. }
        | Statement::ClassDeclaration(_)
        | Statement::DestructuringDeclaration { .. } => {}
    }
}

fn add_export_decl_names(statement: &Statement, entries: &mut Vec<crate::ast::ExportEntry>) {
    let mut names = Vec::new();
    match statement {
        Statement::VariableDeclaration { declarations, .. } => {
            for declaration in declarations {
                if let Some(pattern) = &declaration.pattern {
                    collect_binding_pattern_names(pattern, &mut names);
                } else {
                    names.push(declaration.name.clone());
                }
            }
        }
        Statement::DestructuringDeclaration { pattern, .. } => {
            collect_binding_pattern_names(pattern, &mut names);
        }
        Statement::FunctionDeclaration { name, .. } => names.push(name.clone()),
        Statement::ClassDeclaration(decl) => names.push(decl.name.clone()),
        _ => {}
    }
    entries.extend(names.into_iter().map(|name| crate::ast::ExportEntry {
        export_name: name.clone(),
        local_name: Some(name),
    }));
}

fn export_default_decl_name(statement: &Statement) -> Option<String> {
    match statement {
        Statement::FunctionDeclaration { name, .. } | Statement::ClassDeclaration(crate::ast::ClassDeclaration { name, .. })
            if name != "*default*" =>
        {
            Some(name.clone())
        }
        _ => None,
    }
}

/// Collects all bound identifiers from a function parameter, including names
/// nested inside destructuring patterns. Used by `check_duplicate_params`.
fn collect_param_bound_names(param: &FunctionParam, names: &mut Vec<String>) {
    match param {
        FunctionParam::Simple(name) | FunctionParam::Rest(name) => {
            names.push(name.clone());
        }
        FunctionParam::Default(name, _) => {
            names.push(name.clone());
        }
        FunctionParam::Pattern(pattern, _) => {
            collect_binding_pattern_names(pattern, names);
        }
        FunctionParam::RestPattern(pattern) => {
            collect_binding_pattern_names(pattern, names);
        }
    }
}

fn binding_pattern_name_strs(pattern: &crate::ast::BindingPattern) -> Vec<&str> {
    use crate::ast::BindingPattern;
    match pattern {
        BindingPattern::Identifier(name) => vec![name.as_str()],
        BindingPattern::Array { elements, rest } => {
            let mut names: Vec<&str> = elements
                .iter()
                .flatten()
                .flat_map(|elem| binding_pattern_name_strs(&elem.pattern))
                .collect();
            if let Some(rest_pat) = rest {
                names.extend(binding_pattern_name_strs(rest_pat));
            }
            names
        }
        BindingPattern::Object { props, rest } => {
            let mut names: Vec<&str> = props
                .iter()
                .flat_map(|prop| binding_pattern_name_strs(&prop.value))
                .collect();
            if let Some(rest_pat) = rest {
                names.extend(binding_pattern_name_strs(rest_pat));
            }
            names
        }
    }
}

fn collect_binding_pattern_names(pattern: &crate::ast::BindingPattern, names: &mut Vec<String>) {
    use crate::ast::BindingPattern;
    match pattern {
        BindingPattern::Identifier(name) => {
            names.push(name.clone());
        }
        BindingPattern::Array { elements, rest } => {
            for elem in elements.iter().flatten() {
                collect_binding_pattern_names(&elem.pattern, names);
            }
            if let Some(rest_pat) = rest {
                collect_binding_pattern_names(rest_pat, names);
            }
        }
        BindingPattern::Object { props, rest } => {
            for prop in props {
                collect_binding_pattern_names(&prop.value, names);
            }
            if let Some(rest_pat) = rest {
                collect_binding_pattern_names(rest_pat, names);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{
            Expression, FunctionBody, FunctionParam, Literal, Statement, VariableDeclarator,
            VariableKind,
        },
        lexer::Lexer,
        parser::Parser,
    };

    fn parse(source: &str) -> Vec<Statement> {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        Parser::new(tokens)
            .parse_program()
            .expect("parsing succeeds")
            .body
    }

    fn parse_error(source: &str) -> crate::parser::ParseError {
        let tokens = Lexer::new(source).tokenize().expect("lexing succeeds");
        Parser::new(tokens)
            .parse_program()
            .expect_err("parsing fails")
    }

    fn declarator(name: &str, initializer: Option<Expression>) -> VariableDeclarator {
        VariableDeclarator {
            name: name.into(),
            pattern: None,
            initializer,
        }
    }

    fn param(name: &str) -> FunctionParam {
        FunctionParam::Simple(name.into())
    }

    fn body(statements: Vec<Statement>) -> FunctionBody {
        FunctionBody {
            statements,
            is_strict: false,
        }
    }

    #[test]
    fn parses_empty_statement() {
        assert_eq!(parse(";"), [Statement::Empty]);
    }

    #[test]
    fn parses_var_without_initializer() {
        assert_eq!(
            parse("var x;"),
            [Statement::VariableDeclaration {
                kind: VariableKind::Var,
                declarations: vec![declarator("x", None)],
            }]
        );
    }

    #[test]
    fn parses_var_with_initializer() {
        assert_eq!(
            parse("var x = 1;"),
            [Statement::VariableDeclaration {
                kind: VariableKind::Var,
                declarations: vec![declarator(
                    "x",
                    Some(Expression::Literal(Literal::Number(1.0))),
                )],
            }]
        );
    }

    #[test]
    fn parses_multiple_declarators() {
        assert_eq!(
            parse("var a, b = 1;"),
            [Statement::VariableDeclaration {
                kind: VariableKind::Var,
                declarations: vec![
                    declarator("a", None),
                    declarator("b", Some(Expression::Literal(Literal::Number(1.0)))),
                ],
            }]
        );
    }

    #[test]
    fn parses_block_with_statements() {
        assert_eq!(
            parse("{ ; 1; }"),
            [Statement::Block(vec![
                Statement::Empty,
                Statement::Expression(Expression::Literal(Literal::Number(1.0))),
            ])]
        );
    }

    #[test]
    fn dangling_else_binds_to_nearest_if() {
        let body = parse("if (1) if (2) 3; else 4;");
        let Statement::If {
            consequent,
            alternate,
            ..
        } = &body[0]
        else {
            panic!("expected an if statement");
        };
        assert!(alternate.is_none());
        assert!(matches!(
            consequent.as_ref(),
            Statement::If {
                alternate: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn parses_while_with_break_and_continue() {
        let body = parse("while (1) { break; continue; }");
        let Statement::While { body, .. } = &body[0] else {
            panic!("expected a while statement");
        };
        assert_eq!(
            body.as_ref(),
            &Statement::Block(vec![Statement::Break(None), Statement::Continue(None)])
        );
    }

    #[test]
    fn rejects_break_outside_loop() {
        assert!(parse_error("break;").message.contains("break"));
    }

    #[test]
    fn rejects_continue_outside_loop() {
        assert!(parse_error("continue;").message.contains("continue"));
    }

    #[test]
    fn parses_throw_statement() {
        assert_eq!(
            parse("throw 1;"),
            [Statement::Throw(Expression::Literal(Literal::Number(1.0)))]
        );
    }

    #[test]
    fn rejects_newline_between_throw_and_expression() {
        assert!(parse_error("throw\n1;").message.contains("throw"));
    }

    #[test]
    fn allows_trailing_statement_without_semicolon() {
        assert_eq!(
            parse("1"),
            [Statement::Expression(Expression::Literal(Literal::Number(
                1.0
            )))]
        );
    }

    #[test]
    fn requires_separator_between_statements() {
        let tokens = Lexer::new("1 2").tokenize().unwrap();
        assert!(Parser::new(tokens).parse_program().is_err());
    }

    // -----------------------------------------------------------------------
    // V3 function declaration tests
    // -----------------------------------------------------------------------

    #[test]
    fn parses_function_declaration_no_params() {
        let stmts = parse("function f() { }");
        assert_eq!(
            stmts,
            [Statement::FunctionDeclaration {
                name: "f".into(),
                params: vec![],
                body: body(vec![]),
                is_async: false,
                is_generator: false,
            }]
        );
    }

    #[test]
    fn parses_function_declaration_with_params_and_return() {
        let stmts = parse("function add(a, b) { return a + b; }");
        let Statement::FunctionDeclaration {
            name,
            params,
            body: fn_body,
            ..
        } = &stmts[0]
        else {
            panic!("expected FunctionDeclaration");
        };
        assert_eq!(name, "add");
        assert_eq!(params, &[param("a"), param("b")]);
        assert_eq!(fn_body.statements.len(), 1);
        assert!(matches!(fn_body.statements[0], Statement::Return(Some(_))));
    }

    #[test]
    fn parses_return_without_value() {
        let stmts = parse("function f() { return; }");
        let Statement::FunctionDeclaration { body: fn_body, .. } = &stmts[0] else {
            panic!();
        };
        assert_eq!(fn_body.statements, [Statement::Return(None)]);
    }

    #[test]
    fn parses_return_with_line_terminator_as_empty_return() {
        // `return\n1` should parse as `return;` then `1;` (restricted production)
        let stmts = parse("function f() { return\n1; }");
        let Statement::FunctionDeclaration { body: fn_body, .. } = &stmts[0] else {
            panic!();
        };
        assert_eq!(fn_body.statements.len(), 2);
        assert_eq!(fn_body.statements[0], Statement::Return(None));
    }

    #[test]
    fn rejects_return_outside_function() {
        let err = parse_error("return 1;");
        assert!(err.message.contains("return"));
    }

    #[test]
    fn rejects_missing_function_name() {
        // anonymous function at statement level is not a valid declaration
        assert!(!parse_error("function () {}").message.is_empty());
    }

    #[test]
    fn rejects_missing_function_body_brace() {
        assert!(!parse_error("function f()").message.is_empty());
    }

    #[test]
    fn parses_v5_try_and_switch_statements() {
        let statements = parse(
            "try { throw 1; } catch (error) { error; } finally { true; } \
             switch (value) { case 1: break; default: value; }",
        );
        assert!(matches!(statements[0], Statement::Try { .. }));
        assert!(matches!(statements[1], Statement::Switch { .. }));
    }

    #[test]
    fn rejects_try_without_handler_and_duplicate_switch_default() {
        assert!(parse_error("try {}").message.contains("catch"));
        assert!(
            parse_error("switch (x) { default: ; default: ; }")
                .message
                .contains("default")
        );
    }

    #[test]
    fn rejects_duplicate_lexical_declarations_in_same_scope() {
        assert!(parse_error("let x; let x;").message.contains("duplicate"));
        assert!(
            parse_error("{ const f = 0; function f() {} }")
                .message
                .contains("duplicate")
        );
        assert!(
            parse_error("function f() {} function f() {}")
                .message
                .contains("duplicate")
        );
    }

    #[test]
    fn rejects_var_declarations_conflicting_with_lexical_names() {
        assert!(
            parse_error("const x = 1; var x;")
                .message
                .contains("conflicts")
        );
        assert!(
            parse_error("{ let x; { var x; } }")
                .message
                .contains("conflicts")
        );
        assert!(
            parse_error("let item; for (var item in object) { ; }")
                .message
                .contains("conflicts")
        );
    }

    #[test]
    fn strict_mode_rejects_function_declaration_as_single_statement_body() {
        for source in [
            r#""use strict"; if (x) function f() {}"#,
            r#""use strict"; if (x) ; else function f() {}"#,
            r#""use strict"; while (x) function f() {}"#,
            r#""use strict"; for (;;) function f() {}"#,
        ] {
            assert!(
                parse_error(source).message.contains("single-statement"),
                "{source} should reject function declarations in strict substatements"
            );
        }
    }

    #[test]
    fn sloppy_mode_and_strict_blocks_allow_function_declaration_bodies() {
        parse("if (x) function f() {}");
        parse(r#""use strict"; if (x) { function f() {} }"#);
        parse(r#""use strict"; for (;;) { function f() {} break; }"#);
    }

    #[test]
    fn parses_nested_function_declarations() {
        let stmts =
            parse("function outer(x) { function inner(y) { return x + y; } return inner(2); }");
        let Statement::FunctionDeclaration {
            body: outer_body, ..
        } = &stmts[0]
        else {
            panic!();
        };
        assert_eq!(outer_body.statements.len(), 2);
        assert!(matches!(
            outer_body.statements[0],
            Statement::FunctionDeclaration { .. }
        ));
    }
}
