//! `Object` constructor, static methods, and prototype methods.

use crate::{
    runtime::{
        JsObject, JsValue, NativeContext, ObjectId, ObjectKind, PrimitiveValue, PropertyDescriptor,
        PropertyDescriptorUpdate, PropertyKind, to_property_key,
    },
    vm::{Vm, VmError},
};

pub fn install_object(context: &mut NativeContext) {
    let Some(intrinsics) = context.intrinsics().cloned() else {
        return;
    };
    let Some(JsValue::BuiltinFunction(constructor)) = context.get_global("Object") else {
        return;
    };
    let constructor_object = context.builtin(constructor).unwrap().object;

    // Static methods on Object constructor
    for (name, length, call) in [
        ("create", 2, object_create as crate::runtime::NativeCall),
        (
            "defineProperty",
            3,
            object_define_property as crate::runtime::NativeCall,
        ),
        (
            "defineProperties",
            2,
            object_define_properties as crate::runtime::NativeCall,
        ),
        (
            "getOwnPropertyDescriptor",
            2,
            object_get_own_property_descriptor as crate::runtime::NativeCall,
        ),
        (
            "getOwnPropertyNames",
            1,
            object_get_own_property_names as crate::runtime::NativeCall,
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
        ("values", 1, object_values as crate::runtime::NativeCall),
        ("entries", 2, object_entries as crate::runtime::NativeCall),
        ("assign", 2, object_assign as crate::runtime::NativeCall),
        ("freeze", 1, object_freeze as crate::runtime::NativeCall),
        (
            "isExtensible",
            1,
            object_is_extensible as crate::runtime::NativeCall,
        ),
        (
            "isFrozen",
            1,
            object_is_frozen as crate::runtime::NativeCall,
        ),
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

    // Instance methods on Object.prototype
    for (name, length, call) in [
        (
            "hasOwnProperty",
            1,
            object_has_own_property as crate::runtime::NativeCall,
        ),
        (
            "toString",
            0,
            object_to_string as crate::runtime::NativeCall,
        ),
        ("valueOf", 0, object_value_of as crate::runtime::NativeCall),
        (
            "isPrototypeOf",
            1,
            object_is_prototype_of as crate::runtime::NativeCall,
        ),
        (
            "propertyIsEnumerable",
            1,
            object_property_is_enumerable as crate::runtime::NativeCall,
        ),
    ] {
        let value = context
            .register_builtin(name, length, call, None)
            .expect("install Object.prototype method");
        context
            .define_own_property(
                intrinsics.object_prototype,
                name.into(),
                PropertyDescriptor::data_with(value, true, false, true),
            )
            .expect("define Object.prototype method");
    }
}

pub fn object_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    object_from_argument(vm, context, arguments)
}

pub fn object_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    object_from_argument(vm, context, arguments)
}

fn object_from_argument(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined | JsValue::Null => {
            context.ordinary_object_with_prototype(context.object_prototype())
        }
        value @ (JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)) => {
            Ok(value)
        }
        value => vm.to_object(value, context).map(JsValue::Object),
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

// ── Object.prototype instance methods ────────────────────────────────────────

fn object_has_own_property(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let key = to_property_key(arguments.first().unwrap_or(&JsValue::Undefined))?;
    let object = context.require_object(&this_value, "hasOwnProperty")?;
    Ok(JsValue::Boolean(
        context.get_own_property(object, &key).is_some(),
    ))
}

fn object_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let tag = match &this_value {
        JsValue::Null => "Null",
        JsValue::Undefined => "Undefined",
        JsValue::Boolean(_) => "Boolean",
        JsValue::Number(_) => "Number",
        JsValue::String(_) => "String",
        JsValue::Function(_) | JsValue::BuiltinFunction(_) => "Function",
        JsValue::Object(id) => object_builtin_tag(context, *id)?,
        JsValue::Error(_) => "Error",
    };
    Ok(JsValue::String(format!("[object {tag}]")))
}

fn object_builtin_tag(context: &NativeContext, object: ObjectId) -> Result<&'static str, VmError> {
    let value = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    Ok(match &value.kind {
        ObjectKind::Array { .. } => "Array",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Boolean(_)) => "Boolean",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Number(_)) => "Number",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::String(_)) => "String",
        ObjectKind::RegExp { .. } => "RegExp",
        ObjectKind::Ordinary if context.is_error_object(object) => "Error",
        ObjectKind::Ordinary => "Object",
    })
}

fn object_value_of(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn object_is_prototype_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let proto = match context.require_object(&this_value, "isPrototypeOf") {
        Ok(id) => id,
        Err(_) => return Ok(JsValue::Boolean(false)),
    };
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let Ok(mut current) = context.require_object(&value, "isPrototypeOf target") else {
        return Ok(JsValue::Boolean(false));
    };
    loop {
        match context.get_prototype_of(current) {
            None => return Ok(JsValue::Boolean(false)),
            Some(p) if p == proto => return Ok(JsValue::Boolean(true)),
            Some(p) => current = p,
        }
    }
}

