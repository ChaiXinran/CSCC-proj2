//! `Object` constructor, static methods, and prototype methods.

use crate::{
    runtime::{
        JsObject, JsValue, NativeContext, ObjectId, ObjectKind, PrimitiveValue, PropertyDescriptor,
        PropertyDescriptorUpdate, PropertyKind, SymbolId,
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
        ("seal", 1, object_seal as crate::runtime::NativeCall),
        (
            "preventExtensions",
            1,
            object_prevent_extensions as crate::runtime::NativeCall,
        ),
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
        (
            "isSealed",
            1,
            object_is_sealed as crate::runtime::NativeCall,
        ),
        ("hasOwn", 2, object_has_own as crate::runtime::NativeCall),
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
    let key_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let key = vm.to_property_key_from_builtin(key_arg, context)?;
    let descriptor_value = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    let descriptor_object = context.require_object(&descriptor_value, "read descriptor")?;

    if let JsValue::Symbol(sym_id) = key {
        let update = descriptor_update_from_object(vm, context, descriptor_object)?;
        if context.validate_and_apply_symbol_property_descriptor(object, sym_id, update)? {
            return Ok(target);
        } else {
            return Err(VmError::type_error("cannot define property"));
        }
    }

    let JsValue::String(key) = key else {
        unreachable!("ToPropertyKey returns a string or symbol")
    };
    let update = descriptor_update_from_object(vm, context, descriptor_object)?;
    if context.validate_and_apply_property_descriptor(object, key, update)? {
        Ok(target)
    } else {
        Err(VmError::type_error("cannot define property"))
    }
}

fn object_get_own_property_descriptor(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target, context)?;
    let descriptor = own_descriptor_for_key(
        vm,
        context,
        object,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let Some(descriptor) = descriptor else {
        return Ok(JsValue::Undefined);
    };
    descriptor_to_object(context, descriptor)
}

fn own_descriptor_for_key(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    key_arg: JsValue,
) -> Result<Option<PropertyDescriptor>, VmError> {
    match vm.to_property_key_from_builtin(key_arg, context)? {
        JsValue::Symbol(symbol) => Ok(context.get_own_symbol_property_descriptor(object, symbol)),
        JsValue::String(key) => Ok(context.get_own_property_descriptor(object, &key)),
        _ => {
            unreachable!("ToPropertyKey returns a string or symbol")
        }
    }
}

fn object_get_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target, context)?;
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
    if context.set_prototype_of(object, prototype)? {
        Ok(target)
    } else {
        Err(VmError::type_error("cannot set prototype"))
    }
}

fn object_keys(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target, context)?;
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
    vm.get_property_value_with_receiver_from_builtin(
        JsValue::Object(object),
        JsValue::Object(object),
        key,
        context,
    )
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
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value, context)?;
    let descriptor = own_descriptor_for_key(
        vm,
        context,
        object,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Boolean(descriptor.is_some()))
}

fn object_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    // If the value is an object (or wrapper), check for Symbol.toStringTag first.
    if let Some(object_id) = context.value_object(&this_value) {
        let to_string_tag = context.well_known_symbols().to_string_tag;
        if let Some(JsValue::String(tag)) =
            context.get_symbol_property_value(object_id, to_string_tag)
        {
            return Ok(JsValue::String(format!("[object {tag}]")));
        }
    }

    let tag = match &this_value {
        JsValue::Null => "Null",
        JsValue::Undefined => "Undefined",
        JsValue::Boolean(_) => "Boolean",
        JsValue::Number(_) => "Number",
        JsValue::BigInt(_) => "BigInt",
        JsValue::String(_) => "String",
        JsValue::Symbol(_) => "Symbol",
        JsValue::Function(_) | JsValue::BuiltinFunction(_) => "Function",
        JsValue::Object(id) => object_builtin_tag(context, *id)?,
        JsValue::Error(_) => "Error",
    };
    Ok(JsValue::String(format!("[object {tag}]")))
}

