//! `Object` constructor, static methods, and prototype methods.

use super::proxy;
use crate::{
    runtime::{
        JsObject, JsValue, NativeContext, ObjectId, ObjectKind, PrimitiveValue, PropertyDescriptor,
        PropertyDescriptorUpdate, PropertyKey, PropertyKind,
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
            "getOwnPropertyDescriptors",
            1,
            object_get_own_property_descriptors as crate::runtime::NativeCall,
        ),
        (
            "getOwnPropertyNames",
            1,
            object_get_own_property_names as crate::runtime::NativeCall,
        ),
        (
            "getOwnPropertySymbols",
            1,
            object_get_own_property_symbols as crate::runtime::NativeCall,
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
        (
            "fromEntries",
            1,
            object_from_entries as crate::runtime::NativeCall,
        ),
        ("groupBy", 2, object_group_by as crate::runtime::NativeCall),
        ("assign", 2, object_assign as crate::runtime::NativeCall),
        ("is", 2, object_is as crate::runtime::NativeCall),
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
        (
            "toLocaleString",
            0,
            object_to_locale_string as crate::runtime::NativeCall,
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

fn object_is(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = arguments.first().unwrap_or(&JsValue::Undefined);
    let right = arguments.get(1).unwrap_or(&JsValue::Undefined);
    Ok(JsValue::Boolean(left.same_value(right)))
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
        let properties_value = JsValue::Object(properties);
        let keys = proxy::internal_own_property_keys(vm, context, properties_value.clone())?;
        for key in keys {
            if !proxy::internal_get_own_property(vm, context, properties_value.clone(), &key)?
                .is_some_and(|d| d.enumerable)
            {
                continue;
            }
            let descriptor_value = match &key {
                PropertyKey::String(name) => {
                    vm.get_property_value(properties_value.clone(), name, context)?
                }
                PropertyKey::Symbol(symbol) => vm
                    .get_symbol_property_value_with_receiver_from_builtin(
                        properties_value.clone(),
                        properties_value.clone(),
                        *symbol,
                        context,
                    )?,
            };
            let descriptor_object =
                context.require_object(&descriptor_value, "read property descriptor")?;
            let update = descriptor_update_from_object(vm, context, descriptor_object)?;
            match key {
                PropertyKey::String(name) => {
                    if !context.validate_and_apply_property_descriptor(object, name, update)? {
                        return Err(VmError::type_error("cannot define property"));
                    }
                }
                PropertyKey::Symbol(symbol) => {
                    if !context
                        .validate_and_apply_symbol_property_descriptor(object, symbol, update)?
                    {
                        return Err(VmError::type_error("cannot define property"));
                    }
                }
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
    let key_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let key = proxy::to_property_key(vm, context, key_arg)?;
    let descriptor_value = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    let descriptor_object = context.require_object(&descriptor_value, "read descriptor")?;
    let update = descriptor_update_from_object(vm, context, descriptor_object)?;
    let update = normalize_array_length_update(vm, context, &target, &key, update)?;
    if proxy::internal_define_own_property(
        vm,
        context,
        target.clone(),
        &key,
        descriptor_value,
        update,
    )? {
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

fn object_get_own_property_descriptors(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target.clone(), context)?;
    let target = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(object),
    };
    let result_value = context.create_object(std::iter::empty())?;
    let result = context.require_object(&result_value, "create descriptor result")?;
    for key in proxy::internal_own_property_keys(vm, context, target.clone())? {
        let Some(descriptor) = proxy::internal_get_own_property(vm, context, target.clone(), &key)?
        else {
            continue;
        };
        let descriptor_object = descriptor_to_object(context, descriptor)?;
        let descriptor = PropertyDescriptor::data_with(descriptor_object, true, true, true);
        match key {
            PropertyKey::String(name) => {
                context.define_own_property(result, name, descriptor)?;
            }
            PropertyKey::Symbol(symbol) => {
                context.define_symbol_own_property(result, symbol, descriptor)?;
            }
        }
    }
    Ok(result_value)
}

fn own_descriptor_for_key(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    key_arg: JsValue,
) -> Result<Option<PropertyDescriptor>, VmError> {
    let key = proxy::to_property_key(vm, context, key_arg)?;
    let target = context.object_value(object);
    proxy::internal_get_own_property(vm, context, target, &key)
}

fn object_get_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target.clone(), context)?;
    let target = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(object),
    };
    Ok(proxy::internal_get_prototype_of(vm, context, target)?
        .map_or(JsValue::Null, |prototype| context.object_value(prototype)))
}

fn object_set_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    context.require_object(&target, "set prototype")?;
    let prototype = match arguments.get(1).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Null => None,
        value => Some(context.require_object(&value, "set prototype")?),
    };
    if proxy::internal_set_prototype_of(vm, context, target.clone(), prototype)? {
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
    let object = vm.to_object(target.clone(), context)?;
    let target = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(object),
    };
    let mut keys = Vec::new();
    for key in proxy::internal_own_property_keys(vm, context, target.clone())? {
        let PropertyKey::String(name) = &key else {
            continue;
        };
        if proxy::internal_get_own_property(vm, context, target.clone(), &key)?
            .is_some_and(|descriptor| descriptor.enumerable)
        {
            keys.push(JsValue::String(name.clone()));
        }
    }
    context.create_array(keys)
}

