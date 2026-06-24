//! V9 collection and iterator builtin basics.
//!
//! This module intentionally implements the small observable core for Map/Set
//! and honest skeletons for weak collections and iterator helpers.

use crate::{
    runtime::{
        JsObject, JsValue, NativeCall, NativeContext, ObjectId, PropertyDescriptor, PropertyKind,
    },
    vm::{Vm, VmError},
};

const COLLECTION_KIND: &str = "__agentjs_collection_kind__";
const COLLECTION_SIZE: &str = "__agentjs_collection_size__";
const COLLECTION_NEXT_INDEX: &str = "__agentjs_collection_next_index__";
const ITERATOR_KIND: &str = "__agentjs_iterator_kind__";
const ITERATOR_COLLECTION: &str = "__agentjs_iterator_collection__";
const ITERATOR_NEXT_INDEX: &str = "__agentjs_iterator_next_index__";
const ITERATOR_DONE: &str = "__agentjs_iterator_done__";
const MAX_COLLECTION_ENTRIES: usize = 1 << 15;
const MAX_ITERATOR_STEPS: usize = 1 << 15;

#[derive(Clone, Copy)]
struct IteratorIntrinsic {
    prototype: ObjectId,
}

pub(super) fn install(context: &mut NativeContext) -> Result<(), VmError> {
    let iterator = install_iterator(context)?;
    install_map(context, iterator)?;
    install_set(context, iterator)?;
    install_weak_map(context)?;
    install_weak_set(context)?;
    Ok(())
}

fn method_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, true)
}

fn constant_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, false)
}

fn readonly_configurable_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, true)
}

fn hidden_slot_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, false)
}

fn define_method(
    context: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<JsValue, VmError> {
    let function = context.register_builtin(name, length, call, None)?;
    context.define_own_property(target, name.into(), method_descriptor(function.clone()))?;
    Ok(function)
}

fn declare_standard_global(
    context: &mut NativeContext,
    name: &'static str,
    value: JsValue,
) -> Result<(), VmError> {
    context.declare_global(name, value.clone());
    context.define_own_property(
        context.global_object(),
        name.into(),
        method_descriptor(value),
    )?;
    Ok(())
}

fn new_ordinary_object(
    context: &mut NativeContext,
    prototype: Option<ObjectId>,
) -> Result<ObjectId, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = prototype;
    context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))
}

fn define_hidden(
    context: &mut NativeContext,
    object: ObjectId,
    name: impl Into<String>,
    value: JsValue,
) -> Result<(), VmError> {
    context.define_own_property(object, name.into(), hidden_slot_descriptor(value))?;
    Ok(())
}

fn own_data_value(context: &NativeContext, object: ObjectId, key: &str) -> Option<JsValue> {
    context
        .get_own_property_descriptor(object, key)
        .and_then(|descriptor| match descriptor.kind {
            PropertyKind::Data { value, .. } => Some(value),
            PropertyKind::Accessor { .. } => None,
        })
}

fn own_string(context: &NativeContext, object: ObjectId, key: &str) -> Option<String> {
    match own_data_value(context, object, key)? {
        JsValue::String(value) => Some(value),
        _ => None,
    }
}

fn own_bool(context: &NativeContext, object: ObjectId, key: &str) -> Option<bool> {
    match own_data_value(context, object, key)? {
        JsValue::Boolean(value) => Some(value),
        _ => None,
    }
}

fn own_usize(context: &NativeContext, object: ObjectId, key: &str) -> usize {
    match own_data_value(context, object, key) {
        Some(JsValue::Number(value)) if value.is_finite() && value >= 0.0 => value as usize,
        _ => 0,
    }
}

fn set_hidden_usize(
    context: &mut NativeContext,
    object: ObjectId,
    key: &'static str,
    value: usize,
) -> Result<(), VmError> {
    define_hidden(context, object, key, JsValue::Number(value as f64))
}

fn array_like_length(context: &mut NativeContext, object: ObjectId) -> Result<usize, VmError> {
    let value = context.get_property(context.object_value(object), "length")?;
    Ok(value.to_number().unwrap_or(0.0).max(0.0) as usize)
}

fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn same_value_zero(left: &JsValue, right: &JsValue) -> bool {
    match (left, right) {
        (JsValue::Number(a), JsValue::Number(b)) => (a.is_nan() && b.is_nan()) || a == b,
        _ => left.strict_equals(right),
    }
}

fn entry_key(index: usize) -> String {
    format!("__agentjs_collection_entry_{index}_key__")
}

fn entry_value(index: usize) -> String {
    format!("__agentjs_collection_entry_{index}_value__")
}

fn entry_active(index: usize) -> String {
    format!("__agentjs_collection_entry_{index}_active__")
}

fn entry_is_active(context: &NativeContext, collection: ObjectId, index: usize) -> bool {
    own_bool(context, collection, &entry_active(index)).unwrap_or(false)
}

fn collection_entry_key(
    context: &NativeContext,
    collection: ObjectId,
    index: usize,
) -> Option<JsValue> {
    own_data_value(context, collection, &entry_key(index))
}

fn collection_entry_value(
    context: &NativeContext,
    collection: ObjectId,
    index: usize,
) -> Option<JsValue> {
    own_data_value(context, collection, &entry_value(index))
}

fn require_collection(
    context: &NativeContext,
    this_value: &JsValue,
    expected: &'static str,
) -> Result<ObjectId, VmError> {
    let object = context.require_object(this_value, expected)?;
    match own_string(context, object, COLLECTION_KIND) {
        Some(kind) if kind == expected => Ok(object),
        _ => Err(VmError::type_error(format!(
            "receiver is not a {expected} object"
        ))),
    }
}

fn require_object_key(context: &NativeContext, key: &JsValue, label: &str) -> Result<(), VmError> {
    if context.value_object(key).is_some() {
        Ok(())
    } else {
        Err(VmError::type_error(format!(
            "{label} key must be an object"
        )))
    }
}

fn create_collection_object(
    context: &mut NativeContext,
    prototype: ObjectId,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(
        context,
        object,
        COLLECTION_KIND,
        JsValue::String(kind.into()),
    )?;
    define_hidden(context, object, COLLECTION_SIZE, JsValue::Number(0.0))?;
    define_hidden(context, object, COLLECTION_NEXT_INDEX, JsValue::Number(0.0))?;
    Ok(JsValue::Object(object))
}

fn find_entry(context: &NativeContext, collection: ObjectId, key: &JsValue) -> Option<usize> {
    // ponytail: V9-C stores entries in hidden ordinary properties and scans
    // linearly. A later runtime storage helper can replace this with compact
    // table storage without changing the public builtins.
    let next = own_usize(context, collection, COLLECTION_NEXT_INDEX);
    (0..next).find(|&index| {
        entry_is_active(context, collection, index)
            && collection_entry_key(context, collection, index)
                .is_some_and(|stored| same_value_zero(&stored, key))
    })
}

fn set_collection_entry(
    context: &mut NativeContext,
    collection: ObjectId,
    key: JsValue,
    value: JsValue,
) -> Result<(), VmError> {
    if let Some(index) = find_entry(context, collection, &key) {
        define_hidden(context, collection, entry_value(index), value)?;
        return Ok(());
    }
    let index = own_usize(context, collection, COLLECTION_NEXT_INDEX);
    if index >= MAX_COLLECTION_ENTRIES {
        return Err(VmError::runtime_limit("collection entry limit exceeded"));
    }
    define_hidden(context, collection, entry_key(index), key)?;
    define_hidden(context, collection, entry_value(index), value)?;
    define_hidden(
        context,
        collection,
        entry_active(index),
        JsValue::Boolean(true),
    )?;
    set_hidden_usize(context, collection, COLLECTION_NEXT_INDEX, index + 1)?;
    let size = own_usize(context, collection, COLLECTION_SIZE);
    set_hidden_usize(context, collection, COLLECTION_SIZE, size + 1)
}

fn delete_collection_entry(
    context: &mut NativeContext,
    collection: ObjectId,
    key: &JsValue,
) -> Result<bool, VmError> {
    let Some(index) = find_entry(context, collection, key) else {
        return Ok(false);
    };
    define_hidden(
        context,
        collection,
        entry_active(index),
        JsValue::Boolean(false),
    )?;
    let size = own_usize(context, collection, COLLECTION_SIZE);
    set_hidden_usize(context, collection, COLLECTION_SIZE, size.saturating_sub(1))?;
    Ok(true)
}