fn object_property_is_enumerable(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let key = to_property_key(arguments.first().unwrap_or(&JsValue::Undefined))?;
    let object = match context.require_object(&this_value, "propertyIsEnumerable") {
        Ok(id) => id,
        Err(_) => return Ok(JsValue::Boolean(false)),
    };
    Ok(JsValue::Boolean(
        context
            .get_own_property(object, &key)
            .map(|d| d.enumerable)
            .unwrap_or(false),
    ))
}

// ── Additional Object static methods ─────────────────────────────────────────

fn object_define_properties(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "defineProperties")?;
    let props_value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if matches!(props_value, JsValue::Undefined | JsValue::Null) {
        return Ok(target);
    }
    let props = context.require_object(&props_value, "defineProperties props")?;
    let keys = context
        .heap()
        .object(props)
        .ok_or_else(|| VmError::runtime("missing props object"))?
        .own_property_keys();
    for key in keys {
        if !context
            .get_own_property_descriptor(props, &key)
            .is_some_and(|d| d.enumerable)
        {
            continue;
        }
        let descriptor_value = vm.get_property_value(JsValue::Object(props), &key, context)?;
        let descriptor_object =
            context.require_object(&descriptor_value, "read property descriptor")?;
        let update = descriptor_update_from_object(vm, context, descriptor_object)?;
        if !context.validate_and_apply_property_descriptor(object, key, update)? {
            return Err(VmError::type_error("cannot define property"));
        }
    }
    Ok(target)
}

fn object_get_own_property_names(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "getOwnPropertyNames")?;
    let keys: Vec<JsValue> = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?
        .own_property_keys()
        .into_iter()
        .map(JsValue::String)
        .collect();
    context.create_array(keys)
}

fn object_values(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "Object.values")?;
    let keys = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?
        .own_property_keys();
    let values: Vec<JsValue> = keys
        .into_iter()
        .filter(|key| {
            context
                .get_own_property_descriptor(object, key)
                .is_some_and(|d| d.enumerable)
        })
        .filter_map(|key| context.get_own_property_descriptor(object, &key))
        .filter_map(|d| d.value_cloned())
        .collect();
    context.create_array(values)
}

fn object_entries(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&target, "Object.entries")?;
    let keys = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?
        .own_property_keys();
    let mut pairs: Vec<JsValue> = Vec::new();
    for key in keys {
        let Some(descriptor) = context.get_own_property_descriptor(object, &key) else {
            continue;
        };
        if !descriptor.enumerable {
            continue;
        }
        let value = descriptor.value_cloned().unwrap_or(JsValue::Undefined);
        let pair = context.create_array(vec![JsValue::String(key), value])?;
        pairs.push(pair);
    }
    context.create_array(pairs)
}

fn object_assign(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let target_id = context.require_object(&target, "Object.assign")?;
    for source in arguments.iter().skip(1) {
        if matches!(source, JsValue::Undefined | JsValue::Null) {
            continue;
        }
        let source_id = context.require_object(source, "Object.assign source")?;
        let keys = context
            .heap()
            .object(source_id)
            .ok_or_else(|| VmError::runtime("missing source object"))?
            .own_property_keys();
        for key in keys {
            if !context
                .get_own_property_descriptor(source_id, &key)
                .is_some_and(|d| d.enumerable)
            {
                continue;
            }
            let value = vm.get_property_value(source.clone(), &key, context)?;
            context
                .define_own_property(
                    target_id,
                    key,
                    PropertyDescriptor::data_with(value, true, true, true),
                )
                .ok();
        }
    }
    Ok(target)
}

fn object_freeze(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Object(object) = &target {
        let keys = context
            .heap()
            .object(*object)
            .ok_or_else(|| VmError::runtime("missing object"))?
            .own_property_keys();
        for key in keys {
            if context.get_own_property_descriptor(*object, &key).is_none() {
                continue;
            }
            let update = PropertyDescriptorUpdate {
                writable: Some(false),
                configurable: Some(false),
                ..PropertyDescriptorUpdate::default()
            };
            context
                .validate_and_apply_property_descriptor(*object, key, update)
                .ok();
        }
    }
    Ok(target)
}

fn object_is_extensible(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::Boolean(context.value_object(&target).is_some()))
}

fn object_is_frozen(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let JsValue::Object(object) = target else {
        return Ok(JsValue::Boolean(true));
    };
    let keys = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?
        .own_property_keys();
    for key in keys {
        if context
            .get_own_property_descriptor(object, &key)
            .is_some_and(|d| {
                d.configurable || matches!(d.kind, PropertyKind::Data { writable: true, .. })
            })
        {
            return Ok(JsValue::Boolean(false));
        }
    }
    Ok(JsValue::Boolean(true))
}