fn descriptor_update_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    descriptor_object: ObjectId,
) -> Result<PropertyDescriptorUpdate, VmError> {
    let mut update = PropertyDescriptorUpdate::default();
    // ToPropertyDescriptor reads inherited fields through [[HasProperty]] and
    // [[Get]] in this exact observable order.
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "enumerable")? {
        update.enumerable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "configurable")? {
        update.configurable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "value")? {
        update.value = Some(value);
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "writable")? {
        update.writable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "get")? {
        update.get = Some(optional_callable(context, value, "getter")?);
    }
    if let Some(value) = descriptor_field(vm, context, descriptor_object, "set")? {
        update.set = Some(optional_callable(context, value, "setter")?);
    }
    if (update.value.is_some() || update.writable.is_some())
        && (update.get.is_some() || update.set.is_some())
    {
        return Err(VmError::type_error(
            "property descriptor cannot be both a data and accessor descriptor",
        ));
    }
    Ok(update)
}

fn descriptor_field(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    key: &str,
) -> Result<Option<JsValue>, VmError> {
    let receiver = context.object_value(object);
    let property_key = PropertyKey::String(key.into());
    if !proxy::internal_has_property(vm, context, receiver.clone(), &property_key)? {
        return Ok(None);
    }
    proxy::internal_get(vm, context, receiver.clone(), &property_key, receiver).map(Some)
}

fn optional_callable(
    context: &NativeContext,
    value: JsValue,
    label: &str,
) -> Result<Option<JsValue>, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    if context.is_callable_value(&value) {
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
    let key = proxy::to_property_key(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let object = vm.to_object(this_value, context)?;
    let descriptor =
        proxy::internal_get_own_property(vm, context, context.object_value(object), &key)?;
    Ok(JsValue::Boolean(descriptor.is_some()))
}

pub(crate) fn object_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if !matches!(this_value, JsValue::Null | JsValue::Undefined) {
        let to_string_tag = context.well_known_symbols().to_string_tag;
        let tag = vm.get_symbol_property_value_with_receiver_from_builtin(
            this_value.clone(),
            this_value.clone(),
            to_string_tag,
            context,
        )?;
        if let JsValue::String(tag) = tag {
            return Ok(JsValue::String(format!("[object {tag}]")));
        }
    }

    let tag = if context.is_callable_value(&this_value) {
        "Function"
    } else {
        match &this_value {
            JsValue::Null => "Null",
            JsValue::Undefined => "Undefined",
            JsValue::Boolean(_) => "Boolean",
            JsValue::Number(_) => "Number",
            JsValue::BigInt(_) => "Object",
            JsValue::String(_) => "String",
            JsValue::Symbol(_) => "Object",
            JsValue::Function(_) | JsValue::BuiltinFunction(_) => unreachable!(),
            JsValue::Object(id) => object_builtin_tag(context, *id)?,
            JsValue::Error(_) => "Error",
        }
    };
    Ok(JsValue::String(format!("[object {tag}]")))
}

