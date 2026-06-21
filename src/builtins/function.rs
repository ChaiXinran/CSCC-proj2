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