fn clear_collection(context: &mut NativeContext, collection: ObjectId) -> Result<(), VmError> {
    let next = own_usize(context, collection, COLLECTION_NEXT_INDEX);
    for index in 0..next {
        if entry_is_active(context, collection, index) {
            define_hidden(
                context,
                collection,
                entry_active(index),
                JsValue::Boolean(false),
            )?;
        }
    }
    set_hidden_usize(context, collection, COLLECTION_SIZE, 0)
}

fn initialize_map_like(
    context: &mut NativeContext,
    target: ObjectId,
    iterable: JsValue,
    weak: bool,
) -> Result<(), VmError> {
    if matches!(iterable, JsValue::Undefined | JsValue::Null) {
        return Ok(());
    }
    let source = context.require_object(&iterable, "initialize Map")?;
    let length = array_like_length(context, source)?;
    for index in 0..length.min(MAX_COLLECTION_ENTRIES) {
        let pair = context.get_property(iterable.clone(), &index.to_string())?;
        let pair_object = context.require_object(&pair, "Map initializer entry")?;
        let key = context.get_property(context.object_value(pair_object), "0")?;
        if weak {
            require_object_key(context, &key, "WeakMap")?;
        }
        let value = context.get_property(context.object_value(pair_object), "1")?;
        set_collection_entry(context, target, key, value)?;
    }
    Ok(())
}

fn initialize_set_like(
    context: &mut NativeContext,
    target: ObjectId,
    iterable: JsValue,
    weak: bool,
) -> Result<(), VmError> {
    if matches!(iterable, JsValue::Undefined | JsValue::Null) {
        return Ok(());
    }
    let source = context.require_object(&iterable, "initialize Set")?;
    let length = array_like_length(context, source)?;
    for index in 0..length.min(MAX_COLLECTION_ENTRIES) {
        let value = context.get_property(iterable.clone(), &index.to_string())?;
        if weak {
            require_object_key(context, &value, "WeakSet")?;
        }
        set_collection_entry(context, target, value.clone(), value)?;
    }
    Ok(())
}

fn collection_size_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "collection size")?;
    let kind = own_string(context, object, COLLECTION_KIND);
    if !matches!(kind.as_deref(), Some("Map" | "Set")) {
        return Err(VmError::type_error("receiver is not a sized collection"));
    }
    Ok(JsValue::Number(
        own_usize(context, object, COLLECTION_SIZE) as f64
    ))
}

fn install_iterator(context: &mut NativeContext) -> Result<IteratorIntrinsic, VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor = context.register_builtin(
        "Iterator",
        0,
        iterator_constructor_call,
        Some(iterator_constructor_construct),
    )?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Iterator constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    let species_getter =
        context.register_builtin("get [Symbol.species]", 0, collection_species_get, None)?;
    context.define_symbol_own_property(
        constructor_object,
        context.well_known_symbols().species,
        PropertyDescriptor::accessor(Some(species_getter), None, false, true),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    define_method(context, constructor_object, "from", 1, iterator_from)?;
    define_method(context, prototype, "next", 0, iterator_next)?;
    let iterator_fn = define_method(context, prototype, "values", 0, iterator_identity)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().iterator,
        method_descriptor(iterator_fn),
    )?;
    for (name, length, call) in [
        ("toArray", 0, iterator_to_array as NativeCall),
        ("forEach", 1, iterator_for_each as NativeCall),
        ("some", 1, iterator_some as NativeCall),
        ("every", 1, iterator_every as NativeCall),
        ("find", 1, iterator_find as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    for (name, length) in [
        ("map", 1),
        ("filter", 1),
        ("take", 1),
        ("drop", 1),
        ("flatMap", 1),
        ("reduce", 1),
    ] {
        define_method(
            context,
            prototype,
            name,
            length,
            unsupported_iterator_helper,
        )?;
    }
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Iterator".into())),
    )?;
    declare_standard_global(context, "Iterator", constructor)?;
    Ok(IteratorIntrinsic { prototype })
}

fn iterator_constructor_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("Iterator constructor is abstract"))
}