fn object_to_locale_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let to_string = vm.get_property_value(this_value.clone(), "toString", context)?;
    if !context.is_callable_value(&to_string) {
        return Err(VmError::type_error(
            "Object.prototype.toLocaleString toString is not callable",
        ));
    }
    vm.call_value_from_builtin(to_string, this_value, Vec::new(), context)
}

fn object_builtin_tag(context: &NativeContext, object: ObjectId) -> Result<&'static str, VmError> {
    if context.is_arguments_object(object) {
        return Ok("Arguments");
    }
    if matches!(
        context.object_value(object),
        JsValue::Function(_) | JsValue::BuiltinFunction(_)
    ) {
        return Ok("Function");
    }
    if context.is_array_object(object)? {
        return Ok("Array");
    }
    let value = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    Ok(match &value.kind {
        ObjectKind::Array { .. } => "Array",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Boolean(_)) => "Boolean",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Number(_)) => "Number",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::BigInt(_)) => "Object",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::String(_)) => "String",
        ObjectKind::PrimitiveWrapper(PrimitiveValue::Symbol(_)) => "Object",
        ObjectKind::RegExp { .. } => "RegExp",
        ObjectKind::ArrayBuffer { .. } => "ArrayBuffer",
        ObjectKind::DataView { .. } => "DataView",
        ObjectKind::TypedArray { .. } => "TypedArray",
        ObjectKind::Ordinary if context.is_error_object(object) => "Error",
        ObjectKind::Ordinary => "Object",
        ObjectKind::Iterator { .. } => "Object",
        ObjectKind::Generator { .. } => "Generator",
        ObjectKind::Promise { .. } => "Object",
        ObjectKind::Proxy { .. } => "Object",
        ObjectKind::WeakRef { .. } => "WeakRef",
        ObjectKind::FinalizationRegistry { .. } => "FinalizationRegistry",
    })
}

fn object_value_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value.clone(), context)?;
    Ok(if context.value_object(&this_value).is_some() {
        this_value
    } else {
        context.object_value(object)
    })
}

fn object_is_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&value).is_none() {
        return Ok(JsValue::Boolean(false));
    }
    let proto = vm.to_object(this_value, context)?;
    let mut current = value;
    loop {
        match proxy::internal_get_prototype_of(vm, context, current)? {
            None => return Ok(JsValue::Boolean(false)),
            Some(p) if p == proto => return Ok(JsValue::Boolean(true)),
            Some(p) => current = context.object_value(p),
        }
    }
}

fn object_property_is_enumerable(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let key = proxy::to_property_key(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let object = vm.to_object(this_value.clone(), context)?;
    let target = if context.value_object(&this_value).is_some() {
        this_value
    } else {
        context.object_value(object)
    };
    let descriptor = proxy::internal_get_own_property(vm, context, target, &key)?;
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
    context.require_object(&target, "defineProperties")?;
    let props_value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if matches!(props_value, JsValue::Undefined | JsValue::Null) {
        return Err(VmError::type_error(
            "Object.defineProperties properties must not be null or undefined",
        ));
    }
    let props = vm.to_object(props_value, context)?;
    let props_object = context.object_value(props);
    let keys = proxy::internal_own_property_keys(vm, context, props_object.clone())?;
    let mut descriptors = Vec::new();
    for key in keys {
        if !proxy::internal_get_own_property(vm, context, props_object.clone(), &key)?
            .is_some_and(|d| d.enumerable)
        {
            continue;
        }
        let descriptor_value = match &key {
            PropertyKey::String(name) => {
                vm.get_property_value(props_object.clone(), name, context)?
            }
            PropertyKey::Symbol(symbol) => vm
                .get_symbol_property_value_with_receiver_from_builtin(
                    props_object.clone(),
                    props_object.clone(),
                    *symbol,
                    context,
                )?,
        };
        let descriptor_object =
            context.require_object(&descriptor_value, "read property descriptor")?;
        let update = descriptor_update_from_object(vm, context, descriptor_object)?;
        descriptors.push((key, descriptor_value, update));
    }
    // DefineProperties first converts every enumerable descriptor. No target
    // mutation is allowed if a later conversion completes abruptly.
    for (property_key, descriptor_value, update) in descriptors {
        let update = normalize_array_length_update(vm, context, &target, &property_key, update)?;
        if !proxy::internal_define_own_property(
            vm,
            context,
            target.clone(),
            &property_key,
            descriptor_value,
            update,
        )? {
            return Err(VmError::type_error("cannot define property"));
        }
    }
    Ok(target)
}

fn normalize_array_length_update(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: &JsValue,
    key: &PropertyKey,
    mut update: PropertyDescriptorUpdate,
) -> Result<PropertyDescriptorUpdate, VmError> {
    if !matches!(key, PropertyKey::String(name) if name == "length") {
        return Ok(update);
    }
    let Some(object) = context.value_object(target) else {
        return Ok(update);
    };
    if context.proxy_record(object).is_some() || !context.is_array_object(object)? {
        return Ok(update);
    }
    if let Some(value) = update.value.take() {
        let number = vm.to_number(value, context)?;
        // ArraySetLength rejects fractional, negative, infinite, and >2^32-1
        // values after the observable ToNumber conversion.
        context.array_length_from_value(JsValue::Number(number))?;
        update.value = Some(JsValue::Number(number));
    }
    Ok(update)
}

fn object_get_own_property_names(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target.clone(), context)?;
    let target = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(object),
    };
    let keys: Vec<JsValue> = proxy::internal_own_property_keys(vm, context, target)?
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::String(key) => Some(JsValue::String(key)),
            PropertyKey::Symbol(_) => None,
        })
        .collect();
    context.create_array(keys)
}

