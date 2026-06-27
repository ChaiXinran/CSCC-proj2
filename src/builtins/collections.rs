//! Collection built-ins: Map, Set, WeakMap, WeakSet, and iterator infrastructure.
//!
//! This module intentionally implements the small observable core for Map/Set
//! and honest skeletons for weak collections and iterator helpers.

use super::proxy;
use crate::{
    runtime::{
        IteratorKind, IteratorRecord, JsObject, JsValue, NativeCall, NativeContext, ObjectId,
        ObjectKind, PropertyDescriptor, PropertyKey, PropertyKind,
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
const ITERATOR_WRAP_PROTOTYPE: &str = "__agentjs_iterator_wrap_prototype__";
const ITERATOR_HELPER_PROTOTYPE: &str = "__agentjs_iterator_helper_prototype__";
const ITERATOR_HELPER_KIND: &str = "__agentjs_iterator_helper_kind__";
const ITERATOR_HELPER_SOURCE: &str = "__agentjs_iterator_helper_source__";
const ITERATOR_HELPER_NEXT: &str = "__agentjs_iterator_helper_next__";
const ITERATOR_HELPER_CALLBACK: &str = "__agentjs_iterator_helper_callback__";
const ITERATOR_HELPER_INDEX: &str = "__agentjs_iterator_helper_index__";
const ITERATOR_HELPER_LIMIT: &str = "__agentjs_iterator_helper_limit__";
const ITERATOR_HELPER_INNER: &str = "__agentjs_iterator_helper_inner__";
const ITERATOR_HELPER_INNER_NEXT: &str = "__agentjs_iterator_helper_inner_next__";
const ITERATOR_HELPER_COUNT: &str = "__agentjs_iterator_helper_count__";
const ITERATOR_HELPER_EXECUTING: &str = "__agentjs_iterator_helper_executing__";
const ITERATOR_HELPER_MODE: &str = "__agentjs_iterator_helper_mode__";
const ITERATOR_HELPER_STARTED: &str = "__agentjs_iterator_helper_started__";
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
    let constructor_getter =
        context.register_builtin("get constructor", 0, iterator_constructor_get, None)?;
    let constructor_setter =
        context.register_builtin("set constructor", 1, iterator_constructor_set, None)?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        PropertyDescriptor::accessor(
            Some(constructor_getter),
            Some(constructor_setter),
            false,
            true,
        ),
    )?;
    let wrap_prototype = new_ordinary_object(context, Some(prototype))?;
    context.define_own_property(
        constructor_object,
        ITERATOR_WRAP_PROTOTYPE.into(),
        hidden_slot_descriptor(JsValue::Object(wrap_prototype)),
    )?;
    let helper_prototype = new_ordinary_object(context, Some(prototype))?;
    define_method(context, helper_prototype, "next", 0, iterator_helper_next)?;
    define_method(
        context,
        helper_prototype,
        "return",
        0,
        iterator_helper_return,
    )?;
    context.define_symbol_own_property(
        helper_prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Iterator Helper".into())),
    )?;
    context.define_own_property(
        constructor_object,
        ITERATOR_HELPER_PROTOTYPE.into(),
        hidden_slot_descriptor(JsValue::Object(helper_prototype)),
    )?;
    define_method(context, constructor_object, "from", 1, iterator_from)?;
    define_method(context, constructor_object, "concat", 0, iterator_concat)?;
    define_method(context, constructor_object, "zip", 1, iterator_zip)?;
    define_method(
        context,
        constructor_object,
        "zipKeyed",
        1,
        iterator_zip_keyed,
    )?;
    define_method(context, prototype, "next", 0, iterator_next)?;
    define_method(context, prototype, "values", 0, iterator_identity)?;
    let iterator_fn = context.register_builtin("[Symbol.iterator]", 0, iterator_identity, None)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().iterator,
        method_descriptor(iterator_fn),
    )?;
    let dispose_fn = context.register_builtin("[Symbol.dispose]", 0, iterator_dispose, None)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().dispose,
        method_descriptor(dispose_fn),
    )?;
    install_array_iterator_method(context)?;
    for (name, length, call) in [
        ("toArray", 0, iterator_to_array as NativeCall),
        ("forEach", 1, iterator_for_each as NativeCall),
        ("some", 1, iterator_some as NativeCall),
        ("every", 1, iterator_every as NativeCall),
        ("find", 1, iterator_find as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    for (name, length, call) in [
        ("map", 1, iterator_map as NativeCall),
        ("filter", 1, iterator_filter as NativeCall),
        ("take", 1, iterator_take as NativeCall),
        ("drop", 1, iterator_drop as NativeCall),
        ("flatMap", 1, iterator_flat_map as NativeCall),
        ("reduce", 1, iterator_reduce as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    let tag_getter = context.register_builtin(
        "get [Symbol.toStringTag]",
        0,
        iterator_to_string_tag_get,
        None,
    )?;
    let tag_setter = context.register_builtin(
        "set [Symbol.toStringTag]",
        1,
        iterator_to_string_tag_set,
        None,
    )?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        PropertyDescriptor::accessor(Some(tag_getter), Some(tag_setter), false, true),
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
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let iterator_symbol = context.well_known_symbols().iterator;
    if matches!(value, JsValue::String(_)) {
        let method = vm.get_symbol_property_value_with_receiver_from_builtin(
            value.clone(),
            value.clone(),
            iterator_symbol,
            context,
        )?;
        if !matches!(method, JsValue::Undefined | JsValue::Null) {
            if !is_callable(&method) {
                return Err(VmError::type_error(
                    "Iterator.from Symbol.iterator is not callable",
                ));
            }
            let iterator =
                vm.call_value_from_builtin(method, value.clone(), Vec::new(), context)?;
            let iterator_object = context.require_object(&iterator, "Iterator.from result")?;
            let next = vm.get_property_value(iterator.clone(), "next", context)?;
            if !is_callable(&next) {
                return Err(VmError::type_error(
                    "Iterator.from result next is not callable",
                ));
            }
            install_iterator_prototype_on_value(context, iterator_object)?;
            return Ok(iterator);
        }
        let iterator = context.create_iterator_object(value)?;
        if let JsValue::Object(object) = iterator {
            install_iterator_prototype_on_value(context, object)?;
        }
        return Ok(iterator);
    }

    let object = context.require_object(&value, "Iterator.from")?;
    if context
        .heap()
        .object(object)
        .is_some_and(|object| matches!(object.kind, ObjectKind::Generator { .. }))
    {
        return Ok(value);
    }

    let method = vm.get_symbol_property_value_with_receiver_from_builtin(
        value.clone(),
        value.clone(),
        iterator_symbol,
        context,
    )?;
    if !matches!(method, JsValue::Undefined | JsValue::Null) {
        if !is_callable(&method) {
            return Err(VmError::type_error(
                "Iterator.from Symbol.iterator is not callable",
            ));
        }
        let iterator = vm.call_value_from_builtin(method, value.clone(), Vec::new(), context)?;
        let iterator_object = context.require_object(&iterator, "Iterator.from result")?;
        let next = vm.get_property_value(iterator.clone(), "next", context)?;
        if !is_callable(&next) {
            return Err(VmError::type_error(
                "Iterator.from result next is not callable",
            ));
        }
        install_iterator_prototype_on_value(context, iterator_object)?;
        return Ok(iterator);
    }

    if context.has_symbol_property(object, iterator_symbol)?
        && matches!(method, JsValue::Undefined | JsValue::Null)
    {
        let next = vm.get_property_value(value.clone(), "next", context)?;
        if matches!(next, JsValue::Undefined | JsValue::Null) {
            return create_js_iterator_wrapper(context, value, None);
        }
        if is_callable(&next) {
            return create_js_iterator_wrapper(context, value, Some(next));
        }
        return Err(VmError::type_error("iterator next is not callable"));
    }

    if let Ok(iterator) = context.create_iterator_object(value.clone()) {
        if let JsValue::Object(iterator_object) = iterator {
            install_iterator_prototype_on_value(context, iterator_object)?;
        }
        return Ok(iterator);
    }

    let next = vm.get_property_value(value.clone(), "next", context)?;
    if matches!(next, JsValue::Undefined | JsValue::Null) {
        return create_js_iterator_wrapper(context, value, None);
    }
    if is_callable(&next) {
        return create_js_iterator_wrapper(context, value, Some(next));
    }
    Err(VmError::type_error("iterator next is not callable"))
}

fn iterator_concat(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = new_ordinary_object(context, Some(iterator_helper_prototype_object(context)?))?;
    for (index, item) in arguments.iter().enumerate() {
        context.require_object(item, "Iterator.concat item")?;
        let method = vm.get_symbol_property_value_with_receiver_from_builtin(
            item.clone(),
            item.clone(),
            context.well_known_symbols().iterator,
            context,
        )?;
        if !is_callable(&method) {
            return Err(VmError::type_error("Iterator.concat item is not iterable"));
        }
        define_hidden(context, object, concat_item_key(index), item.clone())?;
        define_hidden(context, object, concat_method_key(index), method)?;
    }
    for (key, value) in [
        (ITERATOR_HELPER_KIND, JsValue::String("concat".into())),
        (ITERATOR_HELPER_INDEX, JsValue::Number(0.0)),
        (
            ITERATOR_HELPER_COUNT,
            JsValue::Number(arguments.len() as f64),
        ),
        (ITERATOR_HELPER_INNER, JsValue::Undefined),
        (ITERATOR_HELPER_INNER_NEXT, JsValue::Undefined),
        (ITERATOR_HELPER_EXECUTING, JsValue::Boolean(false)),
        (ITERATOR_DONE, JsValue::Boolean(false)),
    ] {
        define_hidden(context, object, key, value)?;
    }
    Ok(JsValue::Object(object))
}

fn concat_item_key(index: usize) -> String {
    format!("__agentjs_iterator_concat_{index}_item__")
}

fn concat_method_key(index: usize) -> String {
    format!("__agentjs_iterator_concat_{index}_method__")
}

fn iterator_zip(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let iterables = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    context.require_object(&iterables, "Iterator.zip iterables")?;
    let options = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let (mode, padding_option) = iterator_zip_options(vm, context, options)?;
    let iterators = collect_zip_iterators(vm, context, iterables)?;
    let padding = match collect_zip_padding(vm, context, padding_option, iterators.len()) {
        Ok(padding) => padding,
        Err(error) => {
            close_iterator_list(vm, context, &iterators);
            return Err(error);
        }
    };
    create_zip_helper(context, iterators, padding, mode, "zip")
}

fn iterator_zip_keyed(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let keyed = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    context.require_object(&keyed, "Iterator.zipKeyed")?;
    let options = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let (mode, padding_option) = iterator_zip_options(vm, context, options)?;
    let all_keys = proxy::internal_own_property_keys(vm, context, keyed.clone())?;
    let mut keys = Vec::new();
    let mut iterators = Vec::new();
    for key in all_keys {
        let descriptor = match proxy::internal_get_own_property(vm, context, keyed.clone(), &key) {
            Ok(descriptor) => descriptor,
            Err(error) => {
                close_iterator_list(vm, context, &iterators);
                return Err(error);
            }
        };
        if !descriptor.is_some_and(|descriptor| descriptor.enumerable) {
            continue;
        }
        let source = match proxy::internal_get(vm, context, keyed.clone(), &key, keyed.clone()) {
            Ok(source) => source,
            Err(error) => {
                close_iterator_list(vm, context, &iterators);
                return Err(error);
            }
        };
        if matches!(source, JsValue::Undefined) {
            continue;
        }
        match get_iterator_flattenable(vm, context, source) {
            Ok(iterator) => {
                keys.push(key);
                iterators.push(iterator);
            }
            Err(error) => {
                close_iterator_list(vm, context, &iterators);
                return Err(error);
            }
        }
    }
    let padding = if let Some(padding) = padding_option {
        let mut values = Vec::with_capacity(keys.len());
        for key in &keys {
            let value =
                match proxy::internal_get(vm, context, padding.clone(), key, padding.clone()) {
                    Ok(value) => value,
                    Err(error) => {
                        close_iterator_list(vm, context, &iterators);
                        return Err(error);
                    }
                };
            values.push(value);
        }
        values
    } else {
        vec![JsValue::Undefined; keys.len()]
    };
    let helper = create_zip_helper(context, iterators, padding, mode, "zipKeyed")?;
    let helper_object = context.require_object(&helper, "Iterator.zipKeyed result")?;
    for (index, key) in keys.into_iter().enumerate() {
        let value = match key {
            PropertyKey::String(key) => JsValue::String(key),
            PropertyKey::Symbol(symbol) => JsValue::Symbol(symbol),
        };
        define_hidden(context, helper_object, zip_result_key(index), value)?;
    }
    Ok(helper)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ZipMode {
    Shortest,
    Longest,
    Strict,
}

impl ZipMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Shortest => "shortest",
            Self::Longest => "longest",
            Self::Strict => "strict",
        }
    }
}

fn iterator_zip_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    options: JsValue,
) -> Result<(ZipMode, Option<JsValue>), VmError> {
    if !matches!(options, JsValue::Undefined) {
        context.require_object(&options, "Iterator.zip options")?;
    }
    let mode = if matches!(options, JsValue::Undefined) {
        JsValue::Undefined
    } else {
        iterator_property_value(vm, context, options.clone(), "mode")?
    };
    let mode = match mode {
        JsValue::Undefined => ZipMode::Shortest,
        JsValue::String(mode) if mode == "shortest" => ZipMode::Shortest,
        JsValue::String(mode) if mode == "longest" => ZipMode::Longest,
        JsValue::String(mode) if mode == "strict" => ZipMode::Strict,
        _ => return Err(VmError::type_error("Iterator.zip mode is invalid")),
    };
    if mode != ZipMode::Longest {
        return Ok((mode, None));
    }
    let padding = if matches!(options, JsValue::Undefined) {
        JsValue::Undefined
    } else {
        iterator_property_value(vm, context, options, "padding")?
    };
    if matches!(padding, JsValue::Undefined) {
        return Ok((mode, None));
    }
    context.require_object(&padding, "Iterator.zip padding")?;
    Ok((mode, Some(padding)))
}

