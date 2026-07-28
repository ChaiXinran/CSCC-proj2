//! `Function` constructor and prototype methods.

use crate::{
    bytecode::{Chunk, Compiler, Instruction},
    lexer::Lexer,
    parser::Parser,
    runtime::{JsFunction, JsValue, NativeContext, PropertyDescriptor, PropertyKind},
    vm::{Vm, VmError},
};

pub fn install_function(context: &mut NativeContext) {
    let Some(function_prototype) = context
        .intrinsics()
        .map(|intrinsics| intrinsics.function_prototype)
    else {
        return;
    };
    let throw_type_error = context
        .register_builtin("ThrowTypeError", 0, function_restricted_thrower, None)
        .expect("install %ThrowTypeError%");
    let legacy_caller_getter = context
        .register_builtin("get caller", 0, function_legacy_caller_get, None)
        .expect("install Function caller getter");
    let legacy_arguments_getter = context
        .register_builtin("get arguments", 0, function_legacy_arguments_get, None)
        .expect("install Function arguments getter");
    let legacy_setter = context
        .register_builtin("set caller/arguments", 1, function_legacy_noop_set, None)
        .expect("install Function caller/arguments setter");
    context.set_function_restricted_thrower(throw_type_error.clone());
    context.set_function_legacy_accessors(
        legacy_caller_getter,
        legacy_arguments_getter,
        legacy_setter,
    );
    for name in ["caller", "arguments"] {
        context
            .define_own_property(
                function_prototype,
                name.into(),
                PropertyDescriptor::accessor(
                    Some(throw_type_error.clone()),
                    Some(throw_type_error.clone()),
                    false,
                    true,
                ),
            )
            .expect("define restricted Function.prototype property");
    }

    let call = context
        .register_builtin("call", 1, function_prototype_call, None)
        .expect("install Function.prototype.call");
    if let JsValue::BuiltinFunction(id) = &call {
        context.set_function_prototype_call(*id);
    }
    context
        .define_own_property(
            function_prototype,
            "call".into(),
            PropertyDescriptor::data_with(call, true, false, true),
        )
        .expect("define Function.prototype.call");

    let apply = context
        .register_builtin("apply", 2, function_prototype_apply, None)
        .expect("install Function.prototype.apply");
    if let JsValue::BuiltinFunction(id) = &apply {
        context.set_function_prototype_apply(*id);
    }
    context
        .define_own_property(
            function_prototype,
            "apply".into(),
            PropertyDescriptor::data_with(apply, true, false, true),
        )
        .expect("define Function.prototype.apply");

    let bind = context
        .register_builtin("bind", 1, function_prototype_bind, None)
        .expect("install Function.prototype.bind");
    context
        .define_own_property(
            function_prototype,
            "bind".into(),
            PropertyDescriptor::data_with(bind, true, false, true),
        )
        .expect("define Function.prototype.bind");

    let to_string = context
        .register_builtin("toString", 0, function_prototype_to_string, None)
        .expect("install Function.prototype.toString");
    context
        .define_own_property(
            function_prototype,
            "toString".into(),
            PropertyDescriptor::data_with(to_string, true, false, true),
        )
        .expect("define Function.prototype.toString");

    let has_instance = context
        .register_builtin(
            "[Symbol.hasInstance]",
            1,
            function_prototype_has_instance,
            None,
        )
        .expect("install Function.prototype[Symbol.hasInstance]");
    let symbol = context.well_known_symbols().has_instance;
    context
        .define_symbol_own_property(
            function_prototype,
            symbol,
            PropertyDescriptor::data_with(has_instance, false, false, false),
        )
        .expect("define Function.prototype[Symbol.hasInstance]");
}

