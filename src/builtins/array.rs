//! `Array` constructor and prototype methods.

use crate::{
    runtime::{
        IteratorKind, IteratorMode, JsValue, NativeContext, NativeErrorKind, NativeErrorValue,
        ObjectId, ObjectKind, PrimitiveValue, PropertyDescriptor, PropertyDescriptorUpdate,
        PropertyKey,
        abstract_ops::{self, to_integer_or_infinity},
    },
    vm::{Vm, VmError},
};

pub fn install_array(context: &mut NativeContext) {
    let Some(intrinsics) = context.intrinsics().cloned() else {
        return;
    };
    let JsValue::BuiltinFunction(constructor) = intrinsics.array_constructor else {
        return;
    };
    let constructor_object = context.builtin(constructor).unwrap().object;
    let species_getter = context
        .register_builtin("get [Symbol.species]", 0, array_species_get, None)
        .expect("install Array @@species");
    context
        .define_symbol_own_property(
            constructor_object,
            context.well_known_symbols().species,
            PropertyDescriptor::accessor(Some(species_getter), None, false, true),
        )
        .expect("define Array @@species");

    // Static methods on Array
    let is_array = context
        .register_builtin("isArray", 1, array_is_array, None)
        .expect("install Array.isArray");
    context
        .define_own_property(
            constructor_object,
            "isArray".into(),
            PropertyDescriptor::data_with(is_array, true, false, true),
        )
        .expect("define Array.isArray");

    let from = context
        .register_builtin("from", 1, array_from, None)
        .expect("install Array.from");
    context
        .define_own_property(
            constructor_object,
            "from".into(),
            PropertyDescriptor::data_with(from, true, false, true),
        )
        .expect("define Array.from");

    let from_async = context
        .register_builtin("fromAsync", 1, array_from_async, None)
        .expect("install Array.fromAsync");
    context
        .define_own_property(
            constructor_object,
            "fromAsync".into(),
            PropertyDescriptor::data_with(from_async, true, false, true),
        )
        .expect("define Array.fromAsync");

    let of = context
        .register_builtin("of", 0, array_of, None)
        .expect("install Array.of");
    context
        .define_own_property(
            constructor_object,
            "of".into(),
            PropertyDescriptor::data_with(of, true, false, true),
        )
        .expect("define Array.of");

    // Prototype methods on Array.prototype
    for (name, length, call) in [
        ("push", 1, array_push as crate::runtime::NativeCall),
        ("pop", 0, array_pop as crate::runtime::NativeCall),
        ("toString", 0, array_to_string as crate::runtime::NativeCall),
        ("join", 1, array_join as crate::runtime::NativeCall),
        ("reverse", 0, array_reverse as crate::runtime::NativeCall),
        ("concat", 1, array_concat as crate::runtime::NativeCall),
        ("slice", 2, array_slice as crate::runtime::NativeCall),
        ("splice", 2, array_splice as crate::runtime::NativeCall),
        ("indexOf", 1, array_index_of as crate::runtime::NativeCall),
        (
            "lastIndexOf",
            1,
            array_last_index_of as crate::runtime::NativeCall,
        ),
        ("fill", 1, array_fill as crate::runtime::NativeCall),
        ("includes", 1, array_includes as crate::runtime::NativeCall),
        ("shift", 0, array_shift as crate::runtime::NativeCall),
        ("unshift", 1, array_unshift as crate::runtime::NativeCall),
        ("forEach", 1, array_for_each as crate::runtime::NativeCall),
        ("map", 1, array_map as crate::runtime::NativeCall),
        ("filter", 1, array_filter as crate::runtime::NativeCall),
        ("reduce", 1, array_reduce as crate::runtime::NativeCall),
        (
            "reduceRight",
            1,
            array_reduce_right as crate::runtime::NativeCall,
        ),
        ("every", 1, array_every as crate::runtime::NativeCall),
        ("some", 1, array_some as crate::runtime::NativeCall),
        ("find", 1, array_find as crate::runtime::NativeCall),
        (
            "findIndex",
            1,
            array_find_index as crate::runtime::NativeCall,
        ),
        ("findLast", 1, array_find_last as crate::runtime::NativeCall),
        (
            "findLastIndex",
            1,
            array_find_last_index as crate::runtime::NativeCall,
        ),
        ("flat", 0, array_flat as crate::runtime::NativeCall),
        ("flatMap", 1, array_flat_map as crate::runtime::NativeCall),
        ("sort", 1, array_sort as crate::runtime::NativeCall),
        ("keys", 0, array_keys as crate::runtime::NativeCall),
        ("values", 0, array_values as crate::runtime::NativeCall),
        ("entries", 0, array_entries as crate::runtime::NativeCall),
        (
            "copyWithin",
            2,
            array_copy_within as crate::runtime::NativeCall,
        ),
        ("at", 1, array_at as crate::runtime::NativeCall),
        (
            "toLocaleString",
            0,
            array_to_locale_string as crate::runtime::NativeCall,
        ),
        (
            "toReversed",
            0,
            array_to_reversed as crate::runtime::NativeCall,
        ),
        ("toSorted", 1, array_to_sorted as crate::runtime::NativeCall),
        (
            "toSpliced",
            2,
            array_to_spliced as crate::runtime::NativeCall,
        ),
        ("with", 2, array_with as crate::runtime::NativeCall),
    ] {
        let value = context
            .register_builtin(name, length, call, None)
            .expect("install Array prototype method");
        context
            .define_own_property(
                intrinsics.array_prototype,
                name.into(),
                PropertyDescriptor::data_with(value, true, false, true),
            )
            .expect("define Array prototype method");
    }

    // @@iterator = Array.prototype.values
    let values_fn = context
        .register_builtin("[Symbol.iterator]", 0, array_values, None)
        .expect("install Array @@iterator");
    let iterator_symbol = context.well_known_symbols().iterator;
    context
        .define_symbol_own_property(
            intrinsics.array_prototype,
            iterator_symbol,
            PropertyDescriptor::data_with(values_fn, true, false, true),
        )
        .expect("define Array @@iterator");
}

fn array_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn get_property_preserving_throw(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: &str,
) -> Result<JsValue, VmError> {
    match vm.get_property_value_catching_from_builtin(receiver, key, context)? {
        Ok(value) => Ok(value),
        Err(error) => Err(vm.throw_value_from_builtin(error)),
    }
}

/// Shared SpeciesConstructor abstract operation — delegates to the
/// canonical implementation in `abstract_ops` for consistent semantics
/// across Array, Promise, TypedArray, and RegExp.
pub(crate) fn species_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: JsValue,
    default_constructor: JsValue,
) -> Result<JsValue, VmError> {
    abstract_ops::species_constructor(vm, context, object, default_constructor)
}

/// Shared ArraySpeciesCreate abstract operation — delegates to the
/// canonical implementation in `abstract_ops` for consistent semantics.
pub(crate) fn array_species_create(
    vm: &mut Vm,
    context: &mut NativeContext,
    original_array: JsValue,
    length: usize,
) -> Result<JsValue, VmError> {
    abstract_ops::array_species_create(vm, context, original_array, length)
}

pub fn array_call(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    create_array(context, arguments)
}