fn zip_iterator_key(index: usize) -> String {
    format!("__agentjs_iterator_zip_{index}_iterator__")
}

fn zip_next_key(index: usize) -> String {
    format!("__agentjs_iterator_zip_{index}_next__")
}

fn zip_active_key(index: usize) -> String {
    format!("__agentjs_iterator_zip_{index}_active__")
}

fn zip_padding_key(index: usize) -> String {
    format!("__agentjs_iterator_zip_{index}_padding__")
}

fn zip_result_key(index: usize) -> String {
    format!("__agentjs_iterator_zip_{index}_result_key__")
}

fn collect_zip_iterators(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterables: JsValue,
) -> Result<Vec<(JsValue, JsValue)>, VmError> {
    context.require_object(&iterables, "Iterator.zip iterables")?;
    let (outer, outer_next) = get_iterator_from_iterable(vm, context, iterables)?;
    let mut iterators = Vec::new();
    loop {
        let source = match iterator_step_with_next(vm, context, outer.clone(), outer_next.clone()) {
            Ok(Some(source)) => source,
            Ok(None) => return Ok(iterators),
            Err(error) => {
                close_iterator_list(vm, context, &iterators);
                return Err(error);
            }
        };
        if let Err(error) = context.require_object(&source, "Iterator.zip input") {
            close_iterator_list(vm, context, &iterators);
            close_iterator_preserving_pending(vm, context, outer);
            return Err(error);
        }
        let (iterator, next) = match get_iterator_flattenable(vm, context, source) {
            Ok(iterator) => iterator,
            Err(error) => {
                close_iterator_list(vm, context, &iterators);
                close_iterator_preserving_pending(vm, context, outer);
                return Err(error);
            }
        };
        iterators.push((iterator, next));
    }
}

