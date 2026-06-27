//! ECMAScript Proxy constructor and shared internal-method adapters.

use crate::{
    runtime::{
        JsObject, JsValue, NativeContext, ObjectId, ObjectKind, PropertyDescriptor,
        PropertyDescriptorUpdate, PropertyKey, PropertyKind,
    },
    vm::{Vm, VmError},
};

pub(super) fn install(context: &mut NativeContext) -> Result<(), VmError> {
    let constructor = context.register_builtin("Proxy", 2, proxy_call, Some(proxy_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Proxy constructor object missing"))?;
    let revocable = context.register_builtin("revocable", 2, proxy_revocable, None)?;
    context.define_own_property(
        constructor_object,
        "revocable".into(),
        PropertyDescriptor::data_with(revocable, true, false, true),
    )?;
    context.declare_global("Proxy", constructor.clone());
    context.define_own_property(
        context.global_object(),
        "Proxy".into(),
        PropertyDescriptor::data_with(constructor.clone(), true, false, true),
    )?;

    Ok(())
}

fn proxy_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("Proxy constructor requires 'new'"))
}

fn proxy_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    let target = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let handler = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    context.require_object(&target, "create Proxy target")?;
    context.require_object(&handler, "create Proxy handler")?;

    let callable = context.is_callable_value(&target);
    let constructable = context.is_constructable_value(&target);
    let mut object = JsObject::proxy(target.clone(), handler, callable, constructable);
    object.prototype = context
        .value_object(&target)
        .and_then(|target| context.get_prototype_of(target))
        .or_else(|| context.object_prototype());
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
    Ok(JsValue::Object(id))
}

fn proxy_revocable(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let proxy = proxy_construct(vm, context, arguments, JsValue::Undefined)?;
    let revoke_target =
        context.register_builtin("Proxy.revocable.revoke", 0, proxy_revoke, None)?;
    let revoke = context.register_bound_function(
        revoke_target,
        JsValue::Undefined,
        vec![proxy.clone()],
        0.0,
        "bound Proxy.revocable.revoke".into(),
    )?;
    let revoke_object = if let JsValue::BuiltinFunction(id) = &revoke {
        context.builtin_mut(*id).map(|function| {
            function.name = "";
            function.construct = None;
            function.object
        })
    } else {
        None
    };
    if let Some(object) = revoke_object {
        context.define_own_property(
            object,
            "name".into(),
            PropertyDescriptor::data_with(JsValue::String(String::new()), false, false, true),
        )?;
    }
    context.create_object([("proxy".into(), proxy), ("revoke".into(), revoke)])
}

fn proxy_revoke(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let proxy = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let Some(object) = context.value_object(&proxy) else {
        return Ok(JsValue::Undefined);
    };
    let Some(object) = context.heap_mut().object_mut(object) else {
        return Ok(JsValue::Undefined);
    };
    if let ObjectKind::Proxy { record } = &mut object.kind {
        record.target = JsValue::Null;
        record.handler = JsValue::Null;
    }
    Ok(JsValue::Undefined)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trap {
    Apply,
    Construct,
    DefineProperty,
    DeleteProperty,
    Get,
    GetOwnPropertyDescriptor,
    GetPrototypeOf,
    Has,
    IsExtensible,
    OwnKeys,
    PreventExtensions,
    Set,
    SetPrototypeOf,
}

impl Trap {
    const fn name(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Construct => "construct",
            Self::DefineProperty => "defineProperty",
            Self::DeleteProperty => "deleteProperty",
            Self::Get => "get",
            Self::GetOwnPropertyDescriptor => "getOwnPropertyDescriptor",
            Self::GetPrototypeOf => "getPrototypeOf",
            Self::Has => "has",
            Self::IsExtensible => "isExtensible",
            Self::OwnKeys => "ownKeys",
            Self::PreventExtensions => "preventExtensions",
            Self::Set => "set",
            Self::SetPrototypeOf => "setPrototypeOf",
        }
    }
}