pub fn eval_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::String(source) => source,
        other => return Ok(other),
    };
    // Direct eval inherits strictness from the calling execution context.
    // The parser derives strict mode from the directive prologue, so make
    // inherited strictness explicit before parsing (this also enforces the
    // strict-only early errors for eval/arguments catch bindings).
    let is_strict = context.is_strict_code();
    let strict_source;
    let parse_source: &str = if is_strict {
        strict_source = format!("\"use strict\";\n{source}");
        &strict_source
    } else {
        &source
    };
    let tokens = Lexer::new(parse_source)
        .tokenize()
        .map_err(dynamic_function_syntax_error)?;
    let program = Parser::with_source(tokens, parse_source)
        .parse_program()
        .map_err(dynamic_function_syntax_error)?;
    // EvalDeclarationInstantiation §18.2.1.1: check for binding conflicts
    // between the eval code's declarations and the calling context's scope.
    // Strict eval has its own scope, so we only validate non-strict eval.
    // When the calling context is non-strict but the eval source itself starts
    // with "use strict", the eval also gets its own scope (no conflict check).
    let eval_source_is_strict = source.trim_start().starts_with("\"use strict\"")
        || source.trim_start().starts_with("'use strict'");
    if !is_strict && !eval_source_is_strict {
        validate_eval_declarations(&program, context)?;
    }
    // Annex B §B.3.3.3: for non-strict direct eval in a function scope,
    // pre-declare block-contained function names in the calling context's
    // var environment. This creates mutable bindings initialized to undefined
    // before any eval code runs, matching the spec's pre-hoisting semantics.
    let in_function_scope = context.current_environment() != context.global_environment();
    if !is_strict && !eval_source_is_strict && in_function_scope {
        predeclare_eval_annex_b_functions(&program, context)?;
    }
    let mut chunk = Compiler::new()
        .compile_program(&program)
        .map_err(dynamic_function_syntax_error)?;
    if in_function_scope {
        rewrite_eval_global_accesses(&mut chunk);
    }
    vm.eval_execute(&chunk, context)
}

/// Annex B §B.3.3.3: pre-declares block-contained function names in the
/// calling context's var environment. Only names that do NOT conflict with
/// lexical bindings either (a) within the eval body or (b) in the outer
/// runtime scope are pre-declared.
fn predeclare_eval_annex_b_functions(
    program: &crate::ast::Program,
    context: &mut NativeContext,
) -> Result<(), VmError> {
    let top_lex = crate::bytecode::compiler::collect_top_level_lexical_names(&program.body);
    let mut annex_b_names: Vec<String> = Vec::new();
    crate::bytecode::compiler::collect_annex_b_var_names(
        &program.body,
        &top_lex,
        &std::collections::HashSet::new(),
        &mut annex_b_names,
    );
    for name in &annex_b_names {
        // Skip if already bound in the calling scope (either var or lexical).
        if context.find_binding_kind(name).is_some() {
            continue;
        }
        // Create a mutable var-scoped binding, initialized to undefined.
        context.declare_binding(
            context.current_environment(),
            name.clone(),
            JsValue::Undefined,
            true,  // mutable
            false, // var-scoped (not lexical)
        )?;
    }
    Ok(())
}

/// EvalDeclarationInstantiation §18.2.1.1 binding-conflict checks.
/// In non-strict direct eval:
///   - Var/function declarations conflict with LEXICAL bindings in the
///     enclosing scope chain (var declarations can't override let/const/
///     class/param bindings).
///   - Lexical declarations conflict with any binding in the CURRENT
///     lexical environment only.
///
/// Indirect eval (§18.2.1.2) always uses the global scope as its outer
/// and does not check the calling scope; we approximate this by skipping
/// the check when already at global scope.
fn validate_eval_declarations(
    program: &crate::ast::Program,
    context: &mut NativeContext,
) -> Result<(), VmError> {
    use std::collections::HashSet;

    // Only validate when eval scope might conflict with calling scope.
    // At global scope, var re-declarations are always OK (they create
    // configurable properties) and lexical declarations go to the global
    // lexical environment.
    if context.current_environment() == context.global_environment() {
        return Ok(());
    }

    let mut var_names = HashSet::new();
    let mut lex_names = HashSet::new();
    collect_eval_declaration_names(&program.body, &mut var_names, &mut lex_names);

    // Var/function declarations: SyntaxError if a LEXICAL binding with the
    // same name exists anywhere in the enclosing scope chain (§18.2.1.1
    // step 5.d.ii).
    for name in &var_names {
        if context.find_binding_kind(name) == Some(true) {
            return Err(VmError::syntax_error(format!(
                "Identifier '{name}' has already been declared"
            )));
        }
    }

    // Lexical declarations: SyntaxError if any binding with the same name
    // already exists in the CURRENT lexical environment (§18.2.1.1
    // step 11).
    for name in &lex_names {
        if context.has_binding_in_environment(context.current_environment(), name) {
            return Err(VmError::syntax_error(format!(
                "Identifier '{name}' has already been declared"
            )));
        }
    }

    Ok(())
}

