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
//! `Object` constructor, prototype bootstrap, and C1 static methods.

use crate::{
    runtime::{JsObject, JsValue, NativeContext, PropertyDescriptor, PropertyKind},
    vm::VmError,
};

pub fn install_object(context: &mut NativeContext) {
    install_static_methods(context).expect("Object static method installation must succeed");
}

fn install_static_methods(context: &mut NativeContext) -> Result<(), VmError> {
    let obj_backing = match context.get_global("Object") {
        Some(JsValue::BuiltinFunction(bid)) => context.builtin(bid).unwrap().object,
        _ => return Ok(()),
    };

    macro_rules! add_static {
        ($name:literal, $len:literal, $call:expr) => {{
            let val = context.register_builtin($name, $len, $call, None)?;
            context.define_own_property(
                obj_backing,
                $name.into(),
                PropertyDescriptor::data_with(val, true, false, true),
            )?;
        }};
    }

    add_static!("create", 2, object_create);
    add_static!("defineProperty", 3, object_define_property);
    add_static!(
        "getOwnPropertyDescriptor",
        2,
        object_get_own_property_descriptor
    );
    add_static!("getPrototypeOf", 1, object_get_prototype_of);
    add_static!("setPrototypeOf", 2, object_set_prototype_of);
    add_static!("keys", 1, object_keys);

    Ok(())
}

// ---------------------------------------------------------------------------
// C0 — Object call / construct
// ---------------------------------------------------------------------------

pub fn object_call(
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined | JsValue::Null => new_ordinary_object(context, None),
        other @ (JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)) => {
            Ok(other)
        }
        primitive => {
            // ToObject coercion of primitives is unsupported in the native engine.
            Err(VmError::runtime(format!(
                "Object({}) — primitive coercion not yet supported",
                primitive.type_of()
            )))
        }
    }
}

pub fn object_construct(
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    let proto = context.intrinsics().map(|i| i.object_prototype);
    match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined | JsValue::Null => new_ordinary_object(context, proto),
        other @ (JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)) => {
            Ok(other)
        }
        primitive => Err(VmError::runtime(format!(
            "new Object({}) — primitive coercion not yet supported",
            primitive.type_of()
        ))),
    }
}

fn new_ordinary_object(
    context: &mut NativeContext,
    prototype: Option<crate::runtime::ObjectId>,
) -> Result<JsValue, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = prototype;
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    Ok(JsValue::Object(id))
}

// ---------------------------------------------------------------------------
// C1 — Object static methods
// ---------------------------------------------------------------------------

fn object_create(
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let proto = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let prototype = match proto {
        JsValue::Null => None,
        JsValue::Object(id) => Some(id),
        other => {
            return Err(VmError::type_error(format!(
                "Object.create: prototype must be an object or null, got {}",
                other.type_of()
            )));
        }
    };

    let mut object = JsObject::ordinary();
    object.prototype = prototype;
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    // Optional propertiesObject argument
    if let Some(JsValue::Object(props_id)) = arguments.get(1) {
        let props_id = *props_id;
        let prop_keys: Vec<String> = context
            .heap()
            .object(props_id)
            .map(|o| o.own_property_keys())
            .unwrap_or_default();
        for key in prop_keys {
            let desc_obj_val = context
                .heap()
                .object(props_id)
                .and_then(|o| o.get_own_property_value(&key))
                .unwrap_or(JsValue::Undefined);
            if let JsValue::Object(desc_id) = desc_obj_val {
                let descriptor = descriptor_from_heap(context, desc_id)?;
                context.define_own_property(id, key, descriptor)?;
            }
        }
    }

    Ok(JsValue::Object(id))
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
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object_id = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Object(id) => id,
        other => {
            return Err(VmError::type_error(format!(
                "Object.defineProperty: first argument must be an object, got {}",
                other.type_of()
            )));
        }
    };
    let key = match arguments.get(1).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::String(s) => s,
        other => other.to_js_string().ok_or_else(|| {
            VmError::type_error("Object.defineProperty: key cannot be converted to string")
        })?,
    };
    let descriptor_val = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    let descriptor_id = match descriptor_val {
        JsValue::Object(id) => id,
        other => {
            return Err(VmError::type_error(format!(
                "Object.defineProperty: descriptor must be an object, got {}",
                other.type_of()
            )));
        }
    };

    let descriptor = descriptor_from_heap(context, descriptor_id)?;
    context.define_own_property(object_id, key, descriptor)?;
    Ok(JsValue::Object(object_id))
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
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object_id = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Object(id) => id,
        _ => return Ok(JsValue::Undefined),
    };
    let key = match arguments.get(1).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::String(s) => s,
        other => other
            .to_js_string()
            .ok_or_else(|| VmError::type_error("invalid property key"))?,
    };

    let descriptor = context
        .heap()
        .object(object_id)
        .and_then(|o| o.own_property(&key))
        .cloned();

    match descriptor {
        None => Ok(JsValue::Undefined),
        Some(desc) => build_descriptor_object(context, &desc),
    }
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
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object_id = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Object(id) => id,
        other => {
            return Err(VmError::type_error(format!(
                "Object.getPrototypeOf: argument must be an object, got {}",
                other.type_of()
            )));
        }
    };
    let proto = context.heap().object(object_id).and_then(|o| o.prototype);
    match proto {
        Some(id) => Ok(JsValue::Object(id)),
        None => Ok(JsValue::Null),
    }
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
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object_id = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Object(id) => id,
        other => {
            return Err(VmError::type_error(format!(
                "Object.setPrototypeOf: first argument must be an object, got {}",
                other.type_of()
            )));
        }
    };
    let proto = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let new_proto = match proto {
        JsValue::Null => None,
        JsValue::Object(id) => Some(id),
        other => {
            return Err(VmError::type_error(format!(
                "Object.setPrototypeOf: prototype must be an object or null, got {}",
                other.type_of()
            )));
        }
    };
    context.set_prototype_of(object_id, new_proto)?;
    Ok(JsValue::Object(object_id))
}