fn iterator_constructor_construct(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("Iterator constructor is abstract"))
}

fn iterator_from(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&value, "Iterator.from")?;
    let next = context.get_property(value.clone(), "next")?;
    if is_callable(&next) {
        return Ok(value);
    }
    if context
        .get_symbol_property_value(object, context.well_known_symbols().iterator)
        .is_some_and(|method| is_callable(&method))
    {
        return Err(VmError::type_error(
            "Iterator.from Symbol.iterator dispatch is not implemented in V9-C skeletons",
        ));
    }
    Err(VmError::type_error(
        "Iterator.from requires an iterator object",
    ))
}

fn iterator_identity(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn unsupported_iterator_helper(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "lazy Iterator helper pipelines are not implemented in V9-C skeletons",
    ))
}

fn collection_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn create_collection_iterator(
    context: &mut NativeContext,
    collection: ObjectId,
    iterator: IteratorIntrinsic,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let object = new_ordinary_object(context, Some(iterator.prototype))?;
    define_hidden(
        context,
        object,
        ITERATOR_COLLECTION,
        JsValue::Object(collection),
    )?;
    define_hidden(context, object, ITERATOR_KIND, JsValue::String(kind.into()))?;
    define_hidden(context, object, ITERATOR_NEXT_INDEX, JsValue::Number(0.0))?;
    define_hidden(context, object, ITERATOR_DONE, JsValue::Boolean(false))?;
    Ok(JsValue::Object(object))
}

fn iterator_result(
    context: &mut NativeContext,
    value: JsValue,
    done: bool,
) -> Result<JsValue, VmError> {
    context.create_object([
        ("value".into(), value),
        ("done".into(), JsValue::Boolean(done)),
    ])
}

fn iterator_next(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let iterator = context.require_object(&this_value, "Iterator.prototype.next")?;
    if own_bool(context, iterator, ITERATOR_DONE).unwrap_or(false) {
        return iterator_result(context, JsValue::Undefined, true);
    }
    let Some(JsValue::Object(collection)) = own_data_value(context, iterator, ITERATOR_COLLECTION)
    else {
        return Err(VmError::type_error("receiver is not a collection iterator"));
    };
    let kind = own_string(context, iterator, ITERATOR_KIND).unwrap_or_default();
    let next = own_usize(context, collection, COLLECTION_NEXT_INDEX);
    let mut index = own_usize(context, iterator, ITERATOR_NEXT_INDEX);
    while index < next {
        set_hidden_usize(context, iterator, ITERATOR_NEXT_INDEX, index + 1)?;
        if entry_is_active(context, collection, index) {
            let key =
                collection_entry_key(context, collection, index).unwrap_or(JsValue::Undefined);
            let value =
                collection_entry_value(context, collection, index).unwrap_or(JsValue::Undefined);
            let result = match kind.as_str() {
                "map-key" | "set-value" => key,
                "map-value" => value,
                "map-entry" => context.create_array(vec![key, value])?,
                "set-entry" => context.create_array(vec![key.clone(), key])?,
                _ => JsValue::Undefined,
            };
            return iterator_result(context, result, false);
        }
        index += 1;
    }
    define_hidden(context, iterator, ITERATOR_DONE, JsValue::Boolean(true))?;
    iterator_result(context, JsValue::Undefined, true)
}

fn iterator_step(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator_value: JsValue,
) -> Result<Option<JsValue>, VmError> {
    let next = context.get_property(iterator_value.clone(), "next")?;
    if !is_callable(&next) {
        return Err(VmError::type_error("iterator next is not callable"));
    }
    let result = vm.call_value_from_builtin(next, iterator_value, Vec::new(), context)?;
    let result_object = context.require_object(&result, "iterator result")?;
    let done = vm
        .get_property_value(context.object_value(result_object), "done", context)?
        .to_boolean();
    if done {
        return Ok(None);
    }
    let value = vm.get_property_value(context.object_value(result_object), "value", context)?;
    Ok(Some(value))
}

fn iterator_to_array(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut values = Vec::new();
    while values.len() < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step(vm, context, this_value.clone())? else {
            return context.create_array(values);
        };
        values.push(value);
    }
    Err(VmError::runtime_limit(
        "iterator helper step limit exceeded",
    ))
}