pub fn array_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let array = create_array(context, arguments)?;
    if let Some(prototype) = vm.get_array_prototype_from_constructor(new_target, context)?
        && let Some(object) = context.value_object(&array)
    {
        context.set_prototype_of(object, Some(prototype))?;
    }
    Ok(array)
}

fn create_array(context: &mut NativeContext, arguments: &[JsValue]) -> Result<JsValue, VmError> {
    if arguments.len() == 1 && matches!(arguments[0], JsValue::Number(_)) {
        let length = context.array_length_from_value(arguments[0].clone())?;
        context.create_sparse_array(length)
    } else {
        context.create_array(arguments.to_vec())
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn array_like_length(context: &NativeContext, object: ObjectId) -> usize {
    if let Some(o) = context.heap().object(object) {
        if let Some(len) = o.array_length() {
            return len;
        }
        if let Some(val) = o.own_property("length").and_then(|d| d.value_cloned()) {
            return val
                .to_number()
                .unwrap_or(0.0)
                .max(0.0)
                .min(MAX_ARRAY_LENGTH as f64) as usize;
        }
    }
    0
}

fn to_length_number(number: f64) -> usize {
    if number.is_nan() || number <= 0.0 {
        0
    } else if !number.is_finite() || number >= MAX_SAFE_INTEGER as f64 {
        MAX_SAFE_INTEGER
    } else {
        number.floor() as usize
    }
}

fn array_like_length_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    object: ObjectId,
) -> Result<usize, VmError> {
    if let Some(length) = context
        .heap()
        .object(object)
        .and_then(|object| object.array_length())
    {
        return Ok(length);
    }
    if let Some(PrimitiveValue::String(value)) = context.primitive_value(object) {
        return Ok(value.encode_utf16().count().min(MAX_ARRAY_LENGTH));
    }
    let length = get_property_preserving_throw(vm, context, receiver, "length")?;
    Ok(to_length_number(vm.to_number(length, context)?))
}

fn string_index_value(value: &str, index: usize) -> Option<JsValue> {
    value
        .encode_utf16()
        .nth(index)
        .map(|unit| JsValue::String(String::from_utf16_lossy(&[unit])))
}

fn string_index_value_for_array_like(
    context: &NativeContext,
    receiver: &JsValue,
    object: ObjectId,
    index: usize,
) -> Option<JsValue> {
    if let JsValue::String(value) = receiver {
        return string_index_value(value, index);
    }
    match context.primitive_value(object) {
        Some(PrimitiveValue::String(value)) => string_index_value(value, index),
        _ => None,
    }
}

fn array_index_exists(
    context: &NativeContext,
    receiver: &JsValue,
    object: ObjectId,
    index: usize,
) -> Result<bool, VmError> {
    if string_index_value_for_array_like(context, receiver, object, index).is_some() {
        return Ok(true);
    }
    context
        .find_property_descriptor(object, &index.to_string())
        .map(|descriptor| descriptor.is_some())
}

fn get_existing_elem(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    object: ObjectId,
    index: usize,
) -> Result<JsValue, VmError> {
    if let Some(value) = string_index_value_for_array_like(context, &receiver, object, index) {
        context.consume_loop_iteration()?;
        return Ok(value);
    }
    get_elem(vm, context, receiver, index)
}

fn create_array_data_property(
    context: &mut NativeContext,
    array: &JsValue,
    index: usize,
    value: JsValue,
) -> Result<(), VmError> {
    let JsValue::Object(object) = array else {
        return Err(VmError::runtime("array result is not an object"));
    };
    create_data_property_or_throw(context, *object, index, value)
}

fn set_array_index_strict(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    index: usize,
    value: JsValue,
) -> Result<(), VmError> {
    if vm.set_property_value_strict_from_builtin(target, &index.to_string(), value, context)? {
        Ok(())
    } else {
        Err(VmError::type_error("cannot write array index"))
    }
}

fn set_array_length_strict(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    length: usize,
) -> Result<(), VmError> {
    if vm.set_property_value_strict_from_builtin(
        target,
        "length",
        JsValue::Number(length as f64),
        context,
    )? {
        Ok(())
    } else {
        Err(VmError::type_error("cannot write array length"))
    }
}

fn array_callback_target(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
) -> Result<(ObjectId, JsValue, JsValue, usize), VmError> {
    let object = vm.to_object(this_value.clone(), context)?;
    let object_value = context.object_value(object);
    let receiver = if matches!(this_value, JsValue::String(_)) {
        this_value
    } else {
        object_value.clone()
    };
    let length = array_like_length_from_value(vm, context, receiver.clone(), object)?;
    Ok((object, receiver, object_value, length))
}

fn array_object_target(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
) -> Result<(ObjectId, JsValue, usize), VmError> {
    let object = vm.to_object(this_value.clone(), context)?;
    let object_value = context.object_value(object);
    let receiver = if matches!(this_value, JsValue::String(_)) {
        this_value
    } else {
        object_value.clone()
    };
    let length = array_like_length_from_value(vm, context, receiver, object)?;
    Ok((object, object_value, length))
}

fn normalize_index(raw: f64, length: usize) -> usize {
    let raw = to_integer_or_infinity(raw);
    if raw < 0.0 {
        let from_end = (-raw) as usize;
        length.saturating_sub(from_end)
    } else {
        (raw as usize).min(length)
    }
}

fn array_from_start_index(raw: f64, length: usize) -> usize {
    let integer = to_integer_or_infinity(raw);
    if integer.is_infinite() {
        return if integer.is_sign_positive() {
            length
        } else {
            0
        };
    }
    if integer >= 0.0 {
        (integer as usize).min(length)
    } else {
        length.saturating_sub((-integer) as usize)
    }
}

fn array_from_last_index(raw: f64, length: usize) -> Option<usize> {
    if length == 0 {
        return None;
    }
    let integer = to_integer_or_infinity(raw);
    if integer.is_infinite() {
        return if integer.is_sign_positive() {
            Some(length - 1)
        } else {
            None
        };
    }
    if integer >= 0.0 {
        Some((integer as usize).min(length - 1))
    } else {
        let from_end = (-integer) as usize;
        if from_end > length {
            None
        } else {
            Some(length - from_end)
        }
    }
}

/// ECMAScript maximum array length.
const MAX_ARRAY_LENGTH: usize = 4_294_967_295;
const MAX_SAFE_INTEGER: usize = 9_007_199_254_740_991;
/// Cap iteration/allocation to prevent O(N) hangs on sparse arrays with huge lengths.
/// This covers Test262's common sparse index 99,999 without permitting
/// unbounded iteration over adversarial near-MAX_SAFE_INTEGER lengths.
const MAX_DENSE_ALLOC: usize = 1 << 20;

/// Reads array element `index` via the full VM property-get path (supports accessor getters).
fn get_elem(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    index: usize,
) -> Result<JsValue, VmError> {
    context.consume_loop_iteration()?;
    get_property_preserving_throw(vm, context, value, &index.to_string())
}

fn argument_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
    default: f64,
) -> Result<f64, VmError> {
    match arguments.get(index) {
        None | Some(JsValue::Undefined) => Ok(default),
        Some(value) => vm.to_number(value.clone(), context),
    }
}

fn call_callback(
    vm: &mut Vm,
    context: &mut NativeContext,
    callback: JsValue,
    this_arg: JsValue,
    args: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    if !context.is_callable_value(&callback) {
        return Err(VmError::type_error("callback is not a function"));
    }
    vm.call_value_from_builtin(callback, this_arg, args, context)
}