fn object_get_own_property_symbols(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target.clone(), context)?;
    let target = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(object),
    };
    let keys: Vec<JsValue> = proxy::internal_own_property_keys(vm, context, target)?
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::String(_) => None,
            PropertyKey::Symbol(symbol) => Some(JsValue::Symbol(symbol)),
        })
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
    let object = vm.to_object(target.clone(), context)?;
    let target = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(object),
    };
    let mut values = Vec::new();
    for key in proxy::internal_own_property_keys(vm, context, target.clone())? {
        if !proxy::internal_get_own_property(vm, context, target.clone(), &key)?
            .is_some_and(|d| d.enumerable)
        {
            continue;
        }
        if let PropertyKey::String(name) = key {
            values.push(vm.get_property_value(target.clone(), &name, context)?);
        }
    }
    context.create_array(values)
}

fn object_entries(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = vm.to_object(target.clone(), context)?;
    let target = match target {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => target,
        _ => context.object_value(object),
    };
    let mut pairs: Vec<JsValue> = Vec::new();
    for key in proxy::internal_own_property_keys(vm, context, target.clone())? {
        if !proxy::internal_get_own_property(vm, context, target.clone(), &key)?
            .is_some_and(|d| d.enumerable)
        {
            continue;
        }
        let PropertyKey::String(name) = key else {
            continue;
        };
        let value = vm.get_property_value(target.clone(), &name, context)?;
        let pair = context.create_array(vec![JsValue::String(name), value])?;
        pairs.push(pair);
    }
    context.create_array(pairs)
}