pub(crate) fn to_property_key(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<PropertyKey, VmError> {
    match vm.to_property_key_from_builtin(value, context)? {
        JsValue::String(key) => Ok(PropertyKey::String(key)),
        JsValue::Symbol(symbol) => Ok(PropertyKey::Symbol(symbol)),
        _ => unreachable!("ToPropertyKey returns a string or symbol"),
    }
}

pub(crate) fn internal_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    this_argument: JsValue,
    arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return vm.call_value_from_builtin(target, this_argument, arguments, context);
    };
    if !record.callable {
        return Err(VmError::type_error("proxy target is not callable"));
    }
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::Apply)? else {
        return vm.call_value_from_builtin(record.target, this_argument, arguments, context);
    };
    let argument_array = context.create_array(arguments)?;
    vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target, this_argument, argument_array],
        context,
    )
}

pub(crate) fn internal_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    arguments: Vec<JsValue>,
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return vm
            .construct_value_from_builtin_with_new_target(target, arguments, new_target, context);
    };
    if !record.constructable {
        return Err(VmError::type_error("proxy target is not a constructor"));
    }
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::Construct)? else {
        return vm.construct_value_from_builtin_with_new_target(
            record.target,
            arguments,
            new_target,
            context,
        );
    };
    let argument_array = context.create_array(arguments)?;
    let result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target, argument_array, new_target],
        context,
    )?;
    if context.value_object(&result).is_none() {
        return Err(VmError::type_error(
            "proxy construct trap must return an object",
        ));
    }
    Ok(result)
}

pub(crate) fn internal_get_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<Option<ObjectId>, VmError> {
    get_prototype_of(vm, context, target)
}

pub(crate) fn internal_set_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    prototype: Option<ObjectId>,
) -> Result<bool, VmError> {
    set_prototype_of(vm, context, target, prototype)
}

pub(crate) fn internal_is_extensible(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<bool, VmError> {
    is_extensible(vm, context, target)
}

pub(crate) fn internal_prevent_extensions(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<bool, VmError> {
    prevent_extensions(vm, context, target)
}

pub(crate) fn internal_get_own_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<Option<PropertyDescriptor>, VmError> {
    get_own_property_descriptor(vm, context, target, key)
}

pub(crate) fn internal_define_own_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    descriptor_arg: JsValue,
    update: PropertyDescriptorUpdate,
) -> Result<bool, VmError> {
    define_own_property(vm, context, target, key, descriptor_arg, update)
}

pub(crate) fn internal_has_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<bool, VmError> {
    has_property(vm, context, target, key)
}

pub(crate) fn internal_get(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    receiver: JsValue,
) -> Result<JsValue, VmError> {
    get(vm, context, target, key, receiver)
}

pub(crate) fn internal_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    value: JsValue,
    receiver: JsValue,
) -> Result<bool, VmError> {
    set(vm, context, target, key, value, receiver)
}