fn require_callable(context: &NativeContext, value: &JsValue, method: &str) -> Result<(), VmError> {
    if !context.is_callable_value(value) {
        Err(VmError::type_error(format!(
            "{method}: callback is not callable"
        )))
    } else {
        Ok(())
    }
}

// ── Array static methods ──────────────────────────────────────────────────────

fn array_is_array(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Boolean(is_array_value(
        context,
        arguments.first().unwrap_or(&JsValue::Undefined),
    )?))
}

fn is_array_value(context: &NativeContext, value: &JsValue) -> Result<bool, VmError> {
    let Some(object) = context.value_object(value) else {
        return Ok(false);
    };
    if context.is_array_object(object)? {
        return Ok(true);
    }
    let Some(record) = context.proxy_record(object) else {
        return Ok(false);
    };
    if matches!(record.handler, JsValue::Null) {
        return Err(VmError::type_error("proxy has been revoked"));
    }
    is_array_value(context, &record.target)
}

use super::is_callable;

fn array_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let map_fn_raw = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let map_this = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);

    // Spec 22.1.2.1 step 3: if mapfn is not undefined, it must be callable.
    let map_fn = if matches!(map_fn_raw, JsValue::Undefined) {
        None
    } else if is_callable(&map_fn_raw) {
        Some(map_fn_raw)
    } else {
        return Err(VmError::type_error("Array.from: mapfn is not callable"));
    };

    if matches!(source, JsValue::Null | JsValue::Undefined) {
        return Err(VmError::type_error("cannot Array.from on nullish value"));
    }
    let object = vm.to_object(source.clone(), context)?;
    let source_value = match source {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => source,
        _ => context.object_value(object),
    };
    let iterator_method = vm.get_symbol_property_value_with_receiver_from_builtin(
        source_value.clone(),
        source_value.clone(),
        context.well_known_symbols().iterator,
        context,
    )?;
    if !matches!(iterator_method, JsValue::Undefined | JsValue::Null) {
        if !is_callable(&iterator_method) {
            return Err(VmError::type_error(
                "Array.from: @@iterator is not callable",
            ));
        }
        return array_from_iterator(
            vm,
            context,
            this,
            source_value,
            iterator_method,
            map_fn,
            map_this,
        );
    }
    let length = array_like_length_from_value(vm, context, source_value.clone(), object)?;

    array_from_array_like(vm, context, this, source_value, length, map_fn, map_this)
}

fn array_from_async_error(error: VmError) -> JsValue {
    let kind = match error.kind {
        crate::vm::VmErrorKind::Reference => NativeErrorKind::Reference,
        crate::vm::VmErrorKind::Type => NativeErrorKind::Type,
        crate::vm::VmErrorKind::Syntax => NativeErrorKind::Syntax,
        crate::vm::VmErrorKind::Range => NativeErrorKind::Range,
        crate::vm::VmErrorKind::Test262 => NativeErrorKind::Test262,
        crate::vm::VmErrorKind::RuntimeLimit => NativeErrorKind::RuntimeLimit,
        crate::vm::VmErrorKind::Runtime => NativeErrorKind::Error,
    };
    JsValue::Error(NativeErrorValue::new(kind, error.message))
}
fn array_from_async(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let promise = context.create_promise()?;
    let prototype = context
        .get_global("Promise")
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten());
    let promise_object = context.create_promise_object(promise, prototype)?;
    match array_from_async_operation(vm, context, this, arguments) {
        Ok(value) => crate::builtins::promise::resolve_promise_id(vm, context, promise, value)?,
        Err(error) => {
            let reason = vm
                .take_pending_exception_from_builtin()
                .unwrap_or_else(|| array_from_async_error(error));
            context.reject_promise(promise, reason)?;
        }
    }
    Ok(promise_object)
}

fn array_from_async_operation(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let map_fn = match arguments.get(1).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined => None,
        value if is_callable(&value) => Some(value),
        _ => return Err(VmError::type_error("Array.fromAsync mapfn is not callable")),
    };
    let map_this = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    let source_object = vm.to_object(source.clone(), context)?;
    let source_value = match source {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => source,
        _ => context.object_value(source_object),
    };
    let async_method = vm.get_symbol_property_value_with_receiver_from_builtin(
        source_value.clone(),
        source_value.clone(),
        context.well_known_symbols().async_iterator,
        context,
    )?;
    let (values, await_inputs, constructor_length) =
        if matches!(async_method, JsValue::Undefined | JsValue::Null) {
            let sync_method = vm.get_symbol_property_value_with_receiver_from_builtin(
                source_value.clone(),
                source_value.clone(),
                context.well_known_symbols().iterator,
                context,
            )?;
            if matches!(sync_method, JsValue::Undefined | JsValue::Null) {
                let length =
                    array_like_length_from_value(vm, context, source_value.clone(), source_object)?;
                let mut values = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
                for index in 0..length.min(MAX_DENSE_ALLOC) {
                    values.push(get_elem(vm, context, source_value.clone(), index)?);
                }
                (values, true, Some(length))
            } else {
                if !is_callable(&sync_method) {
                    return Err(VmError::type_error(
                        "Array.fromAsync @@iterator is not callable",
                    ));
                }
                let iterator = call_callback(vm, context, sync_method, source_value, Vec::new())?;
                (
                    vm.collect_iterator_values_from_builtin(iterator, context)?,
                    true,
                    None,
                )
            }
        } else {
            if !is_callable(&async_method) {
                return Err(VmError::type_error(
                    "Array.fromAsync @@asyncIterator is not callable",
                ));
            }
            let iterator = call_callback(vm, context, async_method, source_value, Vec::new())?;
            context.require_object(&iterator, "Array.fromAsync async iterator")?;
            let next = get_property_preserving_throw(vm, context, iterator.clone(), "next")?;
            if !is_callable(&next) {
                return Err(VmError::type_error("async iterator next is not callable"));
            }
            let mut values = Vec::new();
            while values.len() < MAX_DENSE_ALLOC {
                let result =
                    call_callback(vm, context, next.clone(), iterator.clone(), Vec::new())?;
                let result = vm.await_value_from_builtin(result, context)?;
                context.require_object(&result, "async iterator result")?;
                if get_property_preserving_throw(vm, context, result.clone(), "done")?.to_boolean()
                {
                    break;
                }
                values.push(get_property_preserving_throw(vm, context, result, "value")?);
            }
            (values, false, None)
        };
    let final_length = values.len();
    let result = array_from_create_result(vm, context, constructor, constructor_length)?;
    let result_object = context.require_object(&result, "Array.fromAsync result")?;
    for (index, value) in values.into_iter().enumerate() {
        let value = if await_inputs || map_fn.is_some() {
            await_array_from_async_value(vm, context, value)?
        } else {
            value
        };
        let value = if let Some(mapper) = &map_fn {
            let mapped = call_callback(
                vm,
                context,
                mapper.clone(),
                map_this.clone(),
                vec![value, JsValue::Number(index as f64)],
            )?;
            await_array_from_async_value(vm, context, mapped)?
        } else {
            value
        };
        create_data_property_or_throw(context, result_object, index, value)?;
    }
    set_array_from_length(vm, context, result.clone(), final_length)?;
    Ok(result)
}

