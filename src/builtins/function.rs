//! `Function` constructor and prototype methods.

use crate::{
    bytecode::Compiler,
    lexer::Lexer,
    parser::Parser,
    runtime::{JsFunction, JsValue, NativeContext, PropertyDescriptor},
    vm::{Vm, VmError},
};

pub fn install_function(context: &mut NativeContext) {
    let Some(function_prototype) = context
        .intrinsics()
        .map(|intrinsics| intrinsics.function_prototype)
    else {
        return;
    };
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
    let tokens = Lexer::new(&source)
        .tokenize()
        .map_err(dynamic_function_syntax_error)?;
    let program = Parser::with_source(tokens, &source)
        .parse_program()
        .map_err(dynamic_function_syntax_error)?;
    let chunk = Compiler::new()
        .compile_program(&program)
        .map_err(dynamic_function_syntax_error)?;
    vm.eval_execute(&chunk, context)
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
    let source = format!("(function anonymous({params}) {{\n{body}\n}})");
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
    let id = context.allocate_function(JsFunction {
        name: template.name.or_else(|| Some("anonymous".into())),
        params: template.params,
        rest_param: template.rest_param,
        chunk: template.chunk,
        // Dynamic Function is created in the global scope; it must not capture
        // the caller's local lexical environment.
        environment: Some(context.global_environment()),
        is_generator: false,
    })?;
    Ok(JsValue::Function(id))
}

fn dynamic_function_source_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<(String, String), VmError> {
    if arguments.is_empty() {
        return Ok((String::new(), String::new()));
    }
    let body = vm.to_string_coerce(
        arguments.last().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let mut params = Vec::new();
    for argument in &arguments[..arguments.len().saturating_sub(1)] {
        params.push(vm.to_string_coerce(argument.clone(), context)?);
    }
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
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::Boolean(context.ordinary_instance_of(value, this)?))
}

fn function_prototype_bind(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if !matches!(this, JsValue::Function(_) | JsValue::BuiltinFunction(_)) {
        return Err(VmError::type_error(
            "Function.prototype.bind must be called on a function",
        ));
    }
    let bound_this = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let bound_args: Vec<JsValue> = arguments.iter().skip(1).cloned().collect();

    // length = max(0, target.length - boundArgs.length).
    let target_length = context
        .get_property(this.clone(), "length")
        .ok()
        .and_then(|value| value.to_number())
        .unwrap_or(0.0);
    let length = (target_length - bound_args.len() as f64).clamp(0.0, 255.0) as u8;

    context.register_bound_function(this, bound_this, bound_args, length)
}