pub(crate) fn internal_delete(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<bool, VmError> {
    delete_property(vm, context, target, key)
}

pub(crate) fn internal_own_property_keys(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<Vec<PropertyKey>, VmError> {
    own_property_keys(vm, context, target)
}

pub(crate) fn internal_for_in_keys(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<Vec<String>, VmError> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    let mut current = Some(target);
    let mut depth = 0usize;
    while let Some(value) = current {
        if depth > 1024 {
            return Err(VmError::runtime_limit("prototype chain limit exceeded"));
        }
        for key in own_property_keys(vm, context, value.clone())? {
            let PropertyKey::String(name) = key else {
                continue;
            };
            if !seen.insert(name.clone()) {
                continue;
            }
            if get_own_property_descriptor(
                vm,
                context,
                value.clone(),
                &PropertyKey::String(name.clone()),
            )?
            .is_some_and(|descriptor| descriptor.enumerable)
            {
                result.push(name);
            }
        }
        current =
            get_prototype_of(vm, context, value)?.map(|prototype| context.object_value(prototype));
        depth += 1;
    }
    Ok(result)
}

fn define_own_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    descriptor_arg: JsValue,
    update: PropertyDescriptorUpdate,
) -> Result<bool, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return ordinary_define_own_property(context, target, key, update);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::DefineProperty)? else {
        return define_own_property(vm, context, record.target, key, descriptor_arg, update);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone(), key.to_value(), descriptor_arg],
        context,
    )?;
    let accepted = trap_result.to_boolean();
    if !accepted {
        return Ok(false);
    }
    let target_desc = get_own_property_descriptor(vm, context, record.target.clone(), key)?;
    let target_extensible = is_extensible(vm, context, record.target.clone())?;
    if !is_compatible_property_descriptor(target_extensible, &update, target_desc.as_ref()) {
        return Err(VmError::type_error(
            "proxy defineProperty descriptor is not compatible with target",
        ));
    }
    if target_desc.is_none() && !target_extensible {
        return Err(VmError::type_error(
            "proxy defineProperty cannot add property to non-extensible target",
        ));
    }
    if update.configurable == Some(false)
        && target_desc
            .as_ref()
            .is_none_or(|descriptor| descriptor.configurable)
    {
        return Err(VmError::type_error(
            "proxy defineProperty cannot create non-configurable property",
        ));
    }
    if update.writable == Some(false)
        && target_desc.as_ref().is_some_and(|descriptor| {
            !descriptor.configurable
                && matches!(descriptor.kind, PropertyKind::Data { writable: true, .. })
        })
    {
        return Err(VmError::type_error(
            "proxy defineProperty cannot report non-configurable non-writable property",
        ));
    }
    Ok(true)
}

fn delete_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<bool, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return ordinary_delete_property(context, target, key);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::DeleteProperty)? else {
        return delete_property(vm, context, record.target, key);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone(), key.to_value()],
        context,
    )?;
    let deleted = trap_result.to_boolean();
    if !deleted {
        return Ok(false);
    }
    if let Some(descriptor) = get_own_property_descriptor(vm, context, record.target.clone(), key)?
    {
        if !descriptor.configurable {
            return Err(VmError::type_error(
                "proxy deleteProperty cannot report success for non-configurable property",
            ));
        }
        if !is_extensible(vm, context, record.target)? {
            return Err(VmError::type_error(
                "proxy deleteProperty cannot report success on a non-extensible target",
            ));
        }
    }
    Ok(true)
}

fn get(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    receiver: JsValue,
) -> Result<JsValue, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return ordinary_get(vm, context, target, key, receiver);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::Get)? else {
        return get(vm, context, record.target, key, receiver);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone(), key.to_value(), receiver],
        context,
    )?;
    if let Some(target_desc) = get_own_property_descriptor(vm, context, record.target.clone(), key)?
    {
        match target_desc.kind {
            PropertyKind::Data { value, writable }
                if !target_desc.configurable && !writable && !trap_result.same_value(&value) =>
            {
                return Err(VmError::type_error(
                    "proxy get returned a different value for a frozen data property",
                ));
            }
            PropertyKind::Accessor { get: None, .. }
                if !target_desc.configurable && !matches!(trap_result, JsValue::Undefined) =>
            {
                return Err(VmError::type_error(
                    "proxy get returned a value for an accessor without getter",
                ));
            }
            _ => {}
        }
    }
    Ok(trap_result)
}