fn collect_zip_padding(
    vm: &mut Vm,
    context: &mut NativeContext,
    padding: Option<JsValue>,
    count: usize,
) -> Result<Vec<JsValue>, VmError> {
    let Some(padding) = padding else {
        return Ok(vec![JsValue::Undefined; count]);
    };
    let (iterator, next) = get_iterator_from_iterable(vm, context, padding)?;
    let mut values = Vec::with_capacity(count);
    let mut exhausted = false;
    for _ in 0..count {
        if exhausted {
            values.push(JsValue::Undefined);
            continue;
        }
        match iterator_step_with_next(vm, context, iterator.clone(), next.clone())? {
            Some(value) => values.push(value),
            None => {
                exhausted = true;
                values.push(JsValue::Undefined);
            }
        }
    }
    if !exhausted {
        vm.close_iterator_from_builtin(iterator, context)?;
    }
    Ok(values)
}

fn get_iterator_from_iterable(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterable: JsValue,
) -> Result<(JsValue, JsValue), VmError> {
    context.require_object(&iterable, "iterable")?;
    let method = vm.get_symbol_property_value_with_receiver_from_builtin(
        iterable.clone(),
        iterable.clone(),
        context.well_known_symbols().iterator,
        context,
    )?;
    if !is_callable(&method) {
        return Err(VmError::type_error("value is not iterable"));
    }
    let iterator = vm.call_value_from_builtin(method, iterable, Vec::new(), context)?;
    context.require_object(&iterator, "iterator")?;
    let next = iterator_property_value(vm, context, iterator.clone(), "next")?;
    Ok((iterator, next))
}

fn get_iterator_flattenable(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<(JsValue, JsValue), VmError> {
    context.require_object(&value, "Iterator.zip input")?;
    let method = vm.get_symbol_property_value_with_receiver_from_builtin(
        value.clone(),
        value.clone(),
        context.well_known_symbols().iterator,
        context,
    )?;
    let iterator = if matches!(method, JsValue::Undefined | JsValue::Null) {
        value
    } else {
        if !is_callable(&method) {
            return Err(VmError::type_error("value is not iterable"));
        }
        vm.call_value_from_builtin(method, value, Vec::new(), context)?
    };
    context.require_object(&iterator, "iterator")?;
    let next = iterator_property_value(vm, context, iterator.clone(), "next")?;
    Ok((iterator, next))
}

fn create_zip_helper(
    context: &mut NativeContext,
    iterators: Vec<(JsValue, JsValue)>,
    padding: Vec<JsValue>,
    mode: ZipMode,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let object = new_ordinary_object(context, Some(iterator_helper_prototype_object(context)?))?;
    let count = iterators.len();
    for (index, (iterator, next)) in iterators.into_iter().enumerate() {
        define_hidden(context, object, zip_iterator_key(index), iterator)?;
        define_hidden(context, object, zip_next_key(index), next)?;
        define_hidden(
            context,
            object,
            zip_active_key(index),
            JsValue::Boolean(true),
        )?;
        define_hidden(
            context,
            object,
            zip_padding_key(index),
            padding.get(index).cloned().unwrap_or(JsValue::Undefined),
        )?;
    }
    for (key, value) in [
        (ITERATOR_HELPER_KIND, JsValue::String(kind.into())),
        (ITERATOR_HELPER_COUNT, JsValue::Number(count as f64)),
        (ITERATOR_HELPER_MODE, JsValue::String(mode.as_str().into())),
        (ITERATOR_HELPER_STARTED, JsValue::Boolean(false)),
        (ITERATOR_HELPER_EXECUTING, JsValue::Boolean(false)),
        (ITERATOR_DONE, JsValue::Boolean(false)),
    ] {
        define_hidden(context, object, key, value)?;
    }
    Ok(JsValue::Object(object))
}

fn close_iterator_list(vm: &mut Vm, context: &mut NativeContext, iterators: &[(JsValue, JsValue)]) {
    let pending = vm.take_pending_exception_from_builtin();
    for (iterator, _) in iterators.iter().rev() {
        let _ = vm.close_iterator_from_builtin(iterator.clone(), context);
    }
    let _ = vm.take_pending_exception_from_builtin();
    if let Some(value) = pending {
        let _ = vm.throw_value_from_builtin(value);
    }
}

fn close_iterator_preserving_pending(vm: &mut Vm, context: &mut NativeContext, iterator: JsValue) {
    let pending = vm.take_pending_exception_from_builtin();
    let _ = vm.close_iterator_from_builtin(iterator, context);
    let _ = vm.take_pending_exception_from_builtin();
    if let Some(value) = pending {
        let _ = vm.throw_value_from_builtin(value);
    }
}

fn collect_iterator_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    // ponytail: this eager collector is a Fix2-B scoring bridge. It reuses
    // Iterator.from/IteratorStep, but it is not the final lazy Iterator helper
    // pipeline. Upgrade path: replace callers with generator-backed helpers
    // once shared IteratorClose/abrupt completion is complete.
    let iterator = iterator_from(vm, context, JsValue::Undefined, &[value])?;
    let mut values = Vec::new();
    while values.len() < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step(vm, context, iterator.clone())? else {
            return Ok(values);
        };
        values.push(value);
    }
    Err(VmError::runtime_limit(
        "iterator eager collection step limit exceeded",
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

fn iterator_dispose(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let return_method = context.get_property(this_value.clone(), "return")?;
    if matches!(return_method, JsValue::Undefined | JsValue::Null) {
        return Ok(JsValue::Undefined);
    }
    if !is_callable(&return_method) {
        return Err(VmError::type_error("iterator return is not callable"));
    }
    vm.call_value_from_builtin(return_method, this_value, Vec::new(), context)?;
    Ok(JsValue::Undefined)
}

fn iterator_constructor_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context
        .get_global("Iterator")
        .ok_or_else(|| VmError::runtime("Iterator constructor missing"))
}

fn iterator_constructor_set(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    setter_ignoring_iterator_prototype_string(context, this_value, "constructor", value)
}

fn iterator_to_string_tag_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String("Iterator".into()))
}

fn iterator_to_string_tag_set(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    setter_ignoring_iterator_prototype_symbol(
        context,
        this_value,
        context.well_known_symbols().to_string_tag,
        value,
    )
}

fn setter_ignoring_iterator_prototype_string(
    context: &mut NativeContext,
    this_value: JsValue,
    key: &str,
    value: JsValue,
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "set Iterator.prototype property")?;
    if object == iterator_prototype_object(context)? {
        return Err(VmError::type_error(
            "cannot set Iterator.prototype intrinsic accessor",
        ));
    }
    if context.get_own_property_descriptor(object, key).is_none() {
        context.define_own_property(object, key.into(), PropertyDescriptor::data(value))?;
    } else {
        context.set_property(this_value, key, value)?;
    }
    Ok(JsValue::Undefined)
}