fn object_keys(
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object_id = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Object(id) => id,
        other => {
            return Err(VmError::type_error(format!(
                "Object.keys: argument must be an object, got {}",
                other.type_of()
            )));
        }
    };

    let keys: Vec<JsValue> = context
        .heap()
        .object(object_id)
        .map(|o| {
            o.own_property_keys()
                .into_iter()
                .filter(|k| o.own_property(k).is_none_or(|d| d.enumerable))
                .map(JsValue::String)
                .collect()
        })
        .unwrap_or_default();

    let length = keys.len();
    let array_prototype = context.intrinsics().map(|i| i.array_prototype);
    let mut array_obj = JsObject::ordinary();
    if let Some(proto) = array_prototype {
        array_obj.prototype = Some(proto);
    }
    for (i, val) in keys.into_iter().enumerate() {
        array_obj.define_property(
            i.to_string(),
            PropertyDescriptor::data_with(val, true, true, true),
        );
    }
    array_obj.define_property(
        "length",
        PropertyDescriptor::data_with(JsValue::Number(length as f64), true, false, false),
    );

    let id = context
        .heap_mut()
        .allocate_object(array_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    Ok(JsValue::Object(id))
}

// ---------------------------------------------------------------------------
// Descriptor helpers
// ---------------------------------------------------------------------------

fn descriptor_from_heap(
    context: &NativeContext,
    desc_id: crate::runtime::ObjectId,
) -> Result<PropertyDescriptor, VmError> {
    let obj = context
        .heap()
        .object(desc_id)
        .ok_or_else(|| VmError::runtime("descriptor object not in heap"))?;

    let has_value = obj.has_own_property("value");
    let has_writable = obj.has_own_property("writable");
    let has_get = obj.has_own_property("get");
    let has_set = obj.has_own_property("set");

    let enumerable = obj
        .get_own_property_value("enumerable")
        .map(|v| v.to_boolean())
        .unwrap_or(false);
    let configurable = obj
        .get_own_property_value("configurable")
        .map(|v| v.to_boolean())
        .unwrap_or(false);

    if has_get || has_set {
        if has_value || has_writable {
            return Err(VmError::type_error(
                "property descriptor cannot specify both accessor and data attributes",
            ));
        }
        let get = obj.get_own_property_value("get");
        let set = obj.get_own_property_value("set");
        let is_callable_or_absent = |v: &Option<JsValue>| {
            matches!(
                v,
                None | Some(JsValue::Undefined)
                    | Some(JsValue::Function(_))
                    | Some(JsValue::BuiltinFunction(_))
            )
        };
        if !is_callable_or_absent(&get) || !is_callable_or_absent(&set) {
            return Err(VmError::type_error("getter/setter must be a function"));
        }
        Ok(PropertyDescriptor::accessor(
            get,
            set,
            enumerable,
            configurable,
        ))
    } else {
        let value = obj
            .get_own_property_value("value")
            .unwrap_or(JsValue::Undefined);
        let writable = obj
            .get_own_property_value("writable")
            .map(|v| v.to_boolean())
            .unwrap_or(false);
        Ok(PropertyDescriptor::data_with(
            value,
            writable,
            enumerable,
            configurable,
        ))
    }
}