fn get_own_property_descriptor(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<Option<PropertyDescriptor>, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return ordinary_get_own_property_descriptor(context, target, key);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::GetOwnPropertyDescriptor)? else {
        return get_own_property_descriptor(vm, context, record.target, key);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone(), key.to_value()],
        context,
    )?;
    let target_desc = get_own_property_descriptor(vm, context, record.target.clone(), key)?;
    if matches!(trap_result, JsValue::Undefined) {
        if target_desc
            .as_ref()
            .is_some_and(|descriptor| !descriptor.configurable)
        {
            return Err(VmError::type_error(
                "proxy getOwnPropertyDescriptor cannot hide non-configurable property",
            ));
        }
        if target_desc.is_some() && !is_extensible(vm, context, record.target)? {
            return Err(VmError::type_error(
                "proxy getOwnPropertyDescriptor cannot hide property on non-extensible target",
            ));
        }
        return Ok(None);
    }
    context.require_object(&trap_result, "read proxy descriptor result")?;
    let descriptor = descriptor_from_object(vm, context, trap_result)?;
    if target_desc.is_none() && !is_extensible(vm, context, record.target.clone())? {
        return Err(VmError::type_error(
            "proxy getOwnPropertyDescriptor cannot report extra property",
        ));
    }
    if !descriptor.configurable
        && target_desc
            .as_ref()
            .is_none_or(|target_desc| target_desc.configurable)
    {
        return Err(VmError::type_error(
            "proxy getOwnPropertyDescriptor cannot report non-configurable property",
        ));
    }
    if !descriptor.configurable
        && matches!(
            descriptor.kind,
            PropertyKind::Data {
                writable: false,
                ..
            }
        )
        && target_desc.as_ref().is_some_and(|target_desc| {
            matches!(target_desc.kind, PropertyKind::Data { writable: true, .. })
        })
    {
        return Err(VmError::type_error(
            "proxy getOwnPropertyDescriptor cannot report non-writable for writable target",
        ));
    }
    if descriptor.configurable
        && target_desc
            .as_ref()
            .is_some_and(|target_desc| !target_desc.configurable)
    {
        return Err(VmError::type_error(
            "proxy getOwnPropertyDescriptor cannot loosen non-configurable property",
        ));
    }
    Ok(Some(descriptor))
}

fn get_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<Option<ObjectId>, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        let object = context.require_object(&target, "get prototype")?;
        return Ok(context.get_prototype_of(object));
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::GetPrototypeOf)? else {
        return get_prototype_of(vm, context, record.target);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone()],
        context,
    )?;
    let prototype = match trap_result {
        JsValue::Null => None,
        value => Some(context.require_object(&value, "use proxy prototype result")?),
    };
    if !is_extensible(vm, context, record.target.clone())?
        && get_prototype_of(vm, context, record.target)? != prototype
    {
        return Err(VmError::type_error(
            "proxy getPrototypeOf result does not match non-extensible target",
        ));
    }
    Ok(prototype)
}

fn has_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<bool, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return ordinary_has_property(vm, context, target, key);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::Has)? else {
        return has_property(vm, context, record.target, key);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone(), key.to_value()],
        context,
    )?;
    let found = trap_result.to_boolean();
    if !found {
        if get_own_property_descriptor(vm, context, record.target.clone(), key)?
            .is_some_and(|descriptor| !descriptor.configurable)
        {
            return Err(VmError::type_error(
                "proxy has cannot hide non-configurable property",
            ));
        }
        if get_own_property_descriptor(vm, context, record.target.clone(), key)?.is_some()
            && !is_extensible(vm, context, record.target)?
        {
            return Err(VmError::type_error(
                "proxy has cannot hide property on non-extensible target",
            ));
        }
    }
    Ok(found)
}

fn is_extensible(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<bool, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        let object = context.require_object(&target, "test extensibility")?;
        return context.is_extensible(object);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::IsExtensible)? else {
        return is_extensible(vm, context, record.target);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone()],
        context,
    )?;
    let result = trap_result.to_boolean();
    if result != is_extensible(vm, context, record.target)? {
        return Err(VmError::type_error(
            "proxy isExtensible result must match target",
        ));
    }
    Ok(result)
}

fn own_property_keys(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<Vec<PropertyKey>, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return ordinary_own_property_keys(context, target);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::OwnKeys)? else {
        return own_property_keys(vm, context, record.target);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone()],
        context,
    )?;
    let trap_keys = create_list_from_array_like(vm, context, trap_result)?;
    validate_unique_keys(&trap_keys)?;

    let target_keys = own_property_keys(vm, context, record.target.clone())?;
    let mut non_configurable = Vec::new();
    for key in &target_keys {
        if get_own_property_descriptor(vm, context, record.target.clone(), key)?
            .is_some_and(|descriptor| !descriptor.configurable)
        {
            non_configurable.push(key.clone());
        }
    }
    for key in &non_configurable {
        if !trap_keys.contains(key) {
            return Err(VmError::type_error(
                "proxy ownKeys result omitted non-configurable property",
            ));
        }
    }
    if !is_extensible(vm, context, record.target)? {
        for key in &target_keys {
            if !trap_keys.contains(key) {
                return Err(VmError::type_error(
                    "proxy ownKeys result omitted non-extensible target property",
                ));
            }
        }
        for key in &trap_keys {
            if !target_keys.contains(key) {
                return Err(VmError::type_error(
                    "proxy ownKeys result added property for non-extensible target",
                ));
            }
        }
    }
    Ok(trap_keys)
}