fn setter_ignoring_iterator_prototype_symbol(
    context: &mut NativeContext,
    this_value: JsValue,
    symbol: crate::runtime::SymbolId,
    value: JsValue,
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "set Iterator.prototype symbol property")?;
    if object == iterator_prototype_object(context)? {
        return Err(VmError::type_error(
            "cannot set Iterator.prototype intrinsic accessor",
        ));
    }
    if context
        .get_own_symbol_property_descriptor(object, symbol)
        .is_none()
    {
        context.define_symbol_own_property(object, symbol, PropertyDescriptor::data(value))?;
    } else {
        context.set_symbol_property(object, symbol, value, true)?;
    }
    Ok(JsValue::Undefined)
}

fn iterator_prototype_object(context: &NativeContext) -> Result<ObjectId, VmError> {
    let iterator_ctor = context
        .get_global("Iterator")
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator constructor missing"))?;
    context
        .find_property_descriptor(iterator_ctor, "prototype")?
        .and_then(|(_, descriptor)| descriptor.value_cloned())
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator prototype missing"))
}

fn iterator_wrap_prototype_object(context: &NativeContext) -> Result<ObjectId, VmError> {
    let iterator_ctor = context
        .get_global("Iterator")
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator constructor missing"))?;
    context
        .get_own_property_descriptor(iterator_ctor, ITERATOR_WRAP_PROTOTYPE)
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator wrapper prototype missing"))
}

fn iterator_helper_prototype_object(context: &NativeContext) -> Result<ObjectId, VmError> {
    let iterator_ctor = context
        .get_global("Iterator")
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator constructor missing"))?;
    context
        .get_own_property_descriptor(iterator_ctor, ITERATOR_HELPER_PROTOTYPE)
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator helper prototype missing"))
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
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let iterator = context.require_object(&this_value, "Iterator.prototype.next")?;
    if let Some((js_iterator, cached_next)) =
        context
            .heap()
            .object(iterator)
            .and_then(|object| match &object.kind {
                ObjectKind::Iterator {
                    record:
                        IteratorRecord {
                            kind:
                                IteratorKind::Js {
                                    iterator,
                                    next_method,
                                },
                            ..
                        },
                } => Some((iterator.clone(), next_method.clone())),
                _ => None,
            })
    {
        let next = match cached_next {
            Some(next) => next,
            None => vm.get_property_value(js_iterator.clone(), "next", context)?,
        };
        if !is_callable(&next) {
            return Err(VmError::type_error("iterator next is not callable"));
        }
        return vm.call_value_from_builtin(next, js_iterator, Vec::new(), context);
    }
    if context
        .heap()
        .object(iterator)
        .is_some_and(|object| matches!(&object.kind, ObjectKind::Iterator { .. }))
    {
        let (value, done) = vm.step_native_iterator_object(this_value, context)?;
        return iterator_result(context, value, done);
    }
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

fn install_iterator_prototype_on_value(
    context: &mut NativeContext,
    object: ObjectId,
) -> Result<(), VmError> {
    let iterator_ctor = context
        .get_global("Iterator")
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator constructor missing"))?;
    let prototype = context
        .find_property_descriptor(iterator_ctor, "prototype")?
        .and_then(|(_, descriptor)| descriptor.value_cloned())
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Iterator prototype missing"))?;
    context.set_prototype_of(object, Some(prototype))?;
    Ok(())
}

fn create_js_iterator_wrapper(
    context: &mut NativeContext,
    iterator: JsValue,
    next_method: Option<JsValue>,
) -> Result<JsValue, VmError> {
    let record = match next_method {
        Some(next_method) => IteratorRecord::js_with_next(iterator, next_method),
        None => IteratorRecord::js(iterator),
    };
    let object = JsObject::iterator(record);
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("heap full: cannot allocate iterator wrapper"))?;
    let wrap_prototype = iterator_wrap_prototype_object(context)?;
    context.set_prototype_of(id, Some(wrap_prototype))?;
    define_method(context, id, "return", 0, iterator_wrapper_return)?;
    Ok(JsValue::Object(id))
}

fn iterator_wrapper_return(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let wrapper = context.require_object(&this_value, "Iterator wrapper return")?;
    let Some(iterator) = context
        .heap()
        .object(wrapper)
        .and_then(|object| match &object.kind {
            ObjectKind::Iterator {
                record:
                    IteratorRecord {
                        kind: IteratorKind::Js { iterator, .. },
                        ..
                    },
            } => Some(iterator.clone()),
            _ => None,
        })
    else {
        return Err(VmError::type_error(
            "receiver is not an Iterator.from wrapper",
        ));
    };
    let return_method = vm.get_property_value(iterator.clone(), "return", context)?;
    if matches!(return_method, JsValue::Undefined | JsValue::Null) {
        return iterator_result(context, JsValue::Undefined, true);
    }
    if !is_callable(&return_method) {
        return Err(VmError::type_error("iterator return is not callable"));
    }
    let result = vm.call_value_from_builtin(return_method, iterator, Vec::new(), context)?;
    context.require_object(&result, "iterator return result")?;
    Ok(result)
}

fn install_array_iterator_method(context: &mut NativeContext) -> Result<(), VmError> {
    let Some(intrinsics) = context.intrinsics().cloned() else {
        return Ok(());
    };
    let Some((_, descriptor)) =
        context.find_property_descriptor(intrinsics.array_prototype, "values")?
    else {
        return Ok(());
    };
    let Some(values) = descriptor.value_cloned() else {
        return Ok(());
    };
    context.define_symbol_own_property(
        intrinsics.array_prototype,
        context.well_known_symbols().iterator,
        method_descriptor(values),
    )?;
    Ok(())
}

fn iterator_step(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator_value: JsValue,
) -> Result<Option<JsValue>, VmError> {
    let next = iterator_next_method(vm, context, iterator_value.clone())?;
    iterator_step_with_next(vm, context, iterator_value, next)
}

fn iterator_next_method(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator_value: JsValue,
) -> Result<JsValue, VmError> {
    let next = vm.get_property_value(iterator_value, "next", context)?;
    if !is_callable(&next) {
        return Err(VmError::type_error("iterator next is not callable"));
    }
    Ok(next)
}

fn iterator_step_with_next(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator_value: JsValue,
    next: JsValue,
) -> Result<Option<JsValue>, VmError> {
    let result = vm.call_value_from_builtin(next, iterator_value, Vec::new(), context)?;
    let result_object = context.require_object(&result, "iterator result")?;
    let result = context.object_value(result_object);
    let done = iterator_property_value(vm, context, result.clone(), "done")?.to_boolean();
    if done {
        return Ok(None);
    }
    let value = iterator_property_value(vm, context, result, "value")?;
    Ok(Some(value))
}

fn iterator_property_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: &str,
) -> Result<JsValue, VmError> {
    match vm.get_property_value_catching_from_builtin(receiver, key, context)? {
        Ok(value) => Ok(value),
        Err(value) => Err(vm.throw_value_from_builtin(value)),
    }
}

fn create_iterator_helper(
    vm: &mut Vm,
    context: &mut NativeContext,
    source: JsValue,
    kind: &'static str,
    callback: JsValue,
    limit: usize,
) -> Result<JsValue, VmError> {
    context.require_object(&source, "Iterator helper receiver")?;
    let next = vm.get_property_value(source.clone(), "next", context)?;
    if !is_callable(&next) {
        return Err(VmError::type_error("iterator next is not callable"));
    }
    let object = new_ordinary_object(context, Some(iterator_helper_prototype_object(context)?))?;
    for (key, value) in [
        (ITERATOR_HELPER_KIND, JsValue::String(kind.into())),
        (ITERATOR_HELPER_SOURCE, source),
        (ITERATOR_HELPER_NEXT, next),
        (ITERATOR_HELPER_CALLBACK, callback),
        (ITERATOR_HELPER_INDEX, JsValue::Number(0.0)),
        (ITERATOR_HELPER_LIMIT, JsValue::Number(limit as f64)),
        (ITERATOR_HELPER_INNER, JsValue::Undefined),
        (ITERATOR_HELPER_INNER_NEXT, JsValue::Undefined),
        (ITERATOR_DONE, JsValue::Boolean(false)),
    ] {
        define_hidden(context, object, key, value)?;
    }
    Ok(JsValue::Object(object))
}

