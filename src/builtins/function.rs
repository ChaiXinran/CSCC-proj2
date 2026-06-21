//! `Function` constructor and prototype bootstrap.

use crate::{
    runtime::{JsValue, NativeContext},
    vm::VmError,
};

pub fn install_function(_context: &mut NativeContext) {
    // Prototypes and constructor-function wiring are handled in install_globals.
    // Instance-method population (bind, call, apply) is deferred to a later milestone.
}

pub fn function_call(
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "Function() constructor is not yet supported in the native engine",
    ))
}

pub fn function_construct(
    _context: &mut NativeContext,
    _arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "new Function() is not yet supported in the native engine",
    ))
}