fn prevent_extensions(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
) -> Result<bool, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        let object = context.require_object(&target, "prevent extensions")?;
        return context.prevent_extensions(object);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::PreventExtensions)? else {
        return prevent_extensions(vm, context, record.target);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone()],
        context,
    )?;
    let prevented = trap_result.to_boolean();
    if prevented && is_extensible(vm, context, record.target)? {
        return Err(VmError::type_error(
            "proxy preventExtensions returned true for extensible target",
        ));
    }
    Ok(prevented)
}

fn set(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    value: JsValue,
    receiver: JsValue,
) -> Result<bool, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        return ordinary_set(vm, context, target, key, value, receiver);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::Set)? else {
        return set(vm, context, record.target, key, value, receiver);
    };
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![
            record.target.clone(),
            key.to_value(),
            value.clone(),
            receiver,
        ],
        context,
    )?;
    let accepted = trap_result.to_boolean();
    if !accepted {
        return Ok(false);
    }
    if let Some(target_desc) = get_own_property_descriptor(vm, context, record.target.clone(), key)?
    {
        match target_desc.kind {
            PropertyKind::Data {
                value: target_value,
                writable: false,
            } if !target_desc.configurable && !value.same_value(&target_value) => {
                return Err(VmError::type_error(
                    "proxy set cannot change frozen data property",
                ));
            }
            PropertyKind::Accessor { set: None, .. } if !target_desc.configurable => {
                return Err(VmError::type_error(
                    "proxy set cannot report success for accessor without setter",
                ));
            }
            _ => {}
        }
    }
    Ok(true)
}

fn set_prototype_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    prototype: Option<ObjectId>,
) -> Result<bool, VmError> {
    let Some(record) = proxy_record(context, &target)? else {
        let object = context.require_object(&target, "set prototype")?;
        return context.set_prototype_of(object, prototype);
    };
    let Some(trap) = get_trap(vm, context, &record.handler, Trap::SetPrototypeOf)? else {
        return set_prototype_of(vm, context, record.target, prototype);
    };
    let prototype_value = prototype.map_or(JsValue::Null, |object| context.object_value(object));
    let trap_result = vm.call_value_from_builtin(
        trap,
        record.handler.clone(),
        vec![record.target.clone(), prototype_value],
        context,
    )?;
    let accepted = trap_result.to_boolean();
    if !accepted {
        return Ok(false);
    }
    if !is_extensible(vm, context, record.target.clone())?
        && get_prototype_of(vm, context, record.target)? != prototype
    {
        return Err(VmError::type_error(
            "proxy setPrototypeOf changed a non-extensible target",
        ));
    }
    Ok(true)
}

fn proxy_record(
    context: &NativeContext,
    value: &JsValue,
) -> Result<Option<crate::runtime::ProxyRecord>, VmError> {
    let Some(object) = context.value_object(value) else {
        return Ok(None);
    };
    let Some(record) = context.proxy_record(object) else {
        return Ok(None);
    };
    if matches!(record.handler, JsValue::Null) {
        return Err(VmError::type_error("proxy has been revoked"));
    }
    Ok(Some(record))
}

fn get_trap(
    vm: &mut Vm,
    context: &mut NativeContext,
    handler: &JsValue,
    trap: Trap,
) -> Result<Option<JsValue>, VmError> {
    let value = vm.get_property_value_with_receiver_from_builtin(
        handler.clone(),
        handler.clone(),
        trap.name(),
        context,
    )?;
    if matches!(value, JsValue::Undefined | JsValue::Null) {
        return Ok(None);
    }
    if context.is_callable_value(&value) {
        return Ok(Some(value));
    }
    Err(VmError::type_error(format!(
        "proxy trap {} is not callable",
        trap.name()
    )))
}