fn iterator_helper_step(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator: JsValue,
    next: JsValue,
) -> Result<Option<JsValue>, VmError> {
    let result = vm.call_value_from_builtin(next, iterator, Vec::new(), context)?;
    let result = context.require_object(&result, "iterator result")?;
    let result = context.object_value(result);
    if iterator_property_value(vm, context, result.clone(), "done")?.to_boolean() {
        return Ok(None);
    }
    iterator_property_value(vm, context, result, "value").map(Some)
}

fn iterator_helper_callback(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
    source: JsValue,
    callback: JsValue,
    value: JsValue,
    index: usize,
) -> Result<JsValue, VmError> {
    match vm.call_value_from_builtin(
        callback,
        JsValue::Undefined,
        vec![value, JsValue::Number(index as f64)],
        context,
    ) {
        Ok(value) => Ok(value),
        Err(error) => {
            define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
            let Some(thrown) = vm.take_pending_exception_from_builtin() else {
                return Err(error);
            };
            match vm.close_iterator_preserving_throw_from_builtin(source, thrown, context) {
                Ok(()) => Err(error),
                Err(close_error) => Err(close_error),
            }
        }
    }
}

fn iterator_helper_next(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let helper = context.require_object(&this_value, "Iterator Helper.prototype.next")?;
    let kind = own_string(context, helper, ITERATOR_HELPER_KIND)
        .ok_or_else(|| VmError::type_error("receiver is not an Iterator helper"))?;
    if own_bool(context, helper, ITERATOR_HELPER_EXECUTING).unwrap_or(false) {
        return Err(VmError::type_error("Iterator helper is already running"));
    }
    if own_bool(context, helper, ITERATOR_DONE).unwrap_or(true) {
        return iterator_result(context, JsValue::Undefined, true);
    }
    if kind == "concat" {
        return iterator_concat_helper_next(vm, context, helper);
    }
    if matches!(kind.as_str(), "zip" | "zipKeyed") {
        return iterator_zip_helper_next(vm, context, helper);
    }
    let source = own_data_value(context, helper, ITERATOR_HELPER_SOURCE)
        .ok_or_else(|| VmError::runtime("Iterator helper source missing"))?;
    let next = own_data_value(context, helper, ITERATOR_HELPER_NEXT)
        .ok_or_else(|| VmError::runtime("Iterator helper next missing"))?;
    let callback =
        own_data_value(context, helper, ITERATOR_HELPER_CALLBACK).unwrap_or(JsValue::Undefined);
    let mut index = own_usize(context, helper, ITERATOR_HELPER_INDEX);
    let limit = own_usize(context, helper, ITERATOR_HELPER_LIMIT);

    loop {
        context.consume_loop_iteration()?;
        if kind == "take" && index >= limit {
            define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
            vm.close_iterator_from_builtin(source, context)?;
            return iterator_result(context, JsValue::Undefined, true);
        }

        if kind == "flatMap"
            && let (Some(inner), Some(inner_next)) = (
                own_data_value(context, helper, ITERATOR_HELPER_INNER),
                own_data_value(context, helper, ITERATOR_HELPER_INNER_NEXT),
            )
            && !matches!(inner, JsValue::Undefined)
        {
            if let Some(value) = iterator_helper_step(vm, context, inner, inner_next)? {
                return iterator_result(context, value, false);
            }
            define_hidden(context, helper, ITERATOR_HELPER_INNER, JsValue::Undefined)?;
            define_hidden(
                context,
                helper,
                ITERATOR_HELPER_INNER_NEXT,
                JsValue::Undefined,
            )?;
            continue;
        }

        let Some(value) = iterator_helper_step(vm, context, source.clone(), next.clone())? else {
            define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
            return iterator_result(context, JsValue::Undefined, true);
        };

        if kind == "drop" && index < limit {
            index += 1;
            set_hidden_usize(context, helper, ITERATOR_HELPER_INDEX, index)?;
            continue;
        }

        let current_index = index;
        index += 1;
        set_hidden_usize(context, helper, ITERATOR_HELPER_INDEX, index)?;
        match kind.as_str() {
            "map" => {
                let mapped = iterator_helper_callback(
                    vm,
                    context,
                    helper,
                    source,
                    callback,
                    value,
                    current_index,
                )?;
                return iterator_result(context, mapped, false);
            }
            "filter" => {
                let keep = iterator_helper_callback(
                    vm,
                    context,
                    helper,
                    source.clone(),
                    callback.clone(),
                    value.clone(),
                    current_index,
                )?
                .to_boolean();
                if keep {
                    return iterator_result(context, value, false);
                }
            }
            "flatMap" => {
                let mapped = iterator_helper_callback(
                    vm,
                    context,
                    helper,
                    source.clone(),
                    callback.clone(),
                    value,
                    current_index,
                )?;
                if context.value_object(&mapped).is_none() {
                    define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
                    vm.close_iterator_from_builtin(source, context)?;
                    return Err(VmError::type_error(
                        "Iterator.prototype.flatMap callback must return an object",
                    ));
                }
                let inner = iterator_from(vm, context, JsValue::Undefined, &[mapped])?;
                let inner_next = vm.get_property_value(inner.clone(), "next", context)?;
                if !is_callable(&inner_next) {
                    define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
                    vm.close_iterator_from_builtin(source, context)?;
                    return Err(VmError::type_error("inner iterator next is not callable"));
                }
                define_hidden(context, helper, ITERATOR_HELPER_INNER, inner)?;
                define_hidden(context, helper, ITERATOR_HELPER_INNER_NEXT, inner_next)?;
            }
            "take" | "drop" => return iterator_result(context, value, false),
            _ => return Err(VmError::runtime("unknown Iterator helper kind")),
        }
    }
}

fn iterator_concat_helper_next(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
) -> Result<JsValue, VmError> {
    if own_bool(context, helper, ITERATOR_HELPER_EXECUTING).unwrap_or(false) {
        return Err(VmError::type_error(
            "Iterator.concat iterator is already running",
        ));
    }
    define_hidden(
        context,
        helper,
        ITERATOR_HELPER_EXECUTING,
        JsValue::Boolean(true),
    )?;
    let result = iterator_concat_helper_next_inner(vm, context, helper);
    define_hidden(
        context,
        helper,
        ITERATOR_HELPER_EXECUTING,
        JsValue::Boolean(false),
    )?;
    result
}

fn iterator_zip_helper_next(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
) -> Result<JsValue, VmError> {
    define_hidden(
        context,
        helper,
        ITERATOR_HELPER_STARTED,
        JsValue::Boolean(true),
    )?;
    define_hidden(
        context,
        helper,
        ITERATOR_HELPER_EXECUTING,
        JsValue::Boolean(true),
    )?;
    let result = iterator_zip_helper_next_inner(vm, context, helper);
    define_hidden(
        context,
        helper,
        ITERATOR_HELPER_EXECUTING,
        JsValue::Boolean(false),
    )?;
    result
}