fn await_array_from_async_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    mut value: JsValue,
) -> Result<JsValue, VmError> {
    for _ in 0..32 {
        let was_promise = context.promise_id_from_value(&value).is_some();
        value = vm.await_value_from_builtin(value, context)?;
        if !was_promise || context.promise_id_from_value(&value).is_none() {
            return Ok(value);
        }
    }
    Err(VmError::runtime_limit(
        "Array.fromAsync promise assimilation limit exceeded",
    ))
}

fn array_from_array_like(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    source: JsValue,
    length: usize,
    map_fn: Option<JsValue>,
    map_this: JsValue,
) -> Result<JsValue, VmError> {
    let result = array_from_create_result(vm, context, constructor, Some(length))?;
    let result_object = context.require_object(&result, "Array.from result")?;
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, source.clone(), i)?;
        let mapped = if let Some(ref func) = map_fn {
            call_callback(
                vm,
                context,
                func.clone(),
                map_this.clone(),
                vec![val, JsValue::Number(i as f64)],
            )?
        } else {
            val
        };
        create_data_property_or_throw(context, result_object, i, mapped)?;
    }
    set_array_from_length(vm, context, result.clone(), length)?;
    Ok(result)
}

fn array_from_iterator(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    source: JsValue,
    iterator_method: JsValue,
    map_fn: Option<JsValue>,
    map_this: JsValue,
) -> Result<JsValue, VmError> {
    let iterator = call_callback(vm, context, iterator_method, source, Vec::new())?;
    if is_native_iterator(context, &iterator) {
        return array_from_native_iterator(vm, context, constructor, iterator, map_fn, map_this);
    }
    let result = array_from_create_result(vm, context, constructor, None)?;
    let result_object = context.require_object(&result, "Array.from result")?;
    let mut length = 0usize;
    while length < MAX_DENSE_ALLOC {
        let next = vm.get_property_value_with_receiver_from_builtin(
            iterator.clone(),
            iterator.clone(),
            "next",
            context,
        )?;
        if !is_callable(&next) {
            return Err(VmError::type_error(
                "Array.from: iterator next is not callable",
            ));
        }
        let step = call_callback(vm, context, next, iterator.clone(), Vec::new())?;
        let step_object = context.require_object(&step, "Array.from iterator result")?;
        let step_value = context.object_value(step_object);
        let done = vm
            .get_property_value_with_receiver_from_builtin(
                step_value.clone(),
                step_value.clone(),
                "done",
                context,
            )?
            .to_boolean();
        if done {
            set_array_from_length(vm, context, result.clone(), length)?;
            return Ok(result);
        }
        let value = vm.get_property_value_with_receiver_from_builtin(
            step_value.clone(),
            step_value,
            "value",
            context,
        )?;
        let mapped = if let Some(ref func) = map_fn {
            match call_callback(
                vm,
                context,
                func.clone(),
                map_this.clone(),
                vec![value, JsValue::Number(length as f64)],
            ) {
                Ok(value) => value,
                Err(error) => {
                    let _ = array_from_close_iterator(vm, context, iterator.clone());
                    return Err(error);
                }
            }
        } else {
            value
        };
        if let Err(error) = create_data_property_or_throw(context, result_object, length, mapped) {
            let _ = array_from_close_iterator(vm, context, iterator.clone());
            return Err(error);
        }
        length += 1;
    }
    Err(VmError::runtime_limit(
        "Array.from iterator step limit exceeded",
    ))
}

fn array_from_close_iterator(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator: JsValue,
) -> Result<(), VmError> {
    let return_method = vm.get_property_value_with_receiver_from_builtin(
        iterator.clone(),
        iterator.clone(),
        "return",
        context,
    )?;
    if matches!(return_method, JsValue::Undefined | JsValue::Null) {
        return Ok(());
    }
    if !is_callable(&return_method) {
        return Err(VmError::type_error(
            "Array.from: iterator return is not callable",
        ));
    }
    let _ = call_callback(vm, context, return_method, iterator, Vec::new())?;
    Ok(())
}

fn is_native_iterator(context: &NativeContext, value: &JsValue) -> bool {
    context
        .value_object(value)
        .and_then(|object| context.heap().object(object))
        .is_some_and(|object| {
            matches!(
                object.kind,
                ObjectKind::Iterator {
                    record: crate::runtime::IteratorRecord {
                        kind: IteratorKind::Array { .. } | IteratorKind::String { .. },
                        ..
                    }
                }
            )
        })
}

fn array_from_native_iterator(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    iterator: JsValue,
    map_fn: Option<JsValue>,
    map_this: JsValue,
) -> Result<JsValue, VmError> {
    let result = array_from_create_result(vm, context, constructor, None)?;
    let result_object = context.require_object(&result, "Array.from result")?;
    let mut length = 0usize;
    while length < MAX_DENSE_ALLOC {
        let (value, done) = context.step_iterator_object(iterator.clone())?;
        if done {
            set_array_from_length(vm, context, result.clone(), length)?;
            return Ok(result);
        }
        let mapped = if let Some(ref func) = map_fn {
            match call_callback(
                vm,
                context,
                func.clone(),
                map_this.clone(),
                vec![value, JsValue::Number(length as f64)],
            ) {
                Ok(value) => value,
                Err(error) => {
                    let _ = context.close_iterator_object(iterator);
                    return Err(error);
                }
            }
        } else {
            value
        };
        if let Err(error) = create_data_property_or_throw(context, result_object, length, mapped) {
            let _ = context.close_iterator_object(iterator);
            return Err(error);
        }
        length += 1;
    }
    Err(VmError::runtime_limit(
        "Array.from iterator step limit exceeded",
    ))
}

fn array_from_create_result(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    length: Option<usize>,
) -> Result<JsValue, VmError> {
    if context.is_constructable_value(&constructor) {
        let arguments = length
            .map(|length| vec![JsValue::Number(length as f64)])
            .unwrap_or_default();
        return vm.construct_value_from_builtin(constructor, arguments, context);
    }
    match length {
        Some(length) => context.create_sparse_array(length),
        None => context.create_sparse_array(0),
    }
}

fn create_data_property_or_throw(
    context: &mut NativeContext,
    object: ObjectId,
    index: usize,
    value: JsValue,
) -> Result<(), VmError> {
    let update = PropertyDescriptorUpdate {
        value: Some(value),
        writable: Some(true),
        enumerable: Some(true),
        configurable: Some(true),
        get: None,
        set: None,
    };
    if context.validate_and_apply_property_descriptor(object, index.to_string(), update)? {
        Ok(())
    } else {
        Err(VmError::type_error("cannot create Array.from element"))
    }
}

fn set_array_from_length(
    vm: &mut Vm,
    context: &mut NativeContext,
    result: JsValue,
    length: usize,
) -> Result<(), VmError> {
    if vm.set_property_value_strict_from_builtin(
        result,
        "length",
        JsValue::Number(length as f64),
        context,
    )? {
        Ok(())
    } else {
        Err(VmError::type_error("cannot set Array.from length"))
    }
}

