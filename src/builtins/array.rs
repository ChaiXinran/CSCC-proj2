//! `Array` constructor and prototype bootstrap.

use crate::{
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