fn object_builtin_tag(context: &NativeContext, object: ObjectId) -> Result<&'static str, VmError> {
    if matches!(
        context.object_value(object),
        JsValue::Function(_) | JsValue::BuiltinFunction(_)
    ) {
        return Ok("Function");
    }
    let value = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    Ok(match &value.kind {
        ObjectKind::Array { .. } => "Array",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Boolean(_)) => "Boolean",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Number(_)) => "Number",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::BigInt(_)) => "BigInt",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::String(_)) => "String",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Symbol(_)) => "Symbol",
        ObjectKind::RegExp { .. } => "RegExp",
        ObjectKind::ArrayBuffer { .. } => "ArrayBuffer",
        ObjectKind::DataView { .. } => "DataView",
        ObjectKind::TypedArray { .. } => "TypedArray",
        ObjectKind::Ordinary if context.is_error_object(object) => "Error",
        ObjectKind::Ordinary => "Object",
        ObjectKind::Iterator { .. } => "Object",
        ObjectKind::Generator { .. } => "Generator",
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
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = match context.require_object(&this_value, "propertyIsEnumerable") {
        Ok(id) => id,
        Err(_) => return Ok(JsValue::Boolean(false)),
    };
    let descriptor = own_descriptor_for_key(
        vm,
        context,
        object,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Boolean(
        descriptor.map(|d| d.enumerable).unwrap_or(false),
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
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target, context)?;
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
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target, context)?;
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
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target, context)?;
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
    let target_id = vm.to_object(target.clone(), context)?;
    let target_value = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(target_id),
    };
    for source in arguments.iter().skip(1) {
        if matches!(source, JsValue::Undefined | JsValue::Null) {
            continue;
        }
        let source_id = vm.to_object(source.clone(), context)?;
        let source_value = match source {
            JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
                source.clone()
            }
            _ => context.object_value(source_id),
        };
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
            let value = vm.get_property_value_with_receiver_from_builtin(
                source_value.clone(),
                source_value.clone(),
                &key,
                context,
            )?;
            if !vm.set_property_value_strict_from_builtin(
                target_value.clone(),
                &key,
                value,
                context,
            )? {
                return Err(VmError::type_error(format!("cannot write property {key}")));
            }
        }

        let symbols: Vec<_> = context
            .heap()
            .object(source_id)
            .ok_or_else(|| VmError::runtime("missing source object"))?
            .symbol_properties
            .iter()
            .map(|(symbol, _)| *symbol)
            .collect();
        for symbol in symbols {
            if !context
                .get_own_symbol_property_descriptor(source_id, symbol)
                .is_some_and(|d| d.enumerable)
            {
                continue;
            }
            let value = vm.get_symbol_property_value_with_receiver_from_builtin(
                source_value.clone(),
                source_value.clone(),
                symbol,
                context,
            )?;
            if !vm.set_symbol_property_value_with_receiver_from_builtin(
                target_value.clone(),
                target_value.clone(),
                symbol,
                value,
                context,
            )? {
                return Err(VmError::type_error("cannot assign symbol property"));
            }
        }
    }
    Ok(target_value)
}

fn own_symbol_keys(context: &NativeContext, object: ObjectId) -> Result<Vec<SymbolId>, VmError> {
    Ok(context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?
        .symbol_properties
        .iter()
        .map(|(symbol, _)| *symbol)
        .collect())
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
            let Some(descriptor) = context.get_own_property_descriptor(*object, &key) else {
                continue;
            };
            let update = PropertyDescriptorUpdate {
                configurable: Some(false),
                writable: matches!(descriptor.kind, PropertyKind::Data { .. }).then_some(false),
                ..PropertyDescriptorUpdate::default()
            };
            context
                .validate_and_apply_property_descriptor(*object, key, update)
                .ok();
        }
        for symbol in own_symbol_keys(context, *object)? {
            let Some(descriptor) = context.get_own_symbol_property_descriptor(*object, symbol)
            else {
                continue;
            };
            let update = PropertyDescriptorUpdate {
                configurable: Some(false),
                writable: matches!(descriptor.kind, PropertyKind::Data { .. }).then_some(false),
                ..PropertyDescriptorUpdate::default()
            };
            context
                .validate_and_apply_symbol_property_descriptor(*object, symbol, update)
                .ok();
        }
        context.prevent_extensions(*object)?;
    }
    Ok(target)
}

fn object_seal(
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
            let update = PropertyDescriptorUpdate {
                configurable: Some(false),
                ..PropertyDescriptorUpdate::default()
            };
            context
                .validate_and_apply_property_descriptor(*object, key, update)
                .ok();
        }
        for symbol in own_symbol_keys(context, *object)? {
            let update = PropertyDescriptorUpdate {
                configurable: Some(false),
                ..PropertyDescriptorUpdate::default()
            };
            context
                .validate_and_apply_symbol_property_descriptor(*object, symbol, update)
                .ok();
        }
        context.prevent_extensions(*object)?;
    }
    Ok(target)
}

fn object_prevent_extensions(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Object(object) = &target {
        context.prevent_extensions(*object)?;
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
    let Some(object) = context.value_object(&target) else {
        return Ok(JsValue::Boolean(false));
    };
    Ok(JsValue::Boolean(context.is_extensible(object)?))
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
    if context.is_extensible(object)? {
        return Ok(JsValue::Boolean(false));
    }
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
    for symbol in own_symbol_keys(context, object)? {
        if context
            .get_own_symbol_property_descriptor(object, symbol)
            .is_some_and(|d| {
                d.configurable || matches!(d.kind, PropertyKind::Data { writable: true, .. })
            })
        {
            return Ok(JsValue::Boolean(false));
        }
    }
    Ok(JsValue::Boolean(true))
}

fn object_is_sealed(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let JsValue::Object(object) = target else {
        return Ok(JsValue::Boolean(true));
    };
    if context.is_extensible(object)? {
        return Ok(JsValue::Boolean(false));
    }
    let keys = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?
        .own_property_keys();
    for key in keys {
        if context
            .get_own_property_descriptor(object, &key)
            .is_some_and(|d| d.configurable)
        {
            return Ok(JsValue::Boolean(false));
        }
    }
    for symbol in own_symbol_keys(context, object)? {
        if context
            .get_own_symbol_property_descriptor(object, symbol)
            .is_some_and(|d| d.configurable)
        {
            return Ok(JsValue::Boolean(false));
        }
    }
    Ok(JsValue::Boolean(true))
}

fn object_has_own(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target, context)?;
    let descriptor = own_descriptor_for_key(
        vm,
        context,
        object,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Boolean(descriptor.is_some()))
}