fn ordinary_define_own_property(
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    update: PropertyDescriptorUpdate,
) -> Result<bool, VmError> {
    let object = context.require_object(&target, "define property")?;
    match key {
        PropertyKey::String(key) => {
            context.validate_and_apply_property_descriptor(object, key.clone(), update)
        }
        PropertyKey::Symbol(symbol) => {
            context.validate_and_apply_symbol_property_descriptor(object, *symbol, update)
        }
    }
}

fn ordinary_delete_property(
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<bool, VmError> {
    let object = context.require_object(&target, "delete property")?;
    match key {
        PropertyKey::String(key) => context.delete_property(object, key, false),
        PropertyKey::Symbol(symbol) => context.delete_symbol_property(object, *symbol, false),
    }
}

fn ordinary_get(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    receiver: JsValue,
) -> Result<JsValue, VmError> {
    let object = context.require_object(&target, "get property")?;
    if let Some(descriptor) = match key {
        PropertyKey::String(key) => context.get_own_property_descriptor(object, key),
        PropertyKey::Symbol(symbol) => context.get_own_symbol_property_descriptor(object, *symbol),
    } {
        return match descriptor.kind {
            PropertyKind::Data { value, .. } => Ok(value),
            PropertyKind::Accessor { get: None, .. } => Ok(JsValue::Undefined),
            PropertyKind::Accessor {
                get: Some(getter), ..
            } => vm.call_value_from_builtin(getter, receiver, Vec::new(), context),
        };
    }
    let Some(parent) = get_prototype_of(vm, context, target)? else {
        return Ok(JsValue::Undefined);
    };
    get(vm, context, context.object_value(parent), key, receiver)
}

fn ordinary_get_own_property_descriptor(
    context: &NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<Option<PropertyDescriptor>, VmError> {
    let object = context.require_object(&target, "get own property descriptor")?;
    Ok(match key {
        PropertyKey::String(key) => context.get_own_property_descriptor(object, key),
        PropertyKey::Symbol(symbol) => context.get_own_symbol_property_descriptor(object, *symbol),
    })
}

fn ordinary_has_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
) -> Result<bool, VmError> {
    let object = context.require_object(&target, "test property")?;
    let has_own = match key {
        PropertyKey::String(key) => context.get_own_property_descriptor(object, key).is_some(),
        PropertyKey::Symbol(symbol) => context
            .get_own_symbol_property_descriptor(object, *symbol)
            .is_some(),
    };
    if has_own {
        return Ok(true);
    }
    let Some(parent) = get_prototype_of(vm, context, target)? else {
        return Ok(false);
    };
    has_property(vm, context, context.object_value(parent), key)
}

fn ordinary_own_property_keys(
    context: &NativeContext,
    target: JsValue,
) -> Result<Vec<PropertyKey>, VmError> {
    let object = context.require_object(&target, "own keys")?;
    let heap_object = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    let mut keys: Vec<PropertyKey> = heap_object
        .own_property_keys()
        .into_iter()
        .map(PropertyKey::String)
        .collect();
    keys.extend(
        heap_object
            .symbol_properties
            .iter()
            .map(|(symbol, _)| PropertyKey::Symbol(*symbol)),
    );
    Ok(keys)
}

fn ordinary_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    value: JsValue,
    receiver: JsValue,
) -> Result<bool, VmError> {
    let own_desc = ordinary_get_own_property_descriptor(context, target.clone(), key)?;
    ordinary_set_with_own_descriptor(vm, context, target, key, value, receiver, own_desc)
}