fn object_from_entries(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = context.ordinary_object_with_prototype(context.object_prototype())?;
    let JsValue::Object(result_object) = result.clone() else {
        unreachable!()
    };
    let (iterator, next) = object_get_iterator(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    while let Some(entry) = object_iterator_step(vm, context, iterator.clone(), next.clone())? {
        let processed = (|| {
            let entry_object = context.require_object(&entry, "Object.fromEntries entry")?;
            let entry_value = context.object_value(entry_object);
            let key_value = match vm.get_property_value_catching_from_builtin(
                entry_value.clone(),
                "0",
                context,
            )? {
                Ok(value) => value,
                Err(value) => return Err(vm.throw_value_from_builtin(value)),
            };
            // AddEntriesFromIterable observes both entry property gets before
            // coercing the key.
            let value =
                match vm.get_property_value_catching_from_builtin(entry_value, "1", context)? {
                    Ok(value) => value,
                    Err(value) => return Err(vm.throw_value_from_builtin(value)),
                };
            let key = proxy::to_property_key(vm, context, key_value)?;
            Ok::<_, VmError>((key, value))
        })();
        let (key, value) = match processed {
            Ok(pair) => pair,
            Err(error) => {
                let _ = vm.close_iterator_from_builtin(iterator.clone(), context);
                return Err(error);
            }
        };
        let descriptor = PropertyDescriptor::data_with(value, true, true, true);
        match key {
            PropertyKey::String(key) => {
                context.define_own_property(result_object, key, descriptor)?;
            }
            PropertyKey::Symbol(symbol) => {
                context.define_symbol_own_property(result_object, symbol, descriptor)?;
            }
        }
    }
    Ok(result)
}

fn object_get_iterator(
    vm: &mut Vm,
    context: &mut NativeContext,
    items: JsValue,
) -> Result<(JsValue, JsValue), VmError> {
    let method = vm.get_symbol_property_value_with_receiver_from_builtin(
        items.clone(),
        items.clone(),
        context.well_known_symbols().iterator,
        context,
    )?;
    if !context.is_callable_value(&method) {
        return Err(VmError::type_error("value is not iterable"));
    }
    let iterator = vm.call_value_from_builtin(method, items, Vec::new(), context)?;
    context.require_object(&iterator, "iterator method result")?;
    let next = vm.get_property_value(iterator.clone(), "next", context)?;
    if !context.is_callable_value(&next) {
        return Err(VmError::type_error("iterator next is not callable"));
    }
    Ok((iterator, next))
}

fn object_iterator_step(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator: JsValue,
    next: JsValue,
) -> Result<Option<JsValue>, VmError> {
    let result = vm.call_value_from_builtin(next, iterator, Vec::new(), context)?;
    context.require_object(&result, "iterator result")?;
    let done = match vm.get_property_value_catching_from_builtin(result.clone(), "done", context)? {
        Ok(value) => value,
        Err(value) => return Err(vm.throw_value_from_builtin(value)),
    };
    if done.to_boolean() {
        return Ok(None);
    }
    match vm.get_property_value_catching_from_builtin(result, "value", context)? {
        Ok(value) => Ok(Some(value)),
        Err(value) => Err(vm.throw_value_from_builtin(value)),
    }
}

fn object_group_by(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let items = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let callback = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !context.is_callable_value(&callback) {
        return Err(VmError::type_error(
            "Object.groupBy callback is not callable",
        ));
    }

    let result = context.ordinary_object_with_prototype(None)?;
    let result_object = context.require_object(&result, "Object.groupBy result")?;
    let (iterator, next) = object_get_iterator(vm, context, items)?;
    let mut index = 0usize;
    while let Some(value) = object_iterator_step(vm, context, iterator.clone(), next.clone())? {
        context.consume_loop_iteration()?;
        let key_value = match vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![value.clone(), JsValue::Number(index as f64)],
            context,
        ) {
            Ok(key) => key,
            Err(error) => {
                let _ = vm.close_iterator_from_builtin(iterator.clone(), context);
                return Err(error);
            }
        };
        let key = match proxy::to_property_key(vm, context, key_value) {
            Ok(key) => key,
            Err(error) => {
                let _ = vm.close_iterator_from_builtin(iterator.clone(), context);
                return Err(error);
            }
        };
        let existing = match &key {
            PropertyKey::String(key) => context.get_own_property_descriptor(result_object, key),
            PropertyKey::Symbol(symbol) => {
                context.get_own_symbol_property_descriptor(result_object, *symbol)
            }
        };
        if let Some(group) = existing.and_then(|descriptor| descriptor.value_cloned()) {
            let group_object = context.require_object(&group, "Object.groupBy group")?;
            let length = context
                .get_own_property_descriptor(group_object, "length")
                .and_then(|descriptor| descriptor.value_cloned())
                .and_then(|value| match value {
                    JsValue::Number(value) => Some(value as usize),
                    _ => None,
                })
                .unwrap_or(0);
            context.set_property(group, length.to_string(), value)?;
        } else {
            let group = context.create_array(vec![value])?;
            let descriptor = PropertyDescriptor::data_with(group, true, true, true);
            match key {
                PropertyKey::String(key) => {
                    context.define_own_property(result_object, key, descriptor)?;
                }
                PropertyKey::Symbol(symbol) => {
                    context.define_symbol_own_property(result_object, symbol, descriptor)?;
                }
            }
        }
        index += 1;
    }
    Ok(result)
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
        let keys = proxy::internal_own_property_keys(vm, context, source_value.clone())?;
        for key in keys {
            let Some(descriptor) =
                proxy::internal_get_own_property(vm, context, source_value.clone(), &key)?
            else {
                continue;
            };
            if !descriptor.enumerable {
                continue;
            }
            let value = proxy::internal_get(
                vm,
                context,
                source_value.clone(),
                &key,
                source_value.clone(),
            )?;
            if !object_assign_set(vm, context, target_value.clone(), &key, value)? {
                return Err(VmError::type_error(format!(
                    "cannot write property {}",
                    property_key_label(&key)
                )));
            }
        }
    }
    Ok(target_value)
}