fn iterator_zip_helper_next_inner(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
) -> Result<JsValue, VmError> {
    let count = own_usize(context, helper, ITERATOR_HELPER_COUNT);
    if count == 0 {
        define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
        return iterator_result(context, JsValue::Undefined, true);
    }
    let mode = own_string(context, helper, ITERATOR_HELPER_MODE).unwrap_or_default();
    let mut row = Vec::with_capacity(count);
    let mut value_count = 0usize;
    let mut done_count = 0usize;
    let mut strict_saw_done = false;

    for index in 0..count {
        if !own_bool(context, helper, &zip_active_key(index)).unwrap_or(false) {
            done_count += 1;
            row.push(
                own_data_value(context, helper, &zip_padding_key(index))
                    .unwrap_or(JsValue::Undefined),
            );
            continue;
        }
        let iterator = own_data_value(context, helper, &zip_iterator_key(index))
            .ok_or_else(|| VmError::runtime("Iterator.zip iterator missing"))?;
        let next = own_data_value(context, helper, &zip_next_key(index))
            .ok_or_else(|| VmError::runtime("Iterator.zip next missing"))?;
        let step = iterator_helper_step(vm, context, iterator, next);
        match step {
            Ok(Some(value)) => {
                if mode == "strict" && strict_saw_done {
                    define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
                    close_zip_helper_iterators(vm, context, helper, None);
                    return Err(VmError::type_error(
                        "Iterator.zip strict inputs have different lengths",
                    ));
                }
                value_count += 1;
                row.push(value);
            }
            Ok(None) => {
                done_count += 1;
                define_hidden(
                    context,
                    helper,
                    zip_active_key(index),
                    JsValue::Boolean(false),
                )?;
                row.push(
                    own_data_value(context, helper, &zip_padding_key(index))
                        .unwrap_or(JsValue::Undefined),
                );
                if mode == "shortest" {
                    define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
                    close_zip_helper_iterators_result(vm, context, helper, None)?;
                    return iterator_result(context, JsValue::Undefined, true);
                }
                if mode == "strict" {
                    if value_count != 0 {
                        define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
                        close_zip_helper_iterators(vm, context, helper, None);
                        return Err(VmError::type_error(
                            "Iterator.zip strict inputs have different lengths",
                        ));
                    }
                    strict_saw_done = true;
                }
            }
            Err(error) => {
                return zip_helper_abrupt(vm, context, helper, index, error);
            }
        }
    }

    if mode == "strict" && done_count != 0 {
        define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
        if value_count != 0 {
            close_zip_helper_iterators(vm, context, helper, None);
            return Err(VmError::type_error(
                "Iterator.zip strict inputs have different lengths",
            ));
        }
        return iterator_result(context, JsValue::Undefined, true);
    }
    if mode == "longest" && value_count == 0 {
        define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
        return iterator_result(context, JsValue::Undefined, true);
    }
    let value = if own_string(context, helper, ITERATOR_HELPER_KIND).as_deref() == Some("zipKeyed")
    {
        create_zip_keyed_row(context, helper, row)?
    } else {
        context.create_array(row)?
    };
    iterator_result(context, value, false)
}

fn create_zip_keyed_row(
    context: &mut NativeContext,
    helper: ObjectId,
    values: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = None;
    for (index, value) in values.into_iter().enumerate() {
        match own_data_value(context, helper, &zip_result_key(index)) {
            Some(JsValue::String(key)) => {
                object.define_property(key, PropertyDescriptor::data(value));
            }
            Some(JsValue::Symbol(symbol)) => {
                object.define_symbol_property(symbol, PropertyDescriptor::data(value));
            }
            _ => return Err(VmError::runtime("Iterator.zipKeyed result key missing")),
        }
    }
    context
        .heap_mut()
        .allocate_object(object)
        .map(JsValue::Object)
        .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))
}

fn close_zip_helper_iterators(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
    skip_through: Option<usize>,
) {
    let _ = close_zip_helper_iterators_result(vm, context, helper, skip_through);
    let _ = vm.take_pending_exception_from_builtin();
}

fn close_zip_helper_iterators_result(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
    skip_through: Option<usize>,
) -> Result<(), VmError> {
    let count = own_usize(context, helper, ITERATOR_HELPER_COUNT);
    let mut first_error = None;
    for index in (0..count).rev() {
        if skip_through.is_some_and(|last| index <= last)
            || !own_bool(context, helper, &zip_active_key(index)).unwrap_or(false)
        {
            continue;
        }
        let Some(iterator) = own_data_value(context, helper, &zip_iterator_key(index)) else {
            continue;
        };
        let _ = define_hidden(
            context,
            helper,
            zip_active_key(index),
            JsValue::Boolean(false),
        );
        if let Err(error) = vm.close_iterator_from_builtin(iterator, context) {
            let thrown = vm.take_pending_exception_from_builtin();
            if first_error.is_none() {
                first_error = Some((error, thrown));
            }
        }
    }
    if let Some((error, thrown)) = first_error {
        let _ = vm.take_pending_exception_from_builtin();
        if let Some(value) = thrown {
            return Err(vm.throw_value_from_builtin(value));
        }
        return Err(error);
    }
    Ok(())
}

fn zip_helper_abrupt(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
    index: usize,
    error: VmError,
) -> Result<JsValue, VmError> {
    let thrown = vm.take_pending_exception_from_builtin();
    define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
    close_zip_helper_iterators(vm, context, helper, Some(index));
    let _ = vm.take_pending_exception_from_builtin();
    match thrown {
        Some(value) => Err(vm.throw_value_from_builtin(value)),
        None => Err(error),
    }
}

fn iterator_concat_helper_next_inner(
    vm: &mut Vm,
    context: &mut NativeContext,
    helper: ObjectId,
) -> Result<JsValue, VmError> {
    let count = own_usize(context, helper, ITERATOR_HELPER_COUNT);
    loop {
        context.consume_loop_iteration()?;
        let index = own_usize(context, helper, ITERATOR_HELPER_INDEX);
        if index >= count {
            define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
            return iterator_result(context, JsValue::Undefined, true);
        }

        let mut inner =
            own_data_value(context, helper, ITERATOR_HELPER_INNER).unwrap_or(JsValue::Undefined);
        let mut next = own_data_value(context, helper, ITERATOR_HELPER_INNER_NEXT)
            .unwrap_or(JsValue::Undefined);
        if matches!(inner, JsValue::Undefined) {
            let item = own_data_value(context, helper, &concat_item_key(index))
                .ok_or_else(|| VmError::runtime("Iterator.concat item missing"))?;
            let method = own_data_value(context, helper, &concat_method_key(index))
                .ok_or_else(|| VmError::runtime("Iterator.concat method missing"))?;
            inner = vm.call_value_from_builtin(method, item, Vec::new(), context)?;
            context.require_object(&inner, "Iterator.concat iterator")?;
            next = vm.get_property_value(inner.clone(), "next", context)?;
            if !is_callable(&next) {
                return Err(VmError::type_error(
                    "Iterator.concat iterator next is not callable",
                ));
            }
            define_hidden(context, helper, ITERATOR_HELPER_INNER, inner.clone())?;
            define_hidden(context, helper, ITERATOR_HELPER_INNER_NEXT, next.clone())?;
        }

        if let Some(value) = iterator_helper_step(vm, context, inner, next)? {
            return iterator_result(context, value, false);
        }
        define_hidden(context, helper, ITERATOR_HELPER_INNER, JsValue::Undefined)?;
        define_hidden(
            context,
            helper,
            ITERATOR_HELPER_INNER_NEXT,
            JsValue::Undefined,
        )?;
        set_hidden_usize(context, helper, ITERATOR_HELPER_INDEX, index + 1)?;
    }
}