fn array_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = array_from_create_result(vm, context, this, Some(arguments.len()))?;
    let object = context.require_object(&result, "Array.of result")?;
    for (index, value) in arguments.iter().cloned().enumerate() {
        create_data_property_or_throw(context, object, index, value)?;
    }
    set_array_from_length(vm, context, result.clone(), arguments.len())?;
    Ok(result)
}

// ── Array.prototype methods ───────────────────────────────────────────────────

fn array_push(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, mut length) = array_object_target(vm, context, this_value)?;
    for value in arguments {
        // Spec step 4a: ? Set(O, ToString(len), E, true) — always strict
        set_array_index_strict(vm, context, target.clone(), length, value.clone())?;
        length += 1;
    }
    set_array_length_strict(vm, context, target, length)?;
    Ok(JsValue::Number(length as f64))
}

fn array_pop(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, target, length) = array_object_target(vm, context, this_value)?;
    if length == 0 {
        set_array_length_strict(vm, context, target, 0)?;
        return Ok(JsValue::Undefined);
    }
    let new_length = length - 1;
    let key = new_length.to_string();
    let value = get_elem(vm, context, target.clone(), new_length)?;
    context.delete_property(object, &key, false)?;
    set_array_length_strict(vm, context, target, new_length)?;
    Ok(value)
}

fn array_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value, context)?;
    let receiver = context.object_value(object);
    let join = vm.get_property_value(receiver.clone(), "join", context)?;
    if context.is_callable_value(&join) {
        return vm.call_value_from_builtin(join, receiver, Vec::new(), context);
    }
    super::object::object_to_string(vm, context, receiver, &[])
}

fn array_join(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    let sep = match arguments.first() {
        None | Some(JsValue::Undefined) => ",".to_string(),
        Some(value) => vm.to_string_coerce(value.clone(), context)?,
    };
    let mut parts: Vec<String> = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, target.clone(), i)?;
        parts.push(match val {
            JsValue::Undefined | JsValue::Null => String::new(),
            value => vm.to_string_coerce(value, context)?,
        });
    }
    Ok(JsValue::String(parts.join(&sep)))
}

fn array_reverse(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    let mid = length / 2;
    if mid > MAX_DENSE_ALLOC {
        return Err(VmError::runtime_limit(
            "Array.prototype.reverse iteration limit exceeded",
        ));
    }
    for lower in 0..mid {
        context.consume_loop_iteration()?;
        let upper = length - lower - 1;
        let lower_key = PropertyKey::String(lower.to_string());
        let upper_key = PropertyKey::String(upper.to_string());
        let lower_exists =
            super::proxy::internal_has_property(vm, context, target.clone(), &lower_key)?;
        let lower_value = if lower_exists {
            Some(super::proxy::internal_get(
                vm,
                context,
                target.clone(),
                &lower_key,
                target.clone(),
            )?)
        } else {
            None
        };
        let upper_exists =
            super::proxy::internal_has_property(vm, context, target.clone(), &upper_key)?;
        let upper_value = if upper_exists {
            Some(super::proxy::internal_get(
                vm,
                context,
                target.clone(),
                &upper_key,
                target.clone(),
            )?)
        } else {
            None
        };
        match (lower_value, upper_value) {
            (Some(lower_value), Some(upper_value)) => {
                set_array_index_strict(vm, context, target.clone(), lower, upper_value)?;
                set_array_index_strict(vm, context, target.clone(), upper, lower_value)?;
            }
            (None, Some(upper_value)) => {
                set_array_index_strict(vm, context, target.clone(), lower, upper_value)?;
                if !super::proxy::internal_delete(vm, context, target.clone(), &upper_key)? {
                    return Err(VmError::type_error("cannot delete reverse source"));
                }
            }
            (Some(lower_value), None) => {
                if !super::proxy::internal_delete(vm, context, target.clone(), &lower_key)? {
                    return Err(VmError::type_error("cannot delete reverse source"));
                }
                set_array_index_strict(vm, context, target.clone(), upper, lower_value)?;
            }
            (None, None) => {}
        }
    }
    Ok(target)
}

fn array_concat(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = vm.to_object(this_value, context)?;
    let original = context.object_value(this_object);
    let target = array_species_create(vm, context, original.clone(), 0)?;
    let target_object = context
        .value_object(&target)
        .ok_or_else(|| VmError::type_error("Array species result is not an object"))?;
    let mut result_length = 0usize;
    for value in std::iter::once(original).chain(arguments.iter().cloned()) {
        if is_concat_spreadable(vm, context, value.clone())? {
            let object = context
                .value_object(&value)
                .ok_or_else(|| VmError::type_error("spreadable value is not an object"))?;
            let length_value = get_property_preserving_throw(vm, context, value.clone(), "length")?;
            let length = to_length_number(vm.to_number(length_value, context)?);
            if result_length.saturating_add(length) > MAX_SAFE_INTEGER {
                return Err(VmError::type_error(
                    "concat result exceeds safe integer limit",
                ));
            }
            for index in 0..length.min(MAX_DENSE_ALLOC) {
                if array_index_exists(context, &value, object, index)? {
                    let element = get_existing_elem(vm, context, value.clone(), object, index)?;
                    create_data_property_or_throw(context, target_object, result_length, element)?;
                }
                result_length += 1;
            }
            if length > MAX_DENSE_ALLOC {
                result_length = result_length.saturating_add(length - MAX_DENSE_ALLOC);
            }
        } else {
            create_data_property_or_throw(context, target_object, result_length, value)?;
            result_length += 1;
        }
    }
    set_array_from_length(vm, context, target.clone(), result_length)?;
    Ok(target)
}

fn is_concat_spreadable(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<bool, VmError> {
    let Some(object) = context.value_object(&value) else {
        return Ok(false);
    };
    let spreadable = vm.get_symbol_property_value_with_receiver_from_builtin(
        value.clone(),
        value,
        context.well_known_symbols().is_concat_spreadable,
        context,
    )?;
    if !matches!(spreadable, JsValue::Undefined) {
        return Ok(spreadable.to_boolean());
    }
    context.is_array_object(object)
}

fn array_slice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, _, length) = array_callback_target(vm, context, this_value)?;
    let start = normalize_index(argument_number(vm, context, arguments, 0, 0.0)?, length);
    let end = normalize_index(
        argument_number(vm, context, arguments, 1, length as f64)?,
        length,
    );
    let count = end.saturating_sub(start);
    let result = array_species_create(vm, context, receiver.clone(), count)?;
    for (target, source) in (start..end).take(MAX_DENSE_ALLOC).enumerate() {
        if !array_index_exists(context, &receiver, object, source)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, source)?;
        create_array_data_property(context, &result, target, val)?;
    }
    Ok(result)
}