fn object_assign_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &crate::runtime::PropertyKey,
    value: JsValue,
) -> Result<bool, VmError> {
    let target_object = context.require_object(&target, "assign property")?;
    if context.proxy_record(target_object).is_some() {
        return proxy::internal_set(vm, context, target.clone(), key, value, target);
    }
    match key {
        crate::runtime::PropertyKey::String(key) => {
            vm.set_property_value_strict_from_builtin(target, key, value, context)
        }
        crate::runtime::PropertyKey::Symbol(symbol) => vm
            .set_symbol_property_value_with_receiver_from_builtin(
                target.clone(),
                target,
                *symbol,
                value,
                context,
            ),
    }
}

fn property_key_label(key: &crate::runtime::PropertyKey) -> String {
    match key {
        crate::runtime::PropertyKey::String(key) => key.clone(),
        crate::runtime::PropertyKey::Symbol(symbol) => format!("Symbol({})", symbol.0),
    }
}

fn object_freeze(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&target).is_some() {
        set_integrity_level(vm, context, target.clone(), true)?;
    }
    Ok(target)
}

fn object_seal(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&target).is_some() {
        set_integrity_level(vm, context, target.clone(), false)?;
    }
    Ok(target)
}

fn set_integrity_level(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    frozen: bool,
) -> Result<(), VmError> {
    if !proxy::internal_prevent_extensions(vm, context, target.clone())? {
        return Err(VmError::type_error("cannot prevent extensions"));
    }
    let keys = proxy::internal_own_property_keys(vm, context, target.clone())?;
    for key in keys {
        let Some(current) = proxy::internal_get_own_property(vm, context, target.clone(), &key)?
        else {
            continue;
        };
        let update = PropertyDescriptorUpdate {
            configurable: Some(false),
            writable: (frozen && matches!(current.kind, PropertyKind::Data { .. }))
                .then_some(false),
            ..PropertyDescriptorUpdate::default()
        };
        let descriptor_arg = proxy::descriptor_object_from_update(context, &update)?;
        if !proxy::internal_define_own_property(
            vm,
            context,
            target.clone(),
            &key,
            descriptor_arg,
            update,
        )? {
            return Err(VmError::type_error("cannot set object integrity level"));
        }
    }
    Ok(())
}

fn object_prevent_extensions(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&target).is_some()
        && !proxy::internal_prevent_extensions(vm, context, target.clone())?
    {
        return Err(VmError::type_error("cannot prevent extensions"));
    }
    Ok(target)
}

fn object_is_extensible(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&target).is_none() {
        return Ok(JsValue::Boolean(false));
    }
    Ok(JsValue::Boolean(proxy::internal_is_extensible(
        vm, context, target,
    )?))
}

fn object_is_frozen(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&target).is_none() {
        return Ok(JsValue::Boolean(true));
    }
    if proxy::internal_is_extensible(vm, context, target.clone())? {
        return Ok(JsValue::Boolean(false));
    }
    for key in proxy::internal_own_property_keys(vm, context, target.clone())? {
        if proxy::internal_get_own_property(vm, context, target.clone(), &key)?.is_some_and(|d| {
            d.configurable || matches!(d.kind, PropertyKind::Data { writable: true, .. })
        }) {
            return Ok(JsValue::Boolean(false));
        }
    }
    Ok(JsValue::Boolean(true))
}

fn object_is_sealed(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&target).is_none() {
        return Ok(JsValue::Boolean(true));
    }
    if proxy::internal_is_extensible(vm, context, target.clone())? {
        return Ok(JsValue::Boolean(false));
    }
    for key in proxy::internal_own_property_keys(vm, context, target.clone())? {
        if proxy::internal_get_own_property(vm, context, target.clone(), &key)?
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