fn iterator_helper_return(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let helper = context.require_object(&this_value, "Iterator Helper.prototype.return")?;
    if own_string(context, helper, ITERATOR_HELPER_KIND).is_none() {
        return Err(VmError::type_error("receiver is not an Iterator helper"));
    }
    if own_bool(context, helper, ITERATOR_HELPER_EXECUTING).unwrap_or(false) {
        return Err(VmError::type_error("Iterator helper is already running"));
    }
    if own_bool(context, helper, ITERATOR_DONE).unwrap_or(true) {
        return iterator_result(context, JsValue::Undefined, true);
    }
    define_hidden(context, helper, ITERATOR_DONE, JsValue::Boolean(true))?;
    if own_string(context, helper, ITERATOR_HELPER_KIND).as_deref() == Some("concat") {
        let inner = own_data_value(context, helper, ITERATOR_HELPER_INNER);
        if let Some(inner) = inner
            && !matches!(inner, JsValue::Undefined)
        {
            define_hidden(
                context,
                helper,
                ITERATOR_HELPER_EXECUTING,
                JsValue::Boolean(true),
            )?;
            let result = vm.close_iterator_from_builtin(inner, context);
            define_hidden(
                context,
                helper,
                ITERATOR_HELPER_EXECUTING,
                JsValue::Boolean(false),
            )?;
            result?;
        }
        return iterator_result(context, JsValue::Undefined, true);
    }
    if matches!(
        own_string(context, helper, ITERATOR_HELPER_KIND).as_deref(),
        Some("zip" | "zipKeyed")
    ) {
        let started = own_bool(context, helper, ITERATOR_HELPER_STARTED).unwrap_or(false);
        if started {
            define_hidden(
                context,
                helper,
                ITERATOR_HELPER_EXECUTING,
                JsValue::Boolean(true),
            )?;
        }
        let close_result = close_zip_helper_iterators_result(vm, context, helper, None);
        if started {
            define_hidden(
                context,
                helper,
                ITERATOR_HELPER_EXECUTING,
                JsValue::Boolean(false),
            )?;
        }
        close_result?;
        return iterator_result(context, JsValue::Undefined, true);
    }
    if let Some(inner) = own_data_value(context, helper, ITERATOR_HELPER_INNER)
        && !matches!(inner, JsValue::Undefined)
    {
        vm.close_iterator_from_builtin(inner, context)?;
    }
    let source = own_data_value(context, helper, ITERATOR_HELPER_SOURCE)
        .ok_or_else(|| VmError::runtime("Iterator helper source missing"))?;
    vm.close_iterator_from_builtin(source, context)?;
    iterator_result(context, JsValue::Undefined, true)
}

fn iterator_to_array(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let next = iterator_next_method(vm, context, this_value.clone())?;
    let mut values = Vec::new();
    while values.len() < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step_with_next(vm, context, this_value.clone(), next.clone())?
        else {
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
        vm.close_iterator_from_builtin(this_value, context)?;
        return Err(VmError::type_error(
            "Iterator.prototype.forEach callback is not callable",
        ));
    }
    let next = iterator_next_method(vm, context, this_value.clone())?;
    let mut index = 0usize;
    while index < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step_with_next(vm, context, this_value.clone(), next.clone())?
        else {
            return Ok(JsValue::Undefined);
        };
        if let Err(error) = vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![value, JsValue::Number(index as f64)],
            context,
        ) {
            let Some(thrown) = vm.take_pending_exception_from_builtin() else {
                return Err(error);
            };
            return match vm
                .close_iterator_preserving_throw_from_builtin(this_value, thrown, context)
            {
                Ok(()) => Err(error),
                Err(close_error) => Err(close_error),
            };
        }
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

fn iterator_map(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.require_object(&this_value, "Iterator.prototype.map")?;
    let callback = require_helper_callback(vm, context, this_value.clone(), arguments, "map")?;
    create_iterator_helper(vm, context, this_value, "map", callback, 0)
}

fn iterator_filter(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.require_object(&this_value, "Iterator.prototype.filter")?;
    let callback = require_helper_callback(vm, context, this_value.clone(), arguments, "filter")?;
    create_iterator_helper(vm, context, this_value, "filter", callback, 0)
}

fn iterator_take(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.require_object(&this_value, "Iterator.prototype.take")?;
    let limit = match iterator_limit(vm, context, arguments.first(), "Iterator.prototype.take") {
        Ok(limit) => limit,
        Err(error) => {
            vm.close_iterator_from_builtin(this_value, context)?;
            return Err(error);
        }
    };
    create_iterator_helper(vm, context, this_value, "take", JsValue::Undefined, limit)
}

fn iterator_drop(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.require_object(&this_value, "Iterator.prototype.drop")?;
    let limit = match iterator_limit(vm, context, arguments.first(), "Iterator.prototype.drop") {
        Ok(limit) => limit,
        Err(error) => {
            vm.close_iterator_from_builtin(this_value, context)?;
            return Err(error);
        }
    };
    create_iterator_helper(vm, context, this_value, "drop", JsValue::Undefined, limit)
}

fn iterator_flat_map(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.require_object(&this_value, "Iterator.prototype.flatMap")?;
    let callback = require_helper_callback(vm, context, this_value.clone(), arguments, "flatMap")?;
    create_iterator_helper(vm, context, this_value, "flatMap", callback, 0)
}

fn iterator_reduce(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let callback = match require_callback(arguments, "Iterator.prototype.reduce") {
        Ok(callback) => callback,
        Err(error) => {
            vm.close_iterator_from_builtin(this_value, context)?;
            return Err(error);
        }
    };
    let next = iterator_next_method(vm, context, this_value.clone())?;
    let mut index = 0usize;
    let mut accumulator = if let Some(initial) = arguments.get(1) {
        initial.clone()
    } else {
        let Some(value) = iterator_step_with_next(vm, context, this_value.clone(), next.clone())?
        else {
            return Err(VmError::type_error(
                "Reduce of empty iterator with no initial value",
            ));
        };
        index = 1;
        value
    };
    while index < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step_with_next(vm, context, this_value.clone(), next.clone())?
        else {
            return Ok(accumulator);
        };
        accumulator = match vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![accumulator, value, JsValue::Number(index as f64)],
            context,
        ) {
            Ok(value) => value,
            Err(error) => {
                let Some(thrown) = vm.take_pending_exception_from_builtin() else {
                    return Err(error);
                };
                return match vm
                    .close_iterator_preserving_throw_from_builtin(this_value, thrown, context)
                {
                    Ok(()) => Err(error),
                    Err(close_error) => Err(close_error),
                };
            }
        };
        index += 1;
    }
    Err(VmError::runtime_limit(
        "iterator helper step limit exceeded",
    ))
}

fn require_callback(arguments: &[JsValue], label: &str) -> Result<JsValue, VmError> {
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&callback) {
        return Err(VmError::type_error(format!(
            "{label} callback is not callable"
        )));
    }
    Ok(callback)
}

fn require_helper_callback(
    vm: &mut Vm,
    context: &mut NativeContext,
    source: JsValue,
    arguments: &[JsValue],
    name: &str,
) -> Result<JsValue, VmError> {
    match require_callback(arguments, &format!("Iterator.prototype.{name}")) {
        Ok(callback) => Ok(callback),
        Err(error) => {
            vm.close_iterator_from_builtin(source, context)?;
            Err(error)
        }
    }
}