fn array_splice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, target, length) = array_object_target(vm, context, this_value)?;

    let start = normalize_index(argument_number(vm, context, arguments, 0, 0.0)?, length);
    let delete_count = match arguments.get(1) {
        None if arguments.is_empty() => 0,
        None => length - start,
        Some(value) => to_integer_or_infinity(vm.to_number(value.clone(), context)?)
            .max(0.0)
            .min((length - start) as f64) as usize,
    };
    let insert_items: Vec<JsValue> = arguments.get(2..).unwrap_or(&[]).to_vec();
    if length > MAX_DENSE_ALLOC {
        return Err(VmError::runtime_limit(
            "Array.prototype.splice iteration limit exceeded",
        ));
    }

    // Collect removed elements
    let removed = array_species_create(vm, context, target.clone(), delete_count)?;
    for i in 0..delete_count {
        if array_index_exists(context, &target, object, start + i)? {
            let val = get_existing_elem(vm, context, target.clone(), object, start + i)?;
            create_array_data_property(context, &removed, i, val)?;
        }
    }

    // Calculate new length
    let tail_len = length - start - delete_count;
    let new_length = start + insert_items.len() + tail_len;

    if insert_items.len() < delete_count {
        // Shift elements left
        for i in 0..tail_len {
            let src = start + delete_count + i;
            let dst = start + insert_items.len() + i;
            let val = get_elem(vm, context, target.clone(), src)?;
            set_array_index_strict(vm, context, target.clone(), dst, val)?;
        }
        // Delete trailing slots
        for i in new_length..length {
            context.delete_property(object, &i.to_string(), false)?;
        }
    } else if insert_items.len() > delete_count {
        // Shift elements right
        for i in (0..tail_len).rev() {
            let src = start + delete_count + i;
            let dst = start + insert_items.len() + i;
            let val = get_elem(vm, context, target.clone(), src)?;
            set_array_index_strict(vm, context, target.clone(), dst, val)?;
        }
    }

    // Write inserted items
    for (i, item) in insert_items.into_iter().enumerate() {
        set_array_index_strict(vm, context, target.clone(), start + i, item)?;
    }

    set_array_length_strict(vm, context, target, new_length)?;
    Ok(removed)
}

fn array_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, _, length) = array_callback_target(vm, context, this_value)?;
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let from_index =
        array_from_start_index(argument_number(vm, context, arguments, 1, 0.0)?, length);
    if length > MAX_DENSE_ALLOC && context.proxy_record(object).is_none() {
        let mut indices = own_numeric_indices(context, object, from_index, length)?;
        indices.sort_unstable();
        for index in indices {
            let value = get_existing_elem(vm, context, receiver.clone(), object, index)?;
            if value.strict_equals(&search) {
                return Ok(JsValue::Number(index as f64));
            }
        }
        return Ok(JsValue::Number(-1.0));
    }
    for i in from_index..length.min(MAX_DENSE_ALLOC) {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        if val.strict_equals(&search) {
            return Ok(JsValue::Number(i as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn array_last_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, _, length) = array_callback_target(vm, context, this_value)?;
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let from_raw = match arguments.get(1) {
        None => (length - 1) as f64,
        Some(value) => vm.to_number(value.clone(), context)?,
    };
    let Some(from) = array_from_last_index(from_raw, length) else {
        return Ok(JsValue::Number(-1.0));
    };
    if from >= MAX_DENSE_ALLOC && context.proxy_record(object).is_none() {
        let mut indices = own_numeric_indices(context, object, 0, from.saturating_add(1))?;
        indices.sort_unstable_by(|left, right| right.cmp(left));
        for index in indices {
            let value = get_existing_elem(vm, context, receiver.clone(), object, index)?;
            if value.strict_equals(&search) {
                return Ok(JsValue::Number(index as f64));
            }
        }
        return Ok(JsValue::Number(-1.0));
    }
    for i in (0..=from).rev() {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        if val.strict_equals(&search) {
            return Ok(JsValue::Number(i as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn own_numeric_indices(
    context: &NativeContext,
    object: ObjectId,
    start: usize,
    end: usize,
) -> Result<Vec<usize>, VmError> {
    let object = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing array-like object"))?;
    Ok(object
        .own_property_keys()
        .into_iter()
        .filter_map(|key| key.parse::<usize>().ok())
        .filter(|index| (start..end).contains(index))
        .collect())
}

fn array_fill(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_index(argument_number(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_index(
        argument_number(vm, context, arguments, 2, length as f64)?,
        length,
    );
    for i in start..end {
        set_array_index_strict(vm, context, target.clone(), i, value.clone())?;
    }
    Ok(target)
}

fn array_includes(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, _, length) = array_callback_target(vm, context, this_value)?;
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let from = array_from_start_index(argument_number(vm, context, arguments, 1, 0.0)?, length);
    for i in from..length.min(MAX_DENSE_ALLOC) {
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        if abstract_ops::same_value_zero(&val, &search) {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}

fn array_shift(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, target, length) = array_object_target(vm, context, this_value)?;
    if length == 0 {
        set_array_length_strict(vm, context, target, 0)?;
        return Ok(JsValue::Undefined);
    }
    let first = get_elem(vm, context, target.clone(), 0)?;
    for i in 1..length {
        let val = get_elem(vm, context, target.clone(), i)?;
        set_array_index_strict(vm, context, target.clone(), i - 1, val)?;
    }
    context.delete_property(object, &(length - 1).to_string(), false)?;
    set_array_length_strict(vm, context, target, length - 1)?;
    Ok(first)
}

fn array_unshift(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    if length > MAX_DENSE_ALLOC {
        return Err(VmError::range("Array.prototype.unshift: array too large"));
    }
    let count = arguments.len();
    // Shift existing elements right
    for i in (0..length).rev() {
        let val = get_elem(vm, context, target.clone(), i)?;
        set_array_index_strict(vm, context, target.clone(), i + count, val)?;
    }
    // Insert new elements at front
    for (i, item) in arguments.iter().enumerate() {
        set_array_index_strict(vm, context, target.clone(), i, item.clone())?;
    }
    let new_length = length + count;
    set_array_length_strict(vm, context, target, new_length)?;
    Ok(JsValue::Number(new_length as f64))
}

fn array_for_each(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, "Array.prototype.forEach")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), callback_object.clone()],
        )?;
    }
    Ok(JsValue::Undefined)
}

fn array_map(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, "Array.prototype.map")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let result = array_species_create(vm, context, receiver.clone(), length)?;
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        let mapped = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), callback_object.clone()],
        )?;
        create_array_data_property(context, &result, i, mapped)?;
    }
    Ok(result)
}

fn array_filter(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, "Array.prototype.filter")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let result = array_species_create(vm, context, receiver.clone(), 0)?;
    let mut target_index = 0usize;
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        let keep = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![
                val.clone(),
                JsValue::Number(i as f64),
                callback_object.clone(),
            ],
        )?;
        if keep.to_boolean() {
            create_array_data_property(context, &result, target_index, val)?;
            target_index += 1;
        }
    }
    Ok(result)
}

fn array_reduce(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, "Array.prototype.reduce")?;

    let safe_len = length.min(MAX_DENSE_ALLOC);
    let (mut acc, start) = if let Some(init) = arguments.get(1) {
        (init.clone(), 0usize)
    } else {
        let mut first_present = None;
        for i in 0..safe_len {
            if array_index_exists(context, &receiver, object, i)? {
                first_present = Some(i);
                break;
            }
        }
        let Some(first_index) = first_present else {
            return Err(VmError::type_error(
                "reduce of empty array with no initial value",
            ));
        };
        let first = get_existing_elem(vm, context, receiver.clone(), object, first_index)?;
        (first, first_index + 1)
    };

    for i in start..safe_len {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        acc = vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![acc, val, JsValue::Number(i as f64), callback_object.clone()],
            context,
        )?;
    }
    Ok(acc)
}