fn ordinary_set_with_own_descriptor(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    key: &PropertyKey,
    value: JsValue,
    receiver: JsValue,
    own_desc: Option<PropertyDescriptor>,
) -> Result<bool, VmError> {
    let descriptor = if let Some(descriptor) = own_desc {
        descriptor
    } else if let Some(parent) = get_prototype_of(vm, context, target)? {
        return set(
            vm,
            context,
            context.object_value(parent),
            key,
            value,
            receiver,
        );
    } else {
        PropertyDescriptor::data_with(JsValue::Undefined, true, true, true)
    };

    match descriptor.kind {
        PropertyKind::Accessor {
            set: Some(setter), ..
        } => {
            let _ = vm.call_value_from_builtin(setter, receiver, vec![value], context)?;
            Ok(true)
        }
        PropertyKind::Accessor { set: None, .. } => Ok(false),
        PropertyKind::Data {
            writable: false, ..
        } => Ok(false),
        PropertyKind::Data { .. } => {
            if context.value_object(&receiver).is_none() {
                return Ok(false);
            }
            let receiver_desc = get_own_property_descriptor(vm, context, receiver.clone(), key)?;
            let update = if let Some(receiver_desc) = receiver_desc {
                match receiver_desc.kind {
                    PropertyKind::Accessor { .. } => return Ok(false),
                    PropertyKind::Data {
                        writable: false, ..
                    } => return Ok(false),
                    PropertyKind::Data { .. } => PropertyDescriptorUpdate {
                        value: Some(value),
                        ..PropertyDescriptorUpdate::default()
                    },
                }
            } else {
                PropertyDescriptorUpdate {
                    value: Some(value),
                    writable: Some(true),
                    enumerable: Some(true),
                    configurable: Some(true),
                    ..PropertyDescriptorUpdate::default()
                }
            };
            let descriptor_arg = descriptor_object_from_update(context, &update)?;
            define_own_property(vm, context, receiver, key, descriptor_arg, update)
        }
    }
}

fn descriptor_object_from_update(
    context: &mut NativeContext,
    update: &PropertyDescriptorUpdate,
) -> Result<JsValue, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = context
        .intrinsics()
        .map(|intrinsics| intrinsics.object_prototype);
    if let Some(value) = &update.value {
        define_descriptor_field(&mut object, "value", value.clone());
    }
    if let Some(writable) = update.writable {
        define_descriptor_field(&mut object, "writable", JsValue::Boolean(writable));
    }
    if let Some(enumerable) = update.enumerable {
        define_descriptor_field(&mut object, "enumerable", JsValue::Boolean(enumerable));
    }
    if let Some(configurable) = update.configurable {
        define_descriptor_field(&mut object, "configurable", JsValue::Boolean(configurable));
    }
    if let Some(get) = &update.get {
        define_descriptor_field(
            &mut object,
            "get",
            get.clone().unwrap_or(JsValue::Undefined),
        );
    }
    if let Some(set) = &update.set {
        define_descriptor_field(
            &mut object,
            "set",
            set.clone().unwrap_or(JsValue::Undefined),
        );
    }
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("object arena exhausted"))?;
    Ok(JsValue::Object(id))
}

fn define_descriptor_field(object: &mut JsObject, name: &str, value: JsValue) {
    object.define_property(name, PropertyDescriptor::data_with(value, true, true, true));
}

