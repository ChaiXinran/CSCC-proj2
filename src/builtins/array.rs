//! `Array` constructor and prototype methods.

use crate::{
    builtins::{define_data, register_native_function},
    runtime::{JsValue, NativeContext, NativeFunction, ObjectId},
    vm::VmError,
};

pub fn install_array(
    context: &mut NativeContext,
    array_constructor: ObjectId,
    array_prototype: ObjectId,
) {
    let is_array = register_native_function(
        context,
        NativeFunction::ArrayIsArray,
        context.function_prototype_object(),
    );
    define_data(
        context,
        array_constructor,
        "isArray",
        is_array,
        true,
        false,
        true,
    );

    for (name, function) in [
        ("push", NativeFunction::ArrayPrototypePush),
        ("pop", NativeFunction::ArrayPrototypePop),
    ] {
        let value =
            register_native_function(context, function, context.function_prototype_object());
        define_data(context, array_prototype, name, value, true, false, true);
    }
}

pub fn call(
    function: NativeFunction,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    match function {
        NativeFunction::ArrayConstructor => construct(context, arguments),
        NativeFunction::ArrayIsArray => {
            let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
            Ok(JsValue::Boolean(
                context
                    .value_object(&value)
                    .map(|object| context.is_array_object(object))
                    .transpose()?
                    .unwrap_or(false),
            ))
        }
        NativeFunction::ArrayPrototypePush => array_push(context, this_value, &arguments),
        NativeFunction::ArrayPrototypePop => array_pop(context, this_value),
        _ => unreachable!("array::call received a non-Array builtin"),
    }
}

pub fn construct(context: &mut NativeContext, arguments: Vec<JsValue>) -> Result<JsValue, VmError> {
    if arguments.len() == 1
        && let JsValue::Number(_) = arguments[0]
    {
        let length = context.array_length_from_value(arguments[0].clone())?;
        return context.create_sparse_array(length);
    }
    context.create_array(arguments)
}

fn array_push(
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
        if length >= u32::MAX as usize {
            return Err(VmError::range("invalid array length"));
        }
        context.set_element(
            this_value.clone(),
            JsValue::Number(length as f64),
            value.clone(),
        )?;
        length += 1;
    }

    Ok(JsValue::Number(length as f64))
}

fn array_pop(context: &mut NativeContext, this_value: JsValue) -> Result<JsValue, VmError> {
    let object = require_array(context, &this_value, "call Array.prototype.pop")?;
    let length = context
        .heap()
        .object(object)
        .and_then(|object| object.array_length())
        .ok_or_else(|| VmError::runtime("missing array object"))?;

    if length == 0 {
        return Ok(JsValue::Undefined);
    }

    let index = length - 1;
    let key = index.to_string();
    let value = context.get_property(this_value, &key)?;
    context.delete_property(object, &key, false)?;
    context.set_array_length(object, index)?;
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
    runtime::{JsObject, JsValue, NativeContext, ObjectKind, PropertyDescriptor},
    vm::VmError,
};

pub fn install_array(_context: &mut NativeContext) {
    // Prototypes and constructor wiring are handled in install_globals.
    // Instance methods (push, pop, map, etc.) are deferred to a later milestone.
}

pub fn array_call(
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    create_array_from_args(context, arguments)
}

pub fn array_construct(
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    create_array_from_args(context, arguments)
}

fn create_array_from_args(
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let array_prototype = context.intrinsics().map(|i| i.array_prototype);

    let mut object = JsObject::ordinary();
    object.kind = ObjectKind::Array {
        elements: Vec::new(),
        length_writable: true,
    };
    if let Some(proto) = array_prototype {
        object.prototype = Some(proto);
    }

    // Single numeric argument → set length only
    if arguments.len() == 1
        && let JsValue::Number(n) = &arguments[0]
    {
        let len = *n;
        let len_u32 = len as u32;
        if f64::from(len_u32) != len {
            return Err(VmError::range("Invalid array length"));
        }
        object.define_property(
            "length",
            PropertyDescriptor::data_with(JsValue::Number(len), true, false, false),
        );
        let id = context
            .heap_mut()
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime("heap exhausted"))?;
        return Ok(JsValue::Object(id));
    }

    // Otherwise populate elements
    let length = arguments.len();
    for (i, value) in arguments.iter().enumerate() {
        object.define_property(
            i.to_string(),
            PropertyDescriptor::data_with(value.clone(), true, true, true),
        );
    }
    object.define_property(
        "length",
        PropertyDescriptor::data_with(JsValue::Number(length as f64), true, false, false),
    );

    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    Ok(JsValue::Object(id))
}
