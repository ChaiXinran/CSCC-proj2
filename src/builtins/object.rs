//! `Object` constructor and V4 object-model builtins.

use crate::{
    builtins::{define_data, register_native_function},
    runtime::{
        JsValue, NativeContext, NativeFunction, ObjectId, PropertyDescriptor,
        PropertyDescriptorUpdate, PropertyKind, to_property_key,
    },
    vm::VmError,
};

pub fn install_object(context: &mut NativeContext, object_constructor: ObjectId) {
    for (name, function) in [
        ("create", NativeFunction::ObjectCreate),
        ("defineProperty", NativeFunction::ObjectDefineProperty),
        (
            "getOwnPropertyDescriptor",
            NativeFunction::ObjectGetOwnPropertyDescriptor,
        ),
        ("getPrototypeOf", NativeFunction::ObjectGetPrototypeOf),
        ("setPrototypeOf", NativeFunction::ObjectSetPrototypeOf),
        ("keys", NativeFunction::ObjectKeys),
    ] {
        let value =
            register_native_function(context, function, context.function_prototype_object());
        define_data(context, object_constructor, name, value, true, false, true);
    }
}

pub fn call(
    function: NativeFunction,
    context: &mut NativeContext,
    arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    match function {
        NativeFunction::ObjectConstructor => construct(context, arguments),
        NativeFunction::ObjectCreate => object_create(context, &arguments),
        NativeFunction::ObjectDefineProperty => object_define_property(context, &arguments),
        NativeFunction::ObjectGetOwnPropertyDescriptor => {
            object_get_own_property_descriptor(context, &arguments)
        }
        NativeFunction::ObjectGetPrototypeOf => object_get_prototype_of(context, &arguments),
        NativeFunction::ObjectSetPrototypeOf => object_set_prototype_of(context, &arguments),
        NativeFunction::ObjectKeys => object_keys(context, &arguments),
        _ => unreachable!("object::call received a non-Object builtin"),
    }
}

pub fn construct(context: &mut NativeContext, arguments: Vec<JsValue>) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if matches!(
        value,
        JsValue::Object(_) | JsValue::Function(_) | JsValue::NativeFunction(_)
    ) {
        return Ok(value);
    }
    context.create_object([])
}

fn object_create(context: &mut NativeContext, arguments: &[JsValue]) -> Result<JsValue, VmError> {
    let prototype = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let prototype = match prototype {
        JsValue::Null => None,
        value => Some(context.require_object(&value, "create object with prototype")?),
    };
    context.ordinary_object_with_prototype(prototype)
}

fn object_define_property(
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "define property")?;
    let key = to_property_key(arguments.get(1).unwrap_or(&JsValue::Undefined))?;
    let descriptor_value = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    let descriptor_object = context.require_object(&descriptor_value, "read descriptor")?;
    let update = descriptor_update_from_object(context, descriptor_object)?;
    if context.validate_and_apply_property_descriptor(object, key, update)? {
        Ok(target)
    } else {
        Err(VmError::type_error("cannot define property"))
    }
}

fn object_get_own_property_descriptor(
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "get own property descriptor")?;
    let key = to_property_key(arguments.get(1).unwrap_or(&JsValue::Undefined))?;
    let Some(descriptor) = context.get_own_property_descriptor(object, &key) else {
        return Ok(JsValue::Undefined);
    };
    descriptor_to_object(context, descriptor)
}

fn object_get_prototype_of(
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "get prototype")?;
    Ok(context
        .get_prototype_of(object)
        .map_or(JsValue::Null, |prototype| context.object_value(prototype)))
}

fn object_set_prototype_of(
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "set prototype")?;
    let prototype = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let prototype = match prototype {
        JsValue::Null => None,
        value => Some(context.require_object(&value, "set prototype")?),
    };
    context.set_prototype_of(object, prototype)?;
    Ok(target)
}

fn object_keys(context: &mut NativeContext, arguments: &[JsValue]) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "enumerate keys")?;
    let object_ref = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    let mut keys = Vec::new();
    for key in object_ref.own_property_keys() {
        if context
            .get_own_property_descriptor(object, &key)
            .is_some_and(|descriptor| descriptor.enumerable)
        {
            keys.push(JsValue::String(key));
        }
    }
    context.create_array(keys)
}

fn descriptor_update_from_object(
    context: &NativeContext,
    descriptor_object: ObjectId,
) -> Result<PropertyDescriptorUpdate, VmError> {
    let mut update = PropertyDescriptorUpdate::default();
    if let Some(value) = own_data_value(context, descriptor_object, "value") {
        update.value = Some(value);
    }
    if let Some(value) = own_data_value(context, descriptor_object, "writable") {
        update.writable = Some(value.to_boolean());
    }
    if let Some(value) = own_data_value(context, descriptor_object, "enumerable") {
        update.enumerable = Some(value.to_boolean());
    }
    if let Some(value) = own_data_value(context, descriptor_object, "configurable") {
        update.configurable = Some(value.to_boolean());
    }
    if let Some(value) = own_data_value(context, descriptor_object, "get") {
        update.get = Some(optional_callable(value, "getter")?);
    }
    if let Some(value) = own_data_value(context, descriptor_object, "set") {
        update.set = Some(optional_callable(value, "setter")?);
    }
    Ok(update)
}

fn own_data_value(context: &NativeContext, object: ObjectId, key: &str) -> Option<JsValue> {
    context
        .get_own_property_descriptor(object, key)
        .and_then(|descriptor| descriptor.value_cloned())
}

fn optional_callable(value: JsValue, label: &str) -> Result<Option<JsValue>, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    if matches!(value, JsValue::Function(_) | JsValue::NativeFunction(_)) {
        return Ok(Some(value));
    }
    Err(VmError::type_error(format!(
        "descriptor {label} is not callable"
    )))
}

fn descriptor_to_object(
    context: &mut NativeContext,
    descriptor: PropertyDescriptor,
) -> Result<JsValue, VmError> {
    let result = context.create_object([])?;
    let JsValue::Object(object) = result.clone() else {
        unreachable!("create_object returns an object");
    };

    match descriptor.kind {
        PropertyKind::Data { value, writable } => {
            define_data(context, object, "value", value, true, true, true);
            define_data(
                context,
                object,
                "writable",
                JsValue::Boolean(writable),
                true,
                true,
                true,
            );
        }
        PropertyKind::Accessor { get, set } => {
            define_data(
                context,
                object,
                "get",
                get.unwrap_or(JsValue::Undefined),
                true,
                true,
                true,
            );
            define_data(
                context,
                object,
                "set",
                set.unwrap_or(JsValue::Undefined),
                true,
                true,
                true,
            );
        }
    }
    define_data(
        context,
        object,
        "enumerable",
        JsValue::Boolean(descriptor.enumerable),
        true,
        true,
        true,
    );
    define_data(
        context,
        object,
        "configurable",
        JsValue::Boolean(descriptor.configurable),
        true,
        true,
        true,
    );
    Ok(result)
}