/// Walks `statements` recursively to collect var/function-declared names and
/// let/const/class-declared names. Only visits block/statement bodies directly
/// nested under the eval body — nested function bodies have their own scope
/// and are not checked.
fn collect_eval_declaration_names(
    statements: &[crate::ast::Statement],
    var_names: &mut std::collections::HashSet<String>,
    lex_names: &mut std::collections::HashSet<String>,
) {
    use crate::ast::{Statement, VariableKind};
    for stmt in statements {
        match stmt {
            Statement::VariableDeclaration { kind, declarations } => {
                for decl in declarations {
                    if decl.pattern.is_none() {
                        match kind {
                            VariableKind::Var => {
                                var_names.insert(decl.name.clone());
                            }
                            VariableKind::Let | VariableKind::Const => {
                                lex_names.insert(decl.name.clone());
                            }
                        }
                    }
                }
            }
            Statement::FunctionDeclaration { name, .. } => {
                var_names.insert(name.clone());
            }
            Statement::ClassDeclaration(declaration) => {
                lex_names.insert(declaration.name.clone());
            }
            // Recurse into control-flow bodies (same eval scope).
            // Skip nested function bodies — they introduce their own scope.
            Statement::Block(body) => {
                collect_eval_declaration_names(body, var_names, lex_names);
            }
            Statement::Labelled { body, .. } => {
                // Only recurse if the labelled body is a block.
                if let Statement::Block(inner) = body.as_ref() {
                    collect_eval_declaration_names(inner, var_names, lex_names);
                }
            }
            Statement::If {
                consequent,
                alternate,
                ..
            } => {
                if let Statement::Block(inner) = consequent.as_ref() {
                    collect_eval_declaration_names(inner, var_names, lex_names);
                }
                if let Some(alt) = alternate
                    && let Statement::Block(inner) = alt.as_ref()
                {
                    collect_eval_declaration_names(inner, var_names, lex_names);
                }
            }
            Statement::While { body, .. }
            | Statement::DoWhile { body, .. }
            | Statement::With { body, .. } => {
                if let Statement::Block(inner) = body.as_ref() {
                    collect_eval_declaration_names(inner, var_names, lex_names);
                }
            }
            Statement::For { body, .. }
            | Statement::ForIn { body, .. }
            | Statement::ForOf { body, .. } => {
                if let Statement::Block(inner) = body.as_ref() {
                    collect_eval_declaration_names(inner, var_names, lex_names);
                }
            }
            Statement::Try {
                block,
                handler,
                finalizer,
                ..
            } => {
                collect_eval_declaration_names(block, var_names, lex_names);
                if let Some(handler) = handler {
                    collect_eval_declaration_names(&handler.body, var_names, lex_names);
                }
                if let Some(finalizer) = finalizer {
                    collect_eval_declaration_names(finalizer, var_names, lex_names);
                }
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    collect_eval_declaration_names(&case.consequent, var_names, lex_names);
                }
            }
            // Ignore: Expression, Return, Break, Continue, Throw, Empty,
            // FunctionDeclaration (nested scope), ClassDeclaration (already
            // collected above), DestructuringDeclaration.
            _ => {}
        }
    }
}

fn rewrite_eval_global_accesses(chunk: &mut Chunk) {
    for instruction in &mut chunk.instructions {
        *instruction = match *instruction {
            Instruction::DeclareGlobal(index) => Instruction::DeclareLocal(index),
            Instruction::LoadGlobal(index) => Instruction::LoadName(index),
            Instruction::StoreGlobal(index) => Instruction::StoreName(index),
            Instruction::TypeOfGlobal(index) => Instruction::TypeOfName(index),
            other => other,
        };
    }
}

pub fn function_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    create_dynamic_function(vm, context, arguments)
}

pub fn function_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    create_dynamic_function(vm, context, arguments)
}

fn create_dynamic_function(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (params, body) = dynamic_function_source_parts(vm, context, arguments)?;
    // Per spec §19.2.1.1.1, the closing ')' is always on a new line so that HTML
    // comments at the end of the params string don't consume it.
    let source = format!("(function anonymous({params}\n) {{\n{body}\n}})");
    let tokens = Lexer::new(&source)
        .tokenize()
        .map_err(dynamic_function_syntax_error)?;
    let program = Parser::with_source(tokens, &source)
        .parse_program()
        .map_err(dynamic_function_syntax_error)?;
    let chunk = Compiler::new()
        .compile_program(&program)
        .map_err(dynamic_function_syntax_error)?;
    let template =
        chunk.functions.first().cloned().ok_or_else(|| {
            VmError::syntax_error("dynamic Function did not compile to a function")
        })?;
    let is_strict = template.is_strict;
    let uses_arguments = template.uses_arguments;
    let id = context.allocate_function(JsFunction {
        name: template.name.or_else(|| Some("anonymous".into())),
        params: template.params,
        rest_param: template.rest_param,
        length_override: template.length_override,
        chunk: template.chunk,
        // Dynamic Function is created in the global scope; it must not capture
        // the caller's local lexical environment.
        environment: Some(context.global_environment()),
        is_async: false,
        is_generator: false,
        is_arrow: false,
        binds_name_in_activation: false,
        is_derived_constructor: false,
        is_constructable: true,
        has_own_prototype_property: true,
        prototype_writable: true,
        uses_arguments,
        lexical_this: None,
        lexical_new_target: None,
        home_object: None,
    })?;
    if is_strict {
        context.mark_strict_function(id);
        context.remove_own_function_legacy_properties(id)?;
    }
    Ok(JsValue::Function(id))
}