fn iterator_for_each(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&callback) {
        return Err(VmError::type_error(
            "Iterator.prototype.forEach callback is not callable",
        ));
    }
    let mut index = 0usize;
    while index < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step(vm, context, this_value.clone())? else {
            return Ok(JsValue::Undefined);
        };
        vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![value, JsValue::Number(index as f64)],
            context,
        )?;
        index += 1;
    }
    Err(VmError::runtime_limit(
        "iterator helper step limit exceeded",
    ))
}

fn iterator_some(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    iterator_predicate(vm, context, this_value, arguments, PredicateMode::Some)
}

fn iterator_every(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    iterator_predicate(vm, context, this_value, arguments, PredicateMode::Every)
}

fn iterator_find(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    iterator_predicate(vm, context, this_value, arguments, PredicateMode::Find)
}

enum PredicateMode {
    Some,
    Every,
    Find,
}

fn iterator_predicate(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    mode: PredicateMode,
) -> Result<JsValue, VmError> {
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&callback) {
        return Err(VmError::type_error(
            "iterator predicate callback is not callable",
        ));
    }
    let mut index = 0usize;
    while index < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step(vm, context, this_value.clone())? else {
            return Ok(match mode {
                PredicateMode::Some => JsValue::Boolean(false),
                PredicateMode::Every => JsValue::Boolean(true),
                PredicateMode::Find => JsValue::Undefined,
            });
        };
        let keep = vm
            .call_value_from_builtin(
                callback.clone(),
                JsValue::Undefined,
                vec![value.clone(), JsValue::Number(index as f64)],
                context,
            )?
            .to_boolean();
        match mode {
            PredicateMode::Some if keep => return Ok(JsValue::Boolean(true)),
            PredicateMode::Every if !keep => return Ok(JsValue::Boolean(false)),
            PredicateMode::Find if keep => return Ok(value),
            _ => {}
        }
        index += 1;
    }
    Err(VmError::runtime_limit(
        "iterator helper step limit exceeded",
    ))
}

fn install_map(context: &mut NativeContext, iterator: IteratorIntrinsic) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor = context.register_builtin("Map", 0, map_call, Some(map_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Map constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    let species_getter =
        context.register_builtin("get [Symbol.species]", 0, collection_species_get, None)?;
    context.define_symbol_own_property(
        constructor_object,
        context.well_known_symbols().species,
        PropertyDescriptor::accessor(Some(species_getter), None, false, true),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    let size_getter = context.register_builtin("get size", 0, collection_size_get, None)?;
    context.define_own_property(
        prototype,
        "size".into(),
        PropertyDescriptor::accessor(Some(size_getter), None, false, true),
    )?;
    define_method(context, prototype, "get", 1, map_get)?;
    define_method(context, prototype, "set", 2, map_set)?;
    define_method(context, prototype, "has", 1, map_has)?;
    define_method(context, prototype, "delete", 1, map_delete)?;
    define_method(context, prototype, "clear", 0, map_clear)?;
    define_method(context, prototype, "keys", 0, map_keys)?;
    define_method(context, prototype, "values", 0, map_values)?;
    let entries = define_method(context, prototype, "entries", 0, map_entries)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().iterator,
        method_descriptor(entries),
    )?;
    define_method(context, prototype, "forEach", 1, map_for_each)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Map".into())),
    )?;
    define_hidden(
        context,
        constructor_object,
        "__agentjs_map_iterator_prototype__",
        JsValue::Object(iterator.prototype),
    )?;
    declare_standard_global(context, "Map", constructor)
}

fn map_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("Map constructor requires 'new'"))
}

fn map_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Map prototype missing"))?;
    let value = create_collection_object(context, prototype, "Map")?;
    let JsValue::Object(object) = value else {
        unreachable!()
    };
    initialize_map_like(
        context,
        object,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        false,
    )?;
    Ok(JsValue::Object(object))
}

fn map_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "Map")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(find_entry(context, map, &key)
        .and_then(|index| collection_entry_value(context, map, index))
        .unwrap_or(JsValue::Undefined))
}

fn map_set(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "Map")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    set_collection_entry(context, map, key, value)?;
    Ok(this_value)
}