fn iterator_limit(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: Option<&JsValue>,
    label: &str,
) -> Result<usize, VmError> {
    let number = vm
        .to_number(value.cloned().unwrap_or(JsValue::Undefined), context)?
        .floor();
    if number.is_nan() || number < 0.0 {
        return Err(VmError::range(format!(
            "{label} limit must be a non-negative number"
        )));
    }
    Ok(number.min(MAX_ITERATOR_STEPS as f64) as usize)
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
        vm.close_iterator_from_builtin(this_value, context)?;
        return Err(VmError::type_error(
            "iterator predicate callback is not callable",
        ));
    }
    let next = iterator_next_method(vm, context, this_value.clone())?;
    let mut index = 0usize;
    while index < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step_with_next(vm, context, this_value.clone(), next.clone())?
        else {
            return Ok(match mode {
                PredicateMode::Some => JsValue::Boolean(false),
                PredicateMode::Every => JsValue::Boolean(true),
                PredicateMode::Find => JsValue::Undefined,
            });
        };
        let keep = match vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![value.clone(), JsValue::Number(index as f64)],
            context,
        ) {
            Ok(value) => value.to_boolean(),
            Err(error) => {
                let Some(thrown) = vm.take_pending_exception_from_builtin() else {
                    return Err(error);
                };
                return match vm
                    .close_iterator_preserving_throw_from_builtin(this_value, thrown, context)
                {
                    Ok(()) => Err(error),
                    Err(close_error) => Err(close_error),
                };
            }
        };
        match mode {
            PredicateMode::Some if keep => {
                vm.close_iterator_from_builtin(this_value, context)?;
                return Ok(JsValue::Boolean(true));
            }
            PredicateMode::Every if !keep => {
                vm.close_iterator_from_builtin(this_value, context)?;
                return Ok(JsValue::Boolean(false));
            }
            PredicateMode::Find if keep => {
                vm.close_iterator_from_builtin(this_value, context)?;
                return Ok(value);
            }
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
    define_method(context, constructor_object, "groupBy", 2, map_group_by)?;
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

fn map_group_by(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let items = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let callback = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&callback) {
        return Err(VmError::type_error("Map.groupBy callback is not callable"));
    }
    let map_constructor = context
        .get_global("Map")
        .ok_or_else(|| VmError::runtime("Map constructor missing"))?;
    let prototype = context
        .constructor_prototype(&map_constructor)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Map prototype missing"))?;
    let result = create_collection_object(context, prototype, "Map")?;
    let JsValue::Object(map) = result.clone() else {
        unreachable!()
    };

    let iterator = iterator_from(vm, context, JsValue::Undefined, &[items])?;
    let mut index = 0usize;
    while index < MAX_ITERATOR_STEPS {
        let Some(value) = iterator_step(vm, context, iterator.clone())? else {
            return Ok(result);
        };
        let mut key = match vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![value.clone(), JsValue::Number(index as f64)],
            context,
        ) {
            Ok(value) => value,
            Err(error) => {
                let Some(thrown) = vm.take_pending_exception_from_builtin() else {
                    return Err(error);
                };
                return match vm.close_iterator_preserving_throw_from_builtin(
                    iterator, thrown, context,
                ) {
                    Ok(()) => Err(error),
                    Err(close_error) => Err(close_error),
                };
            }
        };
        if matches!(key, JsValue::Number(number) if number == 0.0) {
            key = JsValue::Number(0.0);
        }
        if let Some(entry) = find_entry(context, map, &key) {
            let group = collection_entry_value(context, map, entry).unwrap_or(JsValue::Undefined);
            append_array_value(context, group, value)?;
        } else {
            let group = context.create_array(vec![value])?;
            set_collection_entry(context, map, key, group)?;
        }
        index += 1;
    }
    Err(VmError::runtime_limit("Map.groupBy iterator step limit exceeded"))
}

fn append_array_value(
    context: &mut NativeContext,
    array: JsValue,
    value: JsValue,
) -> Result<(), VmError> {
    let object = context.require_object(&array, "Map.groupBy group")?;
    let length = array_like_length(context, object)?;
    context.set_property(array, length.to_string(), value)?;
    Ok(())
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
    let species_getter =
        context.register_builtin("get [Symbol.species]", 0, set_species_get, None)?;
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
    define_method(context, prototype, "union", 1, set_union)?;
    define_method(context, prototype, "intersection", 1, set_intersection)?;
    define_method(context, prototype, "difference", 1, set_difference)?;
    define_method(
        context,
        prototype,
        "symmetricDifference",
        1,
        set_symmetric_difference,
    )?;
    define_method(context, prototype, "isSubsetOf", 1, set_is_subset_of)?;
    define_method(context, prototype, "isSupersetOf", 1, set_is_superset_of)?;
    define_method(
        context,
        prototype,
        "isDisjointFrom",
        1,
        set_is_disjoint_from,
    )?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Set".into())),
    )?;
    declare_standard_global(context, "Set", constructor)
}

fn set_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
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

fn set_values_from_collection(context: &NativeContext, set: ObjectId) -> Vec<JsValue> {
    let next = own_usize(context, set, COLLECTION_NEXT_INDEX);
    let mut values = Vec::new();
    for index in 0..next {
        if entry_is_active(context, set, index)
            && let Some(value) = collection_entry_key(context, set, index)
        {
            values.push(value);
        }
    }
    values
}

fn set_like_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    if let Ok(set) = require_collection(context, &value, "Set") {
        return Ok(set_values_from_collection(context, set));
    }
    let object = context.require_object(&value, "Set method argument")?;
    let keys = vm.get_property_value(context.object_value(object), "keys", context)?;
    if !is_callable(&keys) {
        return Err(VmError::type_error("set-like keys is not callable"));
    }
    let iterator = vm.call_value_from_builtin(keys, value, Vec::new(), context)?;
    collect_iterator_values(vm, context, iterator)
}

fn value_in_list(values: &[JsValue], needle: &JsValue) -> bool {
    values.iter().any(|value| same_value_zero(value, needle))
}

fn set_result_object(context: &mut NativeContext) -> Result<ObjectId, VmError> {
    let prototype = context
        .get_global("Set")
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten())
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Set prototype missing"))?;
    let value = create_collection_object(context, prototype, "Set")?;
    context
        .value_object(&value)
        .ok_or_else(|| VmError::runtime("Set result object missing"))
}

fn set_from_values(context: &mut NativeContext, values: Vec<JsValue>) -> Result<JsValue, VmError> {
    let result = set_result_object(context)?;
    for value in values {
        set_collection_entry(context, result, value.clone(), value)?;
    }
    Ok(JsValue::Object(result))
}

fn set_union(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let mut values = set_values_from_collection(context, set);
    for value in set_like_values(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )? {
        if !value_in_list(&values, &value) {
            values.push(value);
        }
    }
    set_from_values(context, values)
}

fn set_intersection(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let other = set_like_values(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let values = set_values_from_collection(context, set)
        .into_iter()
        .filter(|value| value_in_list(&other, value))
        .collect();
    set_from_values(context, values)
}

fn set_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let other = set_like_values(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let values = set_values_from_collection(context, set)
        .into_iter()
        .filter(|value| !value_in_list(&other, value))
        .collect();
    set_from_values(context, values)
}

fn set_symmetric_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let left = set_values_from_collection(context, set);
    let other = set_like_values(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let mut values = Vec::new();
    for value in &left {
        if !value_in_list(&other, value) {
            values.push(value.clone());
        }
    }
    for value in other {
        if !value_in_list(&left, &value) && !value_in_list(&values, &value) {
            values.push(value);
        }
    }
    set_from_values(context, values)
}

fn set_is_subset_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let other = set_like_values(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Boolean(
        set_values_from_collection(context, set)
            .iter()
            .all(|value| value_in_list(&other, value)),
    ))
}

fn set_is_superset_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let left = set_values_from_collection(context, set);
    let other = set_like_values(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Boolean(
        other.iter().all(|value| value_in_list(&left, value)),
    ))
}

fn set_is_disjoint_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let set = require_collection(context, &this_value, "Set")?;
    let left = set_values_from_collection(context, set);
    let other = set_like_values(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Boolean(
        left.iter().all(|value| !value_in_list(&other, value)),
    ))
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