fn function_restricted_thrower(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "caller and arguments are restricted function properties",
    ))
}

fn function_legacy_caller_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.legacy_function_caller(&this)
}

fn function_legacy_arguments_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if matches!(this, JsValue::Function(_) | JsValue::BuiltinFunction(_)) {
        Ok(JsValue::Null)
    } else {
        Err(VmError::type_error(
            "Function arguments getter receiver is not callable",
        ))
    }
}

fn function_legacy_noop_set(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Undefined)
}

fn dynamic_function_source_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<(String, String), VmError> {
    if arguments.is_empty() {
        return Ok((String::new(), String::new()));
    }
    let mut params = Vec::new();
    for argument in &arguments[..arguments.len().saturating_sub(1)] {
        params.push(vm.to_string_coerce(argument.clone(), context)?);
    }
    let body = vm.to_string_coerce(
        arguments.last().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    Ok((params.join(","), body))
}

fn dynamic_function_syntax_error(error: impl std::fmt::Display) -> VmError {
    match error_to_native_kind(&error.to_string()) {
        Some(VmErrorKindMarker::Unsupported) => VmError::syntax_error(format!(
            "dynamic Function source is not supported by the native compiler: {error}"
        )),
        None => VmError::syntax_error(format!(
            "dynamic Function source could not be compiled: {error}"
        )),
    }
}

enum VmErrorKindMarker {
    Unsupported,
}

fn error_to_native_kind(message: &str) -> Option<VmErrorKindMarker> {
    if message.contains("does not support") {
        Some(VmErrorKindMarker::Unsupported)
    } else {
        None
    }
}

fn function_prototype_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "Function.prototype.call requires the VM call path",
    ))
}

fn function_prototype_apply(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "Function.prototype.apply requires the VM call path",
    ))
}

/// `Function.prototype.bind(thisArg, ...boundArgs)` — returns a bound function
/// that calls the target with `thisArg` and `boundArgs` prepended. The VM
/// dispatches the resulting value (see `call_value`/`construct_value`).
fn function_prototype_has_instance(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !context.is_callable_value(&this) {
        return Ok(JsValue::Boolean(false));
    }
    Ok(JsValue::Boolean(
        vm.ordinary_instance_of(value, this, context)?,
    ))
}

fn function_prototype_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if !context.is_callable_value(&this) {
        return Err(VmError::type_error(
            "Function.prototype.toString receiver is not callable",
        ));
    }
    let name = match &this {
        JsValue::Function(id) => context
            .function(*id)
            .and_then(|function| function.name.as_deref())
            .unwrap_or(""),
        JsValue::BuiltinFunction(id) => context
            .builtin(*id)
            .map(|function| function.name)
            .unwrap_or(""),
        _ => "",
    };
    Ok(JsValue::String(format!(
        "function {name}() {{ [native code] }}"
    )))
}

fn function_prototype_bind(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if !context.is_callable_value(&this) {
        return Err(VmError::type_error(
            "Function.prototype.bind must be called on a function",
        ));
    }
    let bound_this = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let bound_args: Vec<JsValue> = arguments.iter().skip(1).cloned().collect();

    let target_length = context
        .value_object(&this)
        .and_then(|object| context.get_own_property_descriptor(object, "length"))
        .and_then(|descriptor| match descriptor.kind {
            PropertyKind::Data {
                value: JsValue::Number(value),
                ..
            } => Some(value),
            _ => None,
        })
        .unwrap_or(0.0);
    let length = if target_length.is_infinite() {
        if target_length.is_sign_positive() {
            f64::INFINITY
        } else {
            0.0
        }
    } else if target_length.is_nan() {
        0.0
    } else {
        (target_length.trunc() - bound_args.len() as f64).max(0.0)
    };

    let target_name = context
        .get_property(this.clone(), "name")
        .ok()
        .and_then(|value| match value {
            JsValue::String(name) => Some(name),
            _ => None,
        })
        .unwrap_or_default();
    let display_name = format!("bound {target_name}");

    context.register_bound_function(this, bound_this, bound_args, length, display_name)
}
