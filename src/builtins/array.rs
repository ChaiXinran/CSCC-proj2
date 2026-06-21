//! `Array` constructor and prototype methods.

use crate::{
    runtime::{JsValue, NativeContext, ObjectId, PropertyDescriptor},
    vm::{Vm, VmError},
};

pub fn install_array(context: &mut NativeContext) {
    let Some(intrinsics) = context.intrinsics().cloned() else {
        return;
    };
    let JsValue::BuiltinFunction(constructor) = intrinsics.array_constructor else {
        return;
    };
    let constructor_object = context.builtin(constructor).unwrap().object;

    let is_array = context
        .register_builtin("isArray", 1, array_is_array, None)
        .expect("install Array.isArray");
    context
        .define_own_property(
            constructor_object,
            "isArray".into(),
            PropertyDescriptor::data_with(is_array, true, false, true),
        )
        .expect("define Array.isArray");

    for (name, length, call) in [
        ("push", 1, array_push as crate::runtime::NativeCall),
        ("pop", 0, array_pop as crate::runtime::NativeCall),
    ] {
        let value = context
            .register_builtin(name, length, call, None)
            .expect("install Array prototype method");
        context
            .define_own_property(
                intrinsics.array_prototype,
                name.into(),
                PropertyDescriptor::data_with(value, true, false, true),
            )
            .expect("define Array prototype method");
    }
}

pub fn array_call(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    create_array(context, arguments)
}

pub fn array_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    create_array(context, arguments)
}

fn create_array(context: &mut NativeContext, arguments: &[JsValue]) -> Result<JsValue, VmError> {
    if arguments.len() == 1 && matches!(arguments[0], JsValue::Number(_)) {
        let length = context.array_length_from_value(arguments[0].clone())?;
        context.create_sparse_array(length)
    } else {
        context.create_array(arguments.to_vec())
    }
}

fn array_is_array(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = arguments
        .first()
        .and_then(|value| context.value_object(value))
        .map(|object| context.is_array_object(object))
        .transpose()?
        .unwrap_or(false);
    Ok(JsValue::Boolean(result))
}

fn array_push(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_array(context, &this_value, "call Array.prototype.push")?;
    let mut length = context
        .heap()
        .object(object)
        .and_then(|object| object.array_length())
        .ok_or_else(|| VmError::runtime("missing array object"))?;

    for value in arguments {
        context.set_element(
            this_value.clone(),
            JsValue::Number(length as f64),
            value.clone(),
        )?;
        length += 1;
    }
    Ok(JsValue::Number(length as f64))
}

fn array_pop(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_array(context, &this_value, "call Array.prototype.pop")?;
    let length = context
        .heap()
        .object(object)
        .and_then(|object| object.array_length())
        .ok_or_else(|| VmError::runtime("missing array object"))?;
    if length == 0 {
        return Ok(JsValue::Undefined);
    }

    let new_length = length - 1;
    let key = new_length.to_string();
    let value = context.get_property(this_value, &key)?;
    if !context.delete_property(object, &key, false)? {
        return Err(VmError::type_error("cannot delete array element"));
    }
    if !context.set_array_length(object, new_length)? {
        return Err(VmError::type_error("cannot update array length"));
    }
    Ok(value)
}

fn require_array(
    context: &NativeContext,
    value: &JsValue,
    operation: &str,
) -> Result<ObjectId, VmError> {
    let object = context.require_object(value, operation)?;
    if context.is_array_object(object)? {
        Ok(object)
    } else {
        Err(VmError::type_error(format!(
            "{operation} on non-array value"
        )))
    }
}