fn build_descriptor_object(
    context: &mut NativeContext,
    descriptor: &PropertyDescriptor,
) -> Result<JsValue, VmError> {
    let mut obj = JsObject::ordinary();
    match &descriptor.kind {
        PropertyKind::Data { value, writable } => {
            obj.define_property(
                "value",
                PropertyDescriptor::data_with(value.clone(), true, true, true),
            );
            obj.define_property(
                "writable",
                PropertyDescriptor::data_with(JsValue::Boolean(*writable), true, true, true),
            );
        }
        PropertyKind::Accessor { get, set } => {
            obj.define_property(
                "get",
                PropertyDescriptor::data_with(
                    get.clone().unwrap_or(JsValue::Undefined),
                    true,
                    true,
                    true,
                ),
            );
            obj.define_property(
                "set",
                PropertyDescriptor::data_with(
                    set.clone().unwrap_or(JsValue::Undefined),
                    true,
                    true,
                    true,
                ),
            );
        }
    }
    obj.define_property(
        "enumerable",
        PropertyDescriptor::data_with(JsValue::Boolean(descriptor.enumerable), true, true, true),
    );
    obj.define_property(
        "configurable",
        PropertyDescriptor::data_with(JsValue::Boolean(descriptor.configurable), true, true, true),
    );

    let id = context
        .heap_mut()
        .allocate_object(obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    Ok(JsValue::Object(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{builtins::install_foundation, runtime::NativeContext};

    fn ctx() -> NativeContext {
        let mut c = NativeContext::default();
        install_foundation(&mut c);
        c
    }

    #[test]
    fn object_create_with_null_proto() {
        let mut context = ctx();
        let result = object_create(&mut context, JsValue::Undefined, &[JsValue::Null]).unwrap();
        let JsValue::Object(id) = result else {
            panic!("expected object")
        };
        assert!(context.heap().object(id).unwrap().prototype.is_none());
    }

    #[test]
    fn object_create_with_object_proto() {
        let mut context = ctx();
        let proto = context
            .heap_mut()
            .allocate_object(JsObject::ordinary())
            .unwrap();
        let result =
            object_create(&mut context, JsValue::Undefined, &[JsValue::Object(proto)]).unwrap();
        let JsValue::Object(id) = result else {
            panic!("expected object")
        };
        assert_eq!(context.heap().object(id).unwrap().prototype, Some(proto));
    }

    #[test]
    fn object_create_rejects_non_object_proto() {
        let mut context = ctx();
        let err =
            object_create(&mut context, JsValue::Undefined, &[JsValue::Number(1.0)]).unwrap_err();
        assert!(err.message.contains("null"));
    }

    #[test]
    fn define_and_get_own_property_descriptor_data() {
        let mut context = ctx();
        let obj_id = context
            .heap_mut()
            .allocate_object(JsObject::ordinary())
            .unwrap();

        // Build a descriptor object {value: 42, writable: true, enumerable: true, configurable: false}
        let mut desc = JsObject::ordinary();
        desc.define_property("value", PropertyDescriptor::data(JsValue::Number(42.0)));
        desc.define_property("writable", PropertyDescriptor::data(JsValue::Boolean(true)));
        desc.define_property(
            "enumerable",
            PropertyDescriptor::data(JsValue::Boolean(true)),
        );
        desc.define_property(
            "configurable",
            PropertyDescriptor::data(JsValue::Boolean(false)),
        );
        let desc_id = context.heap_mut().allocate_object(desc).unwrap();

        object_define_property(
            &mut context,
            JsValue::Undefined,
            &[
                JsValue::Object(obj_id),
                JsValue::String("x".into()),
                JsValue::Object(desc_id),
            ],
        )
        .unwrap();

        let result = object_get_own_property_descriptor(
            &mut context,
            JsValue::Undefined,
            &[JsValue::Object(obj_id), JsValue::String("x".into())],
        )
        .unwrap();

        let JsValue::Object(desc_result_id) = result else {
            panic!("expected object")
        };
        let obj = context.heap().object(desc_result_id).unwrap();
        assert_eq!(
            obj.get_own_property_value("value"),
            Some(JsValue::Number(42.0))
        );
        assert_eq!(
            obj.get_own_property_value("writable"),
            Some(JsValue::Boolean(true))
        );
    }

    #[test]
    fn object_keys_returns_enumerable_keys() {
        let mut context = ctx();
        let mut obj = JsObject::ordinary();
        obj.define_property("a", PropertyDescriptor::data(JsValue::Number(1.0)));
        obj.define_property("b", PropertyDescriptor::data(JsValue::Number(2.0)));
        let obj_id = context.heap_mut().allocate_object(obj).unwrap();

        let result =
            object_keys(&mut context, JsValue::Undefined, &[JsValue::Object(obj_id)]).unwrap();

        let JsValue::Object(arr_id) = result else {
            panic!("expected array")
        };
        let arr = context.heap().object(arr_id).unwrap();
        assert_eq!(
            arr.get_own_property_value("length"),
            Some(JsValue::Number(2.0))
        );
        assert_eq!(
            arr.get_own_property_value("0"),
            Some(JsValue::String("a".into()))
        );
        assert_eq!(
            arr.get_own_property_value("1"),
            Some(JsValue::String("b".into()))
        );
    }

    #[test]
    fn get_set_prototype_of() {
        let mut context = ctx();
        let proto_id = context
            .heap_mut()
            .allocate_object(JsObject::ordinary())
            .unwrap();
        let obj_id = context
            .heap_mut()
            .allocate_object(JsObject::ordinary())
            .unwrap();

        object_set_prototype_of(
            &mut context,
            JsValue::Undefined,
            &[JsValue::Object(obj_id), JsValue::Object(proto_id)],
        )
        .unwrap();

        let got =
            object_get_prototype_of(&mut context, JsValue::Undefined, &[JsValue::Object(obj_id)])
                .unwrap();
        assert_eq!(got, JsValue::Object(proto_id));
    }
}