fn array_reduce_right(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, "Array.prototype.reduceRight")?;

    let safe_end = length.min(MAX_DENSE_ALLOC);
    let (mut acc, end) = if let Some(init) = arguments.get(1) {
        (init.clone(), safe_end)
    } else {
        let mut last_present = None;
        for i in (0..safe_end).rev() {
            if array_index_exists(context, &receiver, object, i)? {
                last_present = Some(i);
                break;
            }
        }
        let Some(last_idx) = last_present else {
            return Err(VmError::type_error(
                "reduceRight of empty array with no initial value",
            ));
        };
        let last = get_existing_elem(vm, context, receiver.clone(), object, last_idx)?;
        (last, last_idx)
    };

    for i in (0..end).rev() {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        acc = vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![acc, val, JsValue::Number(i as f64), callback_object.clone()],
            context,
        )?;
    }
    Ok(acc)
}

fn array_every(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, "Array.prototype.every")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        let result = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), callback_object.clone()],
        )?;
        if !result.to_boolean() {
            return Ok(JsValue::Boolean(false));
        }
    }
    Ok(JsValue::Boolean(true))
}

fn array_some(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, "Array.prototype.some")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        if !array_index_exists(context, &receiver, object, i)? {
            continue;
        }
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        let result = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), callback_object.clone()],
        )?;
        if result.to_boolean() {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}

fn array_find(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_find_common(vm, context, this_value, arguments, "find", false, false)
}

fn array_find_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_find_common(vm, context, this_value, arguments, "findIndex", false, true)
}

fn array_find_last(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_find_common(vm, context, this_value, arguments, "findLast", true, false)
}

fn array_find_last_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_find_common(
        vm,
        context,
        this_value,
        arguments,
        "findLastIndex",
        true,
        true,
    )
}

fn array_find_common(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    method: &str,
    reverse: bool,
    return_index: bool,
) -> Result<JsValue, VmError> {
    let (object, receiver, callback_object, length) =
        array_callback_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(context, &callback, &format!("Array.prototype.{method}"))?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let safe_len = length.min(MAX_DENSE_ALLOC);
    let iter: Box<dyn Iterator<Item = usize>> = if reverse {
        Box::new((0..safe_len).rev())
    } else {
        Box::new(0..safe_len)
    };
    for i in iter {
        let val = get_existing_elem(vm, context, receiver.clone(), object, i)?;
        let result = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![
                val.clone(),
                JsValue::Number(i as f64),
                callback_object.clone(),
            ],
        )?;
        if result.to_boolean() {
            return if return_index {
                Ok(JsValue::Number(i as f64))
            } else {
                Ok(val)
            };
        }
    }
    if return_index {
        Ok(JsValue::Number(-1.0))
    } else {
        Ok(JsValue::Undefined)
    }
}

fn array_flat(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    let depth = match argument_number(vm, context, arguments, 0, 1.0)? {
        value if value.is_infinite() && value > 0.0 => usize::MAX,
        value => value.max(0.0) as usize,
    };
    let values = flat_collect(vm, context, &target, length, depth)?;
    let result = array_species_create(vm, context, target, 0)?;
    for (index, value) in values.into_iter().enumerate() {
        create_array_data_property(context, &result, index, value)?;
    }
    Ok(result)
}

fn flat_collect(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: &JsValue,
    length: usize,
    depth: usize,
) -> Result<Vec<JsValue>, VmError> {
    let mut result = Vec::new();
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let elem = get_elem(vm, context, value.clone(), i)?;
        if depth > 0
            && let Some(id) = context
                .value_object(&elem)
                .filter(|&id| context.is_array_object(id).unwrap_or(false))
        {
            let inner_len = array_like_length(context, id);
            let inner = flat_collect(vm, context, &elem, inner_len, depth - 1)?;
            result.extend(inner);
            continue;
        }
        result.push(elem);
    }
    Ok(result)
}

fn array_flat_map(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let result = array_species_create(vm, context, target.clone(), 0)?;
    let mut target_index = 0usize;
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, target.clone(), i)?;
        let mapped = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), target.clone()],
        )?;
        if let Some(id) = context
            .value_object(&mapped)
            .filter(|&id| context.is_array_object(id).unwrap_or(false))
        {
            let inner_len = array_like_length(context, id);
            for j in 0..inner_len {
                let inner = get_elem(vm, context, mapped.clone(), j)?;
                create_array_data_property(context, &result, target_index, inner)?;
                target_index += 1;
            }
            continue;
        }
        create_array_data_property(context, &result, target_index, mapped)?;
        target_index += 1;
    }
    Ok(result)
}

fn array_sort(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, target, length) = array_object_target(vm, context, this_value)?;
    let compare_fn = arguments
        .first()
        .cloned()
        .filter(|v| !matches!(v, JsValue::Undefined));
    if let Some(compare_fn) = &compare_fn {
        require_callable(context, compare_fn, "Array.prototype.sort")?;
    }

    let elements: Vec<JsValue> = {
        let mut v = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
        for i in 0..length.min(MAX_DENSE_ALLOC) {
            if array_index_exists(context, &target, object, i)? {
                v.push(get_existing_elem(vm, context, target.clone(), object, i)?);
            }
        }
        v
    };

    let elements = merge_sort_array_elements(vm, context, elements, &compare_fn)?;
    let item_count = elements.len();

    for (i, elem) in elements.into_iter().enumerate() {
        set_array_index_strict(vm, context, target.clone(), i, elem)?;
    }
    for index in item_count..length.min(MAX_DENSE_ALLOC) {
        context.delete_property(object, &index.to_string(), true)?;
    }
    Ok(target)
}

fn merge_sort_array_elements(
    vm: &mut Vm,
    context: &mut NativeContext,
    mut elements: Vec<JsValue>,
    compare_fn: &Option<JsValue>,
) -> Result<Vec<JsValue>, VmError> {
    if elements.len() <= 1 {
        return Ok(elements);
    }

    let right = elements.split_off(elements.len() / 2);
    let left = merge_sort_array_elements(vm, context, elements, compare_fn)?;
    let right = merge_sort_array_elements(vm, context, right, compare_fn)?;
    merge_sorted_array_elements(vm, context, left, right, compare_fn)
}

fn merge_sorted_array_elements(
    vm: &mut Vm,
    context: &mut NativeContext,
    left: Vec<JsValue>,
    right: Vec<JsValue>,
    compare_fn: &Option<JsValue>,
) -> Result<Vec<JsValue>, VmError> {
    let mut merged = Vec::with_capacity(left.len().saturating_add(right.len()));
    let mut left_index = 0;
    let mut right_index = 0;

    while left_index < left.len() && right_index < right.len() {
        let left_is_greater = compare_two(
            vm,
            context,
            &left[left_index],
            &right[right_index],
            compare_fn,
        )?;
        if left_is_greater {
            merged.push(right[right_index].clone());
            right_index += 1;
        } else {
            merged.push(left[left_index].clone());
            left_index += 1;
        }
    }
    merged.extend_from_slice(&left[left_index..]);
    merged.extend_from_slice(&right[right_index..]);
    Ok(merged)
}

