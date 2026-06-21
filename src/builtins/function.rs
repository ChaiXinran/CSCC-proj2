//! `Function` constructor and prototype methods.

use crate::{
    runtime::{JsValue, NativeContext, PropertyDescriptor},
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
}

pub fn function_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "dynamic Function source compilation is unsupported in native V4",
    ))
}

pub fn function_construct(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "dynamic Function source compilation is unsupported in native V4",
    ))
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

/// `Function.prototype.bind(thisArg, ...boundArgs)` — returns a bound function
/// that calls the target with `thisArg` and `boundArgs` prepended. The VM
/// dispatches the resulting value (see `call_value`/`construct_value`).
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