fn create_list_from_array_like(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Vec<PropertyKey>, VmError> {
    let object = context.require_object(&value, "read proxy ownKeys result")?;
    let object_value = context.object_value(object);
    let length_value = vm.get_property_value_with_receiver_from_builtin(
        object_value.clone(),
        object_value.clone(),
        "length",
        context,
    )?;
    let length_number = vm.to_number(length_value, context)?;
    let length = if !length_number.is_finite() || length_number <= 0.0 {
        0
    } else {
        length_number.floor() as usize
    };
    if length > 1_000_000 {
        return Err(VmError::range("proxy ownKeys result is too large"));
    }
    let mut keys = Vec::with_capacity(length);
    for index in 0..length {
        let key = index.to_string();
        let value = vm.get_property_value_with_receiver_from_builtin(
            object_value.clone(),
            object_value.clone(),
            &key,
            context,
        )?;
        match value {
            JsValue::String(key) => keys.push(PropertyKey::String(key)),
            JsValue::Symbol(symbol) => keys.push(PropertyKey::Symbol(symbol)),
            _ => {
                return Err(VmError::type_error(
                    "proxy ownKeys result entries must be strings or symbols",
                ));
            }
        }
    }
    Ok(keys)
}

fn validate_unique_keys(keys: &[PropertyKey]) -> Result<(), VmError> {
    for (index, key) in keys.iter().enumerate() {
        if keys[index + 1..].contains(key) {
            return Err(VmError::type_error(
                "proxy ownKeys result contains duplicate entries",
            ));
        }
    }
    Ok(())
}

fn descriptor_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<PropertyDescriptor, VmError> {
    let object = context.require_object(&value, "read property descriptor")?;
    let update = descriptor_update_from_object(vm, context, object)?;
    complete_descriptor(update)
}

fn descriptor_update_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
) -> Result<PropertyDescriptorUpdate, VmError> {
    let mut update = PropertyDescriptorUpdate::default();
    if let Some(value) = descriptor_field(vm, context, object, "value")? {
        update.value = Some(value);
    }
    if let Some(value) = descriptor_field(vm, context, object, "writable")? {
        update.writable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, object, "enumerable")? {
        update.enumerable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, object, "configurable")? {
        update.configurable = Some(value.to_boolean());
    }
    if let Some(value) = descriptor_field(vm, context, object, "get")? {
        update.get = Some(optional_callable(context, value, "getter")?);
    }
    if let Some(value) = descriptor_field(vm, context, object, "set")? {
        update.set = Some(optional_callable(context, value, "setter")?);
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

fn complete_descriptor(update: PropertyDescriptorUpdate) -> Result<PropertyDescriptor, VmError> {
    if descriptor_update_has_data(&update) && descriptor_update_has_accessor(&update) {
        return Err(VmError::type_error("invalid mixed property descriptor"));
    }
    let enumerable = update.enumerable.unwrap_or(false);
    let configurable = update.configurable.unwrap_or(false);
    if descriptor_update_has_accessor(&update) {
        return Ok(PropertyDescriptor::accessor(
            update.get.unwrap_or(None),
            update.set.unwrap_or(None),
            enumerable,
            configurable,
        ));
    }
    Ok(PropertyDescriptor::data_with(
        update.value.unwrap_or(JsValue::Undefined),
        update.writable.unwrap_or(false),
        enumerable,
        configurable,
    ))
}

fn descriptor_update_has_data(update: &PropertyDescriptorUpdate) -> bool {
    update.value.is_some() || update.writable.is_some()
}

fn descriptor_update_has_accessor(update: &PropertyDescriptorUpdate) -> bool {
    update.get.is_some() || update.set.is_some()
}

fn same_optional_value(left: Option<&JsValue>, right: Option<&JsValue>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.same_value(right),
        (None, None) => true,
        _ => false,
    }
}

fn is_compatible_property_descriptor(
    extensible: bool,
    update: &PropertyDescriptorUpdate,
    current: Option<&PropertyDescriptor>,
) -> bool {
    let Some(current) = current else {
        return extensible;
    };

    if current.configurable {
        return true;
    }
    if update.configurable == Some(true) {
        return false;
    }
    if let Some(enumerable) = update.enumerable
        && enumerable != current.enumerable
    {
        return false;
    }

    match &current.kind {
        PropertyKind::Data { value, writable } => {
            if descriptor_update_has_accessor(update) {
                return false;
            }
            if !*writable {
                if update.writable == Some(true) {
                    return false;
                }
                if let Some(new_value) = &update.value
                    && !value.same_value(new_value)
                {
                    return false;
                }
            }
        }
        PropertyKind::Accessor { get, set } => {
            if descriptor_update_has_data(update) {
                return false;
            }
            if let Some(new_get) = &update.get
                && !same_optional_value(get.as_ref(), new_get.as_ref())
            {
                return false;
            }
            if let Some(new_set) = &update.set
                && !same_optional_value(set.as_ref(), new_set.as_ref())
            {
                return false;
            }
        }
    }
    true
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