fn compare_two(
    vm: &mut Vm,
    context: &mut NativeContext,
    a: &JsValue,
    b: &JsValue,
    compare_fn: &Option<JsValue>,
) -> Result<bool, VmError> {
    match (a, b) {
        (JsValue::Undefined, JsValue::Undefined) => return Ok(false),
        (JsValue::Undefined, _) => return Ok(true),
        (_, JsValue::Undefined) => return Ok(false),
        _ => {}
    }
    if let Some(func) = compare_fn {
        let result = vm.call_value_from_builtin(
            func.clone(),
            JsValue::Undefined,
            vec![a.clone(), b.clone()],
            context,
        )?;
        let n = vm.to_number(result, context)?;
        Ok(n > 0.0)
    } else {
        let a_str = vm.to_string_coerce(a.clone(), context)?;
        let b_str = vm.to_string_coerce(b.clone(), context)?;
        Ok(a_str > b_str)
    }
}

fn array_keys(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    context.create_array_iterator_object(
        target,
        length,
        IteratorMode::Key,
        iterator_prototype(context),
    )
}

fn array_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    context.create_array_iterator_object(
        target,
        length,
        IteratorMode::Value,
        iterator_prototype(context),
    )
}

fn array_entries(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    context.create_array_iterator_object(
        target,
        length,
        IteratorMode::KeyAndValue,
        iterator_prototype(context),
    )
}

fn iterator_prototype(context: &NativeContext) -> Option<ObjectId> {
    context
        .get_global("Iterator")
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten())
        .or_else(|| context.object_prototype())
}

fn array_copy_within(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target_value, length) = array_object_target(vm, context, this_value)?;
    let mut to = normalize_index(argument_number(vm, context, arguments, 0, 0.0)?, length);
    let mut from = normalize_index(argument_number(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_index(
        argument_number(vm, context, arguments, 2, length as f64)?,
        length,
    );
    let mut count = end.saturating_sub(from).min(length.saturating_sub(to));
    if count > MAX_DENSE_ALLOC {
        return Err(VmError::runtime_limit(
            "Array.prototype.copyWithin iteration limit exceeded",
        ));
    }
    let direction = if from < to && to < from.saturating_add(count) {
        from += count - 1;
        to += count - 1;
        -1isize
    } else {
        1isize
    };
    while count > 0 {
        context.consume_loop_iteration()?;
        let from_key = PropertyKey::String(from.to_string());
        let to_key = PropertyKey::String(to.to_string());
        if super::proxy::internal_has_property(vm, context, target_value.clone(), &from_key)? {
            let value = super::proxy::internal_get(
                vm,
                context,
                target_value.clone(),
                &from_key,
                target_value.clone(),
            )?;
            set_array_index_strict(vm, context, target_value.clone(), to, value)?;
        } else if !super::proxy::internal_delete(vm, context, target_value.clone(), &to_key)? {
            return Err(VmError::type_error("cannot delete copyWithin target"));
        }
        from = from.wrapping_add_signed(direction);
        to = to.wrapping_add_signed(direction);
        count -= 1;
    }
    Ok(target_value)
}

fn array_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_object, target, length) = array_object_target(vm, context, this_value)?;
    let index_raw = argument_number(vm, context, arguments, 0, 0.0)? as i64;
    let index = if index_raw < 0 {
        let from_end = (-index_raw) as usize;
        if from_end > length {
            return Ok(JsValue::Undefined);
        }
        length - from_end
    } else {
        let i = index_raw as usize;
        if i >= length {
            return Ok(JsValue::Undefined);
        }
        i
    };
    get_elem(vm, context, target, index)
}

// ── Change-array-by-copy methods (ES2023) ─────────────────────────────────────

fn array_to_locale_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value.clone(), context)?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;
    let mut parts = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let elem = get_elem(vm, context, this_value.clone(), i)?;
        let part = if matches!(elem, JsValue::Undefined | JsValue::Null) {
            String::new()
        } else {
            let to_locale = vm.get_property_value(elem.clone(), "toLocaleString", context)?;
            if is_callable(&to_locale) {
                let result = vm.call_value_from_builtin(to_locale, elem, vec![], context)?;
                vm.to_string_coerce(result, context)?
            } else {
                vm.to_string_coerce(elem, context)?
            }
        };
        parts.push(part);
    }
    Ok(JsValue::String(parts.join(",")))
}

fn array_to_reversed(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value.clone(), context)?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;
    let capped = length.min(MAX_DENSE_ALLOC);
    // Spec reads from index len-1 down to 0 (descending)
    let mut elems = Vec::with_capacity(capped);
    for i in (0..capped).rev() {
        elems.push(get_elem(vm, context, this_value.clone(), i)?);
    }
    context.create_array(elems)
}

fn array_to_sorted(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let compare_fn_arg = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !matches!(compare_fn_arg, JsValue::Undefined) && !is_callable(&compare_fn_arg) {
        return Err(VmError::type_error(
            "Array.prototype.toSorted comparefn must be callable",
        ));
    }
    let compare_fn = if matches!(compare_fn_arg, JsValue::Undefined) {
        None
    } else {
        Some(compare_fn_arg)
    };

    let object = vm.to_object(this_value.clone(), context)?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;
    let capped = length.min(MAX_DENSE_ALLOC);
    let mut elems: Vec<JsValue> = (0..capped)
        .map(|i| get_elem(vm, context, this_value.clone(), i))
        .collect::<Result<_, _>>()?;

    for i in 1..elems.len() {
        let mut j = i;
        while j > 0 {
            let should_swap = compare_two(vm, context, &elems[j - 1], &elems[j], &compare_fn)?;
            if !should_swap {
                break;
            }
            elems.swap(j - 1, j);
            j -= 1;
        }
    }
    context.create_array(elems)
}

fn array_to_spliced(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value.clone(), context)?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;
    let capped = length.min(MAX_DENSE_ALLOC);

    let start_raw = argument_number(vm, context, arguments, 0, 0.0)?;
    let start = normalize_index(start_raw, capped);

    let delete_count = if arguments.len() < 2 {
        capped - start
    } else {
        to_integer_or_infinity(vm.to_number(arguments[1].clone(), context)?)
            .max(0.0)
            .min((capped - start) as f64) as usize
    };
    let insert_items: Vec<JsValue> = arguments.get(2..).unwrap_or(&[]).to_vec();

    let mut result = Vec::with_capacity(capped + insert_items.len());
    for i in 0..start {
        result.push(get_elem(vm, context, this_value.clone(), i)?);
    }
    for item in insert_items {
        result.push(item);
    }
    for i in (start + delete_count)..capped {
        result.push(get_elem(vm, context, this_value.clone(), i)?);
    }
    context.create_array(result)
}

fn array_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value.clone(), context)?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;

    let index_raw = argument_number(vm, context, arguments, 0, 0.0)?;
    let rel = if index_raw < 0.0 {
        (length as f64 + index_raw) as isize
    } else {
        index_raw as isize
    };
    if rel < 0 || rel as usize >= length {
        return Err(VmError::range("Array.prototype.with: index out of range"));
    }
    let replace_index = rel as usize;
    let new_value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);

    let capped = length.min(MAX_DENSE_ALLOC);
    let mut elems = Vec::with_capacity(capped);
    for i in 0..capped {
        if i == replace_index {
            elems.push(new_value.clone());
        } else {
            elems.push(get_elem(vm, context, this_value.clone(), i)?);
        }
    }
    context.create_array(elems)
}