fn map_has(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "Map")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::Boolean(find_entry(context, map, &key).is_some()))
}

fn map_delete(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "Map")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::Boolean(delete_collection_entry(
        context, map, &key,
    )?))
}

fn map_clear(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "Map")?;
    clear_collection(context, map)?;
    Ok(JsValue::Undefined)
}

fn map_iterator(
    context: &mut NativeContext,
    this_value: JsValue,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "Map")?;
    let iterator_proto = context
        .get_global("Iterator")
        .and_then(|value| context.value_object(&value))
        .and_then(|constructor| {
            context
                .get_own_property_descriptor(constructor, "prototype")
                .and_then(|descriptor| descriptor.value_cloned())
                .and_then(|value| context.value_object(&value))
        })
        .ok_or_else(|| VmError::runtime("Iterator prototype missing"))?;
    create_collection_iterator(
        context,
        map,
        IteratorIntrinsic {
            prototype: iterator_proto,
        },
        kind,
    )
}

fn map_keys(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    map_iterator(context, this_value, "map-key")
}

fn map_values(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    map_iterator(context, this_value, "map-value")
}

fn map_entries(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    map_iterator(context, this_value, "map-entry")
}

fn map_for_each(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "Map")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&callback) {
        return Err(VmError::type_error(
            "Map.prototype.forEach callback is not callable",
        ));
    }
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let next = own_usize(context, map, COLLECTION_NEXT_INDEX);
    for index in 0..next {
        if entry_is_active(context, map, index) {
            let key = collection_entry_key(context, map, index).unwrap_or(JsValue::Undefined);
            let value = collection_entry_value(context, map, index).unwrap_or(JsValue::Undefined);
            vm.call_value_from_builtin(
                callback.clone(),
                this_arg.clone(),
                vec![value, key, this_value.clone()],
                context,
            )?;
        }
    }
    Ok(JsValue::Undefined)
}

fn install_set(context: &mut NativeContext, _iterator: IteratorIntrinsic) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor = context.register_builtin("Set", 0, set_call, Some(set_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Set constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    let size_getter = context.register_builtin("get size", 0, collection_size_get, None)?;
    context.define_own_property(
        prototype,
        "size".into(),
        PropertyDescriptor::accessor(Some(size_getter), None, false, true),
    )?;
    define_method(context, prototype, "add", 1, set_add)?;
    define_method(context, prototype, "has", 1, set_has)?;
    define_method(context, prototype, "delete", 1, set_delete)?;
    define_method(context, prototype, "clear", 0, set_clear)?;
    define_method(context, prototype, "entries", 0, set_entries)?;
    let values = define_method(context, prototype, "values", 0, set_values)?;
    context.define_own_property(prototype, "keys".into(), method_descriptor(values.clone()))?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().iterator,
        method_descriptor(values),
    )?;
    define_method(context, prototype, "forEach", 1, set_for_each)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Set".into())),
    )?;
    declare_standard_global(context, "Set", constructor)
}

fn set_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("Set constructor requires 'new'"))
}

fn set_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Set prototype missing"))?;
    let value = create_collection_object(context, prototype, "Set")?;
    let JsValue::Object(object) = value else {
        unreachable!()
    };
    initialize_set_like(
        context,
        object,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        false,
    )?;
    Ok(JsValue::Object(object))
}

fn set_add(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    set_collection_entry(context, set, value.clone(), value)?;
    Ok(this_value)
}

fn set_has(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::Boolean(find_entry(context, set, &value).is_some()))
}

fn set_delete(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::Boolean(delete_collection_entry(
        context, set, &value,
    )?))
}

fn set_clear(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    clear_collection(context, set)?;
    Ok(JsValue::Undefined)
}

fn set_iterator(
    context: &mut NativeContext,
    this_value: JsValue,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let iterator_proto = context
        .get_global("Iterator")
        .and_then(|value| context.value_object(&value))
        .and_then(|constructor| {
            context
                .get_own_property_descriptor(constructor, "prototype")
                .and_then(|descriptor| descriptor.value_cloned())
                .and_then(|value| context.value_object(&value))
        })
        .ok_or_else(|| VmError::runtime("Iterator prototype missing"))?;
    create_collection_iterator(
        context,
        set,
        IteratorIntrinsic {
            prototype: iterator_proto,
        },
        kind,
    )
}

