//! `Function` constructor and prototype bootstrap.

use crate::{
    builtins::{define_data, register_native_function},
    runtime::{JsValue, NativeContext, NativeFunction, ObjectId},
    vm::VmError,
};

pub fn install_function(context: &mut NativeContext, function_prototype: ObjectId) {
    let call = register_native_function(
        context,
        NativeFunction::FunctionPrototypeCall,
        context.function_prototype_object(),
    );
    define_data(context, function_prototype, "call", call, true, false, true);
}

pub fn call(
    function: NativeFunction,
    _context: &mut NativeContext,
    _arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    match function {
        NativeFunction::FunctionConstructor => Err(VmError::runtime(
            "dynamic Function source compilation is unsupported in native V4",
        )),
        NativeFunction::FunctionPrototype => Ok(JsValue::Undefined),
        _ => unreachable!("function::call received a non-Function builtin"),
    }
}

pub fn construct(
    _context: &mut NativeContext,
    _arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "dynamic Function source compilation is unsupported in native V4",
    ))
}
