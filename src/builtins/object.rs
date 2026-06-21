//! `Object` constructor and static methods.

use crate::{
    runtime::{
        JsObject, JsValue, NativeContext, ObjectId, PropertyDescriptor, PropertyDescriptorUpdate,
        PropertyKind, to_property_key,
    },
    vm::{Vm, VmError},
};

pub fn install_object(context: &mut NativeContext) {
    let Some(JsValue::BuiltinFunction(constructor)) = context.get_global("Object") else {
        return;
    };
    let constructor_object = context.builtin(constructor).unwrap().object;

    for (name, length, call) in [
        ("create", 2, object_create as crate::runtime::NativeCall),
        (
            "defineProperty",
            3,
            object_define_property as crate::runtime::NativeCall,
        ),
        (
            "getOwnPropertyDescriptor",
            2,
            object_get_own_property_descriptor as crate::runtime::NativeCall,
        ),
        (
            "getPrototypeOf",
            1,
            object_get_prototype_of as crate::runtime::NativeCall,
        ),
        (
            "setPrototypeOf",
            2,
            object_set_prototype_of as crate::runtime::NativeCall,
        ),
        ("keys", 1, object_keys as crate::runtime::NativeCall),
    ] {
        let value = context
            .register_builtin(name, length, call, None)
            .expect("install Object static method");
        context
            .define_own_property(
                constructor_object,
                name.into(),
                PropertyDescriptor::data_with(value, true, false, true),
            )
            .expect("define Object static method");
    }
}

pub fn object_call(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = context
        .intrinsics()
        .map(|intrinsics| intrinsics.object_prototype);
    object_from_argument(context, arguments, prototype)
}

pub fn object_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .intrinsics()
        .map(|intrinsics| intrinsics.object_prototype);
    object_from_argument(context, arguments, prototype)
}

fn object_from_argument(
    context: &mut NativeContext,
    arguments: &[JsValue],
    prototype: Option<ObjectId>,
) -> Result<JsValue, VmError> {
    match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined | JsValue::Null => context.ordinary_object_with_prototype(prototype),
        value @ (JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)) => {
            Ok(value)
        }
        value => Err(VmError::runtime(format!(
            "Object({}) primitive coercion is unsupported in native V4",
            value.type_of()
        ))),
    }
}

fn object_create(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Null => None,
        value => Some(context.require_object(&value, "create object with prototype")?),
    };
    let result = context.ordinary_object_with_prototype(prototype)?;
    let JsValue::Object(object) = result else {
        unreachable!()
    };

    if let Some(properties) = arguments.get(1)
        && !matches!(properties, JsValue::Undefined)
    {
        let properties = context.require_object(properties, "read property descriptors")?;
        let keys = context
            .heap()
            .object(properties)
            .ok_or_else(|| VmError::runtime("missing descriptor map"))?
            .own_property_keys();
        for key in keys {
            if !context
                .get_own_property_descriptor(properties, &key)
                .is_some_and(|descriptor| descriptor.enumerable)
            {
                continue;
            }
            let descriptor_value =
                vm.get_property_value(JsValue::Object(properties), &key, context)?;
            let descriptor_object =
                context.require_object(&descriptor_value, "read property descriptor")?;
            let update = descriptor_update_from_object(vm, context, descriptor_object)?;
            if !context.validate_and_apply_property_descriptor(object, key, update)? {
                return Err(VmError::type_error("cannot define property"));
            }
        }
    }
    Ok(JsValue::Object(object))
}

fn object_define_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "define property")?;
    let key = to_property_key(arguments.get(1).unwrap_or(&JsValue::Undefined))?;
    let descriptor_value = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    let descriptor_object = context.require_object(&descriptor_value, "read descriptor")?;
    let update = descriptor_update_from_object(vm, context, descriptor_object)?;
    if context.validate_and_apply_property_descriptor(object, key, update)? {
        Ok(target)
    } else {
        Err(VmError::type_error("cannot define property"))
    }
}

fn object_get_own_property_descriptor(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
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
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "get prototype")?;
    Ok(context
        .get_prototype_of(object)
        .map_or(JsValue::Null, |prototype| context.object_value(prototype)))
}

fn object_set_prototype_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "set prototype")?;
    let prototype = match arguments.get(1).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Null => None,
        value => Some(context.require_object(&value, "set prototype")?),
    };
    context.set_prototype_of(object, prototype)?;
    Ok(target)
}

fn object_keys(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "enumerate keys")?;
    let keys = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?
        .own_property_keys()
        .into_iter()
        .filter(|key| {
            context
                .get_own_property_descriptor(object, key)
                .is_some_and(|descriptor| descriptor.enumerable)
        })
        .map(JsValue::String)
        .collect();
    context.create_array(keys)
}

fn descriptor_update_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    descriptor_object: ObjectId,
) -> Result<PropertyDescriptorUpdate, VmError> {
    let mut update = PropertyDescriptorUpdate::default();
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "value")? {
        update.value = Some(value);
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "writable")? {
        update.writable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "enumerable")? {
        update.enumerable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "configurable")? {
        update.configurable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "get")? {
        update.get = Some(optional_callable(value, "getter")?);
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "set")? {
        update.set = Some(optional_callable(value, "setter")?);
    }
    Ok(update)
}

fn descriptor_field(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    key: &str,
) -> Result<Option<JsValue>, VmError> {
    if !context.has_property(object, key)? {
        return Ok(None);
    }
    vm.get_property_value(JsValue::Object(object), key, context)
        .map(Some)
}

fn optional_callable(value: JsValue, label: &str) -> Result<Option<JsValue>, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    if matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_)) {
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
    let mut object = JsObject::ordinary();
    object.prototype = context
        .intrinsics()
        .map(|intrinsics| intrinsics.object_prototype);

    match descriptor.kind {
        PropertyKind::Data { value, writable } => {
            define_descriptor_field(&mut object, "value", value);
            define_descriptor_field(&mut object, "writable", JsValue::Boolean(writable));
        }
        PropertyKind::Accessor { get, set } => {
            define_descriptor_field(&mut object, "get", get.unwrap_or(JsValue::Undefined));
            define_descriptor_field(&mut object, "set", set.unwrap_or(JsValue::Undefined));
        }
    }
    define_descriptor_field(
        &mut object,
        "enumerable",
        JsValue::Boolean(descriptor.enumerable),
    );
    define_descriptor_field(
        &mut object,
        "configurable",
        JsValue::Boolean(descriptor.configurable),
    );

    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("object arena exhausted"))?;
    Ok(JsValue::Object(id))
}

fn define_descriptor_field(object: &mut JsObject, name: &str, value: JsValue) {
    object.define_property(name, PropertyDescriptor::data_with(value, true, true, true));
}