fn set_values(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    set_iterator(context, this_value, "set-value")
}

fn set_entries(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    set_iterator(context, this_value, "set-entry")
}

fn set_for_each(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&callback) {
        return Err(VmError::type_error(
            "Set.prototype.forEach callback is not callable",
        ));
    }
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let next = own_usize(context, set, COLLECTION_NEXT_INDEX);
    for index in 0..next {
        if entry_is_active(context, set, index) {
            let value = collection_entry_key(context, set, index).unwrap_or(JsValue::Undefined);
            vm.call_value_from_builtin(
                callback.clone(),
                this_arg.clone(),
                vec![value.clone(), value, this_value.clone()],
                context,
            )?;
        }
    }
    Ok(JsValue::Undefined)
}

fn install_weak_map(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor =
        context.register_builtin("WeakMap", 0, weak_map_call, Some(weak_map_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("WeakMap constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    define_method(context, prototype, "get", 1, weak_map_get)?;
    define_method(context, prototype, "set", 2, weak_map_set)?;
    define_method(context, prototype, "has", 1, weak_map_has)?;
    define_method(context, prototype, "delete", 1, weak_map_delete)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("WeakMap".into())),
    )?;
    declare_standard_global(context, "WeakMap", constructor)
}

fn weak_map_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("WeakMap constructor requires 'new'"))
}

fn weak_map_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("WeakMap prototype missing"))?;
    let value = create_collection_object(context, prototype, "WeakMap")?;
    let JsValue::Object(object) = value else {
        unreachable!()
    };
    initialize_map_like(
        context,
        object,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        true,
    )?;
    Ok(JsValue::Object(object))
}

fn weak_map_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "WeakMap")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_object_key(context, &key, "WeakMap")?;
    Ok(find_entry(context, map, &key)
        .and_then(|index| collection_entry_value(context, map, index))
        .unwrap_or(JsValue::Undefined))
}

fn weak_map_set(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "WeakMap")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_object_key(context, &key, "WeakMap")?;
    let value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    set_collection_entry(context, map, key, value)?;
    Ok(this_value)
}

fn weak_map_has(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "WeakMap")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&key).is_none() {
        return Ok(JsValue::Boolean(false));
    }
    Ok(JsValue::Boolean(find_entry(context, map, &key).is_some()))
}

fn weak_map_delete(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let map = require_collection(context, &this_value, "WeakMap")?;
    let key = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&key).is_none() {
        return Ok(JsValue::Boolean(false));
    }
    Ok(JsValue::Boolean(delete_collection_entry(
        context, map, &key,
    )?))
}

fn install_weak_set(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor =
        context.register_builtin("WeakSet", 0, weak_set_call, Some(weak_set_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("WeakSet constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    define_method(context, prototype, "add", 1, weak_set_add)?;
    define_method(context, prototype, "has", 1, weak_set_has)?;
    define_method(context, prototype, "delete", 1, weak_set_delete)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("WeakSet".into())),
    )?;
    declare_standard_global(context, "WeakSet", constructor)
}

fn weak_set_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("WeakSet constructor requires 'new'"))
}

fn weak_set_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("WeakSet prototype missing"))?;
    let value = create_collection_object(context, prototype, "WeakSet")?;
    let JsValue::Object(object) = value else {
        unreachable!()
    };
    initialize_set_like(
        context,
        object,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        true,
    )?;
    Ok(JsValue::Object(object))
}

fn weak_set_add(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "WeakSet")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_object_key(context, &value, "WeakSet")?;
    set_collection_entry(context, set, value.clone(), value)?;
    Ok(this_value)
}

fn weak_set_has(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "WeakSet")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&value).is_none() {
        return Ok(JsValue::Boolean(false));
    }
    Ok(JsValue::Boolean(find_entry(context, set, &value).is_some()))
}

fn weak_set_delete(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "WeakSet")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.value_object(&value).is_none() {
        return Ok(JsValue::Boolean(false));
    }
    Ok(JsValue::Boolean(delete_collection_entry(
        context, set, &value,
    )?))
}
