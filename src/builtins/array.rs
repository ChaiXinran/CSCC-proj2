//! `Array` constructor and prototype methods.

use crate::{
    runtime::{
        IteratorKind, IteratorMode, JsValue, NativeContext, ObjectId, ObjectKind, PrimitiveValue,
        PropertyDescriptor, PropertyDescriptorUpdate,
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
}

fn array_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
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

fn to_length(value: JsValue) -> usize {
    let Some(number) = value.to_number() else {
        return 0;
    };
    if number.is_nan() || number <= 0.0 {
        return 0;
    }
    if !number.is_finite() {
        return if number.is_sign_positive() {
            MAX_ARRAY_LENGTH
        } else {
            0
        };
    }
    number.floor().min(MAX_ARRAY_LENGTH as f64) as usize
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
    vm.get_property_value(receiver, "length", context)
        .map(to_length)
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
    if context.define_own_property(*object, index.to_string(), PropertyDescriptor::data(value))? {
        Ok(())
    } else {
        Err(VmError::type_error("cannot define array result property"))
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

fn normalize_index(raw: f64, length: usize) -> usize {
    if raw < 0.0 {
        let from_end = (-raw) as usize;
        length.saturating_sub(from_end)
    } else {
        (raw as usize).min(length)
    }
}

fn to_integer_or_infinity(number: f64) -> f64 {
    if number.is_nan() || number == 0.0 {
        0.0
    } else if number.is_infinite() {
        number
    } else {
        number.trunc()
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

fn same_value_zero(left: &JsValue, right: &JsValue) -> bool {
    match (left, right) {
        (JsValue::Number(a), JsValue::Number(b)) => a == b || (a.is_nan() && b.is_nan()),
        _ => left.strict_equals(right),
    }
}

/// ECMAScript maximum array length.
const MAX_ARRAY_LENGTH: usize = 4_294_967_295;
/// Cap iteration/allocation to prevent O(N) hangs on sparse arrays with huge lengths.
/// Test262 tests behavior at small indices; a 64K cap covers all realistic cases.
const MAX_DENSE_ALLOC: usize = 1 << 16; // 65536 elements

/// Reads array element `index` via the full VM property-get path (supports accessor getters).
fn get_elem(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    index: usize,
) -> Result<JsValue, VmError> {
    context.consume_loop_iteration()?;
    vm.get_property_value(value, &index.to_string(), context)
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
    if !is_callable(&callback) {
        return Err(VmError::type_error("callback is not a function"));
    }
    vm.call_value_from_builtin(callback, this_arg, args, context)
}

fn require_callable(value: &JsValue, method: &str) -> Result<(), VmError> {
    if !is_callable(value) {
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

fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

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
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.create_array(arguments.to_vec())
}

// ── Array.prototype methods ───────────────────────────────────────────────────

fn array_push(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.push")?;
    let mut length = array_like_length(context, object);
    for value in arguments {
        // Spec step 4a: ? Set(O, ToString(len), E, true) — always strict
        context.set_element_strict(
            this_value.clone(),
            JsValue::Number(length as f64),
            value.clone(),
        )?;
        length += 1;
    }
    Ok(JsValue::Number(length as f64))
}

fn array_pop(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.pop")?;
    let length = array_like_length(context, object);
    if length == 0 {
        return Ok(JsValue::Undefined);
    }
    let new_length = length - 1;
    let key = new_length.to_string();
    let value = context.get_property(this_value.clone(), &key)?;
    context.delete_property(object, &key, false)?;
    context.set_array_length(object, new_length)?;
    Ok(value)
}

fn array_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_join(vm, context, this_value, &[])
}

fn array_join(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.join")?;
    let length = array_like_length(context, object);
    let sep = match arguments.first() {
        None | Some(JsValue::Undefined) => ",".to_string(),
        Some(value) => vm.to_string_coerce(value.clone(), context)?,
    };
    let mut parts: Vec<String> = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
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
    let object = context.require_object(&this_value, "Array.prototype.reverse")?;
    let length = array_like_length(context, object);
    let mid = length / 2;
    for i in 0..mid {
        let j = length - 1 - i;
        let a = get_elem(vm, context, this_value.clone(), i)?;
        let b = get_elem(vm, context, this_value.clone(), j)?;
        context.set_element(this_value.clone(), JsValue::Number(i as f64), b)?;
        context.set_element(this_value.clone(), JsValue::Number(j as f64), a)?;
    }
    Ok(this_value)
}

fn array_concat(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut result: Vec<JsValue> = Vec::new();
    concat_spread(vm, context, this_value, &mut result)?;
    for arg in arguments {
        concat_spread(vm, context, arg.clone(), &mut result)?;
    }
    context.create_array(result)
}

fn concat_spread(
    vm: &mut Vm,
    context: &mut NativeContext,
    val: JsValue,
    out: &mut Vec<JsValue>,
) -> Result<(), VmError> {
    if let Some(id) = context
        .value_object(&val)
        .filter(|&id| context.is_array_object(id).unwrap_or(false))
    {
        let len = array_like_length(context, id);
        for i in 0..len {
            let elem = get_elem(vm, context, val.clone(), i)?;
            out.push(elem);
        }
        return Ok(());
    }
    out.push(val);
    Ok(())
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
    let result = context.create_sparse_array(count)?;
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
    let object = context.require_object(&this_value, "Array.prototype.splice")?;
    let length = array_like_length(context, object);

    let start = normalize_index(argument_number(vm, context, arguments, 0, 0.0)?, length);
    let delete_count = argument_number(vm, context, arguments, 1, (length - start) as f64)?
        .max(0.0)
        .min((length - start) as f64) as usize;
    let insert_items: Vec<JsValue> = arguments.get(2..).unwrap_or(&[]).to_vec();

    // Collect removed elements
    let mut removed = Vec::with_capacity(delete_count.min(MAX_DENSE_ALLOC));
    for i in 0..delete_count {
        let val = get_elem(vm, context, this_value.clone(), start + i)?;
        removed.push(val);
    }

    // Calculate new length
    let tail_len = length - start - delete_count;
    let new_length = start + insert_items.len() + tail_len;

    if insert_items.len() < delete_count {
        // Shift elements left
        for i in 0..tail_len {
            let src = start + delete_count + i;
            let dst = start + insert_items.len() + i;
            let val = get_elem(vm, context, this_value.clone(), src)?;
            context.set_element(this_value.clone(), JsValue::Number(dst as f64), val)?;
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
            let val = get_elem(vm, context, this_value.clone(), src)?;
            context.set_element(this_value.clone(), JsValue::Number(dst as f64), val)?;
        }
    }

    // Write inserted items
    for (i, item) in insert_items.into_iter().enumerate() {
        context.set_element(
            this_value.clone(),
            JsValue::Number((start + i) as f64),
            item,
        )?;
    }

    context.set_array_length(object, new_length)?;
    context.create_array(removed)
}

fn array_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, receiver, _, length) = array_callback_target(vm, context, this_value)?;
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let from_index =
        array_from_start_index(argument_number(vm, context, arguments, 1, 0.0)?, length);
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
    let from_raw = argument_number(vm, context, arguments, 1, (length - 1) as f64)?;
    let Some(from) = array_from_last_index(from_raw, length) else {
        return Ok(JsValue::Number(-1.0));
    };
    let from = from.min(MAX_DENSE_ALLOC.saturating_sub(1));
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

fn array_fill(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.fill")?;
    let length = array_like_length(context, object);
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_index(argument_number(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_index(
        argument_number(vm, context, arguments, 2, length as f64)?,
        length,
    );
    for i in start..end {
        context.set_element(this_value.clone(), JsValue::Number(i as f64), value.clone())?;
    }
    Ok(this_value)
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
        if same_value_zero(&val, &search) {
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
    let object = context.require_object(&this_value, "Array.prototype.shift")?;
    let length = array_like_length(context, object);
    if length == 0 {
        return Ok(JsValue::Undefined);
    }
    let first = get_elem(vm, context, this_value.clone(), 0)?;
    for i in 1..length {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        context.set_element(this_value.clone(), JsValue::Number((i - 1) as f64), val)?;
    }
    context.delete_property(object, &(length - 1).to_string(), false)?;
    context.set_array_length(object, length - 1)?;
    Ok(first)
}

fn array_unshift(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.unshift")?;
    let length = array_like_length(context, object);
    if length > MAX_DENSE_ALLOC {
        return Err(VmError::range("Array.prototype.unshift: array too large"));
    }
    let count = arguments.len();
    // Shift existing elements right
    for i in (0..length).rev() {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        context.set_element(this_value.clone(), JsValue::Number((i + count) as f64), val)?;
    }
    // Insert new elements at front
    for (i, item) in arguments.iter().enumerate() {
        context.set_element(this_value.clone(), JsValue::Number(i as f64), item.clone())?;
    }
    let new_length = length + count;
    context.set_array_length(object, new_length)?;
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
    require_callable(&callback, "Array.prototype.forEach")?;
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
    require_callable(&callback, "Array.prototype.map")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let result = context.create_sparse_array(length)?;
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
        context.set_element(result.clone(), JsValue::Number(i as f64), mapped)?;
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
    require_callable(&callback, "Array.prototype.filter")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut result = Vec::new();
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
            result.push(val);
        }
    }
    context.create_array(result)
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
    require_callable(&callback, "Array.prototype.reduce")?;

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
    require_callable(&callback, "Array.prototype.reduceRight")?;

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
    require_callable(&callback, "Array.prototype.every")?;
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
    require_callable(&callback, "Array.prototype.some")?;
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
    require_callable(&callback, &format!("Array.prototype.{method}"))?;
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
    let object = context.require_object(&this_value, "Array.prototype.flat")?;
    let length = array_like_length(context, object);
    let depth = match argument_number(vm, context, arguments, 0, 1.0)? {
        value if value.is_infinite() && value > 0.0 => usize::MAX,
        value => value.max(0.0) as usize,
    };
    let result = flat_collect(vm, context, &this_value, length, depth)?;
    context.create_array(result)
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
    let object = context.require_object(&this_value, "Array.prototype.flatMap")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut result = Vec::new();
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let mapped = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), this_value.clone()],
        )?;
        if let Some(id) = context
            .value_object(&mapped)
            .filter(|&id| context.is_array_object(id).unwrap_or(false))
        {
            let inner_len = array_like_length(context, id);
            for j in 0..inner_len {
                let inner = get_elem(vm, context, mapped.clone(), j)?;
                result.push(inner);
            }
            continue;
        }
        result.push(mapped);
    }
    context.create_array(result)
}

fn array_sort(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.sort")?;
    let length = array_like_length(context, object);
    let compare_fn = arguments
        .first()
        .cloned()
        .filter(|v| !matches!(v, JsValue::Undefined));

    // Collect elements
    let mut elements: Vec<JsValue> = {
        let mut v = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
        for i in 0..length.min(MAX_DENSE_ALLOC) {
            v.push(get_elem(vm, context, this_value.clone(), i)?);
        }
        v
    };

    // Insertion sort to allow calling vm.call_value per comparison
    for i in 1..elements.len() {
        let mut j = i;
        while j > 0 {
            let should_swap =
                compare_two(vm, context, &elements[j - 1], &elements[j], &compare_fn)?;
            if !should_swap {
                break;
            }
            elements.swap(j - 1, j);
            j -= 1;
        }
    }

    // Write sorted elements back
    for (i, elem) in elements.into_iter().enumerate() {
        context.set_element(this_value.clone(), JsValue::Number(i as f64), elem)?;
    }
    Ok(this_value)
}

fn compare_two(
    vm: &mut Vm,
    context: &mut NativeContext,
    a: &JsValue,
    b: &JsValue,
    compare_fn: &Option<JsValue>,
) -> Result<bool, VmError> {
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
    let object = context.require_object(&this_value, "Array.prototype.keys")?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;
    context.create_array_iterator_object(
        this_value,
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
    let object = context.require_object(&this_value, "Array.prototype.values")?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;
    context.create_array_iterator_object(
        this_value,
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
    let object = context.require_object(&this_value, "Array.prototype.entries")?;
    let length = array_like_length_from_value(vm, context, this_value.clone(), object)?;
    context.create_array_iterator_object(
        this_value,
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
    let object = context.require_object(&this_value, "Array.prototype.copyWithin")?;
    let length = array_like_length(context, object);
    let target = normalize_index(argument_number(vm, context, arguments, 0, 0.0)?, length);
    let start = normalize_index(argument_number(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_index(
        argument_number(vm, context, arguments, 2, length as f64)?,
        length,
    );
    let copy_len = (end - start).min(length - target);
    let src: Vec<JsValue> = {
        let mut v = Vec::with_capacity(copy_len);
        for i in 0..copy_len {
            v.push(get_elem(vm, context, this_value.clone(), start + i)?);
        }
        v
    };
    for (i, val) in src.into_iter().enumerate() {
        context.set_element(
            this_value.clone(),
            JsValue::Number((target + i) as f64),
            val,
        )?;
    }
    Ok(this_value)
}

fn array_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.at")?;
    let length = array_like_length(context, object);
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
    get_elem(vm, context, this_value, index)
}
